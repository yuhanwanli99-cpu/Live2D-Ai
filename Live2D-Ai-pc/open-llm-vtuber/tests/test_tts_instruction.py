"""P4 · TTS instruction 动态拼接 —— RED 测试（初始失败，驱动实现）。

契约来源：`.pi/spoq/contract-pc-pipeline.md` §P4（P4-01 ~ P4-05）
架构来源：`.pi/spoq/architecture-pc-pipeline.md` §3.6 / §4.4
素材来源：`.pi/spoq/search-report-polish-materials.md` §一（instruction 文案库）

目标实现（新建文件 `src/open_llm_vtuber/tts/tts_instruction_library.py`）：

    ```python
    INSTRUCTION_LIBRARY: dict[str, str]   # 8 emotion → 文案（≤80 字符，中文计2）
    MOOD_MODIFIERS: dict[str, str]        # 5 mood_tier → 前缀修饰词（≤20 字符）

    def get_instruction(emotion: str, strength: int = 2, mood_tier: str = "neutral") -> str:
        \"\"\"按 emotion + mood 拼接 TTS instruction 文案。返回 ≤100 字符。
        中文按 2 计字符（DashScope/CosyVoice 规则）。
        未知 emotion 回退 INSTRUCTION_LIBRARY["neutral"]。
        \"\"\"
    ```

字符计数规则（来源素材报告 §1.1）：中文字符（含简体/繁体/日文汉字/韩文汉字）
每字符计 2；标点、字母、数字计 1。CosyVoice instruction 硬上限 100 字符。

⚠️ 文件锁遵守：本测试只读契约/架构/素材，未读 `src/` 实现代码。
"""

import pytest

try:
    from open_llm_vtuber.tts import tts_instruction_library as lib
except ImportError:
    # [RED] 目标实现（tts_instruction_library.py）尚未写出 —— 本测试文件初始预期失败。
    # 采用与同批 tester 一致的优雅 RED 模式：模块缺失时收集期即标记失败，
    # 不产生 ImportError 中断整个 pytest 会话，待 P4 coder 写出实现后转 GREEN。
    lib = None
    pytest.fail(
        "[RED] 目标模块 open_llm_vtuber.tts.tts_instruction_library 未找到，"
        "实现未写出，本测试预期失败。"
    )


# ---- 契约常量锚点（与 architecture §4.4.1 一致）----

VALID_EMOTIONS = {
    "neutral", "joy", "anger", "sadness",
    "surprise", "fear", "smirk", "disgust",
}
VALID_MOOD_TIERS = {
    "furious", "upset", "neutral", "happy", "ecstatic",
}

# joy 文案必须覆盖的"开心/高兴"语义词（任一命中即可）
_JOY_EMOTION_WORDS = ("开心", "高兴", "雀跃", "活泼", "元气", "上扬", "俏皮")

# 强度幅度词（strength 影响文案的预期语义：强度 3 更"强烈/激动/明显"）
_STRENGTH_MAGNITUDE_KEYWORDS = ("非常", "特别", "强烈", "激动", "大声", "明显", "用力")


def _cjkle() -> str:
    """返回一个覆盖中文/日文/韩文汉字区间的占位字符串（用于 char 计数单元内联断言）。"""
    return "中文简体" + "日本語漢字" + "한국한자"  # noqa: RUF001


# ---------------- 字符计数辅助 ----------------

def _instruction_char_count(text: str) -> int:
    """按 CosyVoice 规则计算 instruction 字符数：中文（含日/韩汉字）计 2，
    标点/字母/数字计 1。

    判定依据：素材报告 §1.1 字符计数规则。用 Unicode 的 CJK 统一表意文字
    （U+4E00–U+9FFF，含 CJK Ext-A）判定"汉字"；其余字符（拉丁字母、数字、
    标点、空格）计 1。
    """
    total = 0
    for ch in text:
        if "\u4e00" <= ch <= "\u9fff":
            total += 2
        else:
            total += 1
    return total


class TestCharCount:
    """字符计数规则自身的基准用例（保证后续 ≤100 断言口径正确）。"""

    def test_cjk_char_counts_as_2(self):
        # "中" 一汉字 → 2
        assert _instruction_char_count("中") == 2

    def test_latin_punct_counts_as_1(self):
        # 字母 a + 标点 , → 各 1
        assert _instruction_char_count("a,") == 2

    def test_mixed_cjk_and_latin(self):
        # "开心!"：开(2)+心(2)+!(1) = 5
        assert _instruction_char_count("开心!") == 5

    def test_library_base_texts_within_80(self):
        """素材报告 §1.3 每条文案 ≤80 字符（中文计2）——架构 §4.4.1 的前置约束。"""
        for emotion in VALID_EMOTIONS:
            text = lib.INSTRUCTION_LIBRARY[emotion]
            assert _instruction_char_count(text) <= 80, (
                f"INSTRUCTION_LIBRARY[{emotion!r}] 基准文案超 80 字符: {text!r}"
            )

    def test_mood_modifiers_within_20(self):
        """架构 §4.4.1 MOOD_MODIFIERS 每条 ≤20 字符（中文计2）。"""
        for tier in VALID_MOOD_TIERS:
            modifier = lib.MOOD_MODIFIERS[tier]
            assert _instruction_char_count(modifier) <= 20, (
                f"MOOD_MODIFIERS[{tier!r}] 超 20 字符: {modifier!r}"
            )


# ---------------- P4-01：非空 ----------------

class TestGetInstructionNonEmpty:
    def test_joy_happy_returns_nonempty(self):
        # P4-01
        result = lib.get_instruction("joy", 2, "happy")
        assert isinstance(result, str)
        assert len(result) > 0


# ---------------- P4-02：≤100 字符（中文计2，全组合）----------------

class TestLengthUpperBound:
    def test_all_40_combinations_within_100(self):
        # P4-02：8 emotion × 5 mood_tier = 40 组合，全部 ≤100 字符（中文计2）
        for emotion in VALID_EMOTIONS:
            for tier in VALID_MOOD_TIERS:
                text = lib.get_instruction(emotion, 2, tier)
                assert _instruction_char_count(text) <= 100, (
                    f"get_instruction({emotion!r}, 2, {tier!r}) = {text!r} "
                    f"计 {_instruction_char_count(text)} 字符，超 100"
                )

    def test_all_strengths_within_100(self):
        # 所有 strength 1-3 × 所有 emotion 也需 ≤100（strength 若影响文案不能破坏上限）
        for emotion in VALID_EMOTIONS:
            for strength in (1, 2, 3):
                text = lib.get_instruction(emotion, strength, "happy")
                assert _instruction_char_count(text) <= 100, (
                    f"strength={strength} emotion={emotion!r} 超 100: {text!r}"
                )


# ---------------- P4-03：mood 修饰词前缀 ----------------

class TestMoodModifierPrefix:
    def test_joy_happy_starts_with_happy_modifier(self):
        # P4-03
        result = lib.get_instruction("joy", 2, "happy")
        modifier = lib.MOOD_MODIFIERS["happy"]
        assert result.startswith(modifier)

    def test_all_tiers_prefixed_by_their_modifier(self):
        # 每个 mood_tier 的修饰词都拼在对应 emotion 文案前面（中性修饰词为空字符串时
        # startswith("") 恒真，但仍验证"以该修饰词开头"的契约语义）
        for tier in VALID_MOOD_TIERS:
            modifier = lib.MOOD_MODIFIERS[tier]
            result = lib.get_instruction("joy", 2, tier)
            assert result.startswith(modifier), (
                f"mood_tier={tier!r} 应以此修饰词开头: {modifier!r}，实际: {result!r}"
            )

    def test_unknown_mood_tier_uses_empty_modifier(self):
        # 未知 mood_tier → 无修饰词，文案 = 纯 base 文案
        result = lib.get_instruction("joy", 2, "not_a_tier")
        base = lib.INSTRUCTION_LIBRARY["joy"]
        assert result == base


# ---------------- P4-04：未知 emotion 回退 neutral ----------------

class TestUnknownEmotionFallback:
    def test_unknown_emotion_falls_back_to_neutral(self):
        # P4-04：get_instruction("unknown_emotion", 2, "neutral") == INSTRUCTION_LIBRARY["neutral"]
        result = lib.get_instruction("unknown_emotion", 2, "neutral")
        neutral_base = lib.INSTRUCTION_LIBRARY["neutral"]
        modifier = lib.MOOD_MODIFIERS["neutral"]
        assert result == f"{modifier}{neutral_base}"

    def test_unknown_emotion_does_not_raise(self):
        # 未知 emotion 不抛异常（契约 R7：不得因输入非法而崩溃）
        result = lib.get_instruction("xyz_emotion")
        assert isinstance(result, str) and len(result) > 0

    def test_empty_emotion_falls_back(self):
        # 空字符串 emotion 也应回退 neutral（防御性）
        result = lib.get_instruction("", 2, "neutral")
        neutral_base = lib.INSTRUCTION_LIBRARY["neutral"]
        assert result.startswith(neutral_base)


# ---------------- P4-05：不同 emotion 不同文案 ----------------

class TestEmotionDifferentiation:
    def test_joy_vs_sadness_differ(self):
        # P4-05：joy 与 sadness 文案不同
        joy = lib.get_instruction("joy", 2, "happy")
        sadness = lib.get_instruction("sadness", 2, "happy")
        assert joy != sadness

    def test_each_emotion_yields_distinct_text(self):
        # 同 strength 同 mood_tier 下，8 个 emotion 两两不同（去掉可能的空串天然相等）
        texts = [lib.get_instruction(emotion, 2, "neutral") for emotion in VALID_EMOTIONS]
        non_empty = [t for t in texts if t]
        assert len(set(non_empty)) == len(non_empty), "存在重复的 emotion 文案"


# ---------------- 文案含情绪词（任务扩展断言）----------------

class TestEmotionWordPresence:
    def test_joy_text_contains_happy_words(self):
        # joy 文案必须含"开心/高兴/雀跃/活泼/元气/上扬/俏皮"类情绪词（素材 §1.3 joy）
        text = lib.get_instruction("joy", 2, "neutral")
        assert any(w in text for w in _JOY_EMOTION_WORDS), (
            f"joy 文案应含开心/高兴类词，实际: {text!r}"
        )

    def test_sadness_text_contains_sad_words(self):
        # sadness 文案应含委屈/失落类语义（素材 §1.3 sadness）
        text = lib.get_instruction("sadness", 2, "neutral")
        assert any(w in text for w in ("委屈", "失落", "低落", "难过", "哽咽")), (
            f"sadness 文案应含委屈/失落类词，实际: {text!r}"
        )

    def test_anger_text_contains_anger_words(self):
        # anger 文案应含傲娇/生气/脾气类语义（素材 §1.3 anger）
        text = lib.get_instruction("anger", 2, "neutral")
        assert any(w in text for w in ("傲", "气", "脾", "嘴硬", "生闷气")), (
            f"anger 文案应含傲娇/生气类词，实际: {text!r}"
        )

    def test_smirk_text_contains_sly_words(self):
        # smirk 文案应含得意/腹黑/玩味类语义（素材 §1.3 smirk）
        text = lib.get_instruction("smirk", 2, "neutral")
        assert any(w in text for w in ("得意", "腹黑", "戏谑", "玩味", "挑衅", "轻笑")), (
            f"smirk 文案应含得意/腹黑类词，实际: {text!r}"
        )


# ---------------- strength 影响文案（任务扩展断言）----------------

class TestStrengthInfluence:
    def test_strength_1_2_3_affect_text(self):
        """strength 1-3 文案不同，或至少强度 3 文案含幅度词（"非常/特别/强烈/激动"等）。

        契约未硬性规定 strength 如何影响文案（架构 §4.4.1 标注"当前仅影响选词，可选扩展"），
        但任务要求"strength 影响文案（强度 1-3 文案不同或幅度词）"。用可满足任一分支的
        可机械断言：强度 2 与强度 1 不同，或强度 3 文案含幅度词。
        """
        s1 = lib.get_instruction("joy", 1, "neutral")
        s2 = lib.get_instruction("joy", 2, "neutral")
        s3 = lib.get_instruction("joy", 3, "neutral")

        texts_differ = len({s1, s2, s3}) > 1
        s3_has_magnitude = any(w in s3 for w in _STRENGTH_MAGNITUDE_KEYWORDS)

        assert texts_differ or s3_has_magnitude, (
            f"strength 未影响文案：s1={s1!r}, s2={s2!r}, s3={s3!r} 全相同且无幅度词"
        )
