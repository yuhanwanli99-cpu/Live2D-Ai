//! `dialogue` 模块单元测试：句切分 / 编排收尾。
//!
//! 2026-09-11 用户裁决后，工具调用组装与「文本 vs 工具保序」相关用例整体删除
//! （LLM 不暴露任何工具，只做对话）。

use super::*;
use crate::LlmEvent;

// ------------------------------------------------------------ 句切分

fn sentences_with(max: usize, input: &str) -> Vec<String> {
    let mut s = SentenceAssembler::new(max);
    let mut out = s.push(input);
    out.extend(s.flush());
    out
}

/// 确定性伪随机数（xorshift64），避免引入 rand 依赖。
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
}

/// 断言：整段喂入的结果 == 任意随机切割喂入的结果，且逐字无损、无空句。
fn assert_splitting_invariant(input: &str, max: usize) {
    let baseline = sentences_with(max, input);
    assert!(
        baseline.iter().all(|s| !s.is_empty()),
        "不允许空句: {baseline:?}"
    );
    assert_eq!(
        baseline.iter().map(String::as_str).collect::<String>(),
        input,
        "输出必须逐字保留原文"
    );
    for seed in 1..=32u64 {
        let mut rng = XorShift(seed | 1);
        let mut s = SentenceAssembler::new(max);
        let mut got = Vec::new();
        let mut rest = input;
        while !rest.is_empty() {
            // 每步随机吃 1..=7 个字符（按字符边界）。
            let take = (rng.next() % 7 + 1) as usize;
            let take = take.min(rest.chars().count());
            let split: usize = rest.char_indices().nth(take).map_or(rest.len(), |(i, _)| i);
            let (head, tail) = rest.split_at(split);
            got.extend(s.push(head));
            rest = tail;
        }
        got.extend(s.flush());
        assert_eq!(got, baseline, "seed={seed} 切割结果不一致");
    }
}

#[test]
fn chinese_sentences_keep_punctuation() {
    assert_eq!(
        sentences_with(64, "你好呀！今天天气不错。我们出去走走吧？好啊。"),
        ["你好呀！", "今天天气不错。", "我们出去走走吧？", "好啊。"]
    );
}

#[test]
fn english_sentences_split_on_punctuation() {
    // 句间空格原样保留在下一句开头（不吞任何字符）。
    assert_eq!(
        sentences_with(64, "Hello world! How are you? I'm fine."),
        ["Hello world!", " How are you?", " I'm fine."]
    );
}

#[test]
fn newline_is_a_boundary() {
    assert_eq!(
        sentences_with(64, "第一行\n第二行\n第三行无换行"),
        ["第一行\n", "第二行\n", "第三行无换行"]
    );
}

#[test]
fn ellipsis_and_consecutive_punctuation_never_make_empty_sentences() {
    // …… / 。。。 / ?! 都折叠成一个句界，且不产生空句。
    assert_eq!(sentences_with(64, "等等……好。"), ["等等……", "好。"]);
    assert_eq!(
        sentences_with(64, "想想。。。行吧！"),
        ["想想。。。", "行吧！"]
    );
    assert_eq!(sentences_with(64, "什么?!真的。"), ["什么?!", "真的。"]);
    // 整个输入只有标点：不产出空串；残余在 flush 原样输出（逐字保留）。
    assert_eq!(sentences_with(64, "。\n\n"), ["。\n\n"]);
    let mut s = SentenceAssembler::new(8);
    assert!(s.push("……").is_empty());
    assert_eq!(s.buffered_chars(), 2);
}

#[test]
fn decimals_are_conservatively_not_split() {
    // 中英文小数都不切。
    assert_eq!(
        sentences_with(64, "圆周率约3.14，很好。"),
        ["圆周率约3.14，很好。"]
    );
    assert_eq!(
        sentences_with(64, "pi is 3.14159 ok? yes."),
        ["pi is 3.14159 ok?", " yes."]
    );
    // 「3.」结尾（可能是没写完的小数）也保守不切，flush 时原样保留。
    assert_eq!(sentences_with(64, "值约为3."), ["值约为3."]);
}

#[test]
fn lowercase_following_period_is_conservatively_not_split() {
    // 缩写续词（后随小写字母）不切；正常句界（大写/中文/空白结尾）照切。
    assert_eq!(
        sentences_with(64, "send to mr.smith now."),
        ["send to mr.smith now."]
    );
    assert_eq!(
        sentences_with(64, "use e.g.this thing. Done."),
        ["use e.g.this thing.", " Done."]
    );
}

#[test]
fn arbitrary_delta_splitting_is_consistent() {
    let corpus = [
        "你好呀！今天天气不错。我们出去走走吧？好啊。",
        "等等……让我想想。。。好吧！什么?!来吧。",
        "Hello world! How are you? I'm fine. Me too.",
        "圆周率约3.14，pi=3.14159。see e.g.notes and mr.smith soon. Done.",
        "第一行\n第二行\n第三行无换行",
        "无标点的很长中文输入会在缓冲上限处被强制切开但结果必须稳定",
    ];
    for text in corpus {
        for max in [1usize, 3, 8, 48] {
            assert_splitting_invariant(text, max);
        }
    }
}

// ---------------------------------------------- 一句一单元（不断句）契约

/// **回归（2026-09-10 用户裁决）**：一句话即使**远超旧上限 48 字**，
/// 也必须作为**一整个**句子输出，不得按字符位置劈开。
///
/// 历史缺陷：`DEFAULT_MAX_CHARS = 48` 会把长句硬切成多段，每段各发一次 TTS
/// 请求、各带一段合成延迟 → 播放时逐段空档，听感即「断断续续」。
#[test]
fn long_sentence_is_never_split_at_the_old_48_char_bound() {
    // 60+ 字无逗号长句 + 句号 —— 旧实现会切成 ["…48字", "…剩余字。"]。
    let long = "我一直觉得今天的天气真的很适合出门散步而且附近那家新开的咖啡店据说味道相当不错有空的话我们下午可以一起过去坐坐看看书";
    assert!(long.chars().count() > 48, "用例需超过旧上限");
    let text = format!("{long}。");
    let whole = sentences_with(SentenceAssembler::DEFAULT_MAX_CHARS, &text);
    assert_eq!(whole.len(), 1, "默认上限下长句必须整句输出，实际 {whole:?}");
    assert_eq!(whole[0], text);
    // 旧上限下确实会被切开：证明本用例真的覆盖了那条路径。
    assert!(
        sentences_with(48, &text).len() > 1,
        "旧上限下应当被切开，否则用例没有覆盖回归点"
    );
}

/// 默认上限必须显著高于常见口语句长，否则「一句一单元」名存实亡。
#[test]
fn default_max_chars_leaves_room_for_a_whole_spoken_sentence() {
    // 绑定到局部变量而不是直接断言常量：断言常量表达式会被 clippy 的
    // `assertions_on_constants` 判为无意义（而这条断言的意图是**契约锁**）。
    let bound = SentenceAssembler::DEFAULT_MAX_CHARS;
    assert!(bound >= 120, "默认安全阀过小（{bound}）：会把正常长句劈开");
    // 且必须真的比常见口语句长留有余量：用一条典型长句验证。
    let typical = "今天下午我打算先去图书馆还几本书然后再去超市买点东西晚上回来做饭";
    assert!(
        typical.chars().count() < bound,
        "常见长句（{} 字）不应触及安全阀 {bound}",
        typical.chars().count()
    );
}

/// 安全阀触发时**优先在弱标点处断开**（听感是停顿），而不是按位置硬切。
#[test]
fn safety_valve_prefers_soft_punctuation_over_positional_cut() {
    // 上限 10；第 6 个字符是逗号 → 应在逗号后断开，而不是切在第 10 个字符处。
    let text = "一二三四五，六七八九十十一十二";
    let out = sentences_with(10, text);
    assert_eq!(
        out[0], "一二三四五，",
        "安全阀应切在弱标点之后，实际切在位置 10 会得到「一二三四五六七八九十」"
    );
    assert_eq!(out.iter().map(String::as_str).collect::<String>(), text);
}

/// 完全没有弱标点时仍须有界（退回按位置硬切），且切点落在字符边界上。
#[test]
fn safety_valve_still_bounds_when_no_soft_punctuation_exists() {
    assert_eq!(
        sentences_with(3, "一二三四五六七"),
        ["一二三", "四五六", "七"]
    );
}

/// 弱标点切分同样满足「任意 delta 切割结果一致 + 逐字无损」不变量。
#[test]
fn soft_break_cutting_keeps_the_delta_splitting_invariant() {
    for text in [
        "一二三四五，六七八九十，十一十二十三。",
        "a,b,c,d,e,f,g,h,i,j,k,l",
        "开头，然后很长很长很长很长很长很长很长很长没有句号",
    ] {
        for max in [1usize, 3, 6, 10, 20] {
            assert_splitting_invariant(text, max);
        }
    }
}

#[test]
fn max_buffer_bounds_pending_and_cuts_deterministically() {
    let text = "abcdefghijklmnopqrstuvwxyz"; // 26 字符，无句界
    let mut s = SentenceAssembler::new(10);
    let mut out = s.push(text);
    out.extend(s.flush());
    assert_eq!(out, ["abcdefghij", "klmnopqrst", "uvwxyz"]);
    // 缓冲有界：push 之后永远 ≤ 上限。
    let mut s = SentenceAssembler::new(5);
    let mut rng = XorShift(7);
    let mut rest = text;
    while !rest.is_empty() {
        let take = ((rng.next() % 4 + 1) as usize).min(rest.chars().count());
        let split = rest.char_indices().nth(take).map_or(rest.len(), |(i, _)| i);
        s.push(&rest[..split]);
        assert!(s.buffered_chars() <= 5);
        rest = &rest[split..];
    }
    // 多字节字符不被劈开：强制切点落在字符边界上。
    let wide = "一二三四五六七八九十";
    assert_eq!(
        sentences_with(3, wide),
        ["一二三", "四五六", "七八九", "十"]
    );
}

#[test]
fn flush_resolves_hanging_terminators_and_tail() {
    let mut s = SentenceAssembler::new(16);
    assert!(s.push("还没说完").is_empty());
    assert_eq!(s.flush(), ["还没说完"]);
    // flush 幂等且之后可复用。
    assert!(s.flush().is_empty());
    assert_eq!(s.push("第二句。tail"), ["第二句。"]);
    assert_eq!(s.flush(), ["tail"]);

    // 行尾悬挂的「。」由 flush 封口。
    let mut s = SentenceAssembler::new(16);
    assert!(s.push("你好。").is_empty()); // run 悬挂，等下一个字符
    assert_eq!(s.push("再见"), ["你好。"]); // 「再见」证明 run 结束
    assert_eq!(s.flush(), ["再见"]);
}

// ------------------------------------------------------------ 对话编排

fn sentence_texts(events: &[DialogueEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            DialogueEvent::SentenceReady { text } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn dialogue_text_deltas_become_sentences_then_exactly_one_done() {
    let mut d = DialogueAssembler::new(64);
    let mut out = Vec::new();
    out.extend(d.push(&LlmEvent::TextDelta("好的。".into())));
    // 半句：还不出句界。
    out.extend(d.push(&LlmEvent::TextDelta("完成".into())));
    out.extend(d.push(&LlmEvent::TextDelta("！".into())));
    out.extend(d.push(&LlmEvent::Done));

    assert_eq!(sentence_texts(&out), ["好的。", "完成！"]);
    let seq: Vec<&str> = out
        .iter()
        .map(|e| match e {
            DialogueEvent::SentenceReady { .. } => "S",
            DialogueEvent::Done => "D",
        })
        .collect();
    assert_eq!(seq, ["S", "S", "D"]);
    assert!(matches!(out.last(), Some(DialogueEvent::Done)));
}

#[test]
fn dialogue_flush_is_fallback_done_and_terminal() {
    let mut d = DialogueAssembler::new(64);
    d.push(&LlmEvent::TextDelta("还没说完".into()));
    let out = d.flush();
    assert_eq!(sentence_texts(&out), ["还没说完"]);
    assert!(matches!(out.last(), Some(DialogueEvent::Done)));
    // 终态：flush/push 都不再产生事件。
    assert!(d.flush().is_empty());
    assert!(d.push(&LlmEvent::TextDelta("迟到的".into())).is_empty());
    assert!(d.push(&LlmEvent::Done).is_empty());
}

#[test]
fn dialogue_done_is_idempotent_and_matches_flush_shape() {
    let mut d = DialogueAssembler::new(64);
    d.push(&LlmEvent::TextDelta("你好。".into()));
    let out = d.push(&LlmEvent::Done);
    assert_eq!(out.len(), 2); // SentenceReady + Done
    assert!(matches!(out[0], DialogueEvent::SentenceReady { .. }));
    assert!(matches!(out[1], DialogueEvent::Done));
    // 第二次 Done（或 flush）不再产出。
    assert!(d.push(&LlmEvent::Done).is_empty());
}

#[test]
fn dialogue_overflow_sentence_still_emits_without_punctuation() {
    let mut d = DialogueAssembler::new(4);
    let out = d.push(&LlmEvent::TextDelta("一二三四五六七".into()));
    assert_eq!(sentence_texts(&out), ["一二三四"]);
}
