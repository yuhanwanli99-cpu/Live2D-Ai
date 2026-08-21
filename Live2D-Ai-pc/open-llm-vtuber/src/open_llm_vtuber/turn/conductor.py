"""TurnConductor：并行运行辅助专家，不阻塞核心对话脑。

核心对话脑是热路径；body/mood/memory 等专家以 asyncio task 并行运行，
超时后返回 fallback 状态。所有失败都必须体现在 ExpertStatus 中，不允许静默吞掉。
"""

from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass
from typing import Any, Awaitable, Callable

from .types import ExpertStatus, TurnBundle


@dataclass
class ExpertSpec:
    expert_id: str
    run: Callable[[], Awaitable[dict[str, Any]]]
    timeout: float = 2.0


def _safe_result(expert_id: str, result: Any) -> dict[str, Any]:
    if isinstance(result, dict):
        return result
    return {"value": result}


async def run_experts(
    specs: list[ExpertSpec],
) -> list[tuple[ExpertStatus, dict[str, Any]]]:
    """并行运行所有专家；单个专家异常或超时不会影响其他专家。

    Returns:
        list of (status, result). 结果 dict 由专家自行定义。
    """
    if not specs:
        return []

    async def one(spec: ExpertSpec) -> tuple[ExpertStatus, dict[str, Any]]:
        started = time.monotonic()
        status = ExpertStatus(expert_id=spec.expert_id, status="running")
        try:
            result = await asyncio.wait_for(spec.run(), timeout=spec.timeout)
            status.status = "ok"
            status.elapsed_ms = int((time.monotonic() - started) * 1000)
            return status, _safe_result(spec.expert_id, result)
        except asyncio.TimeoutError:
            status.status = "timeout"
            status.detail = f"timeout after {spec.timeout:.1f}s"
            status.elapsed_ms = int((time.monotonic() - started) * 1000)
            return status, {}
        except Exception as exc:  # noqa: BLE001 - 必须记录并返回，不能静默
            status.status = "error"
            status.detail = f"{type(exc).__name__}: {exc}"
            status.elapsed_ms = int((time.monotonic() - started) * 1000)
            return status, {}

    return await asyncio.gather(*(one(spec) for spec in specs))


class TurnConductor:
    """把核心对话脑文本与辅助专家结果合并为 TurnBundle。"""

    def __init__(self, core_text_provider: Callable[[], Awaitable[str]]):
        self._core_text_provider = core_text_provider

    async def run(
        self,
        expert_specs: list[ExpertSpec] | None = None,
    ) -> TurnBundle:
        specs = expert_specs or []
        text_task = asyncio.create_task(self._core_text_provider())
        expert_task = asyncio.create_task(run_experts(specs))

        bundle = TurnBundle()
        try:
            bundle.text = await text_task
        except Exception as exc:  # noqa: BLE001 - 核心脑失败也要可见
            bundle.expert_status.append(
                ExpertStatus(
                    expert_id="core.dialogue",
                    status="error",
                    detail=f"{type(exc).__name__}: {exc}",
                )
            )

        expert_results = await expert_task
        for status, result in expert_results:
            bundle.expert_status.append(status)
            if status.expert_id == "body-director":
                frames = result.get("body_frames") or result.get("frames") or []
                if isinstance(frames, list):
                    bundle.body_frames = frames
            elif status.expert_id == "mood-tts-director":
                if isinstance(result.get("mood"), dict):
                    bundle.mood = result["mood"]
                if isinstance(result.get("tts_style"), dict):
                    bundle.tts_style = result["tts_style"]
            elif status.expert_id == "memory":
                if isinstance(result.get("memory_updates"), list):
                    bundle.memory_updates = result["memory_updates"]
                if isinstance(result.get("knowledge_hits"), list):
                    bundle.knowledge_hits = result["knowledge_hits"]
        return bundle
