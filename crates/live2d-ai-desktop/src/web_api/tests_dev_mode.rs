//! W7 任务「dev_mode 端到端」测试（拆分承载：web_api/mod.rs 已贴 500
//! 上限；本批新增 6 条 dev_mode 端到端测试单独提到本文件以保留主体可扩展性）。
//!
//! 覆盖闭环：
//! 1. CLI flag / settings 派生优先级（[`StatusContext::new`] 注入语义）
//! 2. PATCH `/api/v1/settings` 切 dev_mode → [`StatusContext::dev_mode()`]
//!    跟随翻转
//! 3. 日志端点门控 403↔200 翻转（`dev_mode=false` PATCH 切 true 后
//!    GET `/api/v1/logs` 由 403 → 200）
//!
//! 入口：`web_api::mod tests_dev_mode;`（仅 `#[cfg(test)]` 下生效）——
//! 本文件作为 `web_api` 的子模块，可直接 `use super::*;` 访问私有项。

use std::io::Read;

use live2d_ai_runtime::AppSettings;
use tiny_http::{Method, Response};

use super::*;

/// 把 Response body 拉成 String（与既有 `tests_reload` 模块同实现）。
fn body_to_string(resp: Response<std::io::Cursor<Vec<u8>>>) -> String {
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    s
}

/// 起一个最小 ServerContext 指向 tmp 配置 + 写一份最小合法 TOML。
///
/// **路径唯一性**：cargo test 默认多线程并发跑测试；同 pid 多 test 共用
/// 路径会撞 tmp（`plan_atomic_write` 也会撞）。本函数用 caller 给的 `name`
/// 拼接 `{pid}-{name}-{thread_id}` 区分；线程 id 由
/// `std::thread::current().id()` 给出。
fn tmp_ctx_with_settings(name: &str, settings: AppSettings) -> (ServerContext, std::path::PathBuf) {
    use std::hash::{Hash, Hasher};
    let thread_id = format!("{:?}", std::thread::current().id());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    thread_id.hash(&mut hasher);
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_dev_mode_{}_{}_{}.toml",
        std::process::id(),
        name,
        hasher.finish()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, settings.to_toml_string()).expect("write initial");
    let ctx = ServerContext::new(settings, tmp.to_string_lossy().into_owned(), None);
    (ctx, tmp)
}

// ============================================================================
// 1) StatusContext 初始化：CLI override > settings 派生 > 默认 false
// ============================================================================

#[test]
fn status_context_init_uses_settings_when_no_override() {
    let s = AppSettings {
        dev_mode: true,
        ..Default::default()
    };
    let ctx = StatusContext::new(s, "/tmp/cfg.toml".into(), None);
    assert!(ctx.dev_mode(), "settings.dev_mode=true 必须派生到 ctx");
}

#[test]
fn status_context_init_cli_override_wins_over_settings() {
    // settings = false, CLI = true → 最终 = true（CLI 覆盖）。
    let s = AppSettings::default(); // dev_mode=false
    let ctx = StatusContext::new(s, "/tmp/cfg.toml".into(), Some(true));
    assert!(ctx.dev_mode(), "CLI override=true 必须覆盖 settings=false");

    // settings = true, CLI = false → 最终 = false（CLI Some(false) 强制覆盖）。
    let s = AppSettings {
        dev_mode: true,
        ..Default::default()
    };
    let ctx = StatusContext::new(s.clone(), "/tmp/cfg.toml".into(), None);
    assert!(
        ctx.dev_mode(),
        "无 override 时 settings.dev_mode=true 必须生效"
    );
    // CLI 显式 false → 强制 false（覆盖 settings=true）。
    let ctx = StatusContext::new(s, "/tmp/cfg.toml".into(), Some(false));
    assert!(
        !ctx.dev_mode(),
        "CLI Some(false) 强制关闭（覆盖 settings=true）"
    );
}

#[test]
fn status_context_default_is_off() {
    // settings 默认 + 无 override → ctx.dev_mode = false。
    let ctx = StatusContext::new(AppSettings::default(), "/tmp/cfg.toml".into(), None);
    assert!(!ctx.dev_mode(), "默认 dev_mode = false");
}

// ============================================================================
// 2) PATCH /api/v1/settings 切 dev_mode → StatusContext 跟随翻转
// ============================================================================

#[test]
fn patch_dev_mode_true_updates_status_context() {
    // 起点：dev_mode=false，logs 端点 403。
    let (ctx, tmp) = tmp_ctx_with_settings("devmode_true", AppSettings::default());
    assert!(!ctx.status_ctx.dev_mode());
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(resp.status_code().0, 403, "起点 dev_mode=false 必须 403");

    // PATCH 切 dev_mode=true。
    let body = r#"{"dev_mode":true}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(
        resp.status_code().0,
        200,
        "PATCH 200；body={}",
        body_to_string(resp)
    );

    // StatusContext.dev_mode 必须已翻成 true（PATCH 写盘后热同步）。
    assert!(
        ctx.status_ctx.dev_mode(),
        "PATCH dev_mode=true 后 StatusContext.dev_mode() 必须为 true"
    );

    // 文件落盘也确认。
    let on_disk = std::fs::read_to_string(&tmp).expect("read after");
    assert!(
        on_disk.contains("dev_mode = true"),
        "settings 文件应含 dev_mode = true（实际 = {on_disk}）"
    );

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn patch_dev_mode_false_flips_logs_back_to_403() {
    // 起点：dev_mode=true（CLI 注入），logs 端点 200。
    use std::hash::{Hash, Hasher};
    let s = AppSettings::default();
    let thread_id = format!("{:?}", std::thread::current().id());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    thread_id.hash(&mut hasher);
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_dev_mode_off_{}_{}.toml",
        std::process::id(),
        hasher.finish()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, s.to_toml_string()).expect("write initial");

    let ctx = ServerContext::new(s, tmp.to_string_lossy().into_owned(), Some(true));
    assert!(ctx.status_ctx.dev_mode(), "CLI 注入 dev_mode=true");
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(resp.status_code().0, 200, "CLI=true 起点 logs 必须 200");

    // PATCH 切 dev_mode=false（SettingsPatch.dev_mode: Some(Some(false))）。
    let body = r#"{"dev_mode":false}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(resp.status_code().0, 200);

    // 同步：StatusContext.dev_mode 翻成 false。
    assert!(
        !ctx.status_ctx.dev_mode(),
        "PATCH dev_mode=false 后 StatusContext.dev_mode() 必须为 false"
    );

    // 日志端点门控：再 GET → 必须回到 403。
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(
        resp.status_code().0,
        403,
        "PATCH 切 false 后 logs 必须回到 403"
    );
    let body = body_to_string(resp);
    assert!(body.contains("dev_mode_required"), "got: {body}");

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn patch_no_op_does_not_change_dev_mode() {
    // 起点：dev_mode=true，PATCH 一个无关字段（llm.model），
    // StatusContext.dev_mode 仍为 true（PATCH 写盘后未变则不调 set_dev_mode）。
    let s = AppSettings {
        llm: live2d_ai_runtime::settings::LlmSettings {
            base_url: "http://x/v1".into(),
            model: "m".into(),
            api_key_env: None,
            max_tokens: None,
        },
        dev_mode: true,
        ..Default::default()
    };
    let (ctx, tmp) = tmp_ctx_with_settings("noop", s);

    // 起点为 true，PATCH llm.model 不影响 dev_mode 字段。
    assert!(ctx.status_ctx.dev_mode());
    let body = r#"{"llm":{"model":"patched"}}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(resp.status_code().0, 200);
    assert!(ctx.status_ctx.dev_mode(), "PATCH 无关字段不翻转 dev_mode");
    // 日志端点仍 200（dev_mode 未变）。
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(resp.status_code().0, 200);

    let _ = std::fs::remove_file(&tmp);
}

// ============================================================================
// 3) 端到端：dev_mode=false PATCH 切 true 后 logs 端点 403→200 翻转
// ============================================================================

#[test]
fn end_to_end_logs_endpoint_flips_403_to_200_after_patch() {
    // 完整端到端：起点 403 → PATCH dev_mode=true → 立即 200。
    let (ctx, tmp) = tmp_ctx_with_settings("e2e", AppSettings::default());

    // 起点 logs 403（dev_mode=false）。
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(resp.status_code().0, 403, "起点 dev_mode=false → logs 403");
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs/levels", "");
    assert_eq!(resp.status_code().0, 403, "logs/levels 起点也 403");

    // PATCH dev_mode=true。
    let body = r#"{"dev_mode":true}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(resp.status_code().0, 200);

    // 验证两端点都翻 200。
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(
        resp.status_code().0,
        200,
        "PATCH 切 true 后 logs 必须 200（无重启）"
    );
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs/levels", "");
    assert_eq!(
        resp.status_code().0,
        200,
        "PATCH 切 true 后 logs/levels 必须 200（无重启）"
    );

    // app/status 也得翻（dto 字段）。
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/app/status", "");
    let body = body_to_string(resp);
    assert!(
        body.contains("\"dev_mode\":true"),
        "app/status 必须含 dev_mode=true（实际 = {body}）"
    );

    // 切回 false：再 403。
    let body = r#"{"dev_mode":false}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(resp.status_code().0, 200);
    let resp = dispatch(&ctx, &Method::Get, "/api/v1/logs", "");
    assert_eq!(
        resp.status_code().0,
        403,
        "PATCH 切回 false 后 logs 必须 403"
    );

    let _ = std::fs::remove_file(&tmp);
}

// ============================================================================
// 4) 边界：PATCH dev_mode=null / 缺省 / 无效值 / 与其它段混用
// ============================================================================

#[test]
fn patch_dev_mode_null_is_explicit_false() {
    // `{"dev_mode":null}` = Some(None) = 显式关闭（与缺省 None 不同）。
    let s = AppSettings {
        dev_mode: true,
        ..Default::default()
    };
    let (ctx, tmp) = tmp_ctx_with_settings("null", s);
    assert!(ctx.status_ctx.dev_mode());

    let body = r#"{"dev_mode":null}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(resp.status_code().0, 200);
    assert!(
        !ctx.status_ctx.dev_mode(),
        "dev_mode:null = 显式关（与缺省不修改不同）"
    );

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn patch_dev_mode_absent_does_not_touch_dev_mode() {
    // `{"llm":{"model":"x"}}` 不动 dev_mode 字段。
    let s = AppSettings {
        llm: live2d_ai_runtime::settings::LlmSettings {
            base_url: "http://x/v1".into(),
            ..Default::default()
        },
        dev_mode: true,
        ..Default::default()
    };
    let (ctx, tmp) = tmp_ctx_with_settings("absent", s);
    assert!(ctx.status_ctx.dev_mode());

    let body = r#"{"llm":{"model":"patched"}}"#;
    let resp = dispatch(&ctx, &Method::Patch, "/api/v1/settings", body);
    assert_eq!(resp.status_code().0, 200);
    assert!(
        ctx.status_ctx.dev_mode(),
        "dev_mode 缺省 = 不修改（保留 true）"
    );
    // 同步断言：文件里 model 改了但 dev_mode 仍是 true。
    let on_disk = std::fs::read_to_string(&tmp).expect("read");
    assert!(on_disk.contains("dev_mode = true"), "{on_disk}");

    let _ = std::fs::remove_file(&tmp);
}
