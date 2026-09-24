//! 表演层回归：**表驱动**合法/非法样例、三态（noop / 只说 / 只动）、
//! schema 与校验器同界、客户端 wire 形态、回退路径与原因码。

use std::sync::Mutex;

use super::*;

// ---------------------------------------------------------------- 表驱动校验

fn allow() -> Vec<String> {
    vec!["nod".to_string(), "smile".to_string()]
}

/// 合法样例：形状、钳位、未知字段丢弃、三态。
#[test]
fn legal_samples_parse_with_clamping_and_unknown_fields_dropped() {
    // 只说：cues 为空。
    let plan = parse_plan(r#"{"speak":"你好呀。","cues":[]}"#, &allow()).expect("合法");
    assert_eq!(plan.speak.as_deref(), Some("你好呀。"));
    assert!(plan.cues.is_empty());
    assert!(!plan.is_noop());

    // noop：null + 空数组。
    let plan = parse_plan(r#"{"speak":null,"cues":[]}"#, &allow()).expect("合法");
    assert!(plan.is_noop());

    // noop 的第二种写法：空串。
    let plan = parse_plan(r#"{"speak":"   ","cues":[]}"#, &allow()).expect("合法");
    assert!(plan.is_noop());

    // 只动：speak 空 + 一条 cue。
    let plan = parse_plan(
        r#"{"speak":"","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":2,"ttl_ms":1800}]}"#,
        &allow(),
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
    )
    .expect("合法");
    assert_eq!(plan.cues[0].intensity, MAX_INTENSITY);
    assert_eq!(plan.cues[0].ttl_ms, MAX_TTL_MS);
    let plan = parse_plan(
        r#"{"speak":"a","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":0,"ttl_ms":0}]}"#,
        &allow(),
    )
    .expect("合法");
    assert_eq!(plan.cues[0].intensity, MIN_INTENSITY);
    assert_eq!(plan.cues[0].ttl_ms, MIN_TTL_MS);

    // 未知字段一律丢弃（不失败）。
    let plan = parse_plan(
        r#"{"speak":"a","cues":[],"reason":"我想多了","mood":"happy"}"#,
        &allow(),
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
        (r#"{"cues":[]}"#, "performance_plan_missing_speak"),
        (r#"{"speak":42,"cues":[]}"#, "performance_plan_speak_type"),
        (r#"{"speak":"a"}"#, "performance_plan_missing_cues"),
        (r#"{"speak":"a","cues":{}}"#, "performance_plan_cues_type"),
        (
            r#"{"speak":"a","cues":[1,2]}"#,
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
    ];
    for (raw, code) in cases {
        let err = parse_plan(raw, &allow()).expect_err("必须整份失败");
        assert_eq!(err.code(), code, "输入 {raw}");
        assert!(!err.message().is_empty());
    }

    // 超量：MAX_CUES + 1 条 → 整份失败。
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
        parse_plan(&raw, &allow()).expect_err("超量").code(),
        "performance_plan_too_many_cues"
    );
}

/// schema 与校验器**同一组边界**（发出去的 schema == 校验器认的东西）。
#[test]
fn schema_and_validator_share_the_same_bounds() {
    let schema = json_schema_strict(&allow());
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["required"], serde_json::json!(["speak", "cues"]));
    let cue = &schema["properties"]["cues"]["items"];
    assert_eq!(cue["additionalProperties"], false);
    assert_eq!(
        cue["required"],
        serde_json::json!(["sentence_seq", "preset_id", "intensity", "ttl_ms"])
    );
    assert_eq!(schema["properties"]["cues"]["maxItems"], MAX_CUES);
    assert_eq!(
        cue["properties"]["preset_id"]["enum"],
        serde_json::json!(allow())
    );
    assert_eq!(cue["properties"]["intensity"]["minimum"], MIN_INTENSITY);
    assert_eq!(cue["properties"]["intensity"]["maximum"], MAX_INTENSITY);
    assert_eq!(cue["properties"]["ttl_ms"]["minimum"], MIN_TTL_MS);
    assert_eq!(cue["properties"]["ttl_ms"]["maximum"], MAX_TTL_MS);
    // speak 两态。
    assert_eq!(
        schema["properties"]["speak"]["type"],
        serde_json::json!(["string", "null"])
    );
}

/// WS action_cue payload 与既有帧逐字段同形（前端 ActionCue.fromJson 直接吃）。
#[test]
fn action_cue_payload_matches_existing_frame_shape() {
    let cues = vec![PerformanceCue {
        sentence_seq: 2,
        preset_id: "smile".to_string(),
        intensity: 2,
        ttl_ms: 1500,
    }];
    let payload = action_cue_payload(7, 2, &cues);
    assert_eq!(payload["epoch"], 7);
    assert_eq!(payload["covers_upto_seq"], 2);
    assert_eq!(payload["cues"][0]["sentence_seq"], 2);
    assert_eq!(payload["cues"][0]["preset_id"], "smile");
    assert_eq!(payload["cues"][0]["intensity"], 2);
    assert_eq!(payload["cues"][0]["ttl_ms"], 1500);
    assert_eq!(payload["cues"][0]["priority"], PRIORITY_PERFORMANCE);
}

// ---------------------------------------------------------------- 宽容解析

#[test]
fn strip_code_fence_takes_the_json_body() {
    assert_eq!(
        strip_code_fence("```json\n{\"speak\":null,\"cues\":[]}\n```"),
        r#"{"speak":null,"cues":[]}"#
    );
    assert_eq!(
        strip_code_fence("好的，这是结果：{\"speak\":\"嗯。\",\"cues\":[]} 以上"),
        r#"{"speak":"嗯。","cues":[]}"#
    );
    // 没有花括号 → 原样（交给校验器判失败，不在这里假装成功）。
    assert_eq!(strip_code_fence("  nope  "), "nope");
}

#[test]
fn parse_completion_ignores_reasoning_and_rejects_blank() {
    let ok = r#"{"choices":[{"message":{"role":"assistant","content":"{\"speak\":null,\"cues\":[]}","reasoning_content":"SECRET"}}]}"#;
    assert_eq!(
        parse_completion(ok).as_deref(),
        Some("{\"speak\":null,\"cues\":[]}")
    );
    assert_eq!(parse_completion(r#"{"choices":[]}"#), None);
    assert_eq!(parse_completion("not json"), None);
    assert_eq!(
        parse_completion(r#"{"choices":[{"message":{"content":"   "}}]}"#),
        None
    );
}

#[test]
fn structured_mode_parse_never_fails() {
    assert_eq!(
        StructuredMode::parse("json_schema"),
        StructuredMode::JsonSchema
    );
    assert_eq!(StructuredMode::parse("PROMPT"), StructuredMode::Prompt);
    assert_eq!(StructuredMode::parse("随便"), StructuredMode::Auto);
    assert_eq!(StructuredMode::default(), StructuredMode::Auto);
    assert_eq!(StructuredMode::Auto.as_str(), "auto");
}

#[test]
fn endpoint_join_rejects_missing_and_bad_scheme() {
    // 收敛后只有一个实现：`live2d_ai_runtime::join_endpoint`。
    assert!(crate::join_endpoint("", "/x").is_err());
    assert!(crate::join_endpoint("ftp://h/v1", "/x").is_err());
    assert_eq!(
        crate::join_endpoint("http://h/v1/", "/chat/completions")
            .expect("合法")
            .as_str(),
        "http://h/v1/chat/completions"
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
        }]
    })
}

/// 成功：speak 与 cues 都来自表演层 JSON（**不**走规则）。
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
    assert_eq!(out.cues.len(), 1);
    assert_eq!(out.cues[0].preset_id, "smile");
    assert_eq!(out.cues[0].sentence_seq, 2);
    assert_eq!(out.structured, Some(true));
    assert_eq!(rt.stats().plans(), 1);
    assert_eq!(rt.stats().fallbacks(), 0);
    assert_eq!(rt.stats().last_fallback(), "performance_ok");
}

/// 三态（成功路）：noop / 只说 / 只动。
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

/// 失败回退：坏 JSON / 请求失败 / 关闸 / 空原文 → speak=清洗(原文) + 规则 cue。
#[tokio::test]
async fn resolve_fallback_paths_use_clean_text_and_rule_cues() {
    let raw = "你好呀（挥手）. *歪头* 再见。";
    // 坏 JSON（过不了同一个校验器）→ InvalidPlan。
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

    // 请求失败（超时 / 非 2xx / 无正文）→ RequestFailed。
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

    // 关闸（客户端不可用）→ Disabled；行为与「从来没有表演层」同义。
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

    // 空原文 → 没有可表演的内容：不说、规则层也没 cue。
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

/// 关闸装配：与「从来没有表演层」逐字一致（runtime=None、无 note）。
#[test]
fn assemble_off_is_none_and_missing_endpoint_is_degraded() {
    let wiring = |enabled: bool, base: &str| crate::settings::PerformanceSettings {
        enabled,
        base_url: base.to_string(),
        model: "m".to_string(),
        ..Default::default()
    };
    let off = assemble(
        &wiring(false, "http://127.0.0.1:11434/v1"),
        allow(),
        Some(rule_one_cue()),
    );
    assert!(off.runtime.is_none());
    assert!(off.note.is_none());

    let no_base = assemble(&wiring(true, ""), allow(), Some(rule_one_cue()));
    assert!(no_base.note.is_some(), "缺端点必须可观察 degraded");
    assert!(!no_base.runtime.expect("仍装配").enabled());

    let configured = assemble(
        &wiring(true, "http://127.0.0.1:11434/v1"),
        allow(),
        Some(rule_one_cue()),
    );
    let rt = configured.runtime.expect("装配");
    assert!(rt.enabled());
    assert_eq!(rt.client_kind(), "openai");
    assert!(configured.note.is_none());
}

// ---------------------------------------------------------------- HTTP wire

/// 脚本化 loopback：按顺序回应 N 个请求，回传每个请求的原始文本。
async fn scripted_server(
    responses: Vec<(u16, String)>,
) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback");
    let addr = listener.local_addr().expect("addr");
    let handle = tokio::spawn(async move {
        let mut raws = Vec::new();
        for (status, body) in responses {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 1024];
            loop {
                let n = stream.read(&mut tmp).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                if let Some(head_end) = find_subslice(&buf, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                    let cl = content_length(&head);
                    if buf.len() >= head_end + 4 + cl {
                        break;
                    }
                }
            }
            raws.push(String::from_utf8_lossy(&buf).to_string());
            let reason = if status == 200 { "OK" } else { "Bad Request" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        }
        raws
    });
    (format!("http://{addr}/v1"), handle)
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn content_length(head: &str) -> usize {
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0)
}

/// structured 路：请求体带 json_schema（枚举就是能力集），只有 system+user。
#[tokio::test]
async fn openai_client_posts_json_schema_and_parses_content() {
    let plan = r#"{"speak":"你好。","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":2,"ttl_ms":1500}]}"#;
    let response = serde_json::json!({
        "choices": [{"message": {"role": "assistant", "content": plan}, "finish_reason": "stop"}],
        "reasoning_content": "SECRET_THOUGHT"
    })
    .to_string();
    let (base, handle) = scripted_server(vec![(200, response)]).await;
    let client = OpenAiPerformanceClient::with_api_key(
        &base,
        "director-model",
        Some(crate::secret::ApiSecret::new("sk-test-123")),
        3_000,
        StructuredMode::JsonSchema,
        &allow(),
    )
    .expect("构造应成功");
    assert!(client.enabled());
    assert_eq!(client.kind(), "openai");
    assert!(client.has_api_key());
    let reply = client
        .request("SYS", "USER", 3_000)
        .await
        .expect("应解析出正文");
    assert!(reply.structured);
    assert_eq!(reply.raw, plan);
    assert!(parse_plan(&reply.raw, &allow()).is_ok());

    let raws = handle.await.expect("mock server");
    let raw = &raws[0];
    assert!(raw.starts_with("POST /v1/chat/completions"), "{raw}");
    assert!(
        raw.to_ascii_lowercase()
            .contains("authorization: bearer sk-test-123"),
        "{raw}"
    );
    assert!(raw.contains("\"stream\":false"), "{raw}");
    assert!(raw.contains("\"model\":\"director-model\""), "{raw}");
    assert!(raw.contains("\"json_schema\""), "{raw}");
    assert!(raw.contains("\"strict\":true"), "{raw}");
    assert!(raw.contains("\"nod\""), "{raw}");
    assert!(
        !raw.contains("reasoning") && !raw.contains("SECRET_THOUGHT"),
        "请求体不得携带思考：{raw}"
    );
    assert_eq!(
        raw.matches("\"role\"").count(),
        2,
        "只有 system+user：{raw}"
    );
}

/// auto：上游 4xx（不认识 response_format）→ 原地降级 prompt 再发一次。
#[tokio::test]
async fn auto_mode_degrades_to_prompt_on_4xx() {
    let plan = r#"{"speak":null,"cues":[]}"#;
    let ok = serde_json::json!({"choices":[{"message":{"content":plan}}]}).to_string();
    let err = r#"{"error":{"message":"response_format is not supported","type":"invalid_request_error"}}"#.to_string();
    let (base, handle) = scripted_server(vec![(400, err), (200, ok)]).await;
    let client = OpenAiPerformanceClient::with_api_key(
        &base,
        "m",
        None,
        3_000,
        StructuredMode::Auto,
        &allow(),
    )
    .expect("构造应成功");
    let reply = client
        .request("SYS", "USER", 3_000)
        .await
        .expect("降级后成功");
    assert!(!reply.structured, "第二次必须不带 response_format");
    let raws = handle.await.expect("mock server");
    assert_eq!(raws.len(), 2, "auto 必须先 json_schema 再 prompt");
    assert!(raws[0].contains("\"response_format\""));
    assert!(!raws[1].contains("\"response_format\""));
    assert!(
        raws[1].contains("只输出 JSON"),
        "降级请求要带 JSON-only system"
    );
}

/// 5xx 不重试；只有思考没有正文也算失败。
#[tokio::test]
async fn server_errors_and_blank_content_fall_back_to_none() {
    let (base, handle) = scripted_server(vec![(500, r#"{"error":"boom"}"#.to_string())]).await;
    let client = OpenAiPerformanceClient::with_api_key(
        &base,
        "m",
        None,
        2_000,
        StructuredMode::Auto,
        &allow(),
    )
    .expect("构造");
    assert_eq!(client.request("s", "u", 2_000).await, None);
    assert_eq!(handle.await.expect("mock").len(), 1, "5xx 不得重试");

    let (base, handle) = scripted_server(vec![(
        200,
        r#"{"choices":[{"message":{"content":"","reasoning_content":"想一想"}}]}"#.to_string(),
    )])
    .await;
    let client = OpenAiPerformanceClient::with_api_key(
        &base,
        "m",
        None,
        2_000,
        StructuredMode::Prompt,
        &allow(),
    )
    .expect("构造");
    assert_eq!(client.request("s", "u", 2_000).await, None);
    let raws = handle.await.expect("mock");
    assert!(
        !raws[0].contains("\"response_format\""),
        "prompt 路不带 schema"
    );
}
