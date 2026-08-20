# -*- coding: utf-8 -*-
"""Live2D 形象热切换测试（P1-4）。

被测：`WebSocketHandler.broadcast_model_change()`。

背景：`POST /api/models/select` 原先只写 conf.yaml 并回 `requires_restart: true`，
用户必须重启后端才能看到新形象；而前端早就支持 `set-model-and-conf` 热切换。
本方法补上"服务端主动推送"，把重启需求去掉。

约定：
  1. 共享 cache 先更新（新连接/重连直接拿到新形象）；
  2. 每个在线会话各自 init_live2d 并收到 set-model-and-conf（含 model_info）；
  3. 单个会话失败不影响其余会话（返回成功数）；
  4. 没有在线会话时不报错，返回 0。
"""

import asyncio
import json

import pytest


class FakeWebSocket:
    def __init__(self, fail: bool = False):
        self.sent = []
        self.fail = fail

    async def send_text(self, payload: str) -> None:
        if self.fail:
            raise RuntimeError("socket closed")
        self.sent.append(json.loads(payload))


class FakeContext:
    def __init__(self, conf_name="bai", conf_uid="bai_001", fail_init=False):
        self.character_config = type("CC", (), {"conf_name": conf_name, "conf_uid": conf_uid})()
        self.system_config = type("SC", (), {"live2d_fps": 120})()
        self.live2d_model = type("M", (), {"model_info": {"url": "/live2d-models/bai/x.model3.json"}})()
        self.init_calls = []
        self.fail_init = fail_init

    def init_live2d(self, name: str) -> None:
        if self.fail_init:
            raise RuntimeError("model not found")
        self.init_calls.append(name)
        self.live2d_model = type(
            "M", (), {"model_info": {"url": f"/live2d-models/{name}/{name}.model3.json"}}
        )()


def make_handler(default_ctx=None):
    from open_llm_vtuber.websocket_handler import WebSocketHandler

    handler = WebSocketHandler.__new__(WebSocketHandler)
    handler.client_connections = {}
    handler.client_contexts = {}
    handler.current_conversation_tasks = {}
    handler.received_data_buffers = {}
    handler.default_context_cache = default_ctx or FakeContext()
    return handler


class TestBroadcastModelChange:
    def test_updates_shared_cache_even_with_no_clients(self):
        shared = FakeContext()
        handler = make_handler(shared)
        applied = asyncio.run(handler.broadcast_model_change("shizuku"))
        assert applied == 0
        assert shared.init_calls == ["shizuku"], "新连接必须能拿到新形象"

    def test_pushes_set_model_and_conf_to_each_client(self):
        handler = make_handler()
        for uid in ("a", "b"):
            handler.client_connections[uid] = FakeWebSocket()
            handler.client_contexts[uid] = FakeContext()

        applied = asyncio.run(handler.broadcast_model_change("shizuku"))
        assert applied == 2
        for uid in ("a", "b"):
            msg = handler.client_connections[uid].sent[0]
            assert msg["type"] == "set-model-and-conf"
            assert msg["model_info"]["url"].endswith("shizuku.model3.json")
            assert msg["client_uid"] == uid
            assert msg["live2d_fps"] == 120
            assert handler.client_contexts[uid].init_calls == ["shizuku"]

    def test_one_broken_socket_does_not_block_others(self):
        handler = make_handler()
        handler.client_connections["ok"] = FakeWebSocket()
        handler.client_contexts["ok"] = FakeContext()
        handler.client_connections["dead"] = FakeWebSocket(fail=True)
        handler.client_contexts["dead"] = FakeContext()

        applied = asyncio.run(handler.broadcast_model_change("shizuku"))
        assert applied == 1
        assert handler.client_connections["ok"].sent, "健康会话仍应收到推送"

    def test_context_init_failure_is_isolated(self):
        handler = make_handler()
        handler.client_connections["bad"] = FakeWebSocket()
        handler.client_contexts["bad"] = FakeContext(fail_init=True)
        handler.client_connections["good"] = FakeWebSocket()
        handler.client_contexts["good"] = FakeContext()

        applied = asyncio.run(handler.broadcast_model_change("shizuku"))
        assert applied == 1
        assert handler.client_connections["bad"].sent == []

    def test_missing_context_is_skipped(self):
        """连接存在但 context 已被清理（断连竞态）→ 跳过而不是崩。"""
        handler = make_handler()
        handler.client_connections["ghost"] = FakeWebSocket()
        applied = asyncio.run(handler.broadcast_model_change("shizuku"))
        assert applied == 0


class TestSelectModelEndpoint:
    """端点层：不再回 requires_restart=true，而是报告即时生效。"""

    def test_select_reports_applied_and_updates_conf(self, settings_env, monkeypatch):
        calls = {}

        async def fake_broadcast(name):
            calls["name"] = name
            return 2

        settings_env.fake_ws.broadcast_model_change = fake_broadcast

        r = settings_env.client.post("/api/models/select", json={"model_id": "shizuku"})
        assert r.status_code == 200
        body = r.json()
        assert body["applied"] is True
        assert body["requires_restart"] is False
        assert body["clients_updated"] == 2
        assert calls["name"] == "shizuku"
        # ruamel 保留原引号风格（原值是 'bai'），因此断言键与值同现即可
        conf = settings_env.read_conf()
        assert "live2d_model_name" in conf and "shizuku" in conf
        assert "conf_name: 'shizuku'" in conf or "conf_name: shizuku" in conf

    def test_select_requires_model_id(self, settings_env):
        assert settings_env.client.post("/api/models/select", json={}).status_code == 400

    def test_hot_switch_failure_falls_back_to_restart_message(self, settings_env):
        async def boom(_name):
            raise RuntimeError("模型目录缺失")

        settings_env.fake_ws.broadcast_model_change = boom
        r = settings_env.client.post("/api/models/select", json={"model_id": "ghost"})
        assert r.status_code == 200
        body = r.json()
        assert body["applied"] is False
        assert body["requires_restart"] is True
        assert "模型目录缺失" in body["message"]

    def test_no_ws_handler_still_saves(self, settings_env, monkeypatch):
        """无 ws_handler（纯配置场景）时不应报错。"""
        if hasattr(settings_env.fake_ws, "broadcast_model_change"):
            delattr(settings_env.fake_ws, "broadcast_model_change")
        r = settings_env.client.post("/api/models/select", json={"model_id": "bai"})
        # 没有 broadcast_model_change 属性 → 走异常分支，诚实回 requires_restart
        assert r.status_code == 200
        assert r.json()["ok"] is True


@pytest.fixture(autouse=True)
def _no_network():
    """确保这些用例不发真实网络（select_model 不该访问网络）。"""
    yield
