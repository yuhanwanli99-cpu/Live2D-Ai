# -*- coding: utf-8 -*-
"""聊天历史绑定与持久化测试（P0-3）。

被测：`WebSocketHandler._attach_session_history()` + `chat_history_manager` 落盘。

要锁定的核心事实（这就是本轮修掉的缺陷）：
  `ServiceContext.history_uid` 初值为空串，而 `process_single_conversation` 只在
  `history_uid` 非空时 `store_message` —— 此前没有任何地方在连接时绑定历史，
  新 renderer 也从不发 `create-new-history`，于是**一条消息都没落过盘**。

约定：
  1. 新连接会绑定历史：有历史 → **续接最近一段**；没有 → 新建；
  2. 绑定后会把该段历史（过滤 metadata/system）以 `history-data` 回灌前端，带 history_uid；
  3. 同时把 LLM 记忆接上（`set_memory_from_history`）；
  4. 任何一步失败都**不能**让连接建立失败（降级为"本次不落盘"）。

所有用例都 chdir 到 tmp_path，绝不写仓库真实 `chat_history/`。
"""

import asyncio
import json
import os

import pytest

from open_llm_vtuber import chat_history_manager as chm


class FakeWebSocket:
    def __init__(self):
        self.sent = []

    async def send_text(self, payload: str) -> None:
        self.sent.append(json.loads(payload))


class FakeAgent:
    def __init__(self, raises: bool = False):
        self.calls = []
        self.raises = raises

    def set_memory_from_history(self, conf_uid: str, history_uid: str) -> None:
        if self.raises:
            raise RuntimeError("memory restore boom")
        self.calls.append((conf_uid, history_uid))


class FakeContext:
    def __init__(self, conf_uid: str = "bai_001", agent: FakeAgent | None = None):
        self.character_config = type("CC", (), {"conf_uid": conf_uid})()
        self.agent_engine = agent or FakeAgent()
        self.history_uid = ""


def make_handler():
    from open_llm_vtuber.websocket_handler import WebSocketHandler

    handler = WebSocketHandler.__new__(WebSocketHandler)
    handler.client_connections = {}
    handler.client_contexts = {}
    handler.current_conversation_tasks = {}
    handler.received_data_buffers = {}
    return handler


@pytest.fixture
def workdir(tmp_path, monkeypatch):
    """把 cwd 切到 tmp_path：chat_history_manager 用的是相对路径 'chat_history'。"""
    monkeypatch.chdir(tmp_path)
    return tmp_path


def attach(handler, ws, ctx):
    return asyncio.run(handler._attach_session_history(ws, ctx))


class TestAttachOnConnect:
    def test_creates_history_when_none_exists(self, workdir):
        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext()
        uid = attach(handler, ws, ctx)

        assert uid, "应创建并返回 history_uid"
        assert ctx.history_uid == uid, "context 必须被绑定，否则消息不会落盘"
        assert os.path.isfile(os.path.join("chat_history", "bai_001", f"{uid}.json"))

    def test_sends_history_data_with_uid(self, workdir):
        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext()
        uid = attach(handler, ws, ctx)

        msg = next(m for m in ws.sent if m["type"] == "history-data")
        assert msg["history_uid"] == uid
        assert msg["messages"] == []  # 新会话没有历史消息

    def test_restores_latest_history_and_messages(self, workdir):
        # 预置一段带消息的历史
        uid = chm.create_new_history("bai_001")
        chm.store_message("bai_001", uid, "human", "你好")
        chm.store_message("bai_001", uid, "ai", "你好呀")

        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext()
        attached = attach(handler, ws, ctx)

        assert attached == uid, "应续接最近一段历史，而不是每次新建"
        msg = next(m for m in ws.sent if m["type"] == "history-data")
        assert [m["content"] for m in msg["messages"]] == ["你好", "你好呀"]
        assert all(m["role"] != "metadata" for m in msg["messages"]), "metadata 不能回灌前端"

    def test_picks_newest_when_multiple(self, workdir):
        old = chm.create_new_history("bai_001")
        chm.store_message("bai_001", old, "human", "旧会话")
        newer = chm.create_new_history("bai_001")
        chm.store_message("bai_001", newer, "human", "新会话")
        # 两次落盘可能落在同一秒（时间戳精度到秒 → 排序会打平变成不确定），
        # 这里显式把旧会话的时间戳改到昨天，让"取最新"这条断言是确定性的。
        old_path = os.path.join("chat_history", "bai_001", f"{old}.json")
        with open(old_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        for item in data:
            item["timestamp"] = "2026-08-19T10:00:00"
        with open(old_path, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False)

        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext()
        attached = attach(handler, ws, ctx)
        msg = next(m for m in ws.sent if m["type"] == "history-data")
        assert [m["content"] for m in msg["messages"]] == ["新会话"]
        assert attached == newer

    def test_connects_llm_memory(self, workdir):
        agent = FakeAgent()
        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext(agent=agent)
        uid = attach(handler, ws, ctx)
        assert agent.calls == [("bai_001", uid)], "界面有上文但模型不记得 = 半个功能"


class TestDegradation:
    def test_missing_conf_uid_does_not_raise(self, workdir):
        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext(conf_uid="")
        assert attach(handler, ws, ctx) is None
        assert ctx.history_uid == ""
        assert ws.sent == []

    def test_memory_restore_failure_still_binds_history(self, workdir):
        """记忆恢复失败不能连带丢掉落盘能力。"""
        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext(agent=FakeAgent(raises=True))
        uid = attach(handler, ws, ctx)
        assert uid and ctx.history_uid == uid
        assert any(m["type"] == "history-data" for m in ws.sent)

    def test_send_failure_is_swallowed(self, workdir):
        """回灌失败（前端已断开）不该让连接建立失败。"""

        class BrokenWs:
            async def send_text(self, _):
                raise RuntimeError("socket closed")

        handler, ctx = make_handler(), FakeContext()
        assert attach(handler, BrokenWs(), ctx) is None  # 降级返回 None，不抛


class TestPersistenceRoundTrip:
    def test_stored_messages_survive_and_are_listed(self, workdir):
        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext()
        uid = attach(handler, ws, ctx)

        # 模拟一轮对话落盘（single_conversation 的 store_message 路径）
        chm.store_message("bai_001", uid, "human", "记得我喜欢猫")
        chm.store_message("bai_001", uid, "ai", "记住了")

        # 新连接（等价于刷新页面）应恢复这两条
        ws2, ctx2 = FakeWebSocket(), FakeContext()
        assert attach(make_handler(), ws2, ctx2) == uid
        restored = next(m for m in ws2.sent if m["type"] == "history-data")["messages"]
        assert [m["content"] for m in restored] == ["记得我喜欢猫", "记住了"]

        listed = chm.get_history_list("bai_001")
        assert listed and listed[0]["uid"] == uid
        assert listed[0]["latest_message"]["content"] == "记住了"

    def test_delete_removes_file(self, workdir):
        handler, ws, ctx = make_handler(), FakeWebSocket(), FakeContext()
        uid = attach(handler, ws, ctx)
        chm.store_message("bai_001", uid, "human", "x")
        assert chm.delete_history("bai_001", uid) is True
        assert not os.path.exists(os.path.join("chat_history", "bai_001", f"{uid}.json"))
