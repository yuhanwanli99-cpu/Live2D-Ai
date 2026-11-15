//! 摘要块与「掉出窗口的轮次」的契约（M4 接线侧）。
//!
//! 复核指出：M4 的摘要**只有纯函数 + 单测**，生产链路既没折叠也没渲染。
//! 这里补上两端的判据——
//! 1. 引擎在 max_history_pairs 挤掉轮次时**交出原文**（不是直接丢），宿主据此折叠；
//! 2. 摘要作为独立一块进 system 文本，空摘要不写标题。

use super::*;
use crate::config::{LlmConfig, TtsConfig};
use crate::conversation::{ConversationConfig, ConversationEngine};

fn engine(max_pairs: usize) -> ConversationEngine {
    let client = crate::OpenAiClient::new(
        LlmConfig::new("http://127.0.0.1:9/v1", "m"),
        TtsConfig::new("http://127.0.0.1:9/v1", "alloy"),
    )
    .expect("client");
    ConversationEngine::new(
        client,
        ConversationConfig {
            max_history_pairs: max_pairs,
            ..ConversationConfig::new("人设")
        },
    )
}

/// 超出窗口的轮次**交出来**而不是静默丢掉——摘要折叠的唯一数据来源。
#[test]
fn engine_hands_out_the_turns_it_evicts() {
    let mut eng = engine(2);
    eng.commit_completed_turn("一", "答一");
    eng.commit_completed_turn("二", "答二");
    assert_eq!(eng.history_len(), 2);
    assert!(
        eng.take_evicted_turns().is_empty(),
        "还没挤掉任何轮次，不该有输出"
    );

    eng.commit_completed_turn("三", "答三");
    assert_eq!(eng.history_len(), 2, "窗口仍是 2");
    let dropped = eng.take_evicted_turns();
    assert_eq!(
        dropped,
        vec![("一".to_string(), "答一".to_string())],
        "最旧的一对被挤出，且**原文交出来**"
    );
    assert!(eng.take_evicted_turns().is_empty(), "取走即清，不重复折叠");
}

/// max_history_pairs = 0（不保留历史）时，**每一轮都算掉出窗口**。
#[test]
fn zero_window_evicts_every_turn() {
    let mut eng = engine(0);
    eng.commit_completed_turn("一", "答一");
    eng.commit_completed_turn("二", "答二");
    assert_eq!(eng.history_len(), 0);
    assert_eq!(
        eng.take_evicted_turns(),
        vec![
            ("一".to_string(), "答一".to_string()),
            ("二".to_string(), "答二".to_string())
        ]
    );
}

/// 折叠后摘要进 system 文本，作为独立一块；空摘要不写标题。
#[test]
fn summary_renders_as_its_own_block() {
    let card = HotCard::from_lines(vec![CardLine::new(CardTier::Clear, "用户喜欢薄荷")], &[]);
    let text = compose_system_text_with_summary(
        "系统句",
        "- 用户叫星梦",
        Some(&card),
        "之前聊过天气 / 说好周末出门",
    );
    assert_eq!(
        text,
        "系统句\n\n【钉住的事】\n- 用户叫星梦\n\n【本轮相关】\n- [清楚] 用户喜欢薄荷\n\n【之前聊过】\n之前聊过天气 / 说好周末出门"
    );
    assert_eq!(
        compose_system_text_with_summary("系统句", "", None, ""),
        "系统句",
        "空摘要不写标题"
    );
    assert_eq!(
        compose_system_text_with_summary("系统句", "", None, "   \n "),
        "系统句",
        "纯空白摘要等同没有"
    );
}

/// 端到端口径：多轮超窗 → 折叠出的摘要 ≤300 字。
#[test]
fn evicted_turns_fold_into_a_bounded_summary() {
    let mut eng = engine(1);
    let mut summary = String::new();
    for round in 0..12 {
        eng.commit_completed_turn(
            &format!("第{round}轮用户说的话，内容足够长以便占位"),
            &format!("第{round}轮助手的回复，内容足够长以便占位"),
        );
        let dropped = eng.take_evicted_turns();
        if !dropped.is_empty() {
            summary = fold_summary(&summary, &dropped);
        }
    }
    assert!(!summary.is_empty(), "应当折叠出内容");
    assert!(
        summary.chars().count() <= SUMMARY_MAX_CHARS,
        "摘要必须落在 300 字内：{}",
        summary.chars().count()
    );
    assert!(
        summary.contains("11") || summary.contains("10"),
        "最新轮次要在：{summary}"
    );
}
