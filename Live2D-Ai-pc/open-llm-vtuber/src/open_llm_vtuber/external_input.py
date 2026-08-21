# -*- coding: utf-8 -*-
"""外部文字接入（External Text Input）——把外部程序传来的文字塞进内部 LLM 链路。

用途：让本机/内网的其它程序（脚本、直播弹幕桥、快捷指令、另一个 agent……）
通过 HTTP 把一句话交给角色，走**完全相同**的内部链路：

    POST /api/external/chat  →  WebSocketHandler.inject_text_input()
      →  conversations.handle_conversation_trigger(msg_type="text-input")
      →  LLM 流式 →  情绪/动作 →  TTS →  前端口型 + 字幕

也就是说：外部文字与前端聊天框输入**没有第二条链路**，行为完全一致
（同一 persona、同一记忆、同一 mood_engine、同一 TTS 与口型）。

安全默认（本项目 host 默认 0.0.0.0 且 CORS 放开，故此处必须自己把门）：
  - **默认关闭**（`enabled=False`）：不打开开关时，接口一律 403；
  - 只允许 **回环地址**（127.0.0.1/::1）调用；要放开局域网必须同时
    显式打开 `allow_remote` **并**设置 `token`（无 token 不允许远程，
    避免同网段任何人都能让角色开口）；
  - `token` 通过请求头 `X-Live2DAI-Token` 或 `Authorization: Bearer <token>` 传递。

开关状态由前端「设置 → 外部接入」实时控制（见 renderer/src/external-input.ts），
落盘在 `external_input.json`（与 conf.yaml 同目录，已 gitignore），重启保留。
"""

from __future__ import annotations

import asyncio
import ipaddress
import os
import secrets
import time
from dataclasses import dataclass, field
from typing import Any

from fastapi import APIRouter, Request
from loguru import logger
from starlette.responses import JSONResponse

from .local_state import JsonState

# 单次注入文本上限（防止外部程序一次灌进一整本书拖死 LLM/TTS）
MAX_TEXT_CHARS = 2000
# wait_reply 的等待上限（秒）；外部调用可传更小值，但不能超过这个封顶
MAX_WAIT_REPLY_S = 180.0
DEFAULT_WAIT_REPLY_S = 60.0
# 最近注入记录的保留条数（供设置页展示"最近收到什么"）
RECENT_LIMIT = 10

STATE_FILENAME = "external_input.json"


def _is_loopback(host: str | None) -> bool:
    """请求来源是否为本机回环地址。"""
    if not host:
        return False
    if host in ("localhost", "testclient"):  # testclient: FastAPI TestClient
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


@dataclass
class ExternalInputState:
    """可落盘的开关状态（不含运行期统计）。"""

    enabled: bool = False
    allow_remote: bool = False
    token: str = ""

    def to_json(self) -> dict[str, Any]:
        return {
            "enabled": self.enabled,
            "allow_remote": self.allow_remote,
            "token": self.token,
        }


@dataclass
class InjectionRecord:
    """一条注入记录（脱敏后用于设置页展示）。"""

    at: float
    source: str
    text: str
    ok: bool
    detail: str = ""

    def to_json(self) -> dict[str, Any]:
        return {
            "at": int(self.at),
            "source": self.source,
            "text": self.text[:120],
            "ok": self.ok,
            "detail": self.detail,
        }


@dataclass
class ExternalInputGate:
    """外部接入的开关 + 鉴权 + 运行期统计（单实例，被路由与 server 共享）。"""

    state_path: str = STATE_FILENAME
    state: ExternalInputState = field(default_factory=ExternalInputState)
    accepted: int = 0
    rejected: int = 0
    recent: list[InjectionRecord] = field(default_factory=list)

    # ---- 落盘 / 载入 ----

    def load(self) -> "ExternalInputGate":
        """从磁盘载入开关状态；文件不存在或损坏 → 保持默认（关闭）。"""
        raw = JsonState(self.state_path).load(ExternalInputState().to_json())
        self.state = ExternalInputState(
            enabled=bool(raw.get("enabled", False)),
            allow_remote=bool(raw.get("allow_remote", False)),
            token=str(raw.get("token", "") or ""),
        )
        return self

    def save(self) -> None:
        """原子写状态文件（临时文件 + os.replace），避免半份 JSON。

        Raises:
            OSError: 落盘失败（调用方降级为"仅本次运行生效"）。
        """
        JsonState(self.state_path).save(self.state.to_json())

    # ---- 鉴权 ----

    def authorize(self, client_host: str | None, token: str | None) -> str | None:
        """返回 None 表示放行；否则返回拒绝原因（中文，直接给用户看）。"""
        if not self.state.enabled:
            return "外部接入已关闭（可在设置页「外部接入」中打开）"

        loopback = _is_loopback(client_host)
        if not loopback:
            if not self.state.allow_remote:
                return f"仅允许本机调用（来源 {client_host}）；需要局域网接入请在设置页打开「允许局域网」并设置访问令牌"
            if not self.state.token:
                return "局域网接入必须设置访问令牌（token）后才能使用"

        if self.state.token and not secrets.compare_digest(
            self.state.token, (token or "")
        ):
            return "访问令牌不正确"
        return None

    # ---- 统计 ----

    def record(self, source: str, text: str, ok: bool, detail: str = "") -> None:
        if ok:
            self.accepted += 1
        else:
            self.rejected += 1
        self.recent.append(
            InjectionRecord(at=time.time(), source=source, text=text, ok=ok, detail=detail)
        )
        if len(self.recent) > RECENT_LIMIT:
            del self.recent[:-RECENT_LIMIT]

    def status(self, clients: list[str]) -> dict[str, Any]:
        return {
            "enabled": self.state.enabled,
            "allow_remote": self.state.allow_remote,
            "token_set": bool(self.state.token),
            "endpoint": "/api/external/chat",
            "clients": clients,
            "accepted": self.accepted,
            "rejected": self.rejected,
            "recent": [r.to_json() for r in reversed(self.recent)],
            "limits": {
                "max_text_chars": MAX_TEXT_CHARS,
                "max_wait_reply_s": MAX_WAIT_REPLY_S,
            },
        }


def _extract_token(request: Request, payload: dict[str, Any] | None) -> str:
    """从请求头（优先）或 body 取 token。"""
    header = request.headers.get("x-live2dai-token")
    if header:
        return header.strip()
    auth = request.headers.get("authorization") or ""
    if auth.lower().startswith("bearer "):
        return auth[7:].strip()
    if payload and isinstance(payload.get("token"), str):
        return payload["token"].strip()
    return ""


def _clamp_timeout(raw: Any) -> float:
    try:
        value = float(raw)
    except (TypeError, ValueError):
        return DEFAULT_WAIT_REPLY_S
    if value <= 0:
        return DEFAULT_WAIT_REPLY_S
    return min(value, MAX_WAIT_REPLY_S)


def init_external_input_routes(
    gate: ExternalInputGate,
    ws_handler,
    plugin_manager=None,
) -> APIRouter:
    """构造外部接入路由。

    Args:
        gate: 共享的开关实例（前端设置页与注入接口读同一份状态）。
        ws_handler: `WebSocketHandler`，提供 `inject_text_input` 与在线会话列表。
        plugin_manager: 可选 PluginManager；提供后，input-port 插件未启用时注入接口不可用。
    """
    router = APIRouter()

    def _clients() -> list[str]:
        return list(getattr(ws_handler, "client_connections", {}) or {})

    def _plugin_reason() -> str | None:
        if plugin_manager is None:
            return None
        if plugin_manager.is_enabled("input-port"):
            return None
        return "input-port 插件未启用，外部文字接入不可用"

    @router.get("/api/external/status")
    async def external_status():
        return JSONResponse({"ok": True, **gate.status(_clients())})

    @router.post("/api/external/settings")
    async def external_settings(request: Request):
        """前端设置页写开关：{enabled?, allow_remote?, token?, token_clear?, regenerate_token?}。

        只接受本机调用——避免"远程调用者自己把远程开关打开"的提权路径。
        """
        if not _is_loopback(request.client.host if request.client else None):
            return JSONResponse(
                {"ok": False, "error": "外部接入开关只能从本机修改"}, status_code=403
            )
        try:
            payload = await request.json()
        except Exception:
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

        # 局域网接入必须有 token：拒绝落成"开着门却没锁"的状态
        if gate.state.allow_remote and not gate.state.token:
            return JSONResponse(
                {
                    "ok": False,
                    "error": "允许局域网接入前必须设置访问令牌（可点「生成令牌」）",
                    **gate.status(_clients()),
                },
                status_code=400,
            )

        try:
            gate.save()
        except Exception as e:  # noqa: BLE001 - 落盘失败不该丢掉本次开关效果
            logger.warning(f"外部接入状态落盘失败（本次运行仍生效）: {e}")
            return JSONResponse(
                {
                    "ok": True,
                    "persisted": False,
                    "message": f"已生效，但状态未能写入磁盘: {e}",
                    **gate.status(_clients()),
                }
            )

        logger.info(
            f"外部接入设置更新: enabled={gate.state.enabled} "
            f"allow_remote={gate.state.allow_remote} token_set={bool(gate.state.token)}"
        )
        return JSONResponse({"ok": True, "persisted": True, **gate.status(_clients())})

    @router.get("/api/external/token")
    async def external_token(request: Request):
        """回显当前 token（仅本机；便于设置页复制粘贴给外部程序）。"""
        if not _is_loopback(request.client.host if request.client else None):
            return JSONResponse(
                {"ok": False, "error": "令牌只能从本机读取"}, status_code=403
            )
        return JSONResponse({"ok": True, "token": gate.state.token})

    @router.post("/api/external/chat")
    async def external_chat(request: Request):
        """把外部文字注入内部 LLM 链路。

        Body:
            text (str, 必填): 要说给角色听的话。
            client_uid (str, 可选): 目标前端会话；缺省用最近连接的会话。
            interrupt (bool, 可选): 该会话正在说话时是否打断（默认 false → 返回 409）。
            wait_reply (bool, 可选): 是否等 LLM 回复文本后再返回（默认 false，立即 202）。
            timeout_s (float, 可选): wait_reply 的等待上限，封顶 180s。
        Header:
            X-Live2DAI-Token 或 Authorization: Bearer <token>（设置了令牌时必填）。
        """
        source = (request.client.host if request.client else None) or "unknown"
        plugin_reason = _plugin_reason()
        if plugin_reason:
            gate.record(source, "", False, plugin_reason)
            return JSONResponse({"ok": False, "error": plugin_reason}, status_code=403)

        try:
            payload = await request.json()
        except Exception:
            payload = {}
        if not isinstance(payload, dict):
            payload = {}

        reason = gate.authorize(source, _extract_token(request, payload))
        if reason:
            gate.record(source, str(payload.get("text", ""))[:120], False, reason)
            return JSONResponse({"ok": False, "error": reason}, status_code=403)

        text = payload.get("text")
        if not isinstance(text, str) or not text.strip():
            gate.record(source, "", False, "text 为空")
            return JSONResponse(
                {"ok": False, "error": "缺少 text 字段（要说的话）"}, status_code=400
            )
        text = text.strip()
        if len(text) > MAX_TEXT_CHARS:
            gate.record(source, text, False, "text 过长")
            return JSONResponse(
                {
                    "ok": False,
                    "error": f"text 过长（{len(text)} 字符，上限 {MAX_TEXT_CHARS}）",
                },
                status_code=413,
            )

        wait_reply = bool(payload.get("wait_reply"))
        timeout_s = _clamp_timeout(payload.get("timeout_s"))

        try:
            result = await ws_handler.inject_text_input(
                text=text,
                client_uid=payload.get("client_uid") or None,
                interrupt=bool(payload.get("interrupt")),
                source=f"external:{source}",
            )
        except LookupError as e:
            gate.record(source, text, False, str(e))
            return JSONResponse({"ok": False, "error": str(e)}, status_code=409)
        except RuntimeError as e:
            gate.record(source, text, False, str(e))
            return JSONResponse({"ok": False, "error": str(e)}, status_code=409)
        except Exception as e:  # noqa: BLE001 - 注入失败要如实告知调用方
            logger.error(f"外部文字注入失败: {e}")
            gate.record(source, text, False, str(e))
            return JSONResponse({"ok": False, "error": str(e)}, status_code=500)

        gate.record(source, text, True)
        client_uid, task = result
        body: dict[str, Any] = {
            "ok": True,
            "accepted": True,
            "client_uid": client_uid,
            "text": text,
        }

        if not wait_reply or task is None:
            return JSONResponse(body, status_code=202)

        # 等回复：shield 保证超时只结束"等待"，不取消正在进行的对话
        try:
            reply = await asyncio.wait_for(asyncio.shield(task), timeout=timeout_s)
            body["reply"] = reply if isinstance(reply, str) else ""
            body["completed"] = True
            return JSONResponse(body)
        except asyncio.TimeoutError:
            body["completed"] = False
            body["message"] = f"等待回复超时（{timeout_s:.0f}s），对话仍在继续"
            return JSONResponse(body, status_code=202)
        except asyncio.CancelledError:
            body["completed"] = False
            body["message"] = "对话被打断"
            return JSONResponse(body, status_code=202)
        except Exception as e:  # noqa: BLE001 - 对话内部异常如实回报
            body["completed"] = False
            body["error"] = str(e)
            return JSONResponse(body, status_code=502)

    return router
