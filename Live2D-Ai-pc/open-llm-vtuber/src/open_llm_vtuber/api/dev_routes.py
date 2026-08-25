"""Local-only developer mode routes.

These endpoints are intentionally not part of the normal user workflow. They are
only accessible from localhost and only when developer_mode is enabled via
config (system_config.developer_mode) or the LIVE2DAI_DEV_MODE environment
variable.
"""

from __future__ import annotations

import os

from fastapi import APIRouter, Request
from fastapi.responses import JSONResponse

from ..halfbody import available_halfbody_channels
from ..logsetup import get_logger
from ..motion_catalog import ALLOWED_ACTIONS, MOTION_METADATA
from ..service_context import ServiceContext
from ..websocket_handler import WebSocketHandler

_log = get_logger("dev")

# Deterministic body action IDs supported by the renderer's BodyActionPlayer.
# Single source of truth: the shared half-body motion catalog. Adding an entry
# there automatically surfaces it in the dev console UI, no UI code changes.
# `bounce_happy` (full-body jump) is intentionally absent; `idle` is release-only.

_DEV_MODE_ENV_KEYS = ("1", "true", "yes")


def _developer_mode_enabled(system_config) -> bool:
    """Check config flag or environment override."""
    if system_config is not None and getattr(system_config, "developer_mode", False):
        return True
    env = os.environ.get("LIVE2DAI_DEV_MODE", "").lower()
    return env in _DEV_MODE_ENV_KEYS


def _is_localhost(request: Request) -> bool:
    host = request.client.host if request.client else None
    # None covers TestClient; restrict to loopback in production.
    # "testclient"/"testserver" are the loopback-equivalent hosts used by
    # starlette TestClient / httpx ASGI transport — they never appear in a
    # real uvicorn listener, so accepting them does not weaken production.
    return host in (
        None,
        "127.0.0.1",
        "localhost",
        "::1",
        "testclient",
        "testserver",
    )


def _gate(request: Request, system_config) -> JSONResponse | None:
    """Return a JSONResponse denying access, or None if allowed."""
    if not _developer_mode_enabled(system_config):
        return JSONResponse(
            {"error": "developer mode disabled"},
            status_code=403,
        )
    if not _is_localhost(request):
        return JSONResponse(
            {"error": "localhost only"},
            status_code=403,
        )
    return None


def _parameter_ids(context: ServiceContext) -> set[str]:
    model = getattr(context, "live2d_model", None)
    if model is None:
        return set()
    # _cdi3_parameter_ids is the runtime whitelist loaded from the model cdi3.json.
    return set(getattr(model, "_cdi3_parameter_ids", set()) or set())


def init_dev_routes(
    default_context_cache: ServiceContext,
    ws_handler: WebSocketHandler | None = None,
) -> APIRouter:
    """Create local-only developer tool routes.

    Args:
        default_context_cache: Shared service context (director, live2d model).
        ws_handler: Optional WebSocket handler for broadcasting dev actions.
    """
    router = APIRouter()

    @router.get("/api/dev/actions/catalog")
    async def catalog(request: Request):
        denied = _gate(request, getattr(default_context_cache, "system_config", None))
        if denied is not None:
            return denied

        channels = available_halfbody_channels(_parameter_ids(default_context_cache))
        action_meta = [
            {
                "id": mid,
                "label": str(meta.get("label") or mid),
                "description": str(meta.get("description") or ""),
                "halfbody": bool(meta.get("halfbody", True)),
                "suggested_duration_ms": int(meta.get("suggested_duration_ms", 1000)),
                "release_only": bool(meta.get("release_only", False)),
            }
            for mid in sorted(ALLOWED_ACTIONS)
            if (meta := MOTION_METADATA.get(mid))
        ]
        return JSONResponse(
            {
                # 兼容层：旧前端 dev-console 的 loadCatalog 期望纯 id 列表；
                # 富元数据（中文 label/描述/halfbody/时长/release）走 action_meta。
                "actions": sorted(ALLOWED_ACTIONS),
                "action_meta": action_meta,
                "halfbody_channels": list(channels),
                "capabilities": {ch: True for ch in channels},
            }
        )

    @router.post("/api/dev/actions/trigger")
    async def trigger(request: Request):
        denied = _gate(request, getattr(default_context_cache, "system_config", None))
        if denied is not None:
            return denied

        try:
            payload = await request.json()
        except Exception:  # noqa: BLE001
            return JSONResponse({"error": "invalid json"}, status_code=400)

        release = bool(payload.get("release", False))
        action = str(payload.get("action", "")).strip().lower()
        # idle 只能作为 release：显式请求 idle 触发一律转为释放回中，
        # 避免「重复触发 idle 回中」造成参数抖动（release_only 语义）。
        if action == "idle" and not release:
            release = True
        if release:
            action = "idle"
        if action not in ALLOWED_ACTIONS:
            return JSONResponse(
                {"error": "unknown action", "allowed": sorted(ALLOWED_ACTIONS)},
                status_code=400,
            )

        strength_raw = payload.get("strength", 2)
        try:
            strength = int(strength_raw)
        except (TypeError, ValueError):
            strength = 2
        if strength not in (1, 2, 3):
            strength = 2

        if ws_handler is not None:
            try:
                await ws_handler.broadcast_dev_action(action, strength)
                _log.info("dev action: action={} strength={} release={}", action, strength, release)
            except Exception as e:  # noqa: BLE001
                _log.warning("dev action broadcast failed: action={} strength={}: {}", action, strength, e)

        return JSONResponse({"ok": True, "action": action, "strength": strength})

    @router.post("/api/dev/director/analyze")
    async def director_analyze(request: Request):
        denied = _gate(request, getattr(default_context_cache, "system_config", None))
        if denied is not None:
            return denied

        try:
            payload = await request.json()
        except Exception:  # noqa: BLE001
            return JSONResponse({"error": "invalid json"}, status_code=400)

        text = str(payload.get("text", "")).strip()
        if not text:
            return JSONResponse({"error": "text required"}, status_code=400)

        director = getattr(default_context_cache, "director", None)
        if director is None:
            # Rule fallback: still useful for testing motion mapping without an LLM.
            from ..director import ExpressionDirector

            director = ExpressionDirector(
                llm=None,
                parameter_ids=_parameter_ids(default_context_cache),
            )

        try:
            result = await director.analyze(text)
        except Exception as e:  # noqa: BLE001
            _log.warning("dev director analyze failed (input_len={}): {}", len(text), e)
            return JSONResponse(
                {"error": f"director failed: {e}"},
                status_code=500,
            )

        from ..director import sanitize_director_result

        sanitized = sanitize_director_result(result.to_dict())
        _log.info("dev director analyze: input_len={} emotion={!r} action={!r}", len(text), sanitized.get("emotion"), sanitized.get("action"))
        return JSONResponse(sanitized)

    return router
