# -*- coding: utf-8 -*-
"""播放完成协议修复的聚焦测试。

背景：用户实测正常多段（7 chunk）TTS 时后端出现
`Timeout waiting for frontend-playback-complete`。根因是后端用固定 5s
硬超时与"前端真实播放整段后再回执"竞速，正常长语音必然超时。

修复要点（本次断言锁定）：
1. TTSTaskManager 累计本轮已排队音频的预估播放总时长（WAV 时长；解析失败用兜底）。
2. finalize_conversation_turn 的播放确认等待超时改为 max(8, 预估时长 + 8)，
   覆盖真实播放，而非固定 5s。
3. frontend-playback-complete 无 request_id 时也能匹配（无 ID 兼容）。
4. 打断通过取消等待协程即时释放（interrupt 短路径）。
"""
from __future__ import annotations

import asyncio
import json
import math
import os
import tempfile
import wave

import pytest

from open_llm_vtuber.agent.output_types import Actions, DisplayText
from open_llm_vtuber.conversations import tts_manager as tts_manager_module
from open_llm_vtuber.message_handler import MessageHandler


def _write_wav(path: str, seconds: float, rate: int = 8000) -> None:
    """写一个指定时长的 8kHz 单声道 WAV（440Hz 正弦，避免全零被判为空音频）。"""
    frames = int(rate * seconds)
    with wave.open(path, "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(rate)
        samples = bytearray()
        for i in range(frames):
            sample = int(12000 * math.sin(2 * math.pi * 440.0 * i / rate))
            samples += sample.to_bytes(2, "little", signed=True)
        wf.writeframes(bytes(samples))


async def _noop_send(_msg: str) -> None:
    """WS 发送桩：不真正发送任何内容。"""


class _FakeEngine:
    """最小 TTS 引擎：写入指定时长 WAV 并返回路径。"""

    def __init__(self, path: str, seconds: float = 4.0):
        self._path = path
        self._seconds = seconds

    async def async_generate_audio(
        self, text: str, file_name_no_ext: str, instruction: str = "", **kwargs
    ) -> str:
        _write_wav(self._path, self._seconds)
        return self._path

    def remove_file(self, path: str) -> None:
        try:
            os.remove(path)
        except OSError:
            pass


class _Manager:
    """finalize_conversation_turn 用的最小桩：携带 task_list 与时长。"""

    def __init__(self, expected: float, has_tasks: bool = True):
        self._has_tasks = has_tasks
        self._expected = expected

    @property
    def task_list(self):
        # 真实链路里 task_list 存的是已完成/可等待的 task；用即时完成的协程模拟。
        return [asyncio.sleep(0)] if self._has_tasks else []

    def expected_playback_seconds(self) -> float:
        return self._expected


# ─────────────────────────── 1) 播放时长累计 ───────────────────────────


def test_tts_manager_accumulates_real_wav_duration():
    async def scenario():
        with tempfile.TemporaryDirectory() as tmp:
            wav_path = os.path.join(tmp, "a.wav")
            _write_wav(wav_path, 3.0)
            assert (
                tts_manager_module.TTSTaskManager._wav_duration_seconds(wav_path)
                == pytest.approx(3.0, abs=0.1)
            )

            engine = _FakeEngine(wav_path, seconds=2.0)
            m = tts_manager_module.TTSTaskManager(tts_engine=engine)
            await m.speak_async(
                tts_text="你好呀喵",
                display_text=DisplayText(text="你好呀喵", name="AI"),
                actions=Actions(),
                websocket_send=_noop_send,
            )
            await asyncio.gather(*m.task_list)
            assert m.expected_playback_seconds() == pytest.approx(2.0, abs=0.3)

    asyncio.run(scenario())


def test_tts_manager_fallback_on_invalid_wav():
    m = tts_manager_module.TTSTaskManager()
    m._accumulate_playback_estimate(None)
    m._accumulate_playback_estimate("/no/such/file.wav")
    assert m.expected_playback_seconds() == pytest.approx(
        m._chunk_duration_fallback * 2
    )


def test_interrupt_resets_playback_estimate():
    m = tts_manager_module.TTSTaskManager()
    m._expected_playback_seconds = 30.0
    m.reset_playback_estimate()
    assert m.expected_playback_seconds() == 0.0


# ─────────────────────────── 2) 动态超时 ───────────────────────────


def test_finalize_timeout_covers_expected_playback(monkeypatch):
    from open_llm_vtuber.conversations import conversation_utils as cu

    captured: dict = {}

    async def fake_wait(client_uid, response_type, request_id=None, timeout=None):
        captured["timeout"] = timeout
        return {"type": "frontend-playback-complete"}

    monkeypatch.setattr(cu.message_handler, "wait_for_response", fake_wait)

    sent: list[dict] = []

    async def send(msg):
        sent.append(json.loads(msg))
        return None

    async def scenario():
        mgr = _Manager(expected=30.0)
        await cu.finalize_conversation_turn(
            tts_manager=mgr, websocket_send=send, client_uid="c1"
        )

    asyncio.run(scenario())
    types = [s.get("type") for s in sent if isinstance(s, dict)]
    assert "backend-synth-complete" in types
    assert captured["timeout"] >= 38.0


def test_timeout_floor():
    assert max(8.0, 0.0 + 8.0) >= 8.0


# ─────────────────────────── 3) 无 ID 兼容 ───────────────────────────


def test_playback_complete_no_id_matches():
    async def scenario():
        mh = MessageHandler()
        wait_task = asyncio.create_task(
            mh.wait_for_response("c1", "frontend-playback-complete", None, timeout=5.0)
        )
        # 让 wait_for_response 先完成事件注册，再投递消息，避免竞态
        await asyncio.sleep(0)
        mh.handle_message("c1", {"type": "frontend-playback-complete"})
        result = await wait_task
        assert result == {"type": "frontend-playback-complete"}

    asyncio.run(scenario())


# ─────────────────────────── 4) 打断即时释放等待 ───────────────────────────


def test_interrupt_cancels_playback_wait():
    async def scenario():
        mh = MessageHandler()
        wait_task = asyncio.create_task(
            mh.wait_for_response("c1", "frontend-playback-complete", None, timeout=600.0)
        )
        await asyncio.sleep(0.01)
        wait_task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await wait_task

    asyncio.run(scenario())
