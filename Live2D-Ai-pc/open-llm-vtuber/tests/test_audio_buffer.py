# -*- coding: utf-8 -*-
"""音频缓冲累积测试（审计 E-4 的回归保护）。

被测：`WebSocketHandler._append_audio` + `conversations.handle_conversation_trigger`
的 mic-audio-end 取用路径。

关键契约（不能因为"改成 list 累积"而破坏）：
  1. 多个 chunk 追加后，取出的是**按到达顺序拼接**的一维 float32 波形；
  2. 取出后缓冲被清空（下一轮不会带上一轮的尾巴）；
  3. 空缓冲取出得到空数组而不是抛错；
  4. int16 原始字节（VAD 路径）与 float 列表（前端 mic 路径）都能正确累积。
"""

import asyncio

import numpy as np
import pytest


def make_handler():
    from open_llm_vtuber.websocket_handler import WebSocketHandler

    handler = WebSocketHandler.__new__(WebSocketHandler)
    handler.client_connections = {}
    handler.client_contexts = {}
    handler.current_conversation_tasks = {}
    handler.received_data_buffers = {}
    return handler


class TestAppendAudio:
    def test_appends_chunks_as_arrays(self):
        handler = make_handler()
        handler._append_audio("c", [0.1, 0.2])
        handler._append_audio("c", [0.3])
        chunks = handler.received_data_buffers["c"]
        assert len(chunks) == 2
        assert all(chunk.dtype == np.float32 for chunk in chunks)
        assert np.concatenate(chunks).tolist() == pytest.approx([0.1, 0.2, 0.3], abs=1e-6)

    def test_empty_chunk_ignored(self):
        handler = make_handler()
        handler._append_audio("c", [])
        handler._append_audio("c", np.array([], dtype=np.float32))
        assert handler.received_data_buffers.get("c", []) == []

    def test_int16_bytes_path(self):
        handler = make_handler()
        raw = np.array([1000, -1000, 32767], dtype=np.int16)
        handler._append_audio("c", raw.astype(np.float32))
        assert np.concatenate(handler.received_data_buffers["c"]).tolist() == [
            1000.0,
            -1000.0,
            32767.0,
        ]

    def test_buffers_are_per_client(self):
        handler = make_handler()
        handler._append_audio("a", [1.0])
        handler._append_audio("b", [2.0, 3.0])
        assert np.concatenate(handler.received_data_buffers["a"]).size == 1
        assert np.concatenate(handler.received_data_buffers["b"]).size == 2

    def test_many_chunks_accumulate_linearly(self):
        """1000 个 chunk 仍是 1000 个引用（不是每次全量拷贝出的新数组）。"""
        handler = make_handler()
        for _ in range(1000):
            handler._append_audio("c", [0.5, 0.5])
        assert len(handler.received_data_buffers["c"]) == 1000
        assert np.concatenate(handler.received_data_buffers["c"]).size == 2000


class TestMicAudioEndConsumption:
    """mic-audio-end 取用路径：拼接顺序 + 清空 + 空缓冲兜底。"""

    def _trigger(self, buffers, client_uid="c"):
        from open_llm_vtuber.conversations import conversation_handler as ch

        captured = {}

        async def fake_process(**kwargs):
            captured["user_input"] = kwargs["user_input"]
            return ""

        original = ch.process_single_conversation
        ch.process_single_conversation = fake_process
        try:

            async def run():
                class FakeGroupManager:
                    def get_client_group(self, uid):
                        return None

                class FakeWs:
                    async def send_text(self, _):
                        return None

                tasks = {}
                await ch.handle_conversation_trigger(
                    msg_type="mic-audio-end",
                    data={},
                    client_uid=client_uid,
                    context=object(),
                    websocket=FakeWs(),
                    client_contexts={},
                    client_connections={},
                    chat_group_manager=FakeGroupManager(),
                    received_data_buffers=buffers,
                    current_conversation_tasks=tasks,
                    broadcast_to_group=None,
                )
                await tasks[client_uid]

            asyncio.run(run())
        finally:
            ch.process_single_conversation = original
        return captured["user_input"]

    def test_concatenates_in_arrival_order_and_clears(self):
        buffers = {
            "c": [
                np.array([0.1, 0.2], dtype=np.float32),
                np.array([0.3], dtype=np.float32),
            ]
        }
        user_input = self._trigger(buffers)
        assert isinstance(user_input, np.ndarray)
        assert user_input.tolist() == pytest.approx([0.1, 0.2, 0.3], abs=1e-6)
        assert buffers["c"] == []  # 清空，避免下一轮串音

    def test_empty_buffer_yields_empty_array(self):
        buffers = {"c": []}
        user_input = self._trigger(buffers)
        assert isinstance(user_input, np.ndarray)
        assert user_input.size == 0

    def test_missing_client_key_does_not_raise(self):
        buffers = {}
        user_input = self._trigger(buffers)
        assert user_input.size == 0
