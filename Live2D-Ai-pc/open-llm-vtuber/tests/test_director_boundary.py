"""导演动作边界测试：LLM 只能在白名单内选动作，越界/缺失走情绪映射兜底。"""

import asyncio

from open_llm_vtuber.director import (
    _DIRECTOR_SYSTEM,
    _EMOTION_MOTION,
    VALID_EMOTIONS,
    _VALID_MOTIONS,
    ExpressionDirector,
)


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
        '{"emotion":"joy","strength":2,"motionLabel":"happy"}'
    )
    assert result.motion_label == "happy"


def test_wave_label_falls_back_to_safe_happy():
    # bai 无手臂参数：wave_big 已从安全目录移除，导演输出越界 → joy 映射兜底到 happy。
    result = _analyze('{"emotion":"joy","strength":2,"motionLabel":"wave_big"}')
    assert result.motion_label == "happy"


def test_invalid_label_falls_back_to_emotion_map():
    result = _analyze('{"emotion":"joy","motionLabel":"dance"}')
    assert result.motion_label == "happy"  # joy 的映射兜底


def test_missing_motion_falls_back_to_emotion_map():
    result = _analyze('{"emotion":"anger","strength":3}')
    assert result.motion_label == "shake_no"


def test_neutral_without_motion_uses_restrained_action():
    # neutral 且无显式动作时不得静止：使用克制动作（nod）+ 最低强度
    result = _analyze('{"emotion":"neutral","motionLabel":null}')
    assert result.motion_label == "nod"
    assert result.strength == 1


def test_emotion_map_full_coverage_and_bounds():
    assert set(_EMOTION_MOTION) == set(VALID_EMOTIONS) - {"neutral"}
    for emotion, motion in _EMOTION_MOTION.items():
        assert motion in _VALID_MOTIONS, emotion


def test_system_prompt_lists_bounded_actions():
    # 提示词里列出的可选动作必须全部落在白名单内（边界与提示词不漂移）
    for motion in _VALID_MOTIONS:
        assert motion in _DIRECTOR_SYSTEM or motion in {"idle"}, motion