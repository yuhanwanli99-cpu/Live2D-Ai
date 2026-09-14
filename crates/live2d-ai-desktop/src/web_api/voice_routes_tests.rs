//! `voice_routes` 回归测试（Wave 2 A 轨）。
//!
//! 经 `#[cfg(test)] #[path = "voice_routes_tests.rs"] mod tests;` 挂在
//! `voice_routes` 下，故 `use super::*` 可见私有项（`dummy_ctx` / 常量等）。
//!
//! 覆盖面（契约逐条，见 `docs/voice-input.md` §4）：405 / 415 / 403 origin /
//! 401 / 403 mod_disabled / 400 invalid_payload / 400 empty_transcript /
//! 400 text_too_long / 200 ok / 200 busy / 503 supervisor_unavailable，
//! 外加 token 优先级与「清洗复用 Mod crate」的行为断言。

use super::*;
use std::sync::Arc;

/// 一次调用（省略 auth 头 = None）。
fn call(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    ct: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    handle_voice_transcript(ctx, method, path, body, origin, ct, None)
}

/// 带 `Authorization` 头的一次调用。
fn call_auth(
    ctx: &ServerContext,
    body: &str,
    auth: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    handle_voice_transcript(
        ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        body,
        None,
        Some("application/json"),
        auth,
    )
}

/// 拉出响应体字符串。
fn body_of(resp: Response<Cursor<Vec<u8>>>) -> String {
    use std::io::Read;
    let mut s = String::new();
    let _ = resp.into_reader().read_to_string(&mut s);
    s
}

/// 宽松 ctx：允许无 Origin（curl 工具模式），空 Mod 注册表。
fn dummy_ctx(sec: crate::web_api::security::SecurityContext) -> ServerContext {
    ServerContext {
        status_ctx: Arc::new(crate::web_api::app_routes::StatusContext::new(
            live2d_ai_runtime::AppSettings::default(),
            ".scratch".to_string(),
            None,
        )),
        capabilities_ctx: Arc::new(crate::web_api::app_routes::CapabilitiesContext::new()),
        supervisor_slot: crate::web_api::supervisor_slot::SupervisorSlot::new(),
        broadcaster: crate::web_api::ws::Broadcaster::new(),
        security: sec,
        models: crate::web_api::models_routes::default_store(),
        mod_registry: Arc::new(std::sync::Mutex::new(
            crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({})),
        )),
    }
}

/// 允许无 Origin 的宽松 ctx（`allow_no_origin = true`）。
fn ctx_allow_no_origin() -> ServerContext {
    dummy_ctx(crate::web_api::security::SecurityContext::new(18099, true))
}

/// 用真实工厂表 + 指定 manifest 装配注册表（启停门禁测试用）。
fn ctx_with_manifest(manifest: &serde_json::Value) -> ServerContext {
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let mut ctx = dummy_ctx(sec);
    ctx.mod_registry = Arc::new(std::sync::Mutex::new(
        crate::mod_registry::ModRegistry::new(crate::AVAILABLE_MOD_FACTORIES, manifest),
    ));
    ctx
}

/// 不可达的 LLM 端点（连接被立即拒绝：回合很快失败，不影响断言）。
const UNREACHABLE_LLM: &str = "http://127.0.0.1:1/v1";

/// 起一个真实 supervisor（默认指向不可达端点：只走「send 成功」路径）。
///
/// `llm_base` 可指向「黑洞」端点以制造**确定性的 in-flight 回合**（见
/// [`blackhole_endpoint`]）。返回 `(ctx, handle, tmp_path)`；caller 收尾需
/// `handle.quit()` + 删 tmp。
fn ctx_with_supervisor(
    tag: &str,
    llm_base: &str,
) -> (
    ServerContext,
    Arc<crate::supervisor::SupervisorHandle>,
    std::path::PathBuf,
) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{:?}", std::thread::current().id()).hash(&mut hasher);
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_voice_routes_{}_{}_{}.toml",
        tag,
        std::process::id(),
        hasher.finish()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(
        &tmp,
        live2d_ai_runtime::AppSettings::default().to_toml_string(),
    )
    .expect("write tmp");

    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new(llm_base, "m"),
        live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
    )
    .expect("client");
    let handle = Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
            mod_events: None,
        },
        |_ev| {},
    ));
    let ctx = dummy_ctx(crate::web_api::security::SecurityContext::new(18099, true))
        .with_supervisor(Arc::clone(&handle));
    (ctx, handle, tmp)
}

/// 起一个「黑洞」TCP 监听：接受连接后**不回任何数据**，让 supervisor 的本轮
/// LLM 请求悬停在 in-flight。turn 期间 `say_rx` 不被消费（`turn.rs` 只在
/// stop 事务里 drain），因此「缓冲已满 → busy」可以**确定性**断言，
/// 不依赖线程调度窗口。返回端口。
fn blackhole_endpoint() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind blackhole");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        // 持有连接（drop 会让客户端立刻看到 EOF，回合就不再 in-flight）。
        let mut held = Vec::new();
        for s in listener.incoming() {
            match s {
                Ok(s) => held.push(s),
                Err(_) => break,
            }
        }
    });
    port
}

// ------------------------------------------------------------ 路由 / 方法

#[test]
fn mismatched_path_returns_none() {
    let ctx = ctx_allow_no_origin();
    assert!(
        call(
            &ctx,
            &Method::Post,
            "/api/v1/external/chat",
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .is_none(),
        "非本端点路径必须返回 None 交还 dispatch"
    );
}

#[test]
fn wrong_method_405() {
    let ctx = ctx_allow_no_origin();
    for m in [Method::Get, Method::Patch, Method::Delete] {
        let resp = call(
            &ctx,
            &m,
            VOICE_TRANSCRIPT_PATH,
            r#"{"text":"hi"}"#,
            None,
            Some("application/json"),
        )
        .expect("路径命中");
        assert_eq!(resp.status_code(), StatusCode(405), "method={m:?}");
        assert!(
            body_of(resp).contains("method_not_allowed"),
            "405 必须带 method_not_allowed"
        );
    }
}

#[test]
fn non_json_ct_415() {
    let ctx = ctx_allow_no_origin();
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi"}"#,
        None,
        Some("text/plain"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(415));
    assert!(body_of(resp).contains("unsupported_media_type"));
}

#[test]
fn missing_ct_415() {
    let ctx = ctx_allow_no_origin();
    let resp = call(&ctx, &Method::Post, VOICE_TRANSCRIPT_PATH, "{}", None, None).unwrap();
    assert_eq!(
        resp.status_code(),
        StatusCode(415),
        "无 Content-Type 同样拒绝"
    );
}

// ------------------------------------------------------------ Origin

#[test]
fn bad_origin_403() {
    let ctx = dummy_ctx(crate::web_api::security::SecurityContext::new(18099, false));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi"}"#,
        Some("http://evil.example"),
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(403));
    let b = body_of(resp);
    assert!(b.contains("origin_denied"), "got: {b}");
}

#[test]
fn missing_origin_without_allow_no_origin_403() {
    let ctx = dummy_ctx(crate::web_api::security::SecurityContext::new(18099, false));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(403));
    assert!(body_of(resp).contains("origin_required"));
}

#[test]
fn same_loopback_origin_passes() {
    let ctx = dummy_ctx(crate::web_api::security::SecurityContext::new(18099, false));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi"}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .unwrap();
    // 过 Origin 后止于 supervisor 未就绪（不是 403）。
    assert_eq!(resp.status_code(), StatusCode(503));
}

// ------------------------------------------------------------ 请求体

#[test]
fn invalid_payload_400() {
    let ctx = ctx_allow_no_origin();
    for body in [
        "not json",
        "{}",
        r#"{"text":123}"#,
        r#"{"text":"hi","token":42}"#,
    ] {
        let resp = call(
            &ctx,
            &Method::Post,
            VOICE_TRANSCRIPT_PATH,
            body,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(400), "body={body}");
        assert!(
            body_of(resp).contains("invalid_payload"),
            "body={body} 必须是 invalid_payload"
        );
    }
}

#[test]
fn empty_transcript_400_distinct_from_invalid_payload() {
    let ctx = ctx_allow_no_origin();
    for raw in ["", "   ", "\n\t\u{3000}", "\u{200B}\u{FEFF}"] {
        let body = serde_json::json!({"text": raw}).to_string();
        let resp = call(
            &ctx,
            &Method::Post,
            VOICE_TRANSCRIPT_PATH,
            &body,
            None,
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(400), "raw={raw:?}");
        let b = body_of(resp);
        assert!(
            b.contains("empty_transcript") && !b.contains("invalid_payload"),
            "空白转写必须是 empty_transcript（不是 invalid_payload）: {b}"
        );
    }
}

#[test]
fn text_too_long_400() {
    let ctx = ctx_allow_no_origin();
    // 2001 个字符（清洗后仍是 2001）→ 超长。
    let body = serde_json::json!({"text": "a".repeat(MAX_TEXT_LEN + 1)}).to_string();
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &body,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
    assert!(body_of(resp).contains("text_too_long"));

    // 边界：恰好 2000 → 不因长度被拒（止于 503 = 已过长度门）。
    let ok_body = serde_json::json!({"text": "a".repeat(MAX_TEXT_LEN)}).to_string();
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &ok_body,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(503), "2000 字符应过长度门");
}

#[test]
fn cleaned_length_is_what_counts() {
    let ctx = ctx_allow_no_origin();
    // 清洗后恰好 2000 字符：**原始输入** > 2000（含空白），但折叠/去首尾后
    // 落在门内 → 不是 text_too_long。
    let messy = format!(" \u{3000}{}\t\n ", "a".repeat(MAX_TEXT_LEN));
    let body = serde_json::json!({"text": messy}).to_string();
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &body,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(
        resp.status_code(),
        StatusCode(503),
        "长度按**清洗后**文本判定"
    );
}

// ------------------------------------------------------------ 启停门禁

#[test]
fn mod_registered_but_disabled_403() {
    let ctx = ctx_with_manifest(&serde_json::json!({}));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(403));
    assert!(body_of(resp).contains("mod_disabled"));
}

#[test]
fn mod_enabled_passes_gate_to_supervisor() {
    let ctx = ctx_with_manifest(&serde_json::json!({
        "mods": {"voice-input": {"enabled": true}}
    }));
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(
        resp.status_code(),
        StatusCode(503),
        "启用后应过门禁，止于 supervisor 未就绪（而非 403）"
    );
}

#[test]
fn absent_mod_is_not_gated() {
    // 空注册表（极简测试上下文）→ 不设门禁，止于 503。
    let ctx = ctx_allow_no_origin();
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(503));
}

// ------------------------------------------------------------ token

#[test]
fn config_token_missing_401() {
    if std::env::var(TOKEN_ENV_VAR).is_ok() {
        return; // 环境已设 env token 时该路径不适用。
    }
    let ctx = ctx_with_manifest(&serde_json::json!({
        "mods": {"voice-input": {"enabled": true, "config": {"token": "cfg-secret"}}}
    }));
    let resp = call_auth(&ctx, r#"{"text":"hi"}"#, None).unwrap();
    assert_eq!(resp.status_code(), StatusCode(401));
    let b = body_of(resp);
    assert!(b.contains("unauthorized"), "got: {b}");
    assert!(
        !b.contains("cfg-secret"),
        "401 响应绝不得回显 token 明文: {b}"
    );
}

#[test]
fn config_token_accepts_bearer_header() {
    if std::env::var(TOKEN_ENV_VAR).is_ok() {
        return;
    }
    let ctx = ctx_with_manifest(&serde_json::json!({
        "mods": {"voice-input": {"enabled": true, "config": {"token": "cfg-secret"}}}
    }));
    let resp = call_auth(&ctx, r#"{"text":"hi"}"#, Some("Bearer cfg-secret")).unwrap();
    assert_ne!(
        resp.status_code(),
        StatusCode(401),
        "Bearer 头匹配 config token 时应放行（止于 503）"
    );
    // 大小写不敏感前缀同样接受。
    let lower = call_auth(&ctx, r#"{"text":"hi"}"#, Some("bearer cfg-secret")).unwrap();
    assert_ne!(lower.status_code(), StatusCode(401));
    // 不匹配 → 401。
    let bad = call_auth(&ctx, r#"{"text":"hi"}"#, Some("Bearer nope")).unwrap();
    assert_eq!(bad.status_code(), StatusCode(401));
}

#[test]
fn config_token_accepts_body_token() {
    if std::env::var(TOKEN_ENV_VAR).is_ok() {
        return;
    }
    let ctx = ctx_with_manifest(&serde_json::json!({
        "mods": {"voice-input": {"enabled": true, "config": {"token": "cfg-secret"}}}
    }));
    let ok = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi","token":"cfg-secret"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_ne!(ok.status_code(), StatusCode(401), "body token 匹配应放行");

    let bad = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"hi","token":"wrong"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(bad.status_code(), StatusCode(401));
}

#[test]
fn no_token_configured_is_not_authenticated() {
    if std::env::var(TOKEN_ENV_VAR).is_ok() {
        return;
    }
    // 注册表里没有 voice-input（无 config token）→ 不鉴权 → 止于 503。
    let ctx = ctx_allow_no_origin();
    let resp = call_auth(&ctx, r#"{"text":"hi"}"#, None).unwrap();
    assert_ne!(resp.status_code(), StatusCode(401));
    assert_eq!(resp.status_code(), StatusCode(503));
}

// ------------------------------------------------------------ 200 / busy

#[test]
fn success_returns_200_with_cleaned_text() {
    let (ctx, handle, tmp) = ctx_with_supervisor("ok", UNREACHABLE_LLM);
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"  你好\u3000世界  "}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let b = body_of(resp);
    let v: serde_json::Value = serde_json::from_str(&b).expect("响应是 JSON");
    assert_eq!(v["ok"], serde_json::json!(true), "got: {b}");
    assert_eq!(
        v["text"],
        serde_json::json!("你好 世界"),
        "响应必须回**清洗后**文本（证明复用 clean_transcript）: {b}"
    );

    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn zero_width_chars_are_cleaned_before_say() {
    let (ctx, handle, tmp) = ctx_with_supervisor("zw", UNREACHABLE_LLM);
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"\uFEFF你\u200B好\u0007"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let v: serde_json::Value = serde_json::from_str(&body_of(resp)).unwrap();
    assert_eq!(v["text"], serde_json::json!("你好"));
    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn busy_returns_200_ok_false() {
    let port = blackhole_endpoint();
    let (ctx, handle, tmp) = ctx_with_supervisor("busy", &format!("http://127.0.0.1:{port}/v1"));
    // 先用一条占位 say 让 supervisor 进入 in-flight 回合（黑洞端点不回包，
    // 回合悬停；turn 期间 say 缓冲不被消费）。
    assert!(handle.say("占位回合"));
    std::thread::sleep(std::time::Duration::from_millis(150));
    // 缓冲空着 → 本条被接受（填充容量 1 的缓冲）。
    let first = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"第一条"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(first.status_code(), StatusCode(200));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body_of(first)).unwrap()["ok"],
        serde_json::json!(true)
    );
    // 缓冲已满 → 200 + ok:false busy（**不是** 5xx）。
    let second = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"第二条"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(
        second.status_code(),
        StatusCode(200),
        "忙碌刻意用 200（发送方自行退避），不要 5xx"
    );
    let b = body_of(second);
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["ok"], serde_json::json!(false), "got: {b}");
    assert_eq!(v["error"]["code"], serde_json::json!("busy"), "got: {b}");

    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn empty_transcript_never_reaches_say() {
    let (ctx, handle, tmp) = ctx_with_supervisor("nosay", UNREACHABLE_LLM);
    let resp = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"\u200B"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
    // 未消费任何 pending：紧接着一条正常转写仍能被接受（= 前一条没进 say）。
    let next = call(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        r#"{"text":"正常一条"}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(next.status_code(), StatusCode(200));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&body_of(next)).unwrap()["ok"],
        serde_json::json!(true),
        "空转写不得占用 say 缓冲"
    );
    handle.quit();
    let _ = std::fs::remove_file(&tmp);
}

// ---------------------------------------------------- token / 清洗 纯函数

#[test]
fn effective_token_prefers_env_then_config_then_none() {
    assert_eq!(effective_token(None, None), None);
    assert_eq!(
        effective_token(Some(""), Some("cfg")).as_deref(),
        Some("cfg"),
        "env 为空 = 未设，回落到 config"
    );
    assert_eq!(
        effective_token(Some("   "), Some("cfg")).as_deref(),
        Some("cfg")
    );
    assert_eq!(
        effective_token(Some("env"), Some("cfg")).as_deref(),
        Some("env"),
        "env 非空时**优先**，压过 config"
    );
    assert_eq!(effective_token(Some("env"), None).as_deref(), Some("env"));
    assert_eq!(
        effective_token(Some(" env "), None).as_deref(),
        Some("env"),
        "两侧空白被 trim"
    );
    assert_eq!(
        effective_token(None, Some("  ")),
        None,
        "config 空白 = 未设"
    );
}

#[test]
fn check_token_semantics() {
    assert!(check_token(None, None, None), "未配置 token → 不鉴权");
    assert!(check_token(Some("x"), None, None));
    assert!(check_token(Some("secret"), None, Some("secret")));
    assert!(check_token(None, Some("Bearer secret"), Some("secret")));
    assert!(check_token(
        Some("secret"),
        Some("Bearer wrong"),
        Some("secret")
    ));
    assert!(!check_token(Some("wrong"), None, Some("secret")));
    assert!(!check_token(None, None, Some("secret")));
    assert!(!check_token(None, Some("Bearer wrong"), Some("secret")));
    assert!(
        !check_token(None, Some("secret"), Some("secret")),
        "缺 Bearer 前缀"
    );
}

#[test]
fn bearer_token_extracts_only_prefixed_values() {
    assert_eq!(bearer_token(Some("Bearer abc")), Some("abc"));
    assert_eq!(bearer_token(Some("bearer abc")), Some("abc"));
    assert_eq!(bearer_token(Some("BEARER abc")), Some("abc"));
    assert_eq!(bearer_token(Some("abc")), None);
    assert_eq!(bearer_token(Some("Bearer ")), None);
    assert_eq!(bearer_token(None), None);
}
