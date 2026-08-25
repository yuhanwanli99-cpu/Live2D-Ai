"""导演动作边界测试：LLM 只能在白名单内选动作，越界/缺失走情绪映射兜底
（定版仅 surprise→surprise）或 None —— 无法诚实映射的一律不产动作。"""

import asyncio

from open_llm_vtuber.director import (
    _DIRECTOR_SYSTEM,
    _EMOTION_MOTION,
    VALID_EMOTIONS,
    _VALID_MOTIONS,
    ExpressionDirector,
)
from open_llm_vtuber.motion_catalog import SAFE_MOTION_IDS


class FakeLLM:
    """chat_completion 异步生成器桩（与真实 StatelessLLM 接口同形）。"""

    def __init__(self, payload):
        self._payload = payload

    async def chat_completion(self, messages, system=None):
        yield self._payload


def _analyze(payload):
    director = ExpressionDirector(llm=FakeLLM(payload))
    return asyncio.run(director.analyze("随便一句话"))


def test_valid_label_kept():
    result = _analyze(
        '{"emotion":"joy","strength":2,"motionLabel":"listen"}'
    )
    assert result.motion_label == "listen"


def test_wave_label_produces_no_motion():
    # bai 无手臂参数：wave_big 已从安全目录移除，导演输出越界 → 直接丢弃；
    # joy 没有情绪兜底动作 → None（不用倾听等近似动作冒充）。
    result = _analyze('{"emotion":"joy","strength":2,"motionLabel":"wave_big"}')
    assert result.motion_label is None


def test_invalid_label_falls_back_to_emotion_map_or_none():
    result = _analyze('{"emotion":"joy","motionLabel":"dance"}')
    assert result.motion_label is None  # joy 无情绪兜底动作
    # surprise 有唯一允许的情绪兜底：越界 label 丢弃后兜底为 surprise 本身
    assert _analyze('{"emotion":"surprise","motionLabel":"dance"}').motion_label == "surprise"


def test_missing_motion_falls_back_to_emotion_map_or_none():
    # 只有 surprise 允许情绪兜底；其余情绪一律不产出动作（None，不冒充）。
    assert _analyze('{"emotion":"anger","strength":3}').motion_label is None
    assert _analyze('{"emotion":"sadness","strength":1}').motion_label is None
    assert _analyze('{"emotion":"joy","strength":3}').motion_label is None
    assert _analyze('{"emotion":"surprise","strength":2}').motion_label == "surprise"


def test_neutral_without_motion_is_none():
    # neutral 且无显式动作：不产出任何动作（None），避免无意义抖动。
    result = _analyze('{"emotion":"neutral","motionLabel":null}')
    assert result.motion_label is None


def test_emotion_map_is_strict_and_bounded():
    # 定版只允许一条情绪兜底：surprise→surprise；其余情绪一律无兜底动作。
    assert set(_EMOTION_MOTION) == {"surprise"}
    for emotion, motion in _EMOTION_MOTION.items():
        assert motion in SAFE_MOTION_IDS, emotion
        assert motion == "surprise"
    for emotion in VALID_EMOTIONS - {"neutral", "surprise"}:
        assert emotion not in _EMOTION_MOTION, emotion


def test_system_prompt_lists_bounded_actions():
    # 提示词里列出的可选动作必须全部落在白名单内（边界与提示词不漂移）
    for motion in _VALID_MOTIONS:
        assert motion in _DIRECTOR_SYSTEM, motion
    # 已删动作名绝不出现在导演提示词里
    for bad in ("wave_big", "bounce_happy", "happy", "excited", "sway", "bow", "lean_in"):
        assert bad not in _DIRECTOR_SYSTEM, bad