//! **D27 golden 回归**（W4f-1，2026-09-26，从 `tests.rs` 拆出以守 AGENTS
//! 「测试文件 ≤ 800 行」；拆分本身属本波次授权范围，内容零改动）。
//!
//! 控制字符信封（`\u{1}v1\u{1}` + JSON 塞进 `preset_id`）退场之后，
//! `to_json()` 的 wire 输出必须与改动前**逐字相同**——基准是改动前的真实帧
//! 原文（见 W4f-1 报告）。这里也钉住「v1 字段不再依赖 preset_id 承载」。

use super::*;

/// 与 `tests.rs` 同源的可接受集合。
fn allow() -> Vec<String> {
    vec!["nod".to_string(), "smile".to_string()]
}

// ---------------------------------------------------------------- D27 golden 回归

/// 断言一帧与**改动前真实帧 JSON** 深度相等且逐字相等。
///
/// 为什么两个都断言：`Value` 深度相等证明语义不变；序列化字符串相等证明
/// **字节不变**（serde_json 默认 BTreeMap，键序稳定）。
fn assert_golden(golden: &str, actual: serde_json::Value) {
    let expected: serde_json::Value = serde_json::from_str(golden).expect("golden 必须是合法 JSON");
    assert_eq!(actual, expected, "wire 深度不等：{actual}");
    assert_eq!(
        serde_json::to_string(&actual).expect("json"),
        golden,
        "wire 逐字不等"
    );
}

/// 从 plan JSON 里取第 [index] 条 cue 的 to_json()。
fn cue_frame(raw: &str, source: &str, index: usize) -> serde_json::Value {
    let plan = parse_plan(raw, &allow(), source).expect("合法");
    plan.cues[index].to_json()
}

/// **D27 golden**：控制字符信封退场、v1 新键改显式可选字段之后，`to_json()`
/// 的 wire 输出与**改动前真实帧**逐字相同（既有键一字不动，V11）。
///
/// 基准原文（改动前实测，见 W4f-1 报告）：
#[test]
fn golden_action_cue_frames_are_unchanged_after_envelope_removal() {
    // legacy：只出既有 5 键。
    assert_golden(
        r#"{"intensity":2,"preset_id":"nod","priority":40,"sentence_seq":1,"ttl_ms":1500}"#,
        PerformanceCue {
            sentence_seq: 1,
            preset_id: "nod".to_string(),
            intensity: 2,
            ttl_ms: 1500,
            ..Default::default()
        }
        .to_json(),
    );
    // v1：field 无轴。
    assert_golden(
        r#"{"at":"now","field":"head","hold":false,"intensity":1,"preset_id":"head","priority":40,"sentence_seq":1,"seq":1,"ttl_ms":900}"#,
        cue_frame(
            r#"{"segments":["好。"],"cues":[{"field":"head","intensity":1,"at":"now","hold":false}]}"#,
            "好。",
            0,
        ),
    );
    // v1：head + x/y。
    assert_golden(
        r#"{"at":"seg:2","field":"head","hold":false,"intensity":2,"preset_id":"head","priority":40,"sentence_seq":2,"seq":1,"ttl_ms":1200,"x":0.25,"y":-0.5}"#,
        cue_frame(
            r#"{"segments":["一","二"],"cues":[{"field":"head","x":0.25,"y":-0.5,"intensity":2,"at":"seg:2","hold":false,"ttl_ms":1200}]}"#,
            "一二",
            0,
        ),
    );
    // v1：head + x/y/z + hold。
    assert_golden(
        r#"{"at":"seg:2","field":"head","hold":true,"intensity":3,"preset_id":"head","priority":40,"sentence_seq":2,"seq":1,"ttl_ms":900,"x":0.25,"y":-0.5,"z":-0.75}"#,
        cue_frame(
            r#"{"segments":["一","二"],"cues":[{"field":"head","x":0.25,"y":-0.5,"z":-0.75,"intensity":3,"at":"seg:2","hold":true}]}"#,
            "一二",
            0,
        ),
    );
    // v1：expression + id。
    assert_golden(
        r#"{"at":"now","field":"expression","hold":true,"id":"smile","intensity":1,"preset_id":"smile","priority":40,"sentence_seq":1,"seq":1,"ttl_ms":2600}"#,
        cue_frame(
            r#"{"segments":["一"],"cues":[{"field":"expression","id":"smile","intensity":1,"at":"now","hold":true}]}"#,
            "一",
            0,
        ),
    );
    // v1：expression 撤销哨兵 none。
    assert_golden(
        r#"{"at":"now","field":"expression","hold":true,"id":"none","intensity":1,"preset_id":"none","priority":40,"sentence_seq":1,"seq":1,"ttl_ms":2600}"#,
        cue_frame(
            r#"{"segments":["一"],"cues":[{"field":"expression","id":"none","intensity":1,"at":"now","hold":true}]}"#,
            "一",
            0,
        ),
    );
    // v1：after_prev 链（两条都钉：at 保留 / 锚段不变）。
    let chained = r#"{"segments":["一","二","三"],"cues":[{"field":"head","x":0.1,"intensity":1,"at":"seg:2","hold":false},{"field":"expression","id":"smile","intensity":1,"at":"after_prev","hold":false}]}"#;
    assert_golden(
        r#"{"at":"seg:2","field":"head","hold":false,"intensity":1,"preset_id":"head","priority":40,"sentence_seq":2,"seq":1,"ttl_ms":900,"x":0.1}"#,
        cue_frame(chained, "一二三", 0),
    );
    assert_golden(
        r#"{"at":"after_prev","field":"expression","hold":false,"id":"smile","intensity":1,"preset_id":"smile","priority":40,"sentence_seq":2,"seq":2,"ttl_ms":2600}"#,
        cue_frame(chained, "一二三", 1),
    );
    // v1：after_prev 遇 hold 退化为 now。
    let degraded = r#"{"segments":["一","二","三"],"cues":[{"field":"head","x":0.1,"intensity":1,"at":"now","hold":true},{"field":"expression","id":"smile","intensity":1,"at":"after_prev","hold":false}]}"#;
    assert_golden(
        r#"{"at":"now","field":"head","hold":true,"intensity":1,"preset_id":"head","priority":40,"sentence_seq":1,"seq":1,"ttl_ms":900,"x":0.1}"#,
        cue_frame(degraded, "一二三", 0),
    );
    assert_golden(
        r#"{"at":"now","field":"expression","hold":false,"id":"smile","intensity":1,"preset_id":"smile","priority":40,"sentence_seq":1,"seq":2,"ttl_ms":2600}"#,
        cue_frame(degraded, "一二三", 1),
    );
    // 整帧 payload（action_cue.data 形态）。
    let plan = parse_plan(
        r#"{"segments":["一","二"],"cues":[{"field":"head","x":0.25,"intensity":3,"at":"seg:2","hold":true},{"field":"expression","id":"smile","intensity":1,"at":"now","hold":false}]}"#,
        &allow(),
        "一二",
    )
    .expect("合法");
    assert_golden(
        r#"{"covers_upto_seq":2,"cues":[{"at":"seg:2","field":"head","hold":true,"intensity":3,"preset_id":"head","priority":40,"sentence_seq":2,"seq":1,"ttl_ms":900,"x":0.25},{"at":"now","field":"expression","hold":false,"id":"smile","intensity":1,"preset_id":"smile","priority":40,"sentence_seq":1,"seq":2,"ttl_ms":2600}],"epoch":9}"#,
        action_cue_payload(9, 2, &plan.cues),
    );
}

/// **D27 硬要求**：v1 字段**不再依赖 preset_id 承载**——任何 cue 的 preset_id
/// 都只是语义 id（不含控制字符 `\u{1}`、不含 JSON）。
#[test]
fn v1_cue_preset_id_never_carries_control_chars_or_json() {
    let plan = parse_plan(
        r#"{"segments":["一","二"],"cues":[
            {"field":"head","x":0.25,"intensity":2,"at":"seg:2","hold":false},
            {"field":"head","z":0.5,"intensity":1,"at":"now","hold":true},
            {"field":"expression","id":"smile","intensity":1,"at":"now","hold":false}
        ]}"#,
        &allow(),
        "一二",
    )
    .expect("合法");
    assert_eq!(plan.cues.len(), 3);
    for (cue, want) in plan.cues.iter().zip(["head", "head", "smile"]) {
        assert_eq!(cue.preset_id, want, "preset_id 只承载语义 id");
        assert!(
            !cue.preset_id.contains('\u{1}'),
            "preset_id 不得含控制字符信封：{}",
            cue.preset_id
        );
        assert!(
            !cue.preset_id.contains('{'),
            "preset_id 不得承载 JSON：{}",
            cue.preset_id
        );
        let wire = cue.to_json();
        let wire_id = wire["preset_id"].as_str().expect("preset_id 是字符串");
        assert_eq!(wire_id, want);
        assert!(!wire_id.contains('\u{1}'));
    }
    // 旧 preset_id 路径仍走 legacy：preset_id 原样、v1 键不出现。
    let legacy = parse_plan(
        r#"{"speak":"你好。","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":1,"ttl_ms":900}]}"#,
        &allow(),
        "",
    )
    .expect("合法");
    assert_eq!(legacy.cues[0].preset_id, "nod");
    assert!(legacy.cues[0].field.is_none(), "legacy 没有 v1 field");
    let wire = legacy.cues[0].to_json();
    for key in ["field", "seq", "x", "y", "z", "id", "at", "hold"] {
        assert!(wire.get(key).is_none(), "legacy 帧不得出现 v1 键 {key}");
    }
}

/// v1 新键是**结构体字段**：改 preset_id 不影响它们（不再随字符串编解码）。
#[test]
fn v1_keys_are_structural_fields_not_derived_from_preset_id() {
    let cue = PerformanceCue {
        sentence_seq: 1,
        preset_id: "smile".to_string(),
        intensity: 2,
        ttl_ms: 900,
        field: Some(CueField::Expression),
        x: serde_json::Number::from_f64(0.5),
        y: None,
        z: None,
        id: Some("smile".to_string()),
        at: Some("seg:3".to_string()),
        hold: Some(true),
        seq: Some(2),
    };
    let wire = cue.to_json();
    assert_eq!(wire["field"], "expression");
    assert_eq!(wire["x"], 0.5);
    assert_eq!(wire["id"], "smile");
    assert_eq!(wire["at"], "seg:3");
    assert_eq!(wire["hold"], true);
    assert_eq!(wire["seq"], 2);
    assert!(wire.get("y").is_none(), "缺省轴不出键");
    assert!(wire.get("z").is_none(), "缺省轴不出键");
}
