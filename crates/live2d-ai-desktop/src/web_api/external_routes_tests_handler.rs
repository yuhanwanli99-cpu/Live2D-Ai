//! `external_routes` 可观察计数 + `state_json` 回归（注入点驱动 + 真实注册表）。
//!
//! 脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;

// --- 可观察计数（Wave 3）：3 成功 / 1 坏 token / 1 忙 ---

/// 用注入点驱动 handler 的计数分支；再经**真实注册表**读 state_json
/// （与 `GET /api/v1/mods/external-input/state` 同一条路径）。
#[test]
fn handler_counts_accepts_rejects_busy_into_state() {
    // W6：注入 lookup（无 env 令牌）→ 不再依赖进程环境探测。
    let _guard = counter_lock();
    let before = live2d_ai_mod_external_input::counters_snapshot();
    let base = |k: &str| before[k].as_u64().expect("计数为整数");

    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"token": "cfg-secret"}
        }}}),
    );
    ctx.mod_registry.lock().unwrap().start_all();

    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls_c = calls.clone();
    let inject = move |_ctx: &ServerContext, _t: String| -> Option<bool> {
        // 前 3 次接受，第 4 次忙碌。
        Some(calls_c.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 3)
    };

    for _ in 0..4 {
        let resp = handle_external_chat_with(
            &ctx,
            req(&method_post(), r#"{"text":"hi","token":"cfg-secret"}"#),
            &inject,
            &no_env_lookup,
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(200));
    }
    // 坏 token → 401 拒绝（文本未进主链路）。
    let resp = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi","token":"wrong"}"#),
        &inject,
        &no_env_lookup,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(401));

    let after = live2d_ai_mod_external_input::counters_snapshot();
    assert_eq!(after["accepts"], base("accepts") + 3, "3 次成功");
    assert_eq!(after["busy"], base("busy") + 1, "1 次忙");
    assert_eq!(after["rejects"], base("rejects") + 1, "1 次坏 token");

    // state_json（`GET /mods/external-input/state` 的真源）逐键一致。
    let state = ctx
        .mod_registry
        .lock()
        .unwrap()
        .runtime_state("external-input")
        .expect("runtime 已 start，state_json 可读");
    assert_eq!(state["accepts"], after["accepts"]);
    assert_eq!(state["busy"], after["busy"]);
    assert_eq!(state["rejects"], after["rejects"]);
    assert_eq!(state["ready"], serde_json::json!(true));
}

/// sidecar 上报的 `v2_ignored` 进 state_json（可选字段，覆盖写）。
#[test]
fn v2_ignored_reported_by_sidecar_surfaces_in_state() {
    let _guard = counter_lock();
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{"enabled": true}}}),
    );
    ctx.mod_registry.lock().unwrap().start_all();
    let inject = |_ctx: &ServerContext, _t: String| -> Option<bool> { Some(true) };
    let resp = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi","v2_ignored":7}"#),
        &inject,
        &no_env_lookup,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));
    let state = ctx
        .mod_registry
        .lock()
        .unwrap()
        .runtime_state("external-input")
        .expect("state 可读");
    assert_eq!(state["v2_ignored"], 7);
}

/// `v2_ignored` 非整数 → 400 invalid_payload（不进链路、不计数）。
#[test]
fn v2_ignored_non_integer_400() {
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let resp = call(
        &ctx,
        &method_post(),
        EXTERNAL_CHAT_PATH,
        r#"{"text":"hi","v2_ignored":-1}"#,
        None,
        Some("application/json"),
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(400));
}

// --- 产品级加强波次：token_set / reset_counters ---

/// `state_json` 报 `token_set=true`，且**绝不回显令牌明文**。
#[test]
fn state_json_reports_token_set_without_echoing_plaintext() {
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"token": "cfg-secret"}
        }}}),
    );
    ctx.mod_registry.lock().unwrap().start_all();
    let state = ctx
        .mod_registry
        .lock()
        .unwrap()
        .runtime_state("external-input")
        .expect("runtime 已 start，state_json 可读");
    assert_eq!(state["token_set"], serde_json::json!(true));
    let rendered = state.to_string();
    assert!(
        !rendered.contains("cfg-secret"),
        "state_json 不得回显令牌明文：{rendered}"
    );
}

/// `command reset_counters`（面板「重置计数」的真实路径）：返回清零前快照
/// 并把 handler 记的账清零。命令经**真实注册表**转达，与
/// `POST /api/v1/mods/external-input/command` 同一条路。
#[test]
fn reset_counters_command_clears_handler_counts() {
    let _guard = counter_lock();
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{"enabled": true}}}),
    );
    ctx.mod_registry.lock().unwrap().start_all();
    let inject = |_ctx: &ServerContext, _t: String| -> Option<bool> { Some(true) };
    let resp = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi","v2_ignored":5}"#),
        &inject,
        &no_env_lookup,
    )
    .unwrap();
    assert_eq!(resp.status_code(), StatusCode(200));

    let before = ctx
        .mod_registry
        .lock()
        .unwrap()
        .runtime_state("external-input")
        .expect("state 可读");
    assert!(
        before["accepts"].as_u64().unwrap_or(0) >= 1,
        "至少记了 1 次接受"
    );
    assert_eq!(before["v2_ignored"], 5);

    let result = ctx
        .mod_registry
        .lock()
        .unwrap()
        .command("external-input", "reset_counters", &serde_json::json!({}))
        .expect("reset_counters 必须被支持");
    assert_eq!(result["reset"], serde_json::json!(true));
    assert_eq!(result["before"]["accepts"], before["accepts"]);
    assert_eq!(result["before"]["v2_ignored"], 5);

    let after = ctx
        .mod_registry
        .lock()
        .unwrap()
        .runtime_state("external-input")
        .expect("state 可读");
    for k in ["accepts", "rejects", "busy", "v2_ignored"] {
        assert_eq!(after[k], serde_json::json!(0), "{k} 清零");
    }
}
