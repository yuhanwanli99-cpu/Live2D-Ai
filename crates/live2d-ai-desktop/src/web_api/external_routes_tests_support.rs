//! `external_routes` 测试共享脚手架（自内联 `mod tests` 拆出）。
//!
//! 仅供同目录 `external_routes_tests_gate` / `_token` / `_handler` 使用：
//! 请求构造、注入点替身、计数串行锁、最小 `ServerContext`。
//! `pub(super)` 只为兄弟测试模块可见性，不进产品 API。

use super::*;

pub(super) fn method_post() -> Method {
    Method::Post
}

/// 一次调用（省略 auth 头 = None）。
pub(super) fn call(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    ct: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    handle_external_chat(ctx, method, path, body, origin, ct, None)
}

/// 测试用密钥查找口径：模拟「进程环境 / `.env` 都没有该令牌」。
/// 生产路径的 lookup 是 `secrets::lookup`（见 `handle_external_chat`）。
pub(super) fn no_env_lookup(_name: &str) -> Option<String> {
    None
}

/// 恒接受的注入点（只走 token / 门禁分支的回归用）。
pub(super) fn accept_inject(_ctx: &ServerContext, _text: String) -> Option<bool> {
    Some(true)
}

/// 计数器是进程级单例 → 会改计数的测试串行，delta 断言才确定。
pub(super) fn counter_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
/// 构造一个「POST + JSON + 无 Origin / 无 auth」的注入请求（计数回归用）。
pub(super) fn req<'a>(method: &'a Method, body: &'a str) -> ExternalRequest<'a> {
    ExternalRequest {
        method,
        path: EXTERNAL_CHAT_PATH,
        body,
        origin: None,
        content_type: Some("application/json"),
        auth_header: None,
    }
}
// --- test helpers ---

pub(super) fn dummy_ctx(sec: crate::web_api::security::SecurityContext) -> ServerContext {
    ServerContext {
        status_ctx: std::sync::Arc::new(crate::web_api::app_routes::StatusContext::new(
            live2d_ai_runtime::AppSettings::default(),
            ".scratch".to_string(),
            None,
        )),
        capabilities_ctx: std::sync::Arc::new(
            crate::web_api::app_routes::CapabilitiesContext::new(),
        ),
        supervisor_slot: crate::web_api::supervisor_slot::SupervisorSlot::new(),
        broadcaster: crate::web_api::ws::Broadcaster::new(),
        security: sec,
        models: crate::web_api::models_routes::default_store(),
        mod_registry: std::sync::Arc::new(std::sync::Mutex::new(
            crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({})),
        )),
    }
}

/// 用真实工厂表 + 指定 manifest 装配注册表（启停门禁测试用）。
pub(super) fn ctx_with_manifest(
    sec: crate::web_api::security::SecurityContext,
    manifest: &serde_json::Value,
) -> ServerContext {
    let mut ctx = dummy_ctx(sec);
    ctx.mod_registry = std::sync::Arc::new(std::sync::Mutex::new(
        crate::mod_registry::ModRegistry::new(crate::AVAILABLE_MOD_FACTORIES, manifest),
    ));
    ctx
}
