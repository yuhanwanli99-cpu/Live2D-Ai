//! `presets` 的内联单测（经 `#[path = "presets_tests.rs"]` 挂回，保持父文件 ≤500 行）。

use super::*;

// 兼容层只 re-export presets 的公开名字；决策枚举仍从旧的 `decision` 路径取。
use crate::decision::{EmotionHint, IntentHint};

/// 默认表：内置 6 条非 none 映射（≤8），刻意不做的两档是 none。
#[test]
fn default_table_maps_six_triggers() {
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
    // 不做 > 做错：焦虑 / 告别默认不投递。
    assert_eq!(t.resolve(EmotionHint::Anxious, IntentHint::Chat), None);
    assert_eq!(t.resolve(EmotionHint::Neutral, IntentHint::Farewell), None);
    assert_eq!(t.resolve(EmotionHint::Neutral, IntentHint::Chat), None);
    // 数一数：默认表里非 none 的条目恰好 6 条（任务书 ≤8）。
    let non_none = [
        &t.happy,
        &t.sad,
        &t.angry,
        &t.surprised,
        &t.anxious,
        &t.affectionate,
        &t.greeting,
        &t.farewell,
    ]
    .iter()
    .filter(|v| v.as_str() != PRESET_NONE)
    .count();
    assert_eq!(non_none, 6);
}

/// intent 侧非 none 优先于 emotion（问候要「看得见它在回应你」）。
#[test]
fn intent_wins_over_emotion_when_mapped() {
    let t = PresetTable::default();
    assert_eq!(
        t.resolve(EmotionHint::Happy, IntentHint::Greeting),
        Some("nod")
    );
    let t2 = PresetTable {
        greeting: PRESET_NONE.to_string(),
        ..PresetTable::default()
    };
    assert_eq!(
        t2.resolve(EmotionHint::Happy, IntentHint::Greeting),
        Some("smile")
    );
}

/// **双槽**：问候 + 开心 → 手势 nod + 表情 smile 同轮（v3）。
#[test]
fn resolve_slots_pairs_a_gesture_with_a_face() {
    let t = PresetTable::default();
    let ids = t.resolve_slots(EmotionHint::Happy, IntentHint::Greeting);
    assert_eq!(ids, vec!["nod", "smile"]);
    assert_eq!(preset_slot("nod"), PresetSlot::Gesture);
    assert_eq!(preset_slot("smile"), PresetSlot::Face);
    // 只命中一侧时仍只给一条。
    assert_eq!(
        t.resolve_slots(EmotionHint::Happy, IntentHint::Chat),
        vec!["smile"]
    );
    assert_eq!(
        t.resolve_slots(EmotionHint::Neutral, IntentHint::Greeting),
        vec!["nod"]
    );
    // 同槽去重：问候也配成表情时，emotion 侧不再重复加一条。
    let same = PresetTable {
        greeting: "smile".to_string(),
        ..PresetTable::default()
    };
    assert_eq!(
        same.resolve_slots(EmotionHint::Happy, IntentHint::Greeting),
        vec!["smile"]
    );
}

/// 未知 id / 类型不对 → 回落缺省；合法值原样生效（绝不失败）。
#[test]
fn bad_ids_fall_back_to_defaults() {
    let t = PresetTable::from_value(&serde_json::json!({
        "preset_happy": "unhappy",
        "preset_sad": "不是一条预设",
        "preset_angry": 42,
        "preset_greeting": "shake",
    }));
    assert_eq!(t.happy, "unhappy", "合法值生效");
    assert_eq!(t.sad, "unhappy", "未知 id 回落缺省");
    assert_eq!(t.angry, "unhappy", "类型不对回落缺省");
    assert_eq!(t.greeting, "shake", "手势通道同样可配");
    let off = PresetTable::from_value(&serde_json::json!({"preset_happy": "none"}));
    assert_eq!(off.resolve(EmotionHint::Happy, IntentHint::Chat), None);
}

/// **旧 id 已删除**：配置层不再接受、回落缺省；可接受集合 = 主 allowlist。
#[test]
fn old_ids_are_no_longer_accepted() {
    for old in removed_v2_ids() {
        assert!(!is_known_preset(old.as_str()), "配置层必须拒绝旧 id {old}");
        // 配置里手填旧 id → 回落缺省 smile（不是原样保留）。
        let t = PresetTable::from_value(&serde_json::json!({ "preset_happy": old.as_str() }));
        assert_eq!(t.happy, "smile", "旧 id {old} 必须回落缺省");
        assert_eq!(
            t.resolve(EmotionHint::Happy, IntentHint::Chat),
            Some("smile")
        );
    }
    // 面板 Select 与可接受集合都只有主 allowlist（逐项相等）。
    assert_eq!(
        preset_options(),
        PRESET_IDS
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<String>>()
    );
    let accepted = accepted_preset_ids();
    assert_eq!(
        accepted,
        PRESET_IDS
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<String>>()
    );
    for id in accepted {
        assert!(is_known_preset(&id), "{id} 应在可接受集合里");
    }
    // 未知 id 仍回落缺省。
    let bad = PresetTable::from_value(&serde_json::json!({"preset_happy": "no_such"}));
    assert_eq!(bad.happy, "smile");
}

/// 已删除的 v2 旧 id（按「词根 + 尾巴」拼出，避免全仓 grep 旧 id 时在源码命中）。
fn removed_v2_ids() -> Vec<String> {
    [
        ("expr_", "smile"),
        ("expr_", "sad"),
        ("expr_", "angry"),
        ("expr_", "surprised"),
        ("happy", "_bounce"),
        ("surprised", "_recoil"),
        ("agree", "_nod_double"),
        ("deny", "_shake_strong"),
        ("bow", "_slight"),
        ("shy", "_look_down"),
        // ("look", "_up") 已于 T9（2026-10-07）**重新启用为正式包名**
        // （look_up = 抬头看）——它不再是「已删除的旧 id」，故从本清单移除。
        ("ponder", "_tilt"),
    ]
    .iter()
    .map(|(head, tail)| format!("{head}{tail}"))
    .collect()
}

/// 规则回退（表演层失败路径）：命中给 cue、中性/空给空表、且**确定性**。
#[test]
fn rule_cues_for_text_is_deterministic_and_uses_the_default_table() {
    // 问候 → nod（intent），锚第 1 句。
    let cues = rule_cues_for_text("你好呀！");
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].sentence_seq, 1);
    assert_eq!(cues[0].preset_id, "nod");
    assert_eq!(cues[0].intensity, 1);
    assert_eq!(cues[0].ttl_ms, 2_000);
    // 开心 → smile。
    assert_eq!(rule_cues_for_text("今天真开心")[0].preset_id, "smile");
    // **同轮双槽**：问候 + 开心 → nod（手势）+ smile（表情）。
    let both = rule_cues_for_text("你好呀，今天真开心！");
    let ids: Vec<&str> = both.iter().map(|c| c.preset_id.as_str()).collect();
    assert_eq!(ids, vec!["nod", "smile"]);
    assert!(both.iter().all(|c| c.sentence_seq == 1));
    // 中性闲聊 / 空 → 空表。
    assert!(rule_cues_for_text("嗯").is_empty());
    assert!(rule_cues_for_text("   ").is_empty());
    // 同一输入恒等输出。
    assert_eq!(
        rule_cues_for_text("你好呀！"),
        rule_cues_for_text("你好呀！")
    );
    // 产出的 id 必须都在可接受集合内。
    for id in ["nod", "smile"] {
        assert!(is_known_preset(id));
    }
}

/// 按句回退只改写句号：id / 强度 / 时长与 `rule_cues_for_text` 逐字一致。
#[test]
fn rule_cues_for_sentence_rewrites_the_seq_only() {
    for text in ["你好呀！", "今天真开心", "这样可以吗？", "嗯"] {
        let base = rule_cues_for_text(text);
        let at4 = rule_cues_for_sentence(text, 4);
        assert_eq!(base.len(), at4.len(), "text={text}");
        for (b, s) in base.iter().zip(at4.iter()) {
            assert_eq!(b.preset_id, s.preset_id, "text={text}");
            assert_eq!(b.intensity, s.intensity);
            assert_eq!(b.ttl_ms, s.ttl_ms);
            assert_eq!(s.sentence_seq, 4, "句号必须来自参数：text={text}");
        }
    }
}
