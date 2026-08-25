# -*- coding: utf-8 -*-
"""PC 通用直播/弹幕事件通道（Live Event Input）。

用途：让本地/内网的直播弹幕桥、礼物机器人或调试脚本，通过 HTTP 把一条
「事件」广播给所有打开中的 Live2D 前端：

    POST /api/live/event
    {"type":"danmaku","text":"你好呀","user":"小明","motion":"nod","emotion":"joy"}

前端收到后由 renderer/src/live-event.ts 消费：身体动作 + 表情 + toast。
motion 只接受动作目录（motion_catalog.SAFE_MOTION_IDS）内的六个 id；
其他值（含已删除的 happy/excited/sway 等）一律丢弃，不产动作。
若事件文本还需要进入 LLM 对话，请使用既有的 /api/external/chat；本接口
只负责「呈现层事件」，避免把弹幕风暴直接灌进 LLM。

安全默认：
  - 默认开启，但只允许回环地址（127.0.0.1/::1）调用；
  - 要放开局域网必须在 /api/live/settings 显式打开 allow_remote 并设置 token。
"""

from __future__ import annotations

import ipaddress
import secrets
import time
from dataclasses import dataclass, field
from typing import Any

from fastapi import APIRouter, Request
from loguru import logger
from starlette.responses import JSONResponse

from .local_state import JsonState
from .motion_catalog import sanitize_motion

MAX_TEXT_CHARS = 500
MAX_EVENTS_PER_MIN = 60
RECENT_LIMIT = 10
STATE_FILENAME = "live_event_state.json"

_ALLOWED_EVENT_FIELDS = {
    "type",
    "event",
    "text",
    "user",
    "motion",
    "emotion",
    "strength",
    "parameterOverrides",
}


def _is_loopback(host: str | None) -> bool:
    if not host:
        return False
    if host in ("localhost", "testclient"):
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


@dataclass
class LiveEventState:
    enabled: bool = True
    allow_remote: bool = False
    token: str = ""

    def to_json(self) -> dict[str, Any]:
        return {
            "enabled": self.enabled,
            "allow_remote": self.allow_remote,
            "token": self.token,
        }


@dataclass
class LiveEventRecord:
    at: float
    source: str
    event: dict[str, Any]
    ok: bool
    detail: str = ""

    def to_json(self) -> dict[str, Any]:
        return {
            "at": int(self.at),
            "source": self.source,
            "event": self.event,
            "ok": self.ok,
            "detail": self.detail,
        }


@dataclass
class LiveEventGate:
    """直播事件通道开关 + 鉴权 + 运行期统计。"""

    state_path: str = STATE_FILENAME
    state: LiveEventState = field(default_factory=LiveEventState)
    accepted: int = 0
    rejected: int = 0
    recent: list[LiveEventRecord] = field(default_factory=list)

    def load(self) -> "LiveEventGate":
        raw = JsonState(self.state_path).load(LiveEventState().to_json())
        self.state = LiveEventState(
            enabled=bool(raw.get("enabled", True)),
            allow_remote=bool(raw.get("allow_remote", False)),
            token=str(raw.get("token", "") or ""),
        )
        return self

    def save(self) -> None:
        JsonState(self.state_path).save(self.state.to_json())

    def authorize(self, client_host: str | None, token: str | None) -> str | None:
        if not self.state.enabled:
            return "直播事件通道已关闭（可在 /api/live/settings 打开）"
        if not _is_loopback(client_host):
            if not self.state.allow_remote:
                return f"仅允许本机调用（来源 {client_host}）"
            if not self.state.token:
                return "局域网接入必须设置访问令牌（token）后才能使用"
        if self.state.token and not secrets.compare_digest(
            self.state.token, (token or "")
        ):
            return "访问令牌不正确"
        return None

    def record(self, source: str, event: dict[str, Any], ok: bool, detail: str = "") -> None:
        if ok:
            self.accepted += 1
        else:
            self.rejected += 1
        self.recent.append(
            LiveEventRecord(at=time.time(), source=source, event=event, ok=ok, detail=detail)
        )
        if len(self.recent) > RECENT_LIMIT:
            del self.recent[:-RECENT_LIMIT]

    def status(self, clients: int) -> dict[str, Any]:
        return {
            "enabled": self.state.enabled,
            "allow_remote": self.state.allow_remote,
            "token_set": bool(self.state.token),
            "endpoint": "/api/live/event",
            "clients": clients,
            "accepted": self.accepted,
            "rejected": self.rejected,
            "recent": [r.to_json() for r in reversed(self.recent)],
        }


def _extract_token(request: Request, payload: dict[str, Any] | None) -> str:
    header = request.headers.get("x-live2dai-token")
    if header:
        return header.strip()
    auth = request.headers.get("authorization") or ""
    if auth.lower().startswith("bearer "):
        return auth[7:].strip()
    if payload and isinstance(payload.get("token"), str):
        return payload["token"].strip()
    return ""


def sanitize_live_event(payload: dict[str, Any]) -> dict[str, Any]:
    """只保留前端需要且安全的字段，未知字段丢弃。

    动作边界：motion 必须落在动作目录白名单（六个安全动作）内，
    已删/未知 id 一律丢弃 —— 任何外部 payload 都不能让前端播已删动作。
    """
    event: dict[str, Any] = {}
    for key in _ALLOWED_EVENT_FIELDS:
        if key in payload:
            event[key] = payload[key]
    if isinstance(event.get("text"), str):
        event["text"] = event["text"].strip()[:MAX_TEXT_CHARS]
    else:
        event.pop("text", None)
    if isinstance(event.get("user"), str):
        event["user"] = event["user"].strip()[:80]
    else:
        event.pop("user", None)
    if isinstance(event.get("strength"), (int, float)):
        event["strength"] = max(1, min(3, int(event["strength"])))
    if not isinstance(event.get("parameterOverrides"), dict):
        event.pop("parameterOverrides", None)
    if "motion" in event:
        raw_motion = event.get("motion")
        safe_motion = (
            sanitize_motion(raw_motion) if isinstance(raw_motion, str) else None
        )
        if safe_motion:
            event["motion"] = safe_motion
        else:
            event.pop("motion", None)
    return event


def init_live_event_routes(
    gate: LiveEventGate,
    ws_handler,
    plugin_manager=None,
) -> APIRouter:
    router = APIRouter()

    def _clients() -> int:
        return len(getattr(ws_handler, "client_connections", {}) or {})

    def _plugin_reason() -> str | None:
        if plugin_manager is None:
            return None
        if plugin_manager.is_enabled("live-ingress"):
            return None
        return "live-ingress 插件未启用，直播事件接入不可用"

    @router.get("/api/live/status")
    async def live_status():
        return JSONResponse({"ok": True, **gate.status(_clients())})

    @router.post("/api/live/settings")
    async def live_settings(request: Request):
        """修改开关；仅本机允许，避免远程自己开门。"""
        if not _is_loopback(request.client.host if request.client else None):
            return JSONResponse(
                {"ok": False, "error": "直播事件开关只能从本机修改"}, status_code=403
            )
        try:
            payload = await request.json()
        except Exception:  # noqa: BLE001
            payload = {}
        if not isinstance(payload, dict):
            payload = {}
        if "enabled" in payload:
            gate.state.enabled = bool(payload["enabled"])
        if "allow_remote" in payload:
            gate.state.allow_remote = bool(payload["allow_remote"])
        if payload.get("regenerate_token"):
            gate.state.token = secrets.token_urlsafe(24)
        elif payload.get("token_clear"):
            gate.state.token = ""
        elif isinstance(payload.get("token"), str) and payload["token"].strip():
            gate.state.token = payload["token"].strip()
        if gate.state.allow_remote and not gate.state.token:
            return JSONResponse(
                {"ok": False, "error": "允许局域网接入前必须设置访问令牌", **gate.status(_clients())},
                status_code=400,
            )
        try:
            gate.save()
        except Exception as e:  # noqa: BLE001
            logger.warning(f"直播事件状态落盘失败（本次运行仍生效）: {e}")
            return JSONResponse(
                {"ok": True, "persisted": False, **gate.status(_clients())}
            )
        return JSONResponse({"ok": True, "persisted": True, **gate.status(_clients())})

    @router.post("/api/live/event")
    async def live_event(request: Request):
        source = (request.client.host if request.client else None) or "unknown"
        plugin_reason = _plugin_reason()
        if plugin_reason:
            gate.record(source, {}, False, plugin_reason)
            return JSONResponse({"ok": False, "error": plugin_reason}, status_code=403)

        try:
            payload = await request.json()
        except Exception:  # noqa: BLE001
            payload = {}
        if not isinstance(payload, dict):
            payload = {}
        reason = gate.authorize(source, _extract_token(request, payload))
        if reason:
            gate.record(source, {}, False, reason)
            return JSONResponse({"ok": False, "error": reason}, status_code=403)

        event = sanitize_live_event(payload)
        if not event:
            gate.record(source, {}, False, "事件为空")
            return JSONResponse({"ok": False, "error": "事件为空"}, status_code=400)

        try:
            clients = await ws_handler.broadcast_live_event(event)
        except Exception as e:  # noqa: BLE001 - 广播失败要如实告知调用方
            logger.error(f"直播事件广播失败: {e}")
            gate.record(source, event, False, str(e))
            return JSONResponse({"ok": False, "error": str(e)}, status_code=500)

        gate.record(source, event, True)
        logger.info(f"直播事件 [{event.get('type') or event.get('event')}] -> {clients} 个会话")
        return JSONResponse({"ok": True, "clients": clients, "event": event})

    return router
