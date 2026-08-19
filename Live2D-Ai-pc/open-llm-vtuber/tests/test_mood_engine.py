# -*- coding: utf-8 -*-
"""
P3 · mood_engine 单测（RED）

契约依据：.pi/spoq/contract-pc-pipeline.md §P3（P3-01 ~ P3-07）
架构依据：.pi/spoq/architecture-pc-pipeline.md §3.5 / §4.3.1
翻译依据：.pi/spoq/translation-polish.md §七-E（mood 打分/衰减/映射 + 三端联动）
素材依据：.pi/spoq/search-report-polish-materials.md（mood 心情值设计参考）

被测模块：src/open_llm_vtuber/mood_engine.py
  类 MoodEngine（async 事件方法）+ 枚举 MoodTier + 三端联动接口
    get_expression_modifier  -> 表情基线微调（default base）
    get_tts_modifier         -> TTS 基调微调（speech_rate/pitch_rate）
    get_prompt_injection     -> prompt 注入文本

文件锁说明：
  - 本测试只依据 契约/架构/翻译/素材 四个文件，未读取 src/ 任何实现代码。
  - 所有期望值均来自契约 schema（EMOTION_MOOD_DELTA）+ 架构 4.3.1
    （MOOD_TTS_MODIFIER / MOOD_EXPRESSION_MODIFIER / injections 文案）。

时间衰减测试采用【可注入时钟】（TickGate monkeypatch asyncio.sleep），
  避免真实 sleep(1.0s)：每次 release() 手动放行一个衰减 tick，事件循环让出后
  衰减循环体执行一次，从而把"等待 1.1s → 一个 tick 衰减"压成瞬时确定性推进。

所有断言机械可执行、初始 RED（mood_engine 模块尚不存在 → import 失败即 RED）。
"""
import asyncio

import pytest

from open_llm_vtuber.mood_engine import MoodEngine, MoodTier

# 测试中所有期望值均来自契约 schema / 架构 §4.3.1（见文件头），非实现细节。


# ============================================================
# 可注入时钟（避免真实 sleep 的时间衰减推进器）
# ============================================================
_ORIG_SLEEP = asyncio.sleep  # 捕获原始 sleep，内部让出用（不被 gate 自递归替换）


class TickGate:
    """
    把 asyncio.sleep 替换为"手动 tick 放行"门闩。

    衰减循环体形如 while True: await asyncio.sleep(interval); <decay>。
    patch 后 await sleep(1.0) 走 gate.sleep，只有 release() 过配额才会返回，
    从而让衰减循环体跑一次（一个 tick），替代真实等待 interval 秒。
    """

    def __init__(self):
        self.released = 0

    async def _fake_sleep(self, delay):  # noqa: ARG002 · delay 参数仅占位
        while self.released <= 0:
            await _ORIG_SLEEP(0)
        self.released -= 1
        await _ORIG_SLEEP(0)  # 让衰减体在同一 tick 内取得调度执行

    def sleep(self, delay):
        return self._fake_sleep(delay)

    def release(self, n: int = 1) -> None:
        self.released += n


@pytest.fixture
def tick_gate(monkeypatch) -> TickGate:
    gate = TickGate()
    monkeypatch.setattr(asyncio, "sleep", gate.sleep)
    return gate


async def _advance_ticks(gate: TickGate, n: int, *, give_time: int = 4) -> None:
    """放行 n 个 tick，并多次让出事件循环确保衰减体各有执行机会。"""
    for _ in range(n):
        gate.release(1)
        for _ in range(give_time):
            await _ORIG_SLEEP(0)


# ============================================================
# 公共辅助：构造引擎 + 驱动 async 用例
# ============================================================
def _run(coro):
    return asyncio.run(coro)


def _make_engine() -> MoodEngine:
    return MoodEngine()


# 契约 EMOTION_MOOD_DELTA 中的几档常用差值（供测试注释，不 import）
#   emo:joy=+7.0 · emo:anger=-5.0 · user_positive=+8.0 · user_negative=-10.0
#   user_question=+2.0 · user_tap=+2.0 · user_interrupt=-3.0 · proactive_speak=+1.0
_DELTA = {
    "joy": 7.0, "anger": -5.0,
    "user_positive": 8.0, "user_negative": -10.0,
    "user_question": 2.0, "user_tap": 2.0,
    "user_interrupt": -3.0, "proactive_speak": 1.0,
}


# ============================================================
# 1. 初始状态
# ============================================================
def test_initial_state_is_neutral_zero():
    """新引擎初始 mood=0.0、tier=NEUTRAL（契约附录 A step1）。"""
    async def _impl():
        engine = _make_engine()
        assert engine.get_mood_value() == pytest.approx(0.0, abs=1e-6)
        assert engine.get_mood_tier() == MoodTier.NEUTRAL
        # 初始无任何事件 → 三端联动全为 neutral 默认
        assert engine.get_expression_modifier() == {}
        assert engine.get_prompt_injection() == ""

    _run(_impl())


def test_initial_tts_modifier_is_neutral():
    """初始 neutral → TTS 基调为 neutral 档。"""
    async def _impl():
        engine = _make_engine()
        assert engine.get_tts_modifier() == {"speech_rate": 1.00, "pitch_rate": 1.05}

    _run(_impl())


# ============================================================
# 2. 事件打分（LLM 情绪 Token 强度）
# ============================================================
def test_emotion_event_delta_default_strength():
    """P3-01 on_emotion_event("joy",2) → +7.0。"""
    async def _impl():
        engine = _make_engine()
        await engine.on_emotion_event("joy", 2)
        assert engine.get_mood_value() == pytest.approx(_DELTA["joy"], abs=1e-6)

    _run(_impl())


def test_emotion_event_strength_scaling():
    """P3-02 on_emotion_event("joy",3) → 7.0×3/2 = +10.5。"""
    async def _impl():
        engine = _make_engine()
        await engine.on_emotion_event("joy", 3)
        assert engine.get_mood_value() == pytest.approx(10.5, abs=1e-6)

    _run(_impl())


def test_emotion_event_negative_delta():
    """On_emotion_event("anger",2) → -5.0（负向偏移）。"""
    async def _impl():
        engine = _make_engine()
        await engine.on_emotion_event("anger", 2)
        assert engine.get_mood_value() == pytest.approx(-5.0, abs=1e-6)

    _run(_impl())


# ============================================================
# 3. 连续事件累积
# ============================================================
def test_repeated_emotion_accumulates():
    """P3-03 anger×5 → -25.0（连续累积，未超限）。"""
    async def _impl():
        engine = _make_engine()
        for _ in range(5):
            await engine.on_emotion_event("anger", 2)
        assert engine.get_mood_value() == pytest.approx(-25.0, abs=1e-6)

    _run(_impl())


def test_positive_accumulation_clamps_at_100():
    """值域钳制（上界）：joy×25 = +125 → 钳制到 +100.0。"""
    async def _impl():
        engine = _make_engine()
        for _ in range(25):
            await engine.on_emotion_event("joy", 2)
        assert engine.get_mood_value() == pytest.approx(100.0, abs=1e-6)
        assert engine.get_mood_value() <= 100.0

    _run(_impl())


def test_negative_accumulation_clamps_at_minus_100():
    """值域钳制（下界）：anger×25 = -125 → 钳制到 -100.0。"""
    async def _impl():
        engine = _make_engine()
        for _ in range(25):
            await engine.on_emotion_event("anger", 2)
        assert engine.get_mood_value() == pytest.approx(-100.0, abs=1e-6)
        assert engine.get_mood_value() >= -100.0

    _run(_impl())


# ============================================================
# 4. 用户消息情感 + 交互事件
# ============================================================
def test_user_sentiment_deltas():
    """用户消息情感偏移：positive+8 / negative-10 / question+2。"""
    async def _impl():
        engine = _make_engine()
        await engine.on_user_message_sentiment("positive")
        assert engine.get_mood_value() == pytest.approx(_DELTA["user_positive"], abs=1e-6)
        await engine.on_user_message_sentiment("negative")
        assert engine.get_mood_value() == pytest.approx(_DELTA["user_positive"] - 10.0, abs=1e-6)
        await engine.on_user_message_sentiment("question")
        assert engine.get_mood_value() == pytest.approx(
            _DELTA["user_positive"] - 10.0 + _DELTA["user_question"], abs=1e-6,
        )

    _run(_impl())


def test_interaction_event_deltas():
    """交互事件偏移：user_tap+2 / user_interrupt-3。"""
    async def _impl():
        engine = _make_engine()
        await engine.on_interaction_event("user_tap")
        assert engine.get_mood_value() == pytest.approx(_DELTA["user_tap"], abs=1e-6)
        await engine.on_interaction_event("user_interrupt")
        assert engine.get_mood_value() == pytest.approx(
            _DELTA["user_tap"] + _DELTA["user_interrupt"], abs=1e-6,
        )

    _run(_impl())


# ============================================================
# 5. 时间衰减（可注入时钟）
# ============================================================
def test_decay_one_tick_reduces_positive_mood(tick_gate):
    """P3-04 mood=70 → 推进 1 tick（≈1 秒）→ 68（每 tick 衰减 DECAY_RATE=2）。"""
    async def _impl():
        engine = _make_engine()
        for _ in range(10):  # joy×2 ×10 = +70
            await engine.on_emotion_event("joy", 2)
        current = engine.get_mood_value()
        assert current == pytest.approx(70.0, abs=1e-6)

        await engine.start()
        try:
            await _advance_ticks(tick_gate, 1)
            assert engine.get_mood_value() == pytest.approx(68.0, abs=0.5)  # 70 - 2
        finally:
            await engine.stop()

    _run(_impl())


def test_decay_returns_negative_toward_zero(tick_gate):
    """P3-05 负值衰减向 0 回归：-40 → 推进 1 tick → -38（不无限低落）。"""
    async def _impl():
        engine = _make_engine()
        for _ in range(8):  # anger×2 ×8 = -40
            await engine.on_emotion_event("anger", 2)
        assert engine.get_mood_value() == pytest.approx(-40.0, abs=1e-6)

        await engine.start()
        try:
            await _advance_ticks(tick_gate, 2)  # -40 → -38 → -36
            assert engine.get_mood_value() >= -40.0  # 下限不无限低落
            assert engine.get_mood_value() == pytest.approx(-36.0, abs=0.5)
        finally:
            await engine.stop()

    _run(_impl())


def test_decay_stops_at_zero(tick_gate):
    """衰减在 0 处停止，不穿过零点变负：mood=+4 → 推 5 tick → 钳在 0。"""
    async def _impl():
        engine = _make_engine()
        # emo:smirk = +4.0 → 恰好 +4，向 0 回归每 tick -2
        await engine.on_emotion_event("smirk", 2)
        assert engine.get_mood_value() == pytest.approx(4.0, abs=1e-6)
        await engine.start()
        try:
            # 4 → 2 → 0 → 0 → 0（到 0 后不再往下穿，保持非负）
            await _advance_ticks(tick_gate, 5)
            assert engine.get_mood_value() == pytest.approx(0.0, abs=0.5)
            assert engine.get_mood_value() >= 0.0  # 不穿过零点变负
        finally:
            await engine.stop()

    _run(_impl())


# ============================================================
# 6. MoodTier 分层判定
# ============================================================
@pytest.mark.parametrize(
    ("mood", "expected_tier"),
    [
        (-100.0, MoodTier.FURIOUS),
        (-60.0, MoodTier.FURIOUS),   # 左闭区间边界
        (-59.0, MoodTier.UPSET),
        (-20.0, MoodTier.UPSET),     # 左闭区间边界
        (-19.0, MoodTier.NEUTRAL),
        (0.0, MoodTier.NEUTRAL),
        (20.0, MoodTier.NEUTRAL),    # 左闭区间边界
        (21.0, MoodTier.HAPPY),
        (60.0, MoodTier.HAPPY),      # 左闭区间边界
        (61.0, MoodTier.ECSTATIC),
        (100.0, MoodTier.ECSTATIC),  # 上界
    ],
)
def test_mood_tier_from_value(mood, expected_tier):
    """P3-06 MoodTier.from_value 各区间映射（含边界）正确。"""
    assert MoodTier.from_value(mood) == expected_tier


def test_get_mood_tier_follows_engine_state():
    """引擎 get_mood_tier() 随值变化正确更新。"""
    async def _impl():
        engine = _make_engine()
        # -70 → FURIOUS
        for _ in range(14):
            await engine.on_emotion_event("anger", 2)
        assert engine.get_mood_value() == pytest.approx(-70.0, abs=1e-6)
        assert engine.get_mood_tier() == MoodTier.FURIOUS
        # +80 → ECSTATIC（先正偏移到 80）
        await engine.on_emotion_event("joy", 2)  # -70+7 = -63
        await engine.on_emotion_event("joy", 2)  # -56
        await engine.on_emotion_event("joy", 2)  # -49
        assert engine.get_mood_tier() == MoodTier.UPSET

    _run(_impl())


# ============================================================
# 7. 三端联动接口（按 tier 返回）
# ============================================================
def test_expression_modifier_by_tier():
    """get_expression_modifier 按 tier 返回表情基线微调（架构 MOOD_EXPRESSION_MODIFIER）。"""
    async def _impl():
        engine = _make_engine()
        # neutral（+? 需在 (-20, 20] 内）→ 用 +1
        await engine.on_emotion_event("neutral", 2)   # emo:neutral=+0.5
        await engine.on_emotion_event("neutral", 2)   # +0.5 → +1.0 → NEUTRAL
        assert engine.get_mood_tier() == MoodTier.NEUTRAL
        assert engine.get_expression_modifier() == {}
        # happy：再 +20（到 ~21）→ 进 happy 区间
        await engine.on_emotion_event("joy", 2)  # +5
        await engine.on_emotion_event("joy", 2)  # +5
        await engine.on_emotion_event("joy", 2)  # +5
        await engine.on_emotion_event("joy", 2)  # +5  (1+20=21 → HAPPY)
        assert engine.get_mood_tier() == MoodTier.HAPPY
        assert engine.get_expression_modifier().get("ParamMouthForm") == pytest.approx(0.10, abs=1e-6)
        assert engine.get_expression_modifier().get("ParamEyeLOpen") == pytest.approx(0.05, abs=1e-6)

    _run(_impl())


def test_tts_modifier_by_tier():
    """get_tts_modifier 按 tier 返回 TTS 基调（架构 MOOD_TTS_MODIFIER）。"""
    async def _impl():
        engine = _make_engine()
        await engine.on_emotion_event("neutral", 2)
        await engine.on_emotion_event("neutral", 2)  # +1.0 → NEUTRAL
        assert engine.get_tts_modifier() == {"speech_rate": 1.00, "pitch_rate": 1.05}
        # 推 mood 到 >60（ECSTATIC 区间），每次 joy=+5
        while engine.get_mood_value() <= 60.0:
            await engine.on_emotion_event("joy", 2)
        assert engine.get_mood_tier() == MoodTier.ECSTATIC
        assert engine.get_tts_modifier() == {"speech_rate": 1.08, "pitch_rate": 1.12}

    _run(_impl())


def test_prompt_injection_by_tier():
    """P3-07 + E4 get_prompt_injection 按 tier 返回注入文案。"""
    async def _impl():
        engine = _make_engine()
        # NEUTRAL → ""
        await engine.on_emotion_event("neutral", 2)
        await engine.on_emotion_event("neutral", 2)
        assert engine.get_prompt_injection() == ""
        # HAPPY → 含 "心情不错"
        for _ in range(5):
            await engine.on_emotion_event("joy", 2)   # 1+25=26 → HAPPY
        assert engine.get_mood_tier() == MoodTier.HAPPY
        injection = engine.get_prompt_injection()
        assert isinstance(injection, str)
        assert "心情不错" in injection
        # ECSTATIC → 含 "超级开心"
        for _ in range(8):
            await engine.on_emotion_event("joy", 2)   # 26+40=66 → ECSTATIC
        assert engine.get_mood_tier() == MoodTier.ECSTATIC
        assert "超级开心" in engine.get_prompt_injection()

    _run(_impl())


def test_furious_prompt_injection_and_modifier():
    """furious 档：prompt 注入 + TTS 基调微调均对应 furious。"""
    async def _impl():
        engine = _make_engine()
        for _ in range(14):
            await engine.on_emotion_event("anger", 2)  # -70 → FURIOUS
        assert engine.get_mood_tier() == MoodTier.FURIOUS
        assert "烦躁" in engine.get_prompt_injection()
        assert engine.get_tts_modifier() == {"speech_rate": 1.05, "pitch_rate": 1.05}
        assert engine.get_expression_modifier().get("ParamBrowY") == pytest.approx(-0.10, abs=1e-6)

    _run(_impl())


# ============================================================
# 8. 完整对话轨迹（契约附录 A 场景）
# ============================================================
def test_full_conversation_mood_trajectory(tick_gate):
    """契约附录 A：完整对话 mood 轨迹，每步断言 ±0.5 容差。"""
    async def _impl():
        engine = _make_engine()
        await engine.start()
        try:
            # 1. start → 0
            assert engine.get_mood_value() == pytest.approx(0.0, abs=0.5)
            # 2. question → +2
            await engine.on_user_message_sentiment("question")
            assert engine.get_mood_value() == pytest.approx(2.0, abs=0.5)
            # 3. joy,2 → +9
            await engine.on_emotion_event("joy", 2)
            assert engine.get_mood_value() == pytest.approx(9.0, abs=0.5)
            # 4. 等待 2s（2 tick）→ +5
            await _advance_ticks(tick_gate, 2)
            assert engine.get_mood_value() == pytest.approx(5.0, abs=0.5)
            # 5. anger,3 → -2.5
            await engine.on_emotion_event("anger", 3)
            assert engine.get_mood_value() == pytest.approx(-2.5, abs=0.5)
            # 6. negative → -12.5
            await engine.on_user_message_sentiment("negative")
            assert engine.get_mood_value() == pytest.approx(-12.5, abs=0.5)
            # 7. get_mood_tier → NEUTRAL（-12.5 > -20）
            assert engine.get_mood_tier() == MoodTier.NEUTRAL
            # 8. get_tts_modifier → neutral
            assert engine.get_tts_modifier() == {"speech_rate": 1.00, "pitch_rate": 1.05}
            # 9. get_expression_modifier → {}（neutral 无修调）
            assert engine.get_expression_modifier() == {}
            # 10. get_prompt_injection → ""
            assert engine.get_prompt_injection() == ""
        finally:
            await engine.stop()

    _run(_impl())


# ============================================================
# 9. 边界：强度缩放对非默认强度的事件
# ============================================================
def test_emotion_strength_one_scales_half():
    """strength=1 → delta ×1/2（如 joy → +3.5）。"""
    async def _impl():
        engine = _make_engine()
        await engine.on_emotion_event("joy", 1)
        assert engine.get_mood_value() == pytest.approx(3.5, abs=1e-6)

    _run(_impl())
