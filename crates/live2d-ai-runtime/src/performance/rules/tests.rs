//! 规则表迁入核心后的回归（rc4，2026-10-17）。
//!
//! 钉子：
//! - `PRESET_IDS` 的字面值**一字不改**（旧 director 契约原样搬入）；
//! - 规则 cue 是纯函数，`rule_cues_for_sentence` 只改句号、实参只能是这一句原文；
//! - 表情槽与表演层的 expression 能力集逐项相等（两份常量不许漂）。

use super::*;
use crate::performance::EXPRESSION_PRESET_IDS;

/// 主 allowlist 的**字面值**冻结：迁移是复制，不是重排 / 增删。
#[test]
fn preset_ids_value_is_frozen_verbatim() {
    assert_eq!(
        PRESET_IDS,
        &[
            "none",
            "smile",
            "unhappy",
            "surprised",
            "thinking",
            "nod",
            "shake",
            "look_left",
            "look_right",
            "look_up",
            "look_down",
            "tilt_left",
            "tilt_right",
        ],
        "PRESET_IDS 字面值不得改动（渲染面 / 面板 / 表演层共用同一份契约）"
    );
    assert_eq!(PRESET_NONE, "none");
    assert_eq!(preset_options(), accepted_preset_ids());
}

/// 表情槽（去掉 none）必须与表演层的 expression 能力集逐项相等。
#[test]
fn expression_slot_matches_the_performance_field_map() {
    let face: Vec<&str> = PRESET_IDS
        .iter()
        .copied()
        .filter(|id| *id != PRESET_NONE && preset_slot(id) == PresetSlot::Face)
        .collect();
    assert_eq!(
        face,
        EXPRESSION_PRESET_IDS.to_vec(),
        "director 的 preset_slot 与 runtime 的 EXPRESSION_PRESET_IDS 漂了"
    );
}

/// 缺省映射表：6 条非 none；问候 / 告别 / 焦虑等按旧契约。
#[test]
fn default_table_keeps_the_six_trigger_mapping() {
    let t = PresetTable::default();
    assert_eq!(
        t.resolve(EmotionHint::Happy, IntentHint::Chat),
        Some("smile")
    );
    assert_eq!(
        t.resolve(EmotionHint::Affectionate, IntentHint::Chat),
        Some("smile")
    );
    assert_eq!(
        t.resolve(EmotionHint::Sad, IntentHint::Complaint),
        Some("unhappy")
    );
    assert_eq!(
        t.resolve(EmotionHint::Angry, IntentHint::Complaint),
        Some("unhappy")
    );
    assert_eq!(
        t.resolve(EmotionHint::Surprised, IntentHint::Question),
        Some("surprised")
    );
    assert_eq!(
        t.resolve(EmotionHint::Neutral, IntentHint::Greeting),
        Some("nod")
    );
    assert_eq!(t.resolve(EmotionHint::Anxious, IntentHint::Chat), None);
    assert_eq!(t.resolve(EmotionHint::Neutral, IntentHint::Farewell), None);
}

/// 规则 cue 的旧契约：seq 恒 1、中性 = 空、问号补空槽。
#[test]
fn rule_cues_for_text_is_deterministic_and_question_patched() {
    assert!(rule_cues_for_text("今天天气不错").is_empty());
    let happy = rule_cues_for_text("今天真开心");
    assert_eq!(happy.len(), 1);
    assert_eq!(happy[0].preset_id, "smile");
    assert_eq!(happy[0].sentence_seq, 1);

    let question = rule_cues_for_text("你在想什么？");
    let ids: Vec<&str> = question.iter().map(|c| c.preset_id.as_str()).collect();
    assert_eq!(ids, ["tilt_left", "thinking"], "问号补空槽（手势 + 表情）");
    assert_eq!(rule_cues_for_text("你在想什么？"), question, "纯函数恒等");
}

/// 按句入口：只改句号，其余字段与 `rule_cues_for_text` 逐项相同。
#[test]
fn rule_cues_for_sentence_rewrites_only_the_seq() {
    let text = "这样可以吗？";
    let one = rule_cues_for_text(text);
    let five = rule_cues_for_sentence(text, 5);
    assert_eq!(one.len(), five.len());
    for (base, seq) in one.iter().zip(&five) {
        assert_eq!(seq.sentence_seq, 5);
        let mut expected = base.clone();
        expected.sentence_seq = 5;
        assert_eq!(seq, &expected);
    }
    // 规则表只认传进来的这一句：别的句子不会改变结果。
    assert_eq!(rule_cues_for_sentence("嗯。", 3), Vec::new());
    assert!(one.iter().all(|c| !c.preset_id.is_empty()));
}
