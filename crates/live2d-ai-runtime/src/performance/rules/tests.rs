//! 规则表迁入核心后的回归（rc4，2026-10-17）。
//!
//! 钉子：
//! - `PRESET_IDS` 的字面值**一字不改**（旧 director 契约原样搬入）；
//! - 规则 cue 是纯函数，`rule_cues_for_sentence` 只改句号、实参只能是这一句原文；
//! - 表情槽与表演层的 expression 能力集逐项相等（两份常量不许漂）。

use super::*;
use crate::performance::EXPRESSION_PRESET_IDS;

/// 主 allowlist 的**字面值**冻结：旧 id 是复制，不是重排 / 增删；
/// 0.3.7 T2（D1 手不做）追加 8 表情 + bow，顺序与资产表逐项一致。
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
            "angry",
            "sad",
            "blush",
            "wink",
            "confused",
            "sleepy",
            "excited",
            "calm",
            "nod",
            "shake",
            "look_left",
            "look_right",
            "look_up",
            "look_down",
            "tilt_left",
            "tilt_right",
            "bow",
        ],
        "PRESET_IDS 字面值不得改动（渲染面 / 面板 / 表演层共用同一份契约）"
    );
    assert_eq!(PRESET_NONE, "none");
    assert_eq!(preset_options(), accepted_preset_ids());
    assert_eq!(
        PRESET_IDS.len(),
        22,
        "21 条包 + 撤销哨兵 none（0.3.7 T2 起）"
    );
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

/// **四向单一真源**（0.3.7 T2）：`PRESET_IDS` ↔ `assets/actions/presets.json` ↔
/// `assets/actions/preset_labels.json` ↔ `preset_slot` / `EXPRESSION_PRESET_IDS`，
/// 21 条（不含 none）逐项一致。任一方向漂了都会红。
#[test]
fn four_way_ids_are_consistent_for_all_21_presets() {
    let allow: Vec<&str> = PRESET_IDS
        .iter()
        .copied()
        .filter(|id| *id != PRESET_NONE)
        .collect();
    assert_eq!(
        allow.len(),
        21,
        "0.3.7 T2 起主 allowlist = 21 条（不含 none）"
    );

    // ② assets/actions/presets.json：id 序列逐项一致（顺序也是契约）。
    let presets: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .expect("出厂 presets.json 必须合法");
    let json_ids: Vec<&str> = presets["presets"]
        .as_array()
        .expect("presets.json 缺 presets 数组")
        .iter()
        .map(|p| p["id"].as_str().expect("presets.json 每条都要有 id"))
        .collect();
    assert_eq!(allow, json_ids, "PRESET_IDS ↔ presets.json 漂了");
    assert_eq!(presets["version"], 3, "包表 schema 版本不变（v3）");

    // ③ preset_labels.json：21 + none 全覆盖、无幽灵、channel 与槽位一致。
    let labels: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/preset_labels.json"
    )))
    .expect("出厂 preset_labels.json 必须合法");
    let map = labels["labels"]
        .as_object()
        .expect("preset_labels.json 缺 labels 对象");
    assert_eq!(map.len(), PRESET_IDS.len(), "标签表 = 21 条包 + none");
    for id in PRESET_IDS {
        let entry = map.get(*id).unwrap_or_else(|| panic!("标签表缺少 {id}"));
        let channel = entry["channel"].as_str().unwrap_or("");
        let want = match preset_slot(id) {
            PresetSlot::Face => "expression",
            PresetSlot::Gesture => "motion",
        };
        assert_eq!(channel, want, "{id} 的 channel 与 preset_slot 漂了");
        for key in ["zh", "en"] {
            let text = entry[key].as_str().unwrap_or("");
            assert!(!text.trim().is_empty(), "{id} 缺 {key} 展示名");
        }
    }
    for id in map.keys() {
        assert!(
            PRESET_IDS.contains(&id.as_str()),
            "标签表出现幽灵 id（{id}）"
        );
    }

    // ④ preset_slot / EXPRESSION_PRESET_IDS：12 表情 + 9 手势，表情集合逐项相等。
    let face: Vec<&str> = allow
        .iter()
        .copied()
        .filter(|id| preset_slot(id) == PresetSlot::Face)
        .collect();
    let gesture: Vec<&str> = allow
        .iter()
        .copied()
        .filter(|id| preset_slot(id) == PresetSlot::Gesture)
        .collect();
    assert_eq!(face.len(), 12, "表情 12 条（4 旧 + 8 新）");
    assert_eq!(gesture.len(), 9, "手势 9 条（8 旧 + bow）");
    assert_eq!(
        face,
        EXPRESSION_PRESET_IDS.to_vec(),
        "preset_slot(Face) ↔ EXPRESSION_PRESET_IDS 漂了"
    );
    assert_eq!(face.len() + gesture.len(), allow.len());
}

/// **D1 裁决（维护者 2026-10-18）**：手不做——不扩参数白名单、不加手臂/手/肩通道；
/// wave / clap / point / cover_mouth / shrug 不存在（不做不冒充）。
#[test]
fn d1_hand_is_out_and_the_five_hand_packs_do_not_exist() {
    for id in ["wave", "clap", "point", "cover_mouth", "shrug"] {
        assert!(!is_known_preset(id), "{id} 不得进入 allowlist（D1 手不做）");
        assert!(
            !preset_options().iter().any(|known| known == id),
            "{id} 不得出现在面板选项里"
        );
    }
    // 手势只剩头 / 身能诚实表达的 9 条，且每一条都只动 ParamAngle* / ParamBodyAngle*。
    for id in [
        "nod",
        "shake",
        "look_left",
        "look_right",
        "look_up",
        "look_down",
        "tilt_left",
        "tilt_right",
        "bow",
    ] {
        assert_eq!(preset_slot(id), PresetSlot::Gesture, "{id} 必须归手势槽");
    }
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
