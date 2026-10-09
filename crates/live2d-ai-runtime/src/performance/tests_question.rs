//! **T9 问句补丁**的回归（从 tests.rs 拆出：AGENTS 测试文件 ≤800 行）。
//!
//! 硬要求四条：有问号且 cues 为空 → 上表两值、分段一个字不改；该段模型交了别的
//! 表情 / 别的 z → 仍换成词典值；没有问号 → 不补（含「吗 / 呢」不算问句）；
//! legacy speak 路径不补。另有「手势 id 填进 expression → 丢条 + warn」与
//! 「schema 的 expression enum 只留表情槽」两条。

use super::*;

// ---------------------------------------------------------------- T9 问句补丁

/// 问句补丁场景的可接受集合（含 T9 新表情 thinking；手势 id 也放进来，
/// 好让「手势 id 填进 expression」那条能走到校验器）。
fn patch_allow() -> Vec<String> {
    vec![
        "nod".to_string(),
        "smile".to_string(),
        "tilt_left".to_string(),
        "thinking".to_string(),
    ]
}

/// **T9 第一条**：有问号且 cues 为空 → 第一处问句所在段得到上表两值；分段不变。
///
/// 删掉 parse_plan 里的 apply_question_patch 调用，这一条必红（cues 会空）。
#[test]
fn question_patch_adds_tilt_and_thinking_on_the_first_question_segment() {
    let source = "嗯……你刚才说什么？我听着呢。";
    let plan = parse_plan(
        r#"{"segments":["嗯……","你刚才说什么？","我听着呢。"],"cues":[]}"#,
        &patch_allow(),
        source,
    )
    .expect("合法切分");
    assert_eq!(plan.cues.len(), 2, "第一处问句补两样：{:?}", plan.warnings);
    // 歪头：head x=0 / y=0.12 / z=0.45 / intensity=2 / hold=false / 1800ms。
    let head = plan.cues[0].to_json();
    assert_eq!(head["field"], "head");
    assert_eq!(head["preset_id"], "head", "非 expression 用字段名占位");
    assert_eq!(head["x"], 0.0);
    assert_eq!(head["y"], QUESTION_HEAD_Y);
    assert_eq!(head["z"], QUESTION_HEAD_Z);
    assert_eq!(head["intensity"], QUESTION_INTENSITY);
    assert_eq!(head["hold"], false);
    assert_eq!(head["ttl_ms"], QUESTION_HEAD_TTL_MS);
    assert_eq!(head["at"], "seg:2");
    assert_eq!(head["sentence_seq"], 2, "锚在第 2 段音频开始");
    // 思考：expression id=thinking / intensity=2 / hold=false / 2600ms。
    let face = plan.cues[1].to_json();
    assert_eq!(face["field"], "expression");
    assert_eq!(face["id"], QUESTION_EXPRESSION_ID);
    assert_eq!(face["preset_id"], QUESTION_EXPRESSION_ID);
    assert_eq!(face["intensity"], QUESTION_INTENSITY);
    assert_eq!(face["hold"], false);
    assert_eq!(face["ttl_ms"], QUESTION_EXPRESSION_TTL_MS);
    assert_eq!(face["at"], "seg:2");
    assert_eq!(face["sentence_seq"], 2);
    // 导演 cue 不得写参数名（口型归 TTS）：wire 上连 Param 都不出现。
    for cue in &plan.cues {
        let wire = cue.to_json().to_string();
        assert!(!wire.contains("Param"), "导演 cue 不得承载参数名：{wire}");
    }
    // 分段一个字不改（V1 拼接恒等仍成立）。
    let segments = plan.segments.clone().unwrap();
    assert_eq!(segments.concat(), source);
    assert_eq!(
        segments,
        vec!["嗯……", "你刚才说什么？", "我听着呢。"]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<String>>()
    );
}

/// **T9 第二条**：该段模型已交了别的表情 / 别的 z → 仍换成上表两值；
/// 别的段上的合法 cue 原样保留。
#[test]
fn question_patch_overrides_the_models_cues_on_that_segment_only() {
    let source = "一？二";
    let plan = parse_plan(
        r#"{"segments":["一？","二"],"cues":[
            {"field":"head","z":-0.9,"intensity":3,"at":"seg:1","hold":true},
            {"field":"expression","id":"smile","intensity":3,"at":"seg:1","hold":true},
            {"field":"head","y":0.5,"intensity":1,"at":"seg:2","hold":false}
        ]}"#,
        &patch_allow(),
        source,
    )
    .expect("合法");
    assert_eq!(
        plan.cues.len(),
        3,
        "丢掉段 1 的两条、补上两条、留下段 2 的一条"
    );
    // 别的段原样。
    let other = plan.cues[0].to_json();
    assert_eq!(other["field"], "head");
    assert_eq!(other["sentence_seq"], 2);
    assert_eq!(other["y"], 0.5);
    assert!(other.get("z").is_none(), "段 2 的 head 没有 z");
    assert_eq!(other["intensity"], 1);
    // 问句那一段：歪头换成词典值（不是 -0.9 / 3 / hold）。
    let head = plan.cues[1].to_json();
    assert_eq!(head["sentence_seq"], 1);
    assert_eq!(head["z"], QUESTION_HEAD_Z);
    assert_eq!(head["y"], QUESTION_HEAD_Y);
    assert_eq!(head["intensity"], QUESTION_INTENSITY);
    assert_eq!(head["hold"], false);
    // 表情换成 thinking（不是 smile）。
    let face = plan.cues[2].to_json();
    assert_eq!(face["sentence_seq"], 1);
    assert_eq!(face["id"], QUESTION_EXPRESSION_ID);
}

/// **T9 第三条**：没有问号 → 不补；「吗 / 呢 / 什么」不算问句；半角 ? 算。
#[test]
fn question_patch_is_absent_without_a_question_mark() {
    for (raw, source) in [
        (r#"{"segments":["一。","二！"],"cues":[]}"#, "一。二！"),
        (r#"{"segments":["这样吗。"],"cues":[]}"#, "这样吗。"),
        (r#"{"segments":["我想想呢。"],"cues":[]}"#, "我想想呢。"),
    ] {
        let plan = parse_plan(raw, &patch_allow(), source).expect("合法");
        assert!(plan.cues.is_empty(), "{source} 不该补：{:?}", plan.cues);
    }
    // 半角 ? 与全角 ？ 等价；只补**第一处**问句。
    let plan = parse_plan(
        r#"{"segments":["一?","二？"],"cues":[]}"#,
        &patch_allow(),
        "一?二？",
    )
    .expect("合法");
    assert_eq!(plan.cues.len(), 2, "只补第一处");
    assert!(plan.cues.iter().all(|cue| cue.sentence_seq == 1));
}

/// legacy speak 路径不补问句（那里没有「第几段」这回事）。
#[test]
fn question_patch_never_touches_the_legacy_speak_path() {
    let plan =
        parse_plan(r#"{"speak":"在吗？","cues":[]}"#, &patch_allow(), "").expect("legacy 合法");
    assert!(plan.cues.is_empty(), "legacy 路径不补问句");
}

/// **T9**：手势 id 填进 expression → 丢该条 + warn（整份仍成功）；
/// thinking 是表情，正常留下。
#[test]
fn gesture_ids_on_expression_are_dropped_with_warning() {
    let plan = parse_plan(
        r#"{"segments":["好。"],"cues":[
            {"field":"expression","id":"tilt_left","intensity":1,"at":"now","hold":true},
            {"field":"expression","id":"thinking","intensity":1,"at":"now","hold":true}
        ]}"#,
        &patch_allow(),
        "好。",
    )
    .expect("丢条不是整份失败");
    assert_eq!(plan.cues.len(), 1, "手势 id 必须丢掉那一条");
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.code() == "performance_expression_unknown_id"),
        "丢条要留 warn：{:?}",
        plan.warnings
    );
    assert_eq!(plan.cues[0].to_json()["id"], "thinking");
}

/// 表情能力集是**单一真源**：schema enum 与校验器认的是同一组（能力集 ∩ 表情槽）。
#[test]
fn schema_expression_enum_is_the_face_slot_intersection() {
    let allow_all: Vec<String> = vec![
        "none".to_string(),
        "smile".to_string(),
        "unhappy".to_string(),
        "surprised".to_string(),
        "nod".to_string(),
        "look_up".to_string(),
        "tilt_left".to_string(),
        "thinking".to_string(),
    ];
    let schema = json_schema_strict(&allow_all);
    let ids: Vec<String> = schema["properties"]["cues"]["items"]["properties"]["id"]["enum"]
        .as_array()
        .expect("id enum")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        ids,
        vec!["smile", "unhappy", "surprised", "thinking", "none"],
        "只留表情槽 id + 撤销哨兵 none"
    );
    for gesture in ["nod", "look_up", "tilt_left"] {
        assert!(!ids.iter().any(|id| id == gesture), "{gesture} 不是表情");
    }
}
