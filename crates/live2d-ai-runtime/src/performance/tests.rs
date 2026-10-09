//! 表演层回归：**表驱动**合法/非法样例、v1 segments 不变量、三态、schema 与
//! 校验器同界、客户端 wire 形态、回退路径与原因码。

use std::sync::Mutex;

use super::plan::PlanWarning;
use super::*;

// ---------------------------------------------------------------- 表驱动校验

fn allow() -> Vec<String> {
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

/// schema 与校验器**同一组边界**（发出去的 schema == 校验器认的东西）。
#[test]
fn schema_and_validator_share_the_same_bounds() {
    let schema = json_schema_strict(&allow());
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"], serde_json::json!(["segments", "cues"]));
    assert_eq!(schema["properties"]["segments"]["maxItems"], MAX_SEGMENTS);
    assert_eq!(schema["properties"]["segments"]["items"]["type"], "string");
    assert_eq!(schema["properties"]["cues"]["maxItems"], MAX_CUES);
    let cue = &schema["properties"]["cues"]["items"];
    assert_eq!(cue["additionalProperties"], false);
    assert_eq!(
        cue["required"],
        serde_json::json!(["field", "intensity", "at", "hold"])
    );
    assert_eq!(
        cue["properties"]["field"]["enum"],
        serde_json::json!(["head", "expression"]),
        "T8：body 已停用，schema 不得再邀请它"
    );
    assert_eq!(cue["properties"]["intensity"]["minimum"], MIN_INTENSITY);
    assert_eq!(cue["properties"]["intensity"]["maximum"], MAX_INTENSITY);
    assert_eq!(cue["properties"]["ttl_ms"]["minimum"], MIN_TTL_MS);
    assert_eq!(cue["properties"]["ttl_ms"]["maximum"], MAX_TTL_MS);
    for axis in ["x", "y", "z"] {
        assert_eq!(cue["properties"][axis]["minimum"], AXIS_MIN);
        assert_eq!(cue["properties"][axis]["maximum"], AXIS_MAX);
    }
    assert_eq!(
        schema["properties"]["speak"]["type"],
        serde_json::json!(["string", "null"])
    );
    let ids = cue["properties"]["id"]["enum"].as_array().expect("id enum");
    assert!(ids.contains(&serde_json::json!("none")), "none 是撤销哨兵");
    // T9：enum = 能力集 ∩ **表情槽**——手势 id（本测试的 nod）不得出现。
    for id in allow() {
        assert_eq!(
            ids.contains(&serde_json::json!(id)),
            EXPRESSION_PRESET_IDS.contains(&id.as_str()),
            "expression enum 必须只留表情槽：{id}"
        );
    }
    // 校验器与 schema 同一组：能力集里的手势 id 填进 expression 也要丢条（不失败）。
    let dropped = parse_plan(
        r#"{"segments":["一"],"cues":[{"field":"expression","id":"nod","intensity":1,"at":"now","hold":false}]}"#,
        &allow(),
        "一",
    )
    .expect("丢条不是整份失败");
    assert!(dropped.cues.is_empty(), "手势 id 不得当表情用");
    assert!(
        dropped
            .warnings
            .iter()
            .any(|w| w.code() == "performance_expression_unknown_id")
    );
    // 校验器用的是**同一组常量**：每一条越界都落在常量边界上。
    let plan = parse_plan(
        r#"{"segments":["一"],"cues":[{"field":"head","x":9.0,"intensity":99,"at":"now","hold":false,"ttl_ms":999999}]}"#,
        &allow(),
        "一",
    )
    .expect("合法");
    let json = plan.cues[0].to_json();
    assert_eq!(json["x"], AXIS_MAX);
    assert_eq!(json["intensity"], MAX_INTENSITY);
    assert_eq!(json["ttl_ms"], MAX_TTL_MS);
    assert_eq!(
        parse_plan(r#"{"segments":["一",7],"cues":[]}"#, &allow(), "一")
            .expect_err("段元素类型")
            .code(),
        "performance_plan_segment_not_string"
    );
}

/// §5.1：表演层提示词 / schema / user 消息里**不得出现任何 Param 参数名**。
#[test]
fn performance_prompt_never_contains_param_names() {
    let schema = json_schema_strict(&allow()).to_string();
    let user = build_user_prompt("你好", "（挥手）嗯……我想到了。", &allow());
    for (name, text) in [
        ("SYSTEM_STRUCTURED", prompt::SYSTEM_STRUCTURED),
        ("SYSTEM_JSON_ONLY", prompt::SYSTEM_JSON_ONLY),
        ("json_schema_strict", schema.as_str()),
        ("build_user_prompt", user.as_str()),
    ] {
        assert!(
            !text.contains("Param"),
            "{name} 不得出现任何 Param 参数名（协议 §5.1）"
        );
        assert!(
            !text.to_ascii_lowercase().contains("param"),
            "{name} 小写也不得出现"
        );
    }
    assert!(prompt::SYSTEM_JSON_ONLY.contains("segments"));
    assert!(prompt::SYSTEM_STRUCTURED.contains("segments"));
}

/// WS action_cue payload 与既有帧逐字段同形（legacy cue 路径不变）。
#[test]
fn action_cue_payload_matches_existing_frame_shape() {
    let cues = vec![PerformanceCue {
        sentence_seq: 2,
        preset_id: "smile".to_string(),
        intensity: 2,
        ttl_ms: 1500,
        ..Default::default()
    }];
    let payload = action_cue_payload(7, 2, &cues);
    assert_eq!(payload["epoch"], 7);
    assert_eq!(payload["covers_upto_seq"], 2);
    assert_eq!(payload["cues"][0]["sentence_seq"], 2);
    assert_eq!(payload["cues"][0]["preset_id"], "smile");
    assert_eq!(payload["cues"][0]["intensity"], 2);
    assert_eq!(payload["cues"][0]["ttl_ms"], 1500);
    assert_eq!(payload["cues"][0]["priority"], PRIORITY_PERFORMANCE);
    assert!(
        payload["cues"][0].get("field").is_none(),
        "legacy cue 不新增键"
    );
}

// ---------------------------------------------------------------- 回退路径

#[derive(Debug)]
struct MockClient {
    reply: Option<PerformanceReply>,
    enabled: bool,
    seen: Mutex<Vec<String>>,
}

impl MockClient {
    fn ok(raw: &str) -> Self {
        Self {
            reply: Some(PerformanceReply {
                raw: raw.to_string(),
                structured: true,
            }),
            enabled: true,
            seen: Mutex::new(Vec::new()),
        }
    }
    fn failing() -> Self {
        Self {
            reply: None,
            enabled: true,
            seen: Mutex::new(Vec::new()),
        }
    }
    fn disabled() -> Self {
        Self {
            reply: None,
            enabled: false,
            seen: Mutex::new(Vec::new()),
        }
    }
}

impl PerformanceClient for MockClient {
    fn enabled(&self) -> bool {
        self.enabled
    }
    fn kind(&self) -> &'static str {
        "injected"
    }
    fn has_api_key(&self) -> bool {
        false
    }
    fn request<'a>(
        &'a self,
        system: &'a str,
        user: &'a str,
        _timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
        self.seen
            .lock()
            .expect("mock lock")
            .push(format!("{system}\n{user}"));
        let reply = self.reply.clone();
        Box::pin(async move { reply })
    }
}

fn rule_one_cue() -> RuleFallback {
    std::sync::Arc::new(|text: &str| {
        if text.trim().is_empty() {
            return Vec::new();
        }
        vec![PerformanceCue {
            sentence_seq: 1,
            preset_id: "nod".to_string(),
            intensity: 1,
            ttl_ms: 2_000,
            ..Default::default()
        }]
    })
}

/// 成功（v0 speak 路径）：speak 与 cues 都来自表演层 JSON（**不**走规则）。
#[tokio::test]
async fn resolve_success_uses_the_plan_not_the_rule() {
    let client = MockClient::ok(
        r#"{"speak":"你好呀。再见。","cues":[{"sentence_seq":2,"preset_id":"smile","intensity":2,"ttl_ms":1200}]}"#,
    );
    let rt = PerformanceRuntime::new(
        Box::new(client),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("你好", "（挥手）你好呀。*歪头*再见。").await;
    assert_eq!(out.reason, FallbackReason::None);
    assert_eq!(out.speak.as_deref(), Some("你好呀。再见。"));
    assert!(out.segments.is_none(), "v0 路径没有 segments");
    assert_eq!(out.cues.len(), 1);
    assert_eq!(out.cues[0].preset_id, "smile");
    assert_eq!(out.cues[0].sentence_seq, 2);
    assert_eq!(out.structured, Some(true));
    assert_eq!(rt.stats().plans(), 1);
    assert_eq!(rt.stats().fallbacks(), 0);
    assert_eq!(rt.stats().last_fallback(), "performance_ok");
}

/// v1 成功：segments 原样交回（引擎一对一），speak 恒 None。
#[tokio::test]
async fn resolve_v1_segments_plan_is_one_to_one_and_speak_is_none() {
    let client = MockClient::ok(
        r#"{"segments":["你好。","再见。"],"cues":[{"field":"expression","id":"smile","intensity":2,"at":"seg:2","hold":false}]}"#,
    );
    let rt = PerformanceRuntime::new(
        Box::new(client),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("你好", "你好。再见。").await;
    assert_eq!(out.reason, FallbackReason::None);
    assert!(out.speak.is_none(), "v1 的文本来源是 segments，不是 speak");
    assert_eq!(
        out.segments.as_deref(),
        Some(&["你好。".to_string(), "再见。".to_string()][..])
    );
    assert_eq!(out.cues.len(), 1);
    assert_eq!(out.cues[0].to_json()["sentence_seq"], 2);
    assert_eq!(rt.stats().plans(), 1);
}

/// v1 拼接不成立 → 整份失败 → 回退（clean + 规则 cue）。
#[tokio::test]
async fn resolve_v1_partition_failure_falls_back_to_clean_and_rule() {
    let raw = "（挥手）你好呀。*歪头*再见。";
    let rt = PerformanceRuntime::new(
        Box::new(MockClient::ok(r#"{"segments":["被改写"],"cues":[]}"#)),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("你好", raw).await;
    assert_eq!(out.reason, FallbackReason::InvalidPlan);
    assert_eq!(rt.stats().last_fallback(), "performance_plan_invalid");
    assert!(out.segments.is_none(), "回退不走 v1 segments");
    assert_eq!(out.speak.as_deref(), Some("你好呀。再见。"));
    assert_eq!(out.cues.len(), 1, "回退的 cue 来自规则层");
}

/// 三态（v0 成功路）：noop / 只说 / 只动。
#[tokio::test]
async fn resolve_three_states_noop_speak_only_cue_only() {
    // noop
    let rt = PerformanceRuntime::new(
        Box::new(MockClient::ok(r#"{"speak":null,"cues":[]}"#)),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("在吗", "（沉默）").await;
    assert!(out.is_noop());
    assert_eq!(
        rt.stats().noops.load(std::sync::atomic::Ordering::Relaxed),
        1
    );

    // 只说
    let rt = PerformanceRuntime::new(
        Box::new(MockClient::ok(r#"{"speak":"在的。","cues":[]}"#)),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("在吗", "在的。").await;
    assert!(!out.is_noop());
    assert!(out.cues.is_empty());
    assert_eq!(out.speak.as_deref(), Some("在的。"));

    // 只动（speak 空 + 一条 cue）
    let rt = PerformanceRuntime::new(
        Box::new(MockClient::ok(
            r#"{"speak":"","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":1,"ttl_ms":900}]}"#,
        )),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("嗯", "（点头）").await;
    assert!(out.speak.is_none());
    assert_eq!(out.cues.len(), 1);
    assert_eq!(
        rt.stats()
            .cue_turns
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

/// 失败回退：坏 JSON / 请求失败 / 关闸 / 空原文 → clean(原文) + 规则 cue。
#[tokio::test]
async fn resolve_fallback_paths_use_clean_text_and_rule_cues() {
    let raw = "你好呀（挥手）. *歪头* 再见。";
    let rt = PerformanceRuntime::new(
        Box::new(MockClient::ok(r#"{"speak":"hi"}"#)),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("你好", raw).await;
    assert_eq!(out.reason, FallbackReason::InvalidPlan);
    assert_eq!(rt.stats().last_fallback(), "performance_plan_invalid");
    assert_eq!(out.speak.as_deref(), Some("你好呀. 再见。"));
    assert_eq!(out.cues.len(), 1, "回退的 cue 来自规则层");

    let rt = PerformanceRuntime::new(
        Box::new(MockClient::failing()),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("你好", "你好呀。").await;
    assert_eq!(out.reason, FallbackReason::RequestFailed);
    assert_eq!(out.speak.as_deref(), Some("你好呀。"));

    let rt = PerformanceRuntime::new(
        Box::new(MockClient::disabled()),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("你好", "你好呀。").await;
    assert_eq!(out.reason, FallbackReason::Disabled);
    assert_eq!(out.speak.as_deref(), Some("你好呀。"));
    assert!(!rt.enabled());

    let rt = PerformanceRuntime::new(
        Box::new(MockClient::ok(r#"{"speak":"不该被用","cues":[]}"#)),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_one_cue()),
    );
    let out = rt.resolve("你好", "   ").await;
    assert_eq!(out.reason, FallbackReason::EmptyAssistant);
    assert!(out.is_noop());
}
