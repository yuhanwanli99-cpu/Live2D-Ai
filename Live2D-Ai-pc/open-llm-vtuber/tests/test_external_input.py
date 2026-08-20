# -*- coding: utf-8 -*-
"""外部文字接入测试（`src/open_llm_vtuber/external_input.py` + WS 注入方法）。

覆盖三层：
  1. `ExternalInputGate` 鉴权矩阵（关/开、回环/远程、token 有无与错误）；
  2. HTTP 层 `/api/external/{status,settings,token,chat}` 的校验与返回形状；
  3. `WebSocketHandler.inject_text_input` 的目标选择 / 忙碌 / 打断 / 回显语义。

原则：全部走 tmp_path 隔离状态文件；不发真实网络；不触碰仓库真实 conf.yaml。
"""

import asyncio
import json

import pytest

from open_llm_vtuber.external_input import (
    ExternalInputGate,
    ExternalInputState,
    MAX_TEXT_CHARS,
    init_external_input_routes,
)


# ─────────────────────────────────────────────────────────────
# 测试替身
# ─────────────────────────────────────────────────────────────
class FakeWsHandler:
    """记录 inject 调用的假 WS handler。"""

    def __init__(self, reply: str | None = "回复内容", raises: Exception | None = None):
        self.client_connections = {"c1": object()}
        self.calls: list[dict] = []
        self._reply = reply
        self._raises = raises

    async def inject_text_input(self, text, client_uid=None, interrupt=False, source=""):
        self.calls.append(
            {"text": text, "client_uid": client_uid, "interrupt": interrupt, "source": source}
        )
        if self._raises:
            raise self._raises
        task = None
        if self._reply is not None:

            async def _turn():
                await asyncio.sleep(0)
                return self._reply

            task = asyncio.create_task(_turn())
        return (client_uid or "c1", task)


def make_client(gate: ExternalInputGate, ws_handler) -> "TestClient":  # noqa: F821
    from fastapi import FastAPI
    from fastapi.testclient import TestClient

    app = FastAPI()
    app.include_router(init_external_input_routes(gate=gate, ws_handler=ws_handler))
    return TestClient(app)


@pytest.fixture
def gate(tmp_path):
    return ExternalInputGate(state_path=str(tmp_path / "external_input.json"))


# ─────────────────────────────────────────────────────────────
# 1. 鉴权矩阵
# ─────────────────────────────────────────────────────────────
class TestGateAuthorize:
    def test_disabled_by_default_rejects_even_loopback(self, gate):
        assert gate.state.enabled is False
        reason = gate.authorize("127.0.0.1", None)
        assert reason and "已关闭" in reason

    def test_enabled_loopback_without_token_allowed(self, gate):
        gate.state.enabled = True
        assert gate.authorize("127.0.0.1", None) is None
        assert gate.authorize("::1", None) is None

    def test_enabled_remote_rejected_without_allow_remote(self, gate):
        gate.state.enabled = True
        reason = gate.authorize("192.168.1.30", None)
        assert reason and "仅允许本机" in reason

    def test_remote_requires_token_configured(self, gate):
        gate.state.enabled = True
        gate.state.allow_remote = True
        reason = gate.authorize("192.168.1.30", "whatever")
        assert reason and "访问令牌" in reason

    def test_remote_with_matching_token_allowed(self, gate):
        gate.state.enabled = True
        gate.state.allow_remote = True
        gate.state.token = "s3cret"
        assert gate.authorize("192.168.1.30", "s3cret") is None
        assert gate.authorize("192.168.1.30", "wrong") == "访问令牌不正确"

    def test_token_also_enforced_for_loopback(self, gate):
        gate.state.enabled = True
        gate.state.token = "s3cret"
        assert gate.authorize("127.0.0.1", "s3cret") is None
        assert gate.authorize("127.0.0.1", None) == "访问令牌不正确"

    def test_state_roundtrip_persists(self, tmp_path):
        path = str(tmp_path / "state.json")
        g1 = ExternalInputGate(state_path=path)
        g1.state = ExternalInputState(enabled=True, allow_remote=True, token="abc")
        g1.save()
        g2 = ExternalInputGate(state_path=path).load()
        assert (g2.state.enabled, g2.state.allow_remote, g2.state.token) == (True, True, "abc")

    def test_corrupt_state_file_falls_back_to_disabled(self, tmp_path):
        path = tmp_path / "state.json"
        path.write_text("{not json", encoding="utf-8")
        g = ExternalInputGate(state_path=str(path)).load()
        assert g.state.enabled is False


# ─────────────────────────────────────────────────────────────
# 2. HTTP 层
# ─────────────────────────────────────────────────────────────
class TestExternalHttp:
    def test_status_shape_default_off(self, gate):
        client = make_client(gate, FakeWsHandler())
        body = client.get("/api/external/status").json()
        assert body["ok"] is True
        assert body["enabled"] is False
        assert body["token_set"] is False
        assert body["endpoint"] == "/api/external/chat"
        assert body["clients"] == ["c1"]
        assert body["limits"]["max_text_chars"] == MAX_TEXT_CHARS

    def test_chat_rejected_when_disabled(self, gate):
        client = make_client(gate, FakeWsHandler())
        r = client.post("/api/external/chat", json={"text": "你好"})
        assert r.status_code == 403
        assert "已关闭" in r.json()["error"]
        assert gate.rejected == 1 and gate.accepted == 0

    def test_settings_enable_then_chat_accepted(self, gate):
        ws = FakeWsHandler(reply=None)
        client = make_client(gate, ws)
        r = client.post("/api/external/settings", json={"enabled": True})
        assert r.status_code == 200 and r.json()["enabled"] is True
        assert r.json()["persisted"] is True

        r = client.post("/api/external/chat", json={"text": "  你好呀  "})
        assert r.status_code == 202
        assert r.json()["accepted"] is True
        assert r.json()["client_uid"] == "c1"
        assert ws.calls[0]["text"] == "你好呀"  # 已 strip
        assert ws.calls[0]["source"].startswith("external:")

    def test_settings_rejects_allow_remote_without_token(self, gate):
        client = make_client(gate, FakeWsHandler())
        r = client.post(
            "/api/external/settings", json={"enabled": True, "allow_remote": True}
        )
        assert r.status_code == 400
        assert "访问令牌" in r.json()["error"]

    def test_settings_regenerate_token_then_readable_locally(self, gate):
        client = make_client(gate, FakeWsHandler())
        r = client.post(
            "/api/external/settings",
            json={"enabled": True, "allow_remote": True, "regenerate_token": True},
        )
        assert r.status_code == 200 and r.json()["token_set"] is True
        token = client.get("/api/external/token").json()["token"]
        assert isinstance(token, str) and len(token) >= 16

    def test_settings_token_clear(self, gate):
        gate.state = ExternalInputState(enabled=True, token="old")
        client = make_client(gate, FakeWsHandler())
        r = client.post("/api/external/settings", json={"token_clear": True})
        assert r.status_code == 200 and r.json()["token_set"] is False

    def test_chat_requires_text(self, gate):
        gate.state.enabled = True
        client = make_client(gate, FakeWsHandler())
        assert client.post("/api/external/chat", json={}).status_code == 400
        assert client.post("/api/external/chat", json={"text": "   "}).status_code == 400

    def test_chat_rejects_overlong_text(self, gate):
        gate.state.enabled = True
        client = make_client(gate, FakeWsHandler())
        r = client.post("/api/external/chat", json={"text": "字" * (MAX_TEXT_CHARS + 1)})
        assert r.status_code == 413

    def test_chat_token_via_header(self, gate):
        gate.state = ExternalInputState(enabled=True, token="tk")
        client = make_client(gate, FakeWsHandler(reply=None))
        assert client.post("/api/external/chat", json={"text": "hi"}).status_code == 403
        ok = client.post(
            "/api/external/chat", json={"text": "hi"}, headers={"X-Live2DAI-Token": "tk"}
        )
        assert ok.status_code == 202
        bearer = client.post(
            "/api/external/chat",
            json={"text": "hi"},
            headers={"Authorization": "Bearer tk"},
        )
        assert bearer.status_code == 202

    def test_chat_wait_reply_returns_text(self, gate):
        gate.state.enabled = True
        client = make_client(gate, FakeWsHandler(reply="我在呢"))
        r = client.post(
            "/api/external/chat", json={"text": "在吗", "wait_reply": True, "timeout_s": 5}
        )
        assert r.status_code == 200
        assert r.json()["reply"] == "我在呢"
        assert r.json()["completed"] is True

    def test_chat_no_client_returns_409(self, gate):
        gate.state.enabled = True
        client = make_client(gate, FakeWsHandler(raises=LookupError("没有已连接的前端会话")))
        r = client.post("/api/external/chat", json={"text": "hi"})
        assert r.status_code == 409
        assert "没有已连接" in r.json()["error"]

    def test_chat_busy_returns_409(self, gate):
        gate.state.enabled = True
        client = make_client(gate, FakeWsHandler(raises=RuntimeError("角色正在说话")))
        r = client.post("/api/external/chat", json={"text": "hi"})
        assert r.status_code == 409

    def test_recent_records_kept(self, gate):
        gate.state.enabled = True
        client = make_client(gate, FakeWsHandler(reply=None))
        client.post("/api/external/chat", json={"text": "第一句"})
        body = client.get("/api/external/status").json()
        assert body["accepted"] == 1
        assert body["recent"][0]["text"] == "第一句"
        assert body["recent"][0]["ok"] is True


# ─────────────────────────────────────────────────────────────
# 3. WebSocketHandler.inject_text_input
# ─────────────────────────────────────────────────────────────
class FakeWebSocket:
    def __init__(self):
        self.sent: list[str] = []

    async def send_text(self, payload: str) -> None:
        self.sent.append(payload)


def make_handler(monkeypatch):
    """构造只做注入所需接线的 WebSocketHandler（不初始化任何引擎）。"""
    from open_llm_vtuber.websocket_handler import WebSocketHandler

    handler = WebSocketHandler.__new__(WebSocketHandler)
    handler.client_connections = {}
    handler.client_contexts = {}
    handler.current_conversation_tasks = {}
    handler.received_data_buffers = {}
    triggered: list[dict] = []

    async def fake_trigger(websocket, client_uid, data):
        triggered.append({"client_uid": client_uid, "data": data})

        async def _turn():
            await asyncio.sleep(0)
            return "假回复"

        handler.current_conversation_tasks[client_uid] = asyncio.create_task(_turn())

    monkeypatch.setattr(handler, "_handle_conversation_trigger", fake_trigger)
    return handler, triggered


class TestInjectTextInput:
    def test_raises_when_no_client(self, monkeypatch):
        handler, _ = make_handler(monkeypatch)
        with pytest.raises(LookupError, match="没有已连接"):
            asyncio.run(handler.inject_text_input("你好"))

    def test_unknown_client_uid_raises(self, monkeypatch):
        handler, _ = make_handler(monkeypatch)
        handler.client_connections["a"] = FakeWebSocket()
        with pytest.raises(LookupError, match="不存在或已断开"):
            asyncio.run(handler.inject_text_input("你好", client_uid="ghost"))

    def test_defaults_to_latest_client_and_echoes(self, monkeypatch):
        handler, triggered = make_handler(monkeypatch)
        handler.client_connections["old"] = FakeWebSocket()
        latest = FakeWebSocket()
        handler.client_connections["new"] = latest

        uid, task = asyncio.run(handler.inject_text_input("外部来的话", source="external:1.2.3.4"))
        assert uid == "new"
        assert task is not None
        assert triggered[0]["client_uid"] == "new"
        assert triggered[0]["data"] == {"type": "text-input", "text": "外部来的话"}
        echo = json.loads(latest.sent[0])
        assert echo["type"] == "external-input"
        assert echo["text"] == "外部来的话"
        assert echo["source"] == "external:1.2.3.4"

    def test_busy_without_interrupt_raises(self, monkeypatch):
        handler, _ = make_handler(monkeypatch)
        handler.client_connections["c"] = FakeWebSocket()

        async def scenario():
            async def _busy():
                await asyncio.sleep(5)

            handler.current_conversation_tasks["c"] = asyncio.create_task(_busy())
            await asyncio.sleep(0)
            try:
                with pytest.raises(RuntimeError, match="正在说话"):
                    await handler.inject_text_input("插一句")
            finally:
                handler.current_conversation_tasks["c"].cancel()

        asyncio.run(scenario())

    def test_busy_with_interrupt_cancels_then_injects(self, monkeypatch):
        handler, triggered = make_handler(monkeypatch)
        handler.client_connections["c"] = FakeWebSocket()
        handler.client_contexts["c"] = type("Ctx", (), {"send_text": None})()

        async def scenario():
            async def _busy():
                await asyncio.sleep(5)

            busy = asyncio.create_task(_busy())
            handler.current_conversation_tasks["c"] = busy
            await asyncio.sleep(0)
            uid, _ = await handler.inject_text_input("打断一下", interrupt=True)
            assert uid == "c"
            await asyncio.sleep(0)  # 让 cancel 真正落地
            assert busy.cancelled() or busy.done()
            assert triggered[0]["data"]["text"] == "打断一下"

        asyncio.run(scenario())
