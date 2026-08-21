"""BodyDirectorPlugin 情绪→手势确定性兜底测试。

契约：
- 已有 motion → 原样透传（优先级最高）；
- 无 motion → 按 8 key 情绪映射手势；再退心情分层（happy/ecstatic→excited）；
- neutral 且无正向心情 → None（不硬造动作）。
"""

from open_llm_vtuber.plugins.native_runtime import BodyDirectorPlugin


def _plugin():
    return BodyDirectorPlugin()


def test_existing_motion_has_priority():
    out = _plugin().handle_event(
        "sentence.after", {"motion": "nod", "emotion": "joy", "mood_tier": "neutral"}
    )
    assert out["motion"] == "nod"


def test_emotion_maps_to_gesture_when_motion_missing():
    cases = {
        "joy": "happy",
        "surprise": "surprise",
        "sadness": "listen",
        "fear": "listen",
        "smirk": "tilt",
        "anger": "sway",
    }
    for emotion, gesture in cases.items():
        out = _plugin().handle_event(
            "sentence.after", {"motion": None, "emotion": emotion}
        )
        assert out["motion"] == gesture, emotion


def test_mood_tier_secondary_fallback():
    out = _plugin().handle_event(
        "sentence.after", {"motion": None, "emotion": "neutral", "mood_tier": "ecstatic"}
    )
    assert out["motion"] == "excited"


def test_neutral_without_positive_mood_stays_none():
    out = _plugin().handle_event(
        "sentence.after", {"motion": None, "emotion": "neutral", "mood_tier": "neutral"}
    )
    assert out["motion"] is None


def test_non_sentence_after_is_noop_and_overrides_pass_through():
    assert _plugin().handle_event("llm.before", {}) == {}
    overrides = {"ParamEyeLOpen": 0.8}
    out = _plugin().handle_event(
        "sentence.after", {"motion": "tilt", "parameterOverrides": overrides}
    )
    assert out["parameterOverrides"] == overrides
