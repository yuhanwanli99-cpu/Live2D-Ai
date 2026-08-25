"""直播事件通道（/api/live/event）动作边界测试。

契约（六动作目录收敛定版）：
- 任何外部 payload 的 motion 都必须落在 motion_catalog 白名单
  （nod/shake_no/tilt/look_around/listen/surprise）内；
- 已删 id（happy/excited/sway/bow/lean_in/wave_big/bounce_happy）、
  release 专用 id（idle）与未知 id 一律丢弃，不下发、不产动作；
- 其余安全字段（text/user/emotion/strength 等）不受影响。
"""

from open_llm_vtuber.live_event import sanitize_live_event
from open_llm_vtuber.motion_catalog import SAFE_MOTION_IDS


def test_valid_six_motion_ids_pass_through():
    for mid in SAFE_MOTION_IDS:
        event = sanitize_live_event({"type": "danmaku", "motion": mid})
        assert event["motion"] == mid, mid


def test_removed_and_release_ids_are_dropped():
    for bad in (
        "happy",
        "excited",
        "sway",
        "bow",
        "lean_in",
        "wave_big",
        "bounce_happy",
        "idle",  # release 专用，不能作为正向 trigger
        "wave",
        "special_02",
        "no_such_gesture",
    ):
        event = sanitize_live_event({"type": "danmaku", "motion": bad, "text": "hi"})
        assert "motion" not in event, bad
        assert event["text"] == "hi"


def test_non_string_motion_is_dropped():
    assert "motion" not in sanitize_live_event({"motion": 123})
    assert "motion" not in sanitize_live_event({"motion": None})
    assert "motion" not in sanitize_live_event({})


def test_other_fields_unaffected_by_motion_gate():
    event = sanitize_live_event(
        {
            "type": "danmaku",
            "text": "  你好呀  ",
            "user": "小明",
            "motion": "nod",
            "emotion": "joy",
            "strength": 9,
            "parameterOverrides": {"ParamAngleX": 5},
            "hacker_field": "drop-me",
        }
    )
    assert event == {
        "type": "danmaku",
        "text": "你好呀",
        "user": "小明",
        "motion": "nod",
        "emotion": "joy",
        "strength": 3,
        "parameterOverrides": {"ParamAngleX": 5},
    }
