//! HTTP 派发（method+path → RouteId → handler；D-P0B 安全校验 + D-P0C
//! 动态装配 supervisor）。
//!
//! 拆分承载：`mod.rs` 主体 ≤ 500；本文件集中 `dispatch` / `dispatch_with_security`
//! 派发循环 + SettingsPatch 副作用。
//!
//! 关键 D-P0C 行为：
//! - `RouteId::SettingsPatch` 200 写盘成功 → 调 `ctx.ensure_supervisor_after_patch`
//!   动态装配 / reload，把 [`ApplyStatus`] 写进响应 body；
//! - `RouteId::ChatPost` / `ChatStop` 从 `supervisor_slot` 借 Arc；
//!   槽位空时 chat 端点自然回 503（chat 路由自身文案）。

use std::io::Cursor;

use tiny_http::{Method, Response};

use crate::web_api::app_routes::{handle_capabilities, handle_status};
use crate::web_api::log_routes::{handle_logs, handle_logs_levels};
use crate::web_api::responses::{
    forbidden_logs as forbidden_logs_response, not_found as not_found_response,
    not_implemented as not_implemented_response,
};
use crate::web_api::settings_routes::{
    TestBody, handle_get, handle_patch, handle_test_llm, handle_test_tts,
};

use super::{ApplyStatus, RouteId, ServerContext, match_route};

/// 派发：method+path → RouteId → handler（**不带**安全校验）。
///
/// 历史单测兼容层：`tests_dev_mode.rs` / `tests_mod.rs` 等不传 Origin/CT
/// 头，本函数临时把 `ctx.security.allow_no_origin` 翻成 `true`（`None`
/// origin + `allow_no_origin=true` 同时跳过 Origin 与 Content-Type 校验，
/// 视作工具模式），再调 [`dispatch_with_security`]。**生产路径必须**经
/// [`dispatch_with_security`]。
#[allow(dead_code)] // 保留供历史单测；非 cfg(test) 下未直接调用。
pub fn dispatch(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body_str: &str,
) -> Response<Cursor<Vec<u8>>> {
    // 临时 clone 一份 ctx，翻 allow_no_origin=true，避免污染共享状态。
    let mut temp_ctx = ctx.clone();
    temp_ctx.security.allow_no_origin = true;
    dispatch_with_security(&temp_ctx, method, path, body_str, None, None)
}

/// 派发：method+path → RouteId → handler（含 Origin/Content-Type 安全校验）。
///
/// **生产入口**：`run_request_loop` 抽 `Origin` / `Content-Type` 后调本函数。
/// mutating 端点先 [`super::security::check_mutating_request`]，失败回 403
/// 短路放行；GET / 只读端点不做 Origin 校验（CSRF 风险为 0）。
pub fn dispatch_with_security(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body_str: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Response<Cursor<Vec<u8>>> {
    let route = match_route(method, path);
    // D-P0B：mutating 端点先做安全校验（Origin + Content-Type）。
    // 只读端点（GET/NotFound 等）跳过——无 CSRF 风险。
    if crate::web_api::security::is_mutating_route(route, method) {
        let body_present = !body_str.is_empty();
        if let Err(err) = crate::web_api::security::check_mutating_request(
            origin,
            content_type,
            &ctx.security,
            body_present,
        ) {
            // 2026-09-11：从 `debug!` 提到 `warn!`——这是一次**会到用户脸上的
            // 失败**（403），默认 level=info 的日志里必须看得见。前端把 403 当
            // 普通报错显示，日志若也静默，就没人能解释那个 403 是怎么来的。
            tracing::warn!(
                code = err.as_code(),
                status = err.as_status(),
                path = %path,
                origin = ?origin,
                content_type = ?content_type,
                body_present,
                "mutating 请求被安全校验拒绝"
            );
            return crate::web_api::security::mutating_check_error_response(&err);
        }
    }
    let resp = match route {
        RouteId::AppCapabilities => handle_capabilities(),
        RouteId::AppStatus => {
            // `active_model_id` 从**真实 registry** 读（rc.2 2026-09-12 之前写死
            // `"bai_001"`，所以「激活了但状态栏不变」是必然的）。
            let model_id = crate::web_api::models_routes::active_model_id(&ctx.models);
            handle_status(&ctx.status_ctx, model_id)
        }
        RouteId::Env => {
            // GET = 读键名 + 是否已设置（永不回显值）；PUT = 写 `.env` + 热重载。
            // 重载走 `ensure_supervisor_after_patch` 的同一套语义：写盘成功后让
            // supervisor 重建 LLM/TTS client，新 key **不重启**即生效。
            let s = ctx.status_ctx.settings_snapshot();
            match *method {
                Method::Get => crate::web_api::env_routes::handle_get(&s),
                Method::Put => match crate::web_api::env_routes::handle_put(&s, method, body_str) {
                    Ok(resp) => {
                        ctx.ensure_supervisor_after_patch();
                        resp
                    }
                    Err(resp) => resp,
                },
                _ => crate::web_api::env_routes::method_not_allowed(),
            }
        }
        RouteId::SettingsGet => {
            let s = ctx.status_ctx.settings_snapshot();
            handle_get(&s)
        }
        RouteId::SettingsPatch => {
            let s = ctx.status_ctx.settings_snapshot();
            // D-P0C：dispatch 传 `patch_after_hook` 闭包，handler 在写盘成功
            // 后回调，让 dispatch **拿回** `apply_status` 并回填到响应。
            // 这样 PATCH 响应体与「真实生效路径」一一对应，前端按状态显示
            // 正确文案（避免「已热重载」与「实际 503」错位）。
            let config_path = ctx.status_ctx.config_path.clone();
            let hook: Box<dyn Fn() -> ApplyStatus> =
                Box::new(|| ctx.ensure_supervisor_after_patch());
            let resp = handle_patch(&s, body_str, &config_path, Some(&*hook));
            // 仅在 PATCH 真正写盘成功（HTTP 200）后才触发 supervisor 重载。
            // 校验失败（400）或磁盘错误（500）→ 不通知 supervisor（保留旧
            // engine，避免「用户改一半」也被吞掉）。仅 200 时刷新
            // StatusContext 内的 settings 视图——失败时 GET 仍能拿到旧值。
            if resp.status_code().0 == 200 {
                // W7：`dev_mode` 跟随 settings 翻转（PATCH 写盘后立即生效；
                // 日志端点门控取 StatusContext.dev_mode()）。与文件监听器
                // 那条外部修改路径**共用同一个** `refresh_from_disk`，避免
                // 「两条更新路径各写一份」而语义漂移。
                if ctx.status_ctx.refresh_from_disk(&config_path) {
                    tracing::debug!(path = %config_path, "PATCH 后已刷新设置快照");
                }
                // 注意：apply_status 已在 hook 内计算并写入 resp body；此处
                // 不再调 `request_reload`（hook 内部已处理动态装配 + reload）。
            }
            resp
        }
        RouteId::SettingsTestLlm => {
            let s = ctx.status_ctx.settings_snapshot();
            let parsed: TestBody = serde_json::from_str(body_str).unwrap_or_default();
            handle_test_llm(&s, &parsed)
        }
        RouteId::SettingsTestTts => {
            let s = ctx.status_ctx.settings_snapshot();
            let parsed: TestBody = serde_json::from_str(body_str).unwrap_or_default();
            handle_test_tts(&s, &parsed)
        }
        // W2：HTTP 对话入口（chat / chat/stop）。
        // P1WS-1：epoch 由 chat_routes 内部从 SupervisorHandle 读取真实值
        // （`handle.current_epoch()`）；此处仅传占位 0 维持旧签名兼容。
        // D-P0C：从 `supervisor_slot` 借出 Arc（读锁短暂持锁即释放）；
        // 槽位空时 = 503（chat 路由自身的 no_supervisor 文案）。
        RouteId::ChatPost => crate::web_api::chat_routes::handle_chat_with_supervisor(
            ctx.try_get_supervisor().as_ref(),
            method,
            path,
            body_str,
            0,
        ),
        RouteId::ChatStop => crate::web_api::chat_routes::handle_stop_with_supervisor(
            ctx.try_get_supervisor().as_ref(),
            method,
            path,
        ),
        // 日志相关端点：dev_mode=true 时开放（StatusContext 持有开关）。
        // 关闭时一律 403（不动 settings 内容、也不暴露日志文件存在性）。
        RouteId::LogsGet => {
            if ctx.status_ctx.dev_mode() {
                handle_logs()
            } else {
                forbidden_logs_response()
            }
        }
        RouteId::LogsLevels => {
            if ctx.status_ctx.dev_mode() {
                handle_logs_levels()
            } else {
                forbidden_logs_response()
            }
        }
        // D3（2026-08-28）：模型资产路由。所有子路由在
        // `models_routes::dispatch` 内按 method+path 细分。
        RouteId::Models => {
            crate::web_api::models_routes::dispatch(&ctx.models, method, path, body_str)
        }
        // D4（2026-08-29）的动作命令端点已于 2026-09-11 整体删除（用户裁决：
        // LLM 不暴露任何工具、只做对话）——`/api/v1/commands*` 现在落回
        // `RouteId::NotImplemented`（501）。
        RouteId::NotImplemented => not_implemented_response(route.as_str()),
        RouteId::NotFound => not_found_response(path),
    };
    log_request_outcome(method, path, route, &resp);
    resp
}

/// 请求级日志（2026-09-11 补）。
///
/// # 为什么必须有
///
/// 用户报「后端出错无具体错误代码；看后端日志，什么都没有」。查证：本函数
/// 所在模块此前**一行请求日志都没有**——成功的请求不可见（改了什么没人知道），
/// 失败的请求也不可见（400/500 的 code 只写进了 HTTP 响应体，落盘为零）。
/// 于是「日志不够细」不是感觉，是事实。
///
/// # 分级口径（与默认 `info` 落盘配合）
///
/// - `>= 500` → `error`：真故障；
/// - `>= 400` → `warn`：用户能在界面上看到的拒绝（含 403 安全校验）；
/// - **mutating 成功**（PATCH/POST/PUT/DELETE）→ `info`：这是「我改过什么」的
///   审计轨迹——「改提示词」这一类操作从此在日志里留痕；
/// - 其余成功读取 → `debug`：GET 太密，不占默认档位（排障时开
///   `LIVE2D_AI_LOG_FILE=debug` 即可全量）。
///
/// 路径/方法/路由/状态码全是非敏感字段；**不记录请求体**（设置补丁里含提示词、
/// 端点地址等用户内容，且没有任何诊断问题需要它）。
fn log_request_outcome(
    method: &Method,
    path: &str,
    route: RouteId,
    resp: &Response<Cursor<Vec<u8>>>,
) {
    let status = resp.status_code().0;
    let route = route.as_str();
    if status >= 500 {
        tracing::error!(%method, path, route, status, "请求处理失败");
    } else if status >= 400 {
        tracing::warn!(%method, path, route, status, "请求被拒绝");
    } else if is_mutating_method(method) {
        tracing::info!(%method, path, route, status, "变更请求成功");
    } else {
        tracing::debug!(%method, path, route, status, "请求成功");
    }
}

/// 是否变更型方法（与 `security::is_mutating_route` 的方法口径一致）。
fn is_mutating_method(method: &Method) -> bool {
    matches!(
        method,
        Method::Post | Method::Patch | Method::Put | Method::Delete
    )
}
