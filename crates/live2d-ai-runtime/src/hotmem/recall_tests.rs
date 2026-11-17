//! 「记」的纯逻辑测试：抽取结果解析 + 近期折摘要。

use super::*;
use crate::hotmem::fact::FactKind;

// ------------------------------------------------------------ 抽取结果解析

#[test]
fn parse_recalled_reads_a_plain_json_array() {
    let raw =
        r#"[{"text":"用户叫星梦","kind":"durable"},{"text":"今天聊了天气","kind":"episode"}]"#;
    let got = parse_recalled(raw);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].text, "用户叫星梦");
    assert_eq!(got[0].kind, FactKind::Durable);
    assert_eq!(got[1].text, "今天聊了天气");
    assert_eq!(got[1].kind, FactKind::Episode);
}

/// 模型很爱把 JSON 包在 markdown 围栏里——这是**真实会遇到**的形状。
#[test]
fn parse_recalled_tolerates_code_fences_and_chatter() {
    let raw = concat!(
        "好的，这是结果：\n",
        "```json\n",
        "[{\"text\":\"用户喜欢薄荷\",\"kind\":\"durable\"}]\n",
        "```\n",
        "希望有用！"
    );
    let got = parse_recalled(raw);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].text, "用户喜欢薄荷");
    assert_eq!(got[0].kind, FactKind::Durable);
}

/// 坏元素逐条丢弃，不整体失败；未知 kind 回落 episode（抽取归一的语义）。
#[test]
fn parse_recalled_drops_bad_items_and_defaults_unknown_kind() {
    let raw = r#"[{"text":"保留一","kind":"durable"},{"kind":"durable"},{"text":"   "},{"text":123},{"text":"保留二"},{"text":"保留二"}]"#;
    let got = parse_recalled(raw);
    assert_eq!(got.len(), 2, "坏的/空的/重复的都要丢：{got:?}");
    assert_eq!(got[0].text, "保留一");
    assert_eq!(got[1].text, "保留二");
    assert_eq!(got[1].kind, FactKind::Episode, "缺 kind → episode");
}

/// 畸形输入一律返回空表（= 抽取失败，调用方走兜底），**绝不 panic**。
#[test]
fn parse_recalled_is_total_on_garbage() {
    for raw in [
        "",
        "没有数组",
        "[",
        "]",
        "[]",
        "[[[",
        "[{",
        "null",
        "[1,2,3]",
    ] {
        let got = parse_recalled(raw);
        assert!(got.is_empty(), "{raw:?} → {got:?}");
    }
    // 前缀合法数组 + 尾部垃圾：仍然能读出这一条。
    let trailing = r#"[{"text":"x","kind":"durable"}] 后续垃圾"#;
    assert_eq!(parse_recalled(trailing).len(), 1);
}

/// 单条正文按 120 字截断、一次最多 8 条（预算有界）。
#[test]
fn parse_recalled_clips_length_and_count() {
    let long = "字".repeat(500);
    let raw = format!(r#"[{{"text":"{long}","kind":"durable"}}]"#);
    let got = parse_recalled(&raw);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].text.chars().count(), MAX_RECALL_TEXT_CHARS);

    let many: Vec<String> = (0..20)
        .map(|i| format!(r#"{{"text":"第{i}条","kind":"episode"}}"#))
        .collect();
    let raw = format!("[{}]", many.join(","));
    assert_eq!(parse_recalled(&raw).len(), MAX_RECALL_ITEMS);
}

// ------------------------------------------------------------ 近期折摘要

#[test]
fn fold_summary_appends_only_the_dropped_turns() {
    let dropped = vec![
        ("你好".to_string(), "你好呀".to_string()),
        ("今天下雨".to_string(), "记得带伞".to_string()),
    ];
    let got = fold_summary("", &dropped);
    assert_eq!(got, "你好 → 你好呀 / 今天下雨 → 记得带伞");
}

/// 摘要**替换**而不无限加长：多次折叠后长度始终 ≤300。
#[test]
fn fold_summary_never_exceeds_the_char_budget() {
    let mut summary = String::new();
    for round in 0..50 {
        let dropped: Vec<(String, String)> = (0..3)
            .map(|i| {
                (
                    format!("第{round}轮第{i}句用户说的话，长度足够占位"),
                    format!("第{round}轮第{i}句助手回的话，长度足够占位"),
                )
            })
            .collect();
        summary = fold_summary(&summary, &dropped);
        assert!(
            summary.chars().count() <= SUMMARY_MAX_CHARS,
            "第 {round} 轮超限：{} 字",
            summary.chars().count()
        );
    }
    assert!(!summary.is_empty(), "折叠不该把内容全丢光");
    assert!(summary.contains("49"), "最新的轮次必须留住：{summary}");
}

#[test]
fn fold_summary_handles_empty_and_half_empty_pairs() {
    assert_eq!(fold_summary("", &[]), "");
    assert_eq!(
        fold_summary("", &[(String::new(), String::new())]),
        "",
        "空对不产生占位"
    );
    assert_eq!(
        fold_summary("", &[("只有用户句".to_string(), String::new())]),
        "只有用户句"
    );
    assert_eq!(
        fold_summary("", &[(String::new(), "只有助手句".to_string())]),
        "只有助手句"
    );
}

// ------------------------------------------------------------ 抽取提示词

#[test]
fn recall_prompt_asks_for_a_json_array_only() {
    assert!(RECALL_SYSTEM_PROMPT.contains("JSON"));
    assert!(RECALL_SYSTEM_PROMPT.contains("durable"));
    assert!(RECALL_SYSTEM_PROMPT.contains("episode"));
    assert!(RECALL_SYSTEM_PROMPT.chars().count() < 300, "抽取提示词要短");
    let msg = recall_user_message("  你好  ", " 你也好 ");
    assert_eq!(msg, "用户说：你好\n助手回：你也好");
}
