"""TurnBundle 与专家状态类型（PC v4 多脑编排数据契约）。

这些类型只描述一轮对话中多个专家如何汇合，不直接调用 LLM 或 TTS。
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


@dataclass
class ExpertStatus:
    expert_id: str
    status: str = "pending"  # pending / running / ok / timeout / fallback / disabled / error
    detail: str = ""
    elapsed_ms: int = 0

    def to_dict(self) -> dict[str, Any]:
        return {
            "expert_id": self.expert_id,
            "status": self.status,
            "detail": self.detail,
            "elapsed_ms": self.elapsed_ms,
        }


@dataclass
class TurnBundle:
    """一轮对话中核心对话脑与辅助专家的合并结果。"""

    text: str = ""
    emotion_events: list[dict[str, Any]] = field(default_factory=list)
    body_frames: list[dict[str, Any]] = field(default_factory=list)
    mood: dict[str, Any] = field(default_factory=dict)
    tts_style: dict[str, Any] = field(default_factory=dict)
    memory_updates: list[dict[str, Any]] = field(default_factory=list)
    knowledge_hits: list[dict[str, Any]] = field(default_factory=list)
    expert_status: list[ExpertStatus] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return {
            "text": self.text,
            "emotion_events": self.emotion_events,
            "body_frames": self.body_frames,
            "mood": self.mood,
            "tts_style": self.tts_style,
            "memory_updates": self.memory_updates,
            "knowledge_hits": self.knowledge_hits,
            "expert_status": [s.to_dict() for s in self.expert_status],
        }
