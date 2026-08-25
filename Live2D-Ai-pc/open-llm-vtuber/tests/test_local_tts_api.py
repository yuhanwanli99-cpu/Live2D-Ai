"""PLAN-V2-PC-LOCAL-TTS · 阶段 3 接口契约测试。

覆盖 POST /api/tts/local/synthesize 与 GET /api/tts/local/audio：
  - 正常合成返回 WAV 元数据，且可下载
  - 空文本 / 超长文本 / 非法语言被拒绝
  - 路径穿越 / 非白名单文件名被拒绝
  - 引擎按参数缓存，重复调用不重复加载

所有 Melo 依赖均 mock，不触发真实模型下载。
"""
import os
import wave
from unittest import mock

import pytest

from open_llm_vtuber.tts import tts_factory as tts_factory_module


def _write_wav(path: str) -> str:
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with wave.open(path, "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(16000)
        wav_file.writeframes(b"\x00\x00" * 8000)  # 0.5s of silence
    return path


class FakeMeloEngine:
    def __init__(self, cache_dir):
        self.cache_dir = cache_dir
        self.texts: list[str] = []

    async def async_generate_audio(self, text, file_name_no_ext=None, instruction=""):
        self.texts.append(text)
        path = os.path.join(self.cache_dir, f"{file_name_no_ext}.wav")
        return _write_wav(path)


@pytest.fixture
def melo_engine_factory(settings_env):
    """把 TTSFactory.get_tts_engine 替换为 fake，并返回调用计数器。"""
    cache_dir = settings_env.conf_path.parent / "cache"
    cache_dir.mkdir(parents=True, exist_ok=True)
    engine = FakeMeloEngine(str(cache_dir))
    calls = {"count": 0, "params": []}

    def fake_get_tts_engine(engine_type, **params):
        calls["count"] += 1
        calls["params"].append(params)
        assert engine_type == "melo_tts"
        return engine

    patcher = mock.patch.object(
        tts_factory_module.TTSFactory, "get_tts_engine", fake_get_tts_engine
    )
    patcher.start()
    try:
        yield engine, calls, cache_dir
    finally:
        patcher.stop()


def test_synthesize_ok_and_audio_download(settings_env, melo_engine_factory):
    engine, calls, _cache_dir = melo_engine_factory
    res = settings_env.client.post(
        "/api/tts/local/synthesize",
        json={"text": "你好，这是本地合成测试。", "language": "ZH", "device": "cpu"},
    )
    assert res.status_code == 200, res.text
    data = res.json()
    assert data["ok"] is True
    assert data["audio_path"].startswith("cache/tts_local_")
    assert data["audio_url"].startswith("/api/tts/local/audio?name=")
    assert data["format"] == "wav"
    assert isinstance(data["duration_ms"], int)
    assert data["duration_ms"] > 0
    assert calls["count"] == 1
    assert engine.texts == ["你好，这是本地合成测试。"]

    audio = settings_env.client.get(data["audio_url"])
    assert audio.status_code == 200, audio.text
    assert audio.headers["content-type"] == "audio/wav"


def test_synthesize_reuses_cached_engine(settings_env, melo_engine_factory):
    engine, calls, _ = melo_engine_factory
    for text in ("第一句", "第二句"):
        res = settings_env.client.post(
            "/api/tts/local/synthesize", json={"text": text, "language": "ZH"}
        )
        assert res.status_code == 200, res.text
    assert calls["count"] == 1
    assert len(engine.texts) == 2


def test_synthesize_rejects_empty_text(settings_env, melo_engine_factory):
    res = settings_env.client.post(
        "/api/tts/local/synthesize", json={"text": "", "language": "ZH"}
    )
    assert res.status_code == 400, res.text
    assert res.json()["ok"] is False


def test_synthesize_rejects_too_long_text(settings_env, melo_engine_factory):
    res = settings_env.client.post(
        "/api/tts/local/synthesize", json={"text": "长" * 201, "language": "ZH"}
    )
    assert res.status_code == 400, res.text
    assert "200" in res.json()["error"]


def test_synthesize_rejects_invalid_language(settings_env, melo_engine_factory):
    res = settings_env.client.post(
        "/api/tts/local/synthesize", json={"text": "你好", "language": "XX"}
    )
    assert res.status_code == 400, res.text
    assert "语言" in res.json()["error"]


@pytest.mark.parametrize(
    "name",
    [
        "../conf.yaml",
        "..%2Fconf.yaml",
        "tts_local_..%2Fevil.wav",
        "settings_preview.wav",
        "tts_local_no_ext",
    ],
)
def test_audio_rejects_bad_names(settings_env, name):
    res = settings_env.client.get(f"/api/tts/local/audio?name={name}")
    assert res.status_code in (400, 403, 404), (name, res.status_code, res.text)


def test_audio_missing_file_returns_404(settings_env):
    res = settings_env.client.get("/api/tts/local/audio?name=tts_local_missing.wav")
    assert res.status_code == 404, res.text
    assert res.json()["ok"] is False
