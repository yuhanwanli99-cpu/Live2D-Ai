//! 表演层 wire / schema / 客户端 runtime 回归（自 `tests.rs` 机械拆出）。

use std::sync::Mutex;

use super::tests::allow;
use super::*;

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
