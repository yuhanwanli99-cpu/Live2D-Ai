//! `dialogue` 模块单元测试：句切分 / 工具调用组装 / 编排保序。

use super::*;
use crate::Live2dPerformActionArgs;
use crate::LlmEvent;
use crate::dialogue::tool::IncompleteReason;
use crate::tool::PerformAction;

// ------------------------------------------------------------ 工具

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

// ------------------------------------------------------------ 工具调用

const NAME: &str = "live2d_perform_action";

fn nod_args(strength: u8) -> Live2dPerformActionArgs {
    Live2dPerformActionArgs {
        action: PerformAction::Nod,
        strength,
        reason: None,
    }
}

#[test]
fn multiple_calls_assembled_by_index_across_fragments() {
    let mut t = ToolCallAssembler::new();
    // 两个调用交错到达；name 也劈成两个分片。
    t.push(0, Some("call_a"), Some("live2d_perform"), "");
    t.push(0, None, Some("_action"), "{\"action\":\"nod\",");
    t.push(1, Some("call_b"), Some(NAME), "{\"action\":\"tilt\"");
    t.push(1, None, None, ",\"strength\":3}");

    // call_0 的参数还缺一截：封存只取走已完整的 call_1（首现顺序语义不变）。
    let ready = t.seal_parseable();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].index, 1);
    assert_eq!(ready[0].id.as_deref(), Some("call_b"));
    assert_eq!(ready[0].arguments.action, PerformAction::Tilt);
    assert_eq!(ready[0].arguments.strength, 3);

    // 补齐 call_0 后 DONE 收尾。
    t.push(0, None, None, "\"strength\":2}");
    assert!(!t.is_empty());
    let rest = t.finish().expect("call_0 应结算成功");
    assert_eq!(rest.len(), 1);
    assert_eq!(rest[0].index, 0);
    assert_eq!(rest[0].id.as_deref(), Some("call_a"));
    assert_eq!(rest[0].arguments, nod_args(2));
}

#[test]
fn partial_arguments_are_not_sealed_until_complete() {
    let mut t = ToolCallAssembler::new();
    t.push(0, Some("c"), Some(NAME), "{\"action\":\"no");
    assert!(t.seal_parseable().is_empty());
    t.push(0, None, None, "d\",\"strength\":1}");
    let ready = t.seal_parseable();
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].arguments, nod_args(1));
    assert!(t.is_empty());
}

#[test]
fn missing_fragment_yields_typed_incomplete() {
    // 丢掉中间一段 arguments → JSON 不完整。
    let t = {
        let mut t = ToolCallAssembler::new();
        t.push(0, Some("c0"), Some(NAME), "{\"action\":\"nod\",");
        // 缺 ",\"strength\":2}"
        t
    };
    let err = t.finish().expect_err("应报类型化 Incomplete");
    assert_eq!(err.calls.len(), 1);
    assert_eq!(err.calls[0].index, 0);
    assert_eq!(err.calls[0].id.as_deref(), Some("c0"));
    assert!(matches!(
        err.calls[0].reason,
        IncompleteReason::InvalidArgumentsJson(_)
    ));
    assert!(err.to_string().contains("1 个工具调用未完成"));
}

#[test]
fn missing_name_and_empty_arguments_are_typed() {
    let mut t = ToolCallAssembler::new();
    t.push(0, None, None, "{}");
    t.push(1, Some("c1"), Some(NAME), "");
    let err = t.finish().expect_err("两个都残缺");
    assert_eq!(err.calls.len(), 2);
    assert_eq!(err.calls[0].reason, IncompleteReason::MissingName);
    assert_eq!(err.calls[1].reason, IncompleteReason::MissingArguments);
}

#[test]
fn unsupported_tool_name_is_rejected() {
    let mut t = ToolCallAssembler::new();
    t.push(0, Some("x"), Some("search_web"), "{\"query\":\"hi\"}");
    let err = t.finish().expect_err("只接受 live2d_perform_action");
    assert_eq!(
        err.calls[0].reason,
        IncompleteReason::UnsupportedTool("search_web".to_string())
    );
    assert!(err.to_string().contains(NAME));
}

#[test]
fn finish_returns_all_remaining_calls_in_first_seen_order() {
    let mut t = ToolCallAssembler::new();
    t.push(
        2,
        Some("c2"),
        Some(NAME),
        "{\"action\":\"listen\",\"strength\":1}",
    );
    t.push(
        0,
        Some("c0"),
        Some(NAME),
        "{\"action\":\"surprise\",\"strength\":3}",
    );
    let done = t.finish().expect("全部完整");
    assert_eq!(done.iter().map(|c| c.index).collect::<Vec<_>>(), [2, 0]);
}

#[test]
fn wire_default_index_zero_groups_id_only_fragments() {
    // wire 层对缺 index 的分片补 0：id-only 分片也能把 id 关联到同一调用。
    let mut t = ToolCallAssembler::new();
    t.push(0, Some("only-id-chunk"), None, "");
    t.push(0, None, Some(NAME), "{\"action\":\"nod\",\"strength\":1}");
    let done = t.finish().expect("ok");
    assert_eq!(done[0].id.as_deref(), Some("only-id-chunk"));
}

// ------------------------------------------------------------ 对话编排

fn tool_delta(index: usize, id: Option<&str>, arguments: &str) -> LlmEvent {
    LlmEvent::ToolCallDelta {
        index,
        id: id.map(str::to_owned),
        name: Some(NAME.to_string()),
        arguments: arguments.to_string(),
    }
}

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
fn dialogue_keeps_network_order_between_text_and_tools() {
    let mut d = DialogueAssembler::new(64);
    let mut out = Vec::new();
    out.extend(d.push(&LlmEvent::TextDelta("好的。".into())));
    out.extend(d.push(&tool_delta(
        0,
        Some("t0"),
        "{\"action\":\"nod\",\"strength\":1}",
    )));
    // 新 index 出现 → 封存 call0（插在正确位置）。
    out.extend(d.push(&tool_delta(
        1,
        Some("t1"),
        "{\"action\":\"tilt\",\"strength\":2}",
    )));
    // 正文恢复 → 先封存 call1，再出句子。
    out.extend(d.push(&LlmEvent::TextDelta("完成！".into())));
    out.extend(d.flush());

    assert_eq!(sentence_texts(&out), ["好的。", "完成！"]);
    let tool_indexes: Vec<usize> = out
        .iter()
        .filter_map(|e| match e {
            DialogueEvent::ToolReady { call } => Some(call.index),
            _ => None,
        })
        .collect();
    assert_eq!(tool_indexes, [0, 1]);
    // 网络顺序：S(T0 前) → T0 → T1 → S(T1 后) → Done。
    assert!(matches!(
        out.last(),
        Some(DialogueEvent::Done {
            incomplete_tools: None
        })
    ));
    let seq: Vec<&str> = out
        .iter()
        .map(|e| match e {
            DialogueEvent::SentenceReady { .. } => "S",
            DialogueEvent::ToolReady { .. } => "T",
            DialogueEvent::Done { .. } => "D",
        })
        .collect();
    assert_eq!(seq, ["S", "T", "T", "S", "D"]);
}

#[test]
fn dialogue_reports_incomplete_tools_at_done() {
    let mut d = DialogueAssembler::new(64);
    let mut out = d.push(&LlmEvent::TextDelta("点头".into()));
    out.extend(d.push(&tool_delta(0, Some("t0"), "{\"action\":\"nod\","))); // 故意截断
    out.extend(d.flush());
    assert_eq!(sentence_texts(&out), ["点头"]);
    match out.last() {
        Some(DialogueEvent::Done {
            incomplete_tools: Some(err),
        }) => {
            assert_eq!(err.calls[0].index, 0);
            assert!(matches!(
                err.calls[0].reason,
                IncompleteReason::InvalidArgumentsJson(_)
            ));
        }
        other => panic!("期望携带 Incomplete 的 Done，实际 {other:?}"),
    }
}

#[test]
fn dialogue_flush_is_fallback_done_and_terminal() {
    let mut d = DialogueAssembler::new(64);
    d.push(&LlmEvent::TextDelta("还没说完".into()));
    let out = d.flush();
    assert_eq!(sentence_texts(&out), ["还没说完"]);
    assert!(matches!(
        out.last(),
        Some(DialogueEvent::Done {
            incomplete_tools: None
        })
    ));
    // 终态：flush/push 都不再产生事件。
    assert!(d.flush().is_empty());
    assert!(d.push(&LlmEvent::TextDelta("迟到的".into())).is_empty());
    assert!(d.push(&LlmEvent::Done).is_empty());
}

#[test]
fn dialogue_explicit_done_matches_flush() {
    let mut d = DialogueAssembler::new(64);
    d.push(&tool_delta(0, None, "{\"action\":\"nod\",\"strength\":2}"));
    let out = d.push(&LlmEvent::Done);
    assert_eq!(out.len(), 2); // ToolReady + Done
    assert!(matches!(out[0], DialogueEvent::ToolReady { .. }));
    assert!(matches!(
        out[1],
        DialogueEvent::Done {
            incomplete_tools: None
        }
    ));
}

#[test]
fn dialogue_tools_only_stream_keeps_order_without_text() {
    let mut d = DialogueAssembler::new(64);
    let mut out = Vec::new();
    out.extend(d.push(&tool_delta(
        0,
        Some("a"),
        "{\"action\":\"listen\",\"strength\":1}",
    )));
    // call_1 出现 → 封存 call_0。
    out.extend(d.push(&tool_delta(
        1,
        Some("b"),
        "{\"action\":\"surprise\",\"strength\":3}",
    )));
    out.extend(d.flush());
    let seq: Vec<&str> = out
        .iter()
        .map(|e| match e {
            DialogueEvent::ToolReady { .. } => "T",
            DialogueEvent::Done { .. } => "D",
            DialogueEvent::SentenceReady { .. } => "S",
        })
        .collect();
    assert_eq!(seq, ["T", "T", "D"]);
    assert!(
        out.iter()
            .take(2)
            .all(|e| matches!(e, DialogueEvent::ToolReady { .. }))
    );
}

#[test]
fn dialogue_overflow_sentence_still_emits_without_punctuation() {
    let mut d = DialogueAssembler::new(4);
    let out = d.push(&LlmEvent::TextDelta("一二三四五六七".into()));
    assert_eq!(sentence_texts(&out), ["一二三四"]);
}
