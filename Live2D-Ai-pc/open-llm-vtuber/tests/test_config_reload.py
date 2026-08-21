"""回归：设置保存并应用热重载时，TTS/ASR/VAD 配置变化必须重建引擎。

历史缺陷：load_from_config() 先把新 character_config 赋到 self.character_config，
导致 init_tts() 的 `self.character_config.tts_config != tts_config` 恒为 False，
TTS 永不重建（切到 melo 后实际仍是旧引擎）。
"""
import types

from open_llm_vtuber.config_manager.tts import TTSConfig
from open_llm_vtuber.service_context import ServiceContext
from open_llm_vtuber.tts import tts_factory


class _FakeEngine:
    def __init__(self, engine_type):
        self.engine_type = engine_type
        self._closed = False

    def close(self):
        self._closed = True


def test_init_tts_reinitializes_on_config_change(monkeypatch):
    calls: list[str] = []

    def fake_get_tts_engine(engine_type, **kwargs):
        calls.append(engine_type)
        return _FakeEngine(engine_type)

    monkeypatch.setattr(tts_factory.TTSFactory, "get_tts_engine", fake_get_tts_engine)

    ctx = ServiceContext()
    ctx.character_config = types.SimpleNamespace(tts_config=None)

    edge_cfg = TTSConfig.model_validate(
        {"tts_model": "edge_tts", "edge_tts": {"voice": "zh-CN-XiaoxiaoNeural"}}
    )
    melo_cfg = TTSConfig.model_validate(
        {
            "tts_model": "melo_tts",
            "melo_tts": {
                "language": "ZH",
                "device": "cpu",
                "speaker": None,
                "speed": 1.0,
                "sdp_ratio": 0.2,
                "noise_scale": 0.6,
                "noise_scale_w": 0.8,
                "format": "wav",
            },
        }
    )

    # 首次初始化（启动）
    ctx.init_tts(edge_cfg)
    assert calls == ["edge_tts"]

    # 模拟热重载：先把新 character_config 赋上，再 init_*
    # （与 load_from_config 的顺序一致）—— 必须重建为 melo。
    ctx.character_config.tts_config = melo_cfg
    ctx.init_tts(melo_cfg)
    assert calls == ["edge_tts", "melo_tts"]

    # 同配置重复调用不应再次重建
    ctx.init_tts(melo_cfg)
    assert calls == ["edge_tts", "melo_tts"]
