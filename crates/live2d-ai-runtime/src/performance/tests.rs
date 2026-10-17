//! 表演层回归：**表驱动**合法/非法样例、v1 segments 不变量、三态、schema 与
//! 校验器同界、客户端 wire 形态、回退路径与原因码。

use super::plan::PlanWarning;
use super::*;

// ---------------------------------------------------------------- 表驱动校验

pub(super) fn allow() -> Vec<String> {
    vec!["nod".to_string(), "smile".to_string()]
}

/// v0 legacy 样例：形状、钳位、未知字段丢弃。
#[test]
fn legal_samples_parse_with_clamping_and_unknown_fields_dropped() {
    // 只说：cues 为空。
    let plan = parse_plan(r#"{"speak":"你好呀。","cues":[]}"#, &allow(), "").expect("合法");
    assert_eq!(plan.speak.as_deref(), Some("你好呀。"));
    assert!(plan.cues.is_empty());
    assert!(!plan.is_noop());

    // noop：null + 空数组。
    let plan = parse_plan(r#"{"speak":null,"cues":[]}"#, &allow(), "").expect("合法");
    assert!(plan.is_noop());

    // noop 的第二种写法：空串。
    let plan = parse_plan(r#"{"speak":"   ","cues":[]}"#, &allow(), "").expect("合法");
    assert!(plan.is_noop());

    // 只动：speak 空 + 一条 cue。
    let plan = parse_plan(
        r#"{"speak":"","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":2,"ttl_ms":1800}]}"#,
        &allow(),
        "",
    )
    .expect("合法");
    assert!(plan.speak.is_none());
    assert_eq!(plan.cues.len(), 1);
    assert_eq!(plan.cues[0].intensity, 2);
    assert_eq!(plan.cues[0].ttl_ms, 1800);

    // 越界 → 钳位（不是失败）。
    let plan = parse_plan(
        r#"{"speak":"a","cues":[{"sentence_seq":3,"preset_id":"nod","intensity":9,"ttl_ms":999999}]}"#,
        &allow(),
        "",
    )
    .expect("合法");
    assert_eq!(plan.cues[0].intensity, MAX_INTENSITY);
    assert_eq!(plan.cues[0].ttl_ms, MAX_TTL_MS);
    let plan = parse_plan(
        r#"{"speak":"a","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":0,"ttl_ms":0}]}"#,
        &allow(),
        "",
    )
    .expect("合法");
    assert_eq!(plan.cues[0].intensity, MIN_INTENSITY);
    assert_eq!(plan.cues[0].ttl_ms, MIN_TTL_MS);

    // 未知字段一律丢弃（不失败）。
    let plan = parse_plan(
        r#"{"speak":"a","cues":[],"reason":"我想多了","mood":"happy"}"#,
        &allow(),
        "",
    )
    .expect("未知字段应被丢弃而不是失败");
    assert_eq!(plan.speak.as_deref(), Some("a"));
}

/// 非法样例：坏 JSON / 顶层不是对象 / 缺字段 / 类型不对 / 未知 preset / 超量。
#[test]
fn illegal_samples_fail_as_a_whole() {
    let cases: Vec<(&str, &str)> = vec![
        ("not json at all", "performance_plan_not_json"),
        ("[1,2,3]", "performance_plan_not_object"),
        (r#"{"cues":[]}"#, "performance_plan_missing_segments"),
        (r#"{"segments":[],"cues":[]}"#, ""),
        (r#"{"speak":42,"cues":[]}"#, "performance_plan_speak_type"),
        (r#"{"speak":"a"}"#, "performance_plan_missing_cues"),
        (r#"{"segments":["嗯"]}"#, "performance_plan_missing_cues"),
        (r#"{"speak":"a","cues":{}}"#, "performance_plan_cues_type"),
        (
            r#"{"segments":["嗯"],"cues":{}}"#,
            "performance_plan_cues_type",
        ),
        (
            r#"{"speak":"a","cues":[1,2]}"#,
            "performance_plan_cue_not_object",
        ),
        (
            r#"{"segments":["嗯"],"cues":[1,2]}"#,
            "performance_plan_cue_not_object",
        ),
        (
            r#"{"speak":"a","cues":[{"preset_id":"nod","intensity":1,"ttl_ms":10}]}"#,
            "performance_plan_cue_field",
        ),
        (
            r#"{"speak":"a","cues":[{"sentence_seq":0,"preset_id":"nod","intensity":1,"ttl_ms":10}]}"#,
            "performance_plan_cue_field",
        ),
        (
            r#"{"speak":"a","cues":[{"sentence_seq":1,"preset_id":"no_such","intensity":1,"ttl_ms":10}]}"#,
            "performance_plan_unknown_preset",
        ),
        (
            r#"{"speak":"a","cues":[{"sentence_seq":1,"preset_id":"nod","ttl_ms":10}]}"#,
            "performance_plan_cue_field",
        ),
        (
            r#"{"speak":"a","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":"强","ttl_ms":10}]}"#,
            "performance_plan_cue_field",
        ),
        // v1 形状错误。
        (
            r#"{"segments":"嗯","cues":[]}"#,
            "performance_plan_segments_type",
        ),
        (
            r#"{"segments":["嗯",7],"cues":[]}"#,
            "performance_plan_segment_not_string",
        ),
        (
            r#"{"segments":["嗯"],"cues":[{"field":"tail","intensity":1,"at":"now","hold":true}]}"#,
            "performance_plan_unknown_field",
        ),
        (
            r#"{"segments":["嗯"],"cues":[{"field":"head","intensity":1,"at":"now"}]}"#,
            "performance_plan_cue_field",
        ),
        (
            r#"{"segments":["嗯"],"cues":[{"field":"expression","intensity":1,"at":"now","hold":true}]}"#,
            "performance_plan_cue_field",
        ),
        (
            r#"{"segments":["嗯"],"cues":[{"field":"head","intensity":1,"at":"now","hold":true,"x":"大"}]}"#,
            "performance_plan_cue_field",
        ),
    ];
    for (raw, code) in cases {
        if code.is_empty() {
            // 空切分 + 空 cues 是合法 noop（空原文）。
            assert!(parse_plan(raw, &allow(), "").is_ok(), "输入 {raw}");
            continue;
        }
        let err = parse_plan(raw, &allow(), "嗯").expect_err("必须整份失败");
        assert_eq!(err.code(), code, "输入 {raw}");
        assert!(!err.message().is_empty());
    }

    // 超量：MAX_CUES + 1 条 → 整份失败（legacy + v1 各一次）。
    let many = (0..=MAX_CUES)
        .map(|i| {
            format!(
                r#"{{"sentence_seq":{},"preset_id":"nod","intensity":1,"ttl_ms":10}}"#,
                i + 1
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let raw = format!(r#"{{"speak":"a","cues":[{many}]}}"#);
    assert_eq!(
        parse_plan(&raw, &allow(), "").expect_err("超量").code(),
        "performance_plan_too_many_cues"
    );
    let many_v1 = (0..=MAX_CUES)
        .map(|_| r#"{"field":"head","intensity":1,"at":"now","hold":true}"#)
        .collect::<Vec<_>>()
        .join(",");
    let raw_v1 = format!(r#"{{"segments":["a"],"cues":[{many_v1}]}}"#);
    assert_eq!(
        parse_plan(&raw_v1, &allow(), "a").expect_err("超量").code(),
        "performance_plan_too_many_cues"
    );
}

// ---------------------------------------------------------------- v1 不变量

/// **V1 核心**：segments 只能是原文的逐字切分；改写 / 缩写 / 漏字 / 换序 / 拼接不等
/// 全部整份失败（performance_plan_segments_not_partition）。
#[test]
fn segments_must_be_a_verbatim_partition_of_the_source() {
    let source = "嗯……我想到了。";
    // 正例：任意切点都合法。
    let plan = parse_plan(
        r#"{"segments":["嗯……","我想到了。"],"cues":[]}"#,
        &allow(),
        source,
    )
    .expect("合法切分");
    assert_eq!(
        plan.segments.as_deref(),
        Some(&["嗯……".to_string(), "我想到了。".to_string()][..])
    );
    assert_eq!(plan.segments.clone().unwrap().concat(), source);

    // 反例：每一条都必须整份失败。
    let bad = [
        r#"{"segments":["嗯……","我想到了"],"cues":[]}"#, // 漏字
        r#"{"segments":["嗯……","想到了。"],"cues":[]}"#, // 漏字
        r#"{"segments":["嗯……","我想到了。。"],"cues":[]}"#, // 增字
        r#"{"segments":["我想到了。","嗯……"],"cues":[]}"#, // 换序
        r#"{"segments":["嗯……","我想到了？"],"cues":[]}"#, // 改写（。→？）
        r#"{"segments":["嗯……","想到了。"],"cues":[]}"#, // 缩写
    ];
    for raw in bad {
        let err = parse_plan(raw, &allow(), source).expect_err("必须整份失败");
        assert_eq!(
            err.code(),
            "performance_plan_segments_not_partition",
            "输入 {raw}"
        );
    }
    // 空原文：segments 必须是 []（非空切分即使拼接为空也不接受）。
    assert!(parse_plan(r#"{"segments":[],"cues":[]}"#, &allow(), "").is_ok());
    assert_eq!(
        parse_plan(r#"{"segments":[""],"cues":[]}"#, &allow(), "")
            .expect_err("空原文不接受非空切分")
            .code(),
        "performance_plan_segments_not_partition"
    );
}

/// 切分前后原文逐字相同：只改切点、不改字符。
#[test]
fn segments_are_split_but_never_rewritten() {
    let source = "下午好，今天天气不错。";
    let plan = parse_plan(
        r#"{"segments":["下午好，","今天天气","不错。"],"cues":[]}"#,
        &allow(),
        source,
    )
    .expect("合法切分");
    let segments = plan.segments.clone().unwrap();
    assert_eq!(segments.concat(), source, "拼接必须逐字等于原文");
    assert_eq!(segments.join(""), source);
    assert_eq!(segments.len(), 3, "只决定切点，不合并/拆分字符");
}

/// V11：只给 speak 时仍走 v0 旧语义（缺省即旧语义）。
#[test]
fn legacy_speak_still_parses_when_segments_absent() {
    let plan = parse_plan(
        r#"{"speak":"你好。","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":1,"ttl_ms":900}]}"#,
        &allow(),
        "",
    )
    .expect("v0 旧路径必须可解析");
    assert_eq!(plan.speak.as_deref(), Some("你好。"));
    assert!(plan.segments.is_none(), "旧路径没有 segments");
    assert_eq!(plan.cues.len(), 1);
    assert_eq!(plan.cues[0].preset_id, "nod");
}

/// O1：segments 与 speak 同现 → segments 优先、speak 忽略 + warn（**不整份失败**）。
#[test]
fn both_segments_and_speak_prefers_segments_and_warns() {
    let plan = parse_plan(
        r#"{"segments":["好。"],"speak":"被改写的话","cues":[]}"#,
        &allow(),
        "好。",
    )
    .expect("O1 不得整份失败");
    assert!(plan.speak.is_none(), "speak 必须被忽略");
    assert_eq!(plan.segments.as_deref(), Some(&["好。".to_string()][..]));
    assert!(
        plan.warnings.contains(&PlanWarning::SpeakIgnored),
        "必须 warn：{:?}",
        plan.warnings
    );
}

/// at 词表外 / 越界锚点（seg:0 或 seg:N > 段数）→ 整份失败。
#[test]
fn anchor_beyond_segment_count_fails_the_whole_plan() {
    let source = "一二";
    let ok = parse_plan(
        r#"{"segments":["一","二"],"cues":[{"field":"head","intensity":1,"at":"seg:2","hold":false}]}"#,
        &allow(),
        source,
    )
    .expect("seg:2 在段数内");
    assert_eq!(ok.cues[0].to_json()["sentence_seq"], 2);

    for at in ["seg:0", "seg:3", "seg:x", "in:800", "later", ""] {
        let raw = format!(
            r#"{{"segments":["一","二"],"cues":[{{"field":"head","intensity":1,"at":"{at}","hold":false}}]}}"#
        );
        let err = parse_plan(&raw, &allow(), source).expect_err("必须整份失败");
        assert_eq!(err.code(), "performance_plan_bad_anchor", "at={at}");
    }
}

/// O2：段数上限 / 总字符上限 → 整份失败（不截断）。
#[test]
fn segments_count_and_chars_limits_fail_as_a_whole() {
    // 65 段（每段 1 字）→ 超段数。
    let source: String = "啊".repeat(MAX_SEGMENTS + 1);
    let segments: Vec<String> = (0..=MAX_SEGMENTS).map(|_| "\"啊\"".to_string()).collect();
    let raw = format!(r#"{{"segments":[{}],"cues":[]}}"#, segments.join(","));
    assert_eq!(
        parse_plan(&raw, &allow(), &source)
            .expect_err("超段数")
            .code(),
        "performance_plan_segments_too_long"
    );
    // 单段总字符 > 上限 → 超长。
    let long_source = "啊".repeat(MAX_SEGMENT_CHARS + 1);
    let long_raw = format!(r#"{{"segments":["{long_source}"],"cues":[]}}"#);
    assert_eq!(
        parse_plan(&long_raw, &allow(), &long_source)
            .expect_err("超长")
            .code(),
        "performance_plan_segments_too_long"
    );
}

/// T8：`field=body` 丢该条 + warn（**不整份失败**）；顺带钉住轴越界钳位与强度钳位。
#[test]
fn body_cue_is_dropped_with_warning_and_axis_rules_still_hold() {
    let plan = parse_plan(
        r#"{"segments":["好。"],"cues":[
            {"field":"body","x":1.7,"y":-9.0,"z":0.5,"intensity":9,"at":"now","hold":false},
            {"field":"head","x":1.7,"y":-9.0,"z":0.5,"intensity":9,"at":"now","hold":true}
        ]}"#,
        &allow(),
        "好。",
    )
    .expect("body 丢条不是整份失败；越界只钳位");
    assert_eq!(plan.cues.len(), 1, "body 必须丢掉该条：{:?}", plan.warnings);
    assert!(
        plan.warnings
            .iter()
            .any(|w| matches!(w, PlanWarning::BodyNotAllowed { index: 0 })),
        "body 必须 warn：{:?}",
        plan.warnings
    );
    let head = plan.cues[0].to_json();
    assert_eq!(head["field"], "head");
    assert_eq!(head["x"], AXIS_MAX, "越界轴值钳位到上界");
    assert_eq!(head["y"], AXIS_MIN, "越界轴值钳位到下界");
    assert_eq!(head["z"], 0.5, "head 的 z 保留（颈随头一起转）");
    assert_eq!(head["intensity"], MAX_INTENSITY, "intensity 钳位");
    assert_eq!(head["preset_id"], "head", "非 expression 用字段名占位");
    assert_eq!(head["seq"], 2, "丢条不重排 cue 序号（按 plan 内位置）");

    // 轴键裁剪仍要覆盖：expression 不接受 x/y/z（丢键 + warn，不失败）。
    let plan = parse_plan(
        r#"{"segments":["好。"],"cues":[{"field":"expression","id":"smile","x":0.5,"y":0.5,"z":0.5,"intensity":1,"at":"now","hold":true}]}"#,
        &allow(),
        "好。",
    )
    .expect("丢键不失败");
    assert_eq!(plan.cues.len(), 1);
    assert_eq!(
        plan.warnings
            .iter()
            .filter(|w| matches!(w, PlanWarning::AxisNotAllowed { .. }))
            .count(),
        3,
        "x/y/z 三个键都要 warn：{:?}",
        plan.warnings
    );
    let face = plan.cues[0].to_json();
    for key in ["x", "y", "z"] {
        assert!(face.get(key).is_none(), "expression 的 {key} 必须被丢弃");
    }
}

/// §2.4 #16：未知表情 id 只丢该条 cue + warn（其余照演）。
#[test]
fn expression_unknown_id_is_dropped_with_warning() {
    let plan = parse_plan(
        r#"{"segments":["好。"],"cues":[
            {"field":"expression","id":"nope","intensity":1,"at":"now","hold":true},
            {"field":"expression","id":"none","intensity":1,"at":"now","hold":true}
        ]}"#,
        &allow(),
        "好。",
    )
    .expect("未知 id 不整份失败");
    assert_eq!(plan.cues.len(), 1, "只丢该条");
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.code() == "performance_expression_unknown_id")
    );
    let kept = plan.cues[0].to_json();
    assert_eq!(kept["preset_id"], "none");
    assert_eq!(kept["seq"], 2, "序号按 plan 内位置，丢条不重排");
}

/// O4：after_prev 遇 hold=true（没有「做完」点）→ 退化为 now，**不得永不生效**。
#[test]
fn after_prev_degrades_to_now_when_prev_holds() {
    let source = "一二三";
    // 上一条 hold=true → 退化。
    let plan = parse_plan(
        r#"{"segments":["一","二","三"],"cues":[
            {"field":"head","x":0.1,"intensity":1,"at":"now","hold":true},
            {"field":"expression","id":"smile","intensity":1,"at":"after_prev","hold":false}
        ]}"#,
        &allow(),
        source,
    )
    .expect("合法");
    let degraded = plan.cues[1].to_json();
    assert_eq!(degraded["at"], "now", "hold 的 after_prev 必须退化为 now");
    assert_eq!(degraded["sentence_seq"], 1, "退化后停在上一条的锚段");
    // 上一条 hold=false → 保留 after_prev。
    let plan = parse_plan(
        r#"{"segments":["一","二","三"],"cues":[
            {"field":"head","x":0.1,"intensity":1,"at":"seg:2","hold":false},
            {"field":"expression","id":"smile","intensity":1,"at":"after_prev","hold":false}
        ]}"#,
        &allow(),
        source,
    )
    .expect("合法");
    let chained = plan.cues[1].to_json();
    assert_eq!(chained["at"], "after_prev");
    assert_eq!(chained["sentence_seq"], 2, "after_prev 锚在上一条的段");
}

/// v1 cue → wire：既有键逐字保留 + v1 新键以可选键摊平（B9 / V11）。
#[test]
fn v1_cue_projects_with_existing_and_new_keys() {
    let plan = parse_plan(
        r#"{"segments":["一","二"],"cues":[
            {"field":"head","x":0.25,"y":-0.5,"z":-0.75,"intensity":3,"at":"seg:2","hold":false,"ttl_ms":1200}
        ]}"#,
        &allow(),
        "一二",
    )
    .expect("合法");
    let json = plan.cues[0].to_json();
    for key in [
        "sentence_seq",
        "preset_id",
        "intensity",
        "ttl_ms",
        "priority",
    ] {
        assert!(json.get(key).is_some(), "既有键 {key} 必须保留：{json}");
    }
    assert_eq!(json["sentence_seq"], 2);
    assert_eq!(json["intensity"], 3);
    assert_eq!(json["ttl_ms"], 1200);
    assert_eq!(json["priority"], PRIORITY_PERFORMANCE);
    assert_eq!(json["field"], "head");
    assert_eq!(json["seq"], 1);
    assert_eq!(json["x"], 0.25);
    assert_eq!(json["y"], -0.5);
    assert_eq!(json["z"], -0.75);
    assert_eq!(json["at"], "seg:2");
    assert_eq!(json["hold"], false);
    // 默认 ttl（O3）。
    let plan = parse_plan(
        r#"{"segments":["一"],"cues":[{"field":"expression","id":"smile","intensity":1,"at":"now","hold":false}]}"#,
        &allow(),
        "一",
    )
    .expect("合法");
    assert_eq!(plan.cues[0].to_json()["ttl_ms"], DEFAULT_TTL_MS_EXPRESSION);
}
