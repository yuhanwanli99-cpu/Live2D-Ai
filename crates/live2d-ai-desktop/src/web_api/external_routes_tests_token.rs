//! `external_routes` `.env` / 模板膨胀 / `check_token` 回归。
//!
//! 脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;

/// **F-0062-01 端到端**：`POST /api/v1/mods/external-input/config` 只提交
/// 非 secret 字段（= 前端表单的形状：它从 GET 的**脱敏**配置出发，本来就
/// 拿不到 token）后，已存的 token 必须仍在 → 注入端点**继续**要 token。
///
/// 守的是「界面回成功、鉴权却静默失效」：旧实现整份替换配置，保存一个普通
/// 字段就把 token 删了，端点悄悄从「要鉴权」退回「不鉴权」。
#[test]
fn mod_config_save_without_secret_keeps_endpoint_auth() {
    let _guard = counter_lock(); // 401 会计 reject → 与计数回归串行。
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"token": "cfg-secret", "prefix": ""}
        }}}),
    );
    // 保存：只带非 secret 字段（token 缺席）。
    let saved = crate::web_api::mods_routes::handle_mods_route(
        &ctx,
        &method_post(),
        "/api/v1/mods/external-input/config",
        r#"{"config":{"prefix":"[弹幕] "}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .expect("mods 路由应命中");
    // 2026-10-09：配置**写盘同步完成**，但紧随其后的 Mod restart 挪到后台
    // （不堵接受循环）⇒ 受理态是 202。配置本身已经落内存（下面立刻就读到了）。
    assert_eq!(saved.status_code(), StatusCode(202), "保存应被受理");
    // 1) 配置里 token 仍在、prefix 已更新。
    let cfg = ctx
        .mod_registry
        .lock()
        .unwrap()
        .config("external-input")
        .cloned()
        .expect("external-input 在册");
    assert_eq!(
        live2d_ai_mod_external_input::token_from_config(&cfg),
        Some("cfg-secret".to_string()),
        "保存非 secret 字段不得抹掉已存密钥：{cfg}"
    );
    assert_eq!(cfg["prefix"], serde_json::json!("[弹幕] "));
    // 2) 鉴权仍生效：不带 token 的注入请求 → 401（不是「静默放行」）。
    let injected = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi"}"#),
        &accept_inject,
        &no_env_lookup,
    )
    .expect("external chat 路由应命中");
    assert_eq!(
        injected.status_code(),
        StatusCode(401),
        "密钥还在 ⇒ 端点必须继续要 token"
    );
}

/// **F-0062-01（另一半，端到端）**：把**已声明 secret** 的键显式置空串 → 密钥
/// **真的被删**，注入端点随之回到「不鉴权」——**预期行为**，不是缺陷。与上一条
/// 「留空 = 保留」互为两个方向，缺一半就会被读成「密钥永远删不掉」。
#[test]
fn explicit_blank_config_clears_token_and_opens_the_token_gate() {
    let _guard = counter_lock(); // 200 会计 accept → 与计数回归串行。
    let ctx = ctx_with_manifest(
        crate::web_api::security::SecurityContext::new(18099, true),
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true, "config": {"token": "cfg-secret"}
        }}}),
    );
    // 显式清除：只发 `""`（工具客户端 / curl 的删除语法）。
    let saved = crate::web_api::mods_routes::handle_mods_route(
        &ctx,
        &method_post(),
        "/api/v1/mods/external-input/config",
        r#"{"config":{"token":""}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .expect("mods 路由应命中");
    assert_eq!(
        saved.status_code(),
        StatusCode(202),
        "保存应被受理（restart 在后台）"
    );
    let cfg = ctx
        .mod_registry
        .lock()
        .unwrap()
        .config("external-input")
        .cloned()
        .unwrap();
    assert!(
        cfg.get("token").is_none(),
        "显式空串应删除 secret 键：{cfg}"
    );
    // 闸门随之打开：不带 token 的请求不再 401（= 预期的「不鉴权」）。
    let opened = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi"}"#),
        &accept_inject,
        &no_env_lookup,
    )
    .expect("external chat 路由应命中");
    assert_eq!(
        opened.status_code(),
        StatusCode(200),
        "清除后端点应回「不鉴权」并放行"
    );
}

/// **F-0062-01 / 语义边界**：**非** secret 键的显式空串**照存**（它是字段值，
/// 不是删除语法）——与 `PUT /api/v1/env` 的「空值 = 清除整键」刻意区分。
#[test]
fn blank_non_secret_string_is_stored_not_cleared() {
    let ctx = ctx_with_manifest(
        crate::web_api::security::SecurityContext::new(18099, true),
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"token": "cfg-secret", "text_template": "[T]{text}"}
        }}}),
    );
    let saved = crate::web_api::mods_routes::handle_mods_route(
        &ctx,
        &Method::Post,
        "/api/v1/mods/external-input/config",
        r#"{"config":{"text_template":""}}"#,
        Some("http://127.0.0.1:18099"),
        Some("application/json"),
    )
    .expect("mods 路由应命中");
    assert_eq!(saved.status_code(), StatusCode(202));
    let reg = ctx.mod_registry.lock().unwrap();
    let cfg = reg.config("external-input").unwrap();
    assert_eq!(
        cfg.get("text_template").and_then(|v| v.as_str()),
        Some(""),
        "非 secret 空串必须照存（不是删除）：{cfg}"
    );
    assert!(
        cfg.get("token").is_some(),
        "同一次保存不得顺手抹掉 secret：{cfg}"
    );
}

/// W6 回归（症状④ E5）：令牌**只写在 `.env`**（注入 lookup 有值、进程环境
/// 没有）时也读得到；且既有优先级链不变：`.env`/env > Mod config > 不鉴权。
#[test]
fn dotenv_token_is_read_and_still_wins_over_config() {
    let _guard = counter_lock(); // 被压过的 config token → 401，计 reject。
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"token": "cfg-secret"}
        }}}),
    );
    // 注入「值只在 .env」的 lookup：进程环境没有这个变量。
    let dotenv = |name: &str| (name == TOKEN_ENV_VAR).then(|| "dotenv-secret".to_string());

    let ok = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi","token":"dotenv-secret"}"#),
        &accept_inject,
        &dotenv,
    )
    .unwrap();
    assert_eq!(
        ok.status_code(),
        StatusCode(200),
        "`.env` 里的令牌必须被读到（过鉴权后经注入点接受）"
    );

    // 优先级链不变：`.env`（env 档）压过 Mod config。
    let shadowed = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi","token":"cfg-secret"}"#),
        &accept_inject,
        &dotenv,
    )
    .unwrap();
    assert_eq!(
        shadowed.status_code(),
        StatusCode(401),
        "env 档优先时，Mod config 的令牌不再作为有效令牌"
    );
}

// --- 模板膨胀后的长度门禁 ---

#[test]
fn template_inflated_text_too_long_400() {
    let sec = crate::web_api::security::SecurityContext::new(18099, true);
    let ctx = ctx_with_manifest(
        sec,
        &serde_json::json!({"mods":{"external-input":{
            "enabled": true,
            "config": {"prefix": "x".repeat(MAX_TEXT_LEN + 5)}
        }}}),
    );
    let resp = handle_external_chat_with(
        &ctx,
        req(&method_post(), r#"{"text":"hi"}"#),
        &accept_inject,
        &no_env_lookup,
    )
    .unwrap();
    assert_eq!(
        resp.status_code(),
        StatusCode(400),
        "渲染后超长同样应 400 text_too_long"
    );
}

// --- 可选 token 鉴权（纯函数，无 env 副作用） ---

#[test]
fn check_token_no_env_always_ok() {
    assert!(check_token(None, None, None));
    assert!(check_token(Some("x"), None, None));
    assert!(check_token(None, Some("Bearer x"), None));
    assert!(check_token(Some("x"), Some("Bearer y"), None));
}

#[test]
fn check_token_body_match_passes() {
    assert!(check_token(Some("secret"), None, Some("secret")));
}

#[test]
fn check_token_body_mismatch_fails() {
    assert!(!check_token(Some("wrong"), None, Some("secret")));
}

#[test]
fn check_token_missing_fails() {
    assert!(!check_token(None, None, Some("secret")));
}

#[test]
fn check_token_bearer_match_passes() {
    assert!(check_token(None, Some("Bearer secret"), Some("secret")));
}

#[test]
fn check_token_bearer_mismatch_fails() {
    assert!(!check_token(None, Some("Bearer wrong"), Some("secret")));
    assert!(!check_token(None, Some("wrong"), Some("secret")));
}

#[test]
fn bearer_token_extracts_correctly() {
    assert_eq!(bearer_token(Some("Bearer abc")), Some("abc"));
    assert_eq!(bearer_token(Some("bearer abc")), Some("abc"));
    assert_eq!(bearer_token(Some("abc")), None);
    assert_eq!(bearer_token(Some("Bearer ")), None);
    assert_eq!(bearer_token(None), None);
}

#[test]
fn check_token_body_takes_priority_over_mismatched_bearer() {
    assert!(check_token(
        Some("secret"),
        Some("Bearer wrong"),
        Some("secret")
    ));
}
