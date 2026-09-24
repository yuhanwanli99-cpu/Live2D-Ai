//! `voice_routes` 的令牌来源回归（W6：`.env` 快照 > 进程环境）。
//!
//! 挂载见 `voice_routes_tests.rs` 末尾的 `#[path]`；`use super::*` 拿到主测试
//! 模块的 helper（`ctx_with_manifest` / `with_wake` / `WAKE` …）。

use super::*;

/// W6 回归（症状④ E5）：令牌**只写在 `.env`**（注入 lookup 有值、进程环境
/// 没有）时也读得到；且既有优先级链不变：`.env`/env > Mod config > 不鉴权。
///
/// 用注入的 [`LookupFn`] 构造，**不**用 `std::env::set_var`（并发下不可靠，
/// 且本项目已禁止）。
#[test]
fn dotenv_token_is_read_and_still_wins_over_config() {
    let ctx = ctx_with_manifest(&serde_json::json!({
        "mods": {"voice-input": {"enabled": true, "config": {
            "token": "cfg-secret",
            "wake_phrase": WAKE
        }}}
    }));
    // 注入「值只在 .env」的 lookup：进程环境没有这个变量。
    let dotenv = |name: &str| (name == TOKEN_ENV_VAR).then(|| "dotenv-secret".to_string());

    let ok = handle_voice_transcript_with(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("你好"), "token": "dotenv-secret"}).to_string(),
        None,
        Some("application/json"),
        None,
        &dotenv,
    )
    .unwrap();
    assert_eq!(
        ok.status_code(),
        StatusCode(503),
        "`.env` 里的令牌必须被读到（过鉴权 + 闸门后止于 supervisor 未就绪）"
    );

    // 优先级链不变：`.env`（env 档）压过 Mod config。
    let shadowed = handle_voice_transcript_with(
        &ctx,
        &Method::Post,
        VOICE_TRANSCRIPT_PATH,
        &serde_json::json!({"text": with_wake("你好"), "token": "cfg-secret"}).to_string(),
        None,
        Some("application/json"),
        None,
        &dotenv,
    )
    .unwrap();
    assert_eq!(
        shadowed.status_code(),
        StatusCode(401),
        "env 档优先时，Mod config 的令牌不再作为有效令牌"
    );
}
