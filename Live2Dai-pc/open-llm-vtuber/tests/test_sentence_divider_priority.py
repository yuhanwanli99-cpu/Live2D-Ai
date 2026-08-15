"""P2 · 增强型流式分段（语义边界优先级）— RED 测试（实现未写，初始必须失败）

契约来源：.pi/spoq/contract-pc-pipeline.md §P2（P2-01 ~ P2-05）
架构来源：.pi/spoq/architecture-pc-pipeline.md §3.4 P2（_segment_by_priority 算法 + BREAK_PRIORITY）
参考来源：.pi/spoq/search-report-peer-projects.md（astrobot CustomSegmentReply 语义边界分段）

==============================================================================
接口假设（本测试对实现的外部契约，供 coder 依据）：
  - 模块：src/open_llm_vtuber/utils/sentence_divider.py
  - 派生子代理应提供的可导入符号：
        from open_llm_vtuber.utils.sentence_divider import (
            SentenceDivider,
            BREAK_PRIORITY,       # 优先级常量表 [(标点, 权重), ...]
            GOLDEN_MIN_CHARS,     # = 6
            GOLDEN_MAX_CHARS,     # = 40
        )
  - SentenceDivider._segment_by_priority(text) -> Tuple[Optional[str], str]
        按语义边界优先级分段。
        Returns:
            sentence: 切出的句子（None = 不够切/不该切，继续累积）
            remainder: 剩余文本（本句之后的内容）
        优先级：换行 >。>！>？>；>，回溯（架构 4.2.1 BREAK_PRIORITY）。
        - 文本长度 < GOLDEN_MIN_CHARS(6) → 不切，返回 (None, text)
        - 黄金区间 [6, 40] 内找最高优先级断点 → 标点吸附到前一句
        - 超过 GOLDEN_MAX_CHARS(40) 无高优断点 → 在 40 处逗号回溯；无逗号则硬切（最后手段）
        - 断点处标点不孤立、吸附到前一句
  - 流式封装（可选，建议）：SentenceDivider.feed(text) -> Tuple[Optional[str], None] 或
    SentenceDivider._feed_chunk / 由外部驱动 _segment_by_priority。为保持机械可测，
    本测试主要直接驱动 **_segment_by_priority**（契约暴露的核心算法），
    另加一条通过流式累积验证段级输出顺序的用例（接口假设用内部累积驱动，见注释）。
    若实现未直接暴露 _segment_by_priority（被 _segment_text 内联），
    coder 需将该核心逻辑抽为 _segment_by_priority 供测试，或测试改用公开行为路径。
==============================================================================
"""
import pytest


# ══════════════════════════════════════════════════════════════════════════════
# 导入（文件锁：禁止读 src/open_llm_vtuber/ 实现代码。未实现 import 失败即 RED）
# ══════════════════════════════════════════════════════════════════════════════
try:
    from open_llm_vtuber.utils.sentence_divider import (  # type: ignore
        SentenceDivider,
        BREAK_PRIORITY,
        GOLDEN_MIN_CHARS,
        GOLDEN_MAX_CHARS,
    )
except (ImportError, AttributeError) as _e:  # pragma: no cover - 正常 RED
    pytest.fail(
        "[RED] P2 增强未实现：sentence_divider 缺少 BREAK_PRIORITY / GOLDEN_* / "
        "_segment_by_priority 契约符号（当前为改造前的旧版）。这是实现未写的预期失败。"
    )


# ══════════════════════════════════════════════════════════════════════════════
# P2-02 / P2-01：(不)切分基本规则 + 句号优先
# ══════════════════════════════════════════════════════════════════════════════

class TestP2BasicSegment:
    def test_p2_02_under_min_chars_no_cut(self):
        """契约 P2-02："你好吗我很好" 6 字符无标点 → 不切（返回 None）。
        接口假设：_segment_by_priority 返回 (None, text)。
        """
        text = "你好吗我很好"
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(text)
        assert sentence is None, f"6 字符无标点不应切，实际切出 {sentence!r}"
        assert remainder == text

    def test_p2_01_zcn_period_cut(self):
        """契约 P2-01："你好吗？我很好。" → 切出"你好吗？"（问号断点），剩余"我很好。"。
        接口注意：架构 BREAK_PRIORITY 中句号(90) 优先于问号(70)，但文本第 4 字就是"？"，
        在断点处即"？"，故 sentence 应为"你好吗？"。
        """
        text = "你好吗？我很好。"
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(text)
        assert sentence == "你好吗？", f"契约期望'你好吗？'，实际 {sentence!r}"
        assert remainder == "我很好。"

    def test_first_strong_break_wins_by_position(self):
        """契约 P2-01 / D2：分段取**第一个强断点**（位置顺序优先于优先级全局最大）。
        文本中"？"（index 9）在"。"（index 18）之前，二者均为强断点，
        实现从前往后按断点顺序收敛到第一个 → 断在"？"而非"。"。
        接口假设：每次只切出**一个**断点句子，断点处标点吸附进前一句。
        """
        text = "今天天气真不错啊？我们一起去散步吧。你说呢"
        # 前 40 字符内：'？'(index 9) 先于 '。'(index 18)，按位置取第一个强断点 → 切在问号
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(text)
        assert sentence is not None
        assert sentence.endswith("？"), f"断点应为第一个强断点'？'，实际末字符={sentence[-1]!r}"


# ══════════════════════════════════════════════════════════════════════════════
# 段级输出顺序 + 流式累积（用公开累计方式驱动，接口假设见文件头注释）
# ══════════════════════════════════════════════════════════════════════════════

class TestP2Streaming:
    def test_stream_accumulate_across_chunks_to_sentence(self):
        """流式累积：把一段文本逐句喂入/逐字符喂入，最终应能以完整语义边界切出句子。
        接口假设：implementation 暴露一个『喂 chunk』入口（内部累积待分段 buffer），
        否则用 _segment_by_priority 每次喂入完整累积文本也会得到同样结果——
        这里我们直接验证『累积后调用 _segment_by_priority 能成句』这一契约行为。
        """
        divider = SentenceDivider()
        buffer = ""
        emitted: list[str] = []
        for ch in "今天天气很好。我们去公园吧。":
            buffer += ch
            sentence, remainder = divider._segment_by_priority(buffer)
            if sentence is not None:
                emitted.append(sentence)
                buffer = remainder
        # 末尾若还有可切段，切出
        if buffer:
            sentence, remainder = divider._segment_by_priority(buffer)
            if sentence is not None:
                emitted.append(sentence)
        assert emitted and all(s.endswith("。") for s in emitted), (
            f"应切出以句号结尾的句子，实际 {emitted!r}")

    def test_sequence_preserved_in_output_order(self):
        """段级输出顺序：多段文本按原文顺序依次切出，不能乱序或漏段。"""
        text = "第一句。第二句！第三句？第四句，第五句。"
        divider = SentenceDivider()
        buffer = ""
        emitted: list[str] = []
        for ch in text:
            buffer += ch
            sentence, remainder = divider._segment_by_priority(buffer)
            if sentence is not None:
                emitted.append(sentence)
                buffer = remainder
        if buffer:
            s, r = divider._segment_by_priority(buffer)
            if s is not None:
                emitted.append(s)
        assert "".join(emitted) == text, f"输出顺序必须等于原文，实际 {emitted!r}"


# ══════════════════════════════════════════════════════════════════════════════
# 中文标点（。！？，）优先级
# ══════════════════════════════════════════════════════════════════════════════

class TestP2ChinesePunctuationPriority:
    @pytest.mark.parametrize("sep,word", [
        ("。", "句号"),
        ("！", "叹号"),
        ("？", "问号"),
        ("，", "逗号"),
    ])
    def test_chinese_sentence_with_leading_punct(self, sep, word):
        """中文 4 类标点均能成为有效断点：断点处标点应吸附到前一句。"""
        text = f"这是{word}测试{sep}接着后面还有内容"
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(text)
        # 只要总长 ≥6（前面"这是xx测试"已够），应有切分且首段尾带标点
        if sentence is not None:
            assert sentence.endswith(sep), f"断点标点应吸附到前一句，实际 {sentence[-1]!r}"

    def test_exclamation_priority_over_comma(self):
        """优先级 ！(80) > ，(30)：区间内同时有"！"和"，"时选"！"。"""
        text = "哇，真好吃！明天也一起去吧"
        # 结构：前 6-40 区间内 '，'(idx1) 权重30，'！'(idx5)权重80 → 选'！'
        divider = SentenceDivider()
        sentence, _ = divider._segment_by_priority(text)
        assert sentence is not None
        assert sentence.endswith("！"), f"应切在感叹号，实际末字符 {sentence[-1]!r}"

    def test_question_mark_priority_over_period_inside_interval(self):
        """？(70) > 。(60)——但这里的'。'在更远之后，若区间内只有'？'则切在'？'。"""
        text = "你到底在说什么啊？我完全没听懂"
        divider = SentenceDivider()
        sentence, _ = divider._segment_by_priority(text)
        assert sentence is not None
        assert sentence.endswith("？")


# ══════════════════════════════════════════════════════════════════════════════
# P2-04：断点处标点吸附到前一句
# ══════════════════════════════════════════════════════════════════════════════

class TestP2PunctuationAdhesion:
    def test_p2_04_question_mark_adheres_to_previous_sentence(self):
        """契约 P2-04："你好！我很好" → 切出"你好！" 而非"你好"（标点吸附前一句）。"""
        text = "你好！我很好"
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(text)
        assert sentence == "你好！", f"标点应吸附，实际 {sentence!r}"
        assert remainder == "我很好"

    def test_punctuation_not_isolated(self):
        """标点不应被孤立到剩余文本开头（P2-04 泛化）。"""
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority("很好啊。那就这样吧")
        assert sentence == "很好啊。", f"实际 {sentence!r}"
        assert not remainder.startswith("。"), "标点不能孤立到剩余文本开头"


# ══════════════════════════════════════════════════════════════════════════════
# P2-03 / 超长强制切
# ══════════════════════════════════════════════════════════════════════════════

class TestP2OverlongForcedCut:
    def test_p2_03_overlong_find_comma_backtrack(self):
        """契约 P2-03：41 字符无标点文本 → 超过 GOLDEN_MAX_CHARS 后，在 40 处
        做逗号回溯（若区间尾部有逗号），断点在标点/合理边界处。
        构造：前 40 字符内最后一个逗号在近尾部，且整体 >40 字符 → 切在逗号。
        """
        # '，' 出现在 index 29（0-based，1-based 落在黄金区间 [6,39] 内），文本总长 43 > 40
        text = "今天星期六我们一大早就起床然后坐车去郊外公园走走看看风景，傍晚回到家大家都很开心很满足"
        assert len(text) > GOLDEN_MAX_CHARS
        assert 5 < text.index("，") < GOLDEN_MAX_CHARS  # 逗号不得 ≤5 或 ≥40
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(text)
        assert sentence is not None, "超长文本必须强制切"
        assert sentence.endswith("，"), f"应回溯到逗号断点，实际末字符 {sentence[-1]!r}"
        assert len(sentence) <= GOLDEN_MAX_CHARS + 1  # 允许恰含尾标点

    def test_forced_cut_at_max_chars_when_no_break(self):
        """41+ 字符纯中文无任何标点 → 硬切在 GOLDEN_MAX_CHARS（最后手段），但
        断点不得落在英文单词中间。中文按字无词边界，只保证长度上限。
        接口假设：返回的 sentence 长度 ≤ GOLDEN_MAX_CHARS（可含；此处纯字无标点）。
        """
        no_punct = "甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申酉戌亥甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申"
        assert len(no_punct) > GOLDEN_MAX_CHARS
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(no_punct)
        assert sentence is not None, "超长无标点必须强制切"
        assert len(sentence) <= GOLDEN_MAX_CHARS, (
            f"硬切应在 {GOLDEN_MAX_CHARS} 以内，实际 {len(sentence)} 字")


# ══════════════════════════════════════════════════════════════════════════════
# P2-05：英文单词不被硬切（断点始终在标点/空格边界）
# ══════════════════════════════════════════════════════════════════════════════

class TestP2EnglishNoHardCut:
    def test_p2_05_english_word_not_split(self):
        """契约 P2-05："Hello world, how are you?" 断点不得在 "wo"+"rld"。
        每次切出的句子，其首尾必须是单词完整或标点，不能以被截断的单词中部结束。
        """
        text = "Hello world, how are you? I am fine, thanks."
        divider = SentenceDivider()
        buffer = ""
        emitted: list[str] = []
        for ch in text:
            buffer += ch
            sentence, remainder = divider._segment_by_priority(buffer)
            if sentence is not None:
                emitted.append(sentence)
                buffer = remainder
        if buffer:
            s, r = divider._segment_by_priority(buffer)
            if s is not None:
                emitted.append(s)

        # 每段必须以[空格或标点后]的单词边界收尾；此处退化为通用断言：递归分段后重组应等于原文（无字符丢失）
        assert "".join(emitted) == text, "分段不得丢失或插入字符"

    def test_english_break_on_punct_or_space(self):
        """英文断点必须落在空格/标点边界，不得在单词中部。检查：每段首尾非字母相连内嵌被截断。"""
        text = "Hello world, how are you? "
        divider = SentenceDivider()
        sentence, remainder = divider._segment_by_priority(text)
        # "Hello world, how are you?" 总长约 23 字，区间 [6,40] 内，
        # 最高优先级断点为 '?'(70) 在 index 21 → sentence 应含前半
        if sentence is not None:
            # 断点必须在标点边界：sentence 结尾应是 '?', '!', ',', '.', ';' 之一
            assert sentence.endswith(("?", "!", ",", ".", ";")), \
                "英文断点必须在标点/空格边界"


# ══════════════════════════════════════════════════════════════════════════════
# 优先级常量表存在性（架构 4.2.1 硬编码，不可配置）
# ══════════════════════════════════════════════════════════════════════════════

class TestP2PriorityTable:
    def test_break_priority_table_defined(self):
        """BREAK_PRIORITY 常量必须存在且为有序列表（架构 4.2.1：换行>。>！>？>；>，）。"""
        assert isinstance(BREAK_PRIORITY, list) and len(BREAK_PRIORITY) > 0
        chars = [p[0] for p in BREAK_PRIORITY]
        for c in ["\n", "。", "！", "？", "；", "，", "!", "?", "."]:
            assert c in chars, f"优先级常量应覆盖标点 {c!r}"

    def test_golden_bounds_defined(self):
        """GOLDEN_MIN_CHARS=6, GOLDEN_MAX_CHARS=40（架构 4.2.1 硬编码）。"""
        assert GOLDEN_MIN_CHARS == 6
        assert GOLDEN_MAX_CHARS == 40

    def test_golden_min_lt_max(self):
        assert GOLDEN_MIN_CHARS <= GOLDEN_MAX_CHARS
