"""BodyDirectorPlugin 情绪→动作确定性兜底测试（六动作目录收敛定版）。

契约：
- 已有 motion → 过白名单后原样透传（合法 id 优先级最高）；
- 已删/未知 motion（happy/sway/excited/bow/lean_in/wave_big/bounce_happy）
  一律清空为 None —— 插件任何输出都不允许出现已删 id；
- 无 motion → 情绪兜底只认 surprise→surprise；其余情绪（含心情分层
  happy/ecstatic）一律 None，不强行动作。
"""

from open_llm_vtuber.motion_catalog import SAFE_MOTION_IDS
from open_llm_vtuber.plugins.native_runtime import (
    _EMOTION_GESTURE_FALLBACK,
    BodyDirectorPlugin,
)


def _plugin():
    return BodyDirectorPlugin()


def test_existing_motion_has_priority():
    out = _plugin().handle_event(
        "sentence.after", {"motion": "nod", "emotion": "joy", "mood_tier": "neutral"}
    )
    assert out["motion"] == "nod"


def test_passthrough_motion_goes_through_whitelist():
    # 透传也必须过目录白名单：上游塞进来的已删 id 在插件出口就被清空。
    for bad in ("happy", "excited", "sway", "bow", "lean_in", "wave_big", "bounce_happy"):
        out = _plugin().handle_event(
            "sentence.after", {"motion": bad, "emotion": "joy"}
        )
        assert out["motion"] is None, bad


def test_emotion_fallback_only_surprise():
    # 定版：情绪兜底只有 surprise → surprise；其余情绪一律 None。
    assert _EMOTION_GESTURE_FALLBACK == {"surprise": "surprise"}
    cases = {
        "surprise": "surprise",
        "joy": None,
        "sadness": None,
        "fear": None,
        "smirk": None,
        "anger": None,
        "disgust": None,
        "neutral": None,
    }
    for emotion, gesture in cases.items():
        out = _plugin().handle_event(
            "sentence.after", {"motion": None, "emotion": emotion}
        )
        assert out["motion"] == gesture, emotion
        assert gesture is None or gesture in SAFE_MOTION_IDS


def test_mood_tier_never_produces_motion():
    # 心情分层（PC-L4）不再触发任何动作：happy/ecstatic 不产 excited 等已删 id。
    for tier in ("furious", "upset", "neutral", "happy", "ecstatic"):
        out = _plugin().handle_event(
            "sentence.after", {"motion": None, "emotion": "neutral", "mood_tier": tier}
        )
        assert out["motion"] is None, tier


def test_non_sentence_after_is_noop_and_overrides_pass_through():
    assert _plugin().handle_event("llm.before", {}) == {}
    overrides = {"ParamEyeLOpen": 0.8}
    out = _plugin().handle_event(
        "sentence.after", {"motion": "tilt", "parameterOverrides": overrides}
    )
    assert out["parameterOverrides"] == overrides
