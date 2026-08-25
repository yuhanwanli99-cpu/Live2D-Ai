"""PLAN-V2-PC-LOCAL-TTS · 阶段 1 Melo 引擎与配置模型测试。

覆盖：
  - MeloTTSConfig 默认值
  - spk2id 解析与回退
  - 四韵律参数范围钳制
  - tts_to_file 参数完整传递
  - TTSFactory 的 melo 分支（voice->language / model->device 兼容映射）
"""
from typing import ClassVar
from unittest import mock

import pytest

from open_llm_vtuber.config_manager.tts import MeloTTSConfig
from open_llm_vtuber.tts import melo_tts as melo_module
from open_llm_vtuber.tts import tts_factory as factory_module


class FakeHpsData:
    spk2id: ClassVar[dict] = {"ZH": 0, "EN": 1, "ES": 2}


class FakeHps:
    data = FakeHpsData()


class FakeTTS:
    def __init__(self, language="ZH", device="cpu"):
        self.language = language
        self.device = device
        self.hps = FakeHps()
        self.calls: list[dict] = []

    def tts_to_file(self, **kwargs):
        self.calls.append(kwargs)


@pytest.fixture
def fake_melo(monkeypatch):
    fake = FakeTTS()
    monkeypatch.setattr(melo_module, "TTS", lambda language="ZH", device="cpu": fake)
    monkeypatch.setattr(melo_module, "_MELO_IMPORT_ERROR", None)
    return fake


def test_melo_config_defaults():
    cfg = MeloTTSConfig()
    assert cfg.model_dump() == {
        "language": "ZH",
        "device": "cpu",
        "speaker": None,
        "speed": 1.0,
        "sdp_ratio": 0.2,
        "noise_scale": 0.6,
        "noise_scale_w": 0.8,
        "format": "wav",
    }


def test_spk2id_language_fallback(fake_melo):
    engine = melo_module.TTSEngine(language="EN")
    assert engine.speaker_id == 1
    engine = melo_module.TTSEngine(language="ZH")
    assert engine.speaker_id == 0


def test_speaker_explicit_lookup(fake_melo):
    engine = melo_module.TTSEngine(language="ZH", speaker="EN")
    assert engine.speaker_id == 1


def test_params_clamped(fake_melo):
    engine = melo_module.TTSEngine(
        language="ZH",
        speed=99,
        sdp_ratio=-1,
        noise_scale=5,
        noise_scale_w=0.5,
    )
    assert engine.speed == 2.0
    assert engine.sdp_ratio == 0.0
    assert engine.noise_scale == 1.0
    assert engine.noise_scale_w == 0.5


def test_generate_audio_passes_all_params(fake_melo):
    engine = melo_module.TTSEngine(
        language="ZH",
        device="cpu",
        speed=1.2,
        sdp_ratio=0.3,
        noise_scale=0.4,
        noise_scale_w=0.5,
        format="wav",
    )
    path = engine.generate_audio("你好，世界", file_name_no_ext="unit_test_melo")
    assert path.endswith(".wav")
    call = fake_melo.calls[-1]
    assert call["text"] == "你好，世界"
    assert call["speaker_id"] == 0
    assert call["speed"] == 1.2
    assert call["sdp_ratio"] == 0.3
    assert call["noise_scale"] == 0.4
    assert call["noise_scale_w"] == 0.5
    assert call["format"] == "wav"


def test_missing_melo_and_external_unavailable_raises_readable_error(monkeypatch):
    class FakeClient:
        @staticmethod
        def singleton():
            return FakeClient()

        def availability_error(self):
            return "模拟外部 Melo Python 不存在"

    monkeypatch.setattr(melo_module, "TTS", None)
    monkeypatch.setattr(melo_module, "_MELO_IMPORT_ERROR", ImportError("no melo"))
    monkeypatch.setattr(melo_module, "MeloExternalClient", FakeClient)
    with pytest.raises(RuntimeError, match="未安装"):
        melo_module.TTSEngine()


def test_external_mode_delegates_to_worker_client(monkeypatch):
    captured = {}

    class FakeClient:
        @staticmethod
        def singleton():
            return FakeClient()

        def availability_error(self):
            return None

        def synthesize(self, **kwargs):
            captured.update(kwargs)
            return kwargs["output_path"]

    monkeypatch.setattr(melo_module, "TTS", None)
    monkeypatch.setattr(melo_module, "_MELO_IMPORT_ERROR", ImportError("no melo"))
    monkeypatch.setattr(melo_module, "MeloExternalClient", FakeClient)
    engine = melo_module.TTSEngine(
        language="EN",
        device="cpu",
        speaker="EN-Default",
        speed=1.5,
        sdp_ratio=0.3,
        noise_scale=0.4,
        noise_scale_w=0.5,
        format="wav",
    )
    path = engine.generate_audio("Hello world", file_name_no_ext="external_unit")
    assert path.endswith("external_unit.wav")
    assert captured["text"] == "Hello world"
    assert captured["language"] == "EN"
    assert captured["speaker"] == "EN-Default"
    assert captured["speed"] == 1.5
    assert captured["sdp_ratio"] == 0.3
    assert captured["noise_scale"] == 0.4
    assert captured["noise_scale_w"] == 0.5
    assert captured["format"] == "wav"


def test_factory_melo_branch_maps_legacy_voice_model():
    captured = {}

    def fake_engine(**kwargs):
        captured.update(kwargs)

    patcher = mock.patch.object(melo_module, "TTSEngine", fake_engine)
    patcher.start()
    try:
        factory_module.TTSFactory.get_tts_engine(
            "melo_tts",
            voice="EN",
            model="auto",
            speed=1.5,
            sdp_ratio=0.4,
            noise_scale=0.5,
            noise_scale_w=0.6,
            format="wav",
        )
    finally:
        patcher.stop()

    assert captured["language"] == "EN"
    assert captured["device"] == "auto"
    assert captured["speed"] == 1.5
    assert captured["sdp_ratio"] == 0.4
    assert captured["noise_scale"] == 0.5
    assert captured["noise_scale_w"] == 0.6
    assert captured["format"] == "wav"
