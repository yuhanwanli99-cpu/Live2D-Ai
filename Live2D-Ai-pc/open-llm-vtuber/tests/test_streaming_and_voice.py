# -*- coding: utf-8 -*-
"""流式上屏（P1-2）与语音上行链路（P0-2）的后端契约测试。

P1-2：`emo_interceptor` 在产出干净文本的同时旁听一路 `text_delta` 事件，
      `single_conversation` 把它转成 WS `text-delta` 发给前端；
      **分段与 TTS 链路必须一字未改**（干净文本照原样继续下行）。

P0-2：前端把麦克风音频送回后端（`mic-audio-data` → `mic-audio-end`），
      走的是既有 WS 处理器；本测试锁定"音频进得来、能被 ASR 取到"。
"""

import asyncio

import numpy as np
import pytest

from open_llm_vtuber.agent.transformers import emo_interceptor


def collect(stream_factory):
    async def run():
        items = []
        async for item in stream_factory():
            items.append(item)
        return items

    return asyncio.run(run())


class TestTextDeltaStream:
    def test_emits_text_delta_alongside_clean_text(self):
        @emo_interceptor()
        async def stream():
            yield "你好"
            yield "，今天"
            yield "很开心"

        items = collect(stream)
        deltas = [i["text"] for i in items if isinstance(i, dict) and i.get("type") == "text_delta"]
        texts = [i for i in items if isinstance(i, str)]

        # 增量事件拼起来 == 下行文本拼起来（不能只发其中一路）
        assert "".join(deltas) == "".join(texts)
        assert "你好" in "".join(deltas)

    def test_downstream_text_unchanged(self):
        """分段/TTS 拿到的仍是原样干净文本（旁听不改主链）。"""

        @emo_interceptor()
        async def stream():
            yield "第一句。"
            yield "第二句。"

        items = collect(stream)
        assert "".join(i for i in items if isinstance(i, str)) == "第一句。第二句。"

    def test_emotion_tag_stripped_from_delta(self):
        """`[emo:joy]` 由拦截器剥离 → 增量里不该出现控制标记。"""

        @emo_interceptor()
        async def stream():
            yield "[emo:joy]"
            yield "太好了"

        items = collect(stream)
        deltas = "".join(
            i["text"] for i in items if isinstance(i, dict) and i.get("type") == "text_delta"
        )
        assert "emo:" not in deltas
        assert "太好了" in deltas
        # 情绪事件本身仍要产出（表情先行）
        assert any(isinstance(i, dict) and i.get("type") == "emo_token" for i in items)

    def test_no_delta_for_empty_stream(self):
        @emo_interceptor()
        async def stream():
            if False:  # pragma: no cover
                yield ""
            return

        items = collect(stream)
        assert not any(isinstance(i, dict) and i.get("type") == "text_delta" for i in items)

    def test_conversation_forwards_text_delta_as_ws_message(self):
        """`single_conversation` 必须把 text_delta 转成 `{"type":"text-delta"}`。"""
        import inspect

        from open_llm_vtuber.conversations import single_conversation

        source = inspect.getsource(single_conversation.process_single_conversation)
        assert '"text_delta"' in source
        assert '"type": "text-delta"' in source


class TestMicAudioUplink:
    """P0-2：`mic-audio-data` 累积 → `mic-audio-end` 时能被 ASR 取到完整波形。"""

    def _handler(self):
        from open_llm_vtuber.websocket_handler import WebSocketHandler

        handler = WebSocketHandler.__new__(WebSocketHandler)
        handler.client_connections = {}
        handler.client_contexts = {}
        handler.current_conversation_tasks = {}
        handler.received_data_buffers = {}
        return handler

    def test_float_frames_accumulate_in_order(self):
        handler = self._handler()

        async def run():
            for frame in ([0.1, 0.2], [0.3], [0.4, 0.5]):
                await handler._handle_audio_data(None, "c", {"audio": frame})

        asyncio.run(run())
        merged = np.concatenate(handler.received_data_buffers["c"])
        assert merged.tolist() == pytest.approx([0.1, 0.2, 0.3, 0.4, 0.5], abs=1e-6)
        assert merged.dtype == np.float32

    def test_empty_frame_ignored(self):
        handler = self._handler()
        asyncio.run(handler._handle_audio_data(None, "c", {"audio": []}))
        assert handler.received_data_buffers.get("c", []) == []

    def test_realistic_frame_size_2048(self):
        """前端按 2048 样本/片（16kHz 下 128ms）上行；1 秒语音 ≈ 8 片。"""
        handler = self._handler()
        frame = [0.01] * 2048

        async def run():
            for _ in range(8):
                await handler._handle_audio_data(None, "c", {"audio": frame})

        asyncio.run(run())
        merged = np.concatenate(handler.received_data_buffers["c"])
        assert merged.size == 2048 * 8  # ≈ 1.024s @16kHz
        assert len(handler.received_data_buffers["c"]) == 8  # 仍是 8 个引用（未做 O(n²) 拷贝）

    def test_mic_audio_end_consumes_and_clears(self):
        """mic-audio-end 取用后缓冲清空，下一轮不会带上一轮尾巴。"""
        from open_llm_vtuber.conversations import conversation_handler as ch

        handler = self._handler()
        captured = {}

        async def fake_process(**kwargs):
            captured["input"] = kwargs["user_input"]
            return ""

        original = ch.process_single_conversation
        ch.process_single_conversation = fake_process
        try:

            async def run():
                await handler._handle_audio_data(None, "c", {"audio": [0.5] * 4})

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
                    client_uid="c",
                    context=object(),
                    websocket=FakeWs(),
                    client_contexts={},
                    client_connections={},
                    chat_group_manager=FakeGroupManager(),
                    received_data_buffers=handler.received_data_buffers,
                    current_conversation_tasks=tasks,
                    broadcast_to_group=None,
                )
                await tasks["c"]

            asyncio.run(run())
        finally:
            ch.process_single_conversation = original

        assert isinstance(captured["input"], np.ndarray)
        assert captured["input"].size == 4
        assert handler.received_data_buffers["c"] == []
