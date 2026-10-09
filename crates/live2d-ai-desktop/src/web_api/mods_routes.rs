//! Mod 管理 HTTP API（节点 E5 补充，2026-08-30）。
//!
//! 职责：暴露 `GET /api/v1/mods`（带 `settings_spec` + `config`，rc.4 M2）、
//! `GET /api/v1/mods/{id}/config` 与 `POST /api/v1/mods/{id}/{action}`
//! （action ∈ enable/disable/restart/config/command），把 host 端 `ModRegistry` 的
//! 运行状态与可编辑配置透出给前端。
//!
//! `command`（产品级加强波次）是**一次性动作**通道（body
//! `{"command":"clear","args":{}}`）：与 `config` 的差别是它不改配置、
//! 也不重启 Mod，只让 Mod 执行一个有副作用的本地动作（如清空记忆库）。
//!
//! # 安全边界
//!
//! - GET /api/v1/mods 与 GET …/config：只读，Origin 校验不强制（CSRF 风险为 0）；
//!   **secret=true 的字段值永不出现在响应里**（`redacted_config`）。
//! - POST enable/disable/restart/config：mutating，需 loopback Origin；
//!   **仅当携带 body 时**才要求 application/json（`enable`/`disable` 是
//!   无 body 控制命令，浏览器 `fetch` 不带 Content-Type）——与
//!   [`crate::web_api::security::check_mutating_request`] 同一口径。
//!   校验复用 [`crate::web_api::security::is_allowed_origin`] /
//!   [`crate::web_api::security::is_json_content_type`] /
//!   [`crate::web_api::security::mutating_check_error_response`]。
//! - 监听本身仅 loopback（`start_server` 绑定 127.0.0.1）。
//! - ModError → 409 `{"error":{"code":"mod_error","message":...}}`；
//!   未知 id → 404 `{"error":{"code":"not_found","message":...}}`。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use live2d_ai_mod_system::{ModSettingField, ModSettingsSpec};

use crate::mod_registry::ModCommandError;
use crate::web_api::ServerContext;

/// 路由前缀。
const MODS_LIST_PATH: &str = "/api/v1/mods";

/// Mod 管理路由前缀（用于 starts_with 判定）。
const MODS_PREFIX: &str = "/api/v1/mods/";

/// 构造一个 JSON 响应（status + body；Content-Type application/json）。
fn ok_response(status: StatusCode, body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body.as_bytes().to_vec())
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
                .expect("Cache-Control header"),
        )
}

/// 通用 JSON 错误响应。
fn json_error(status: StatusCode, code: &str, message: String) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    ok_response(status, &body)
}

/// 处理 `/api/v1/mods*` 路由。
///
/// - 路径不属于本族 → `None`（交给后续 dispatch）。
/// - `GET /api/v1/mods` → 列举全部 Mod 状态。
/// - `POST /api/v1/mods/{id}/{action}`（action ∈ enable/disable/restart）→ 操作。
/// - 其它 `/api/v1/mods*` → 404。
pub fn handle_mods_route(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    // 非本族路径：交还 dispatch。
    if !path.starts_with(MODS_PREFIX) && path != MODS_LIST_PATH {
        return None;
    }

    // GET /api/v1/mods → 列举。
    if path == MODS_LIST_PATH {
        if *method == Method::Get {
            return Some(handle_mods_list(ctx));
        }
        // 其它方法 → 405。
        return Some(json_error(
            StatusCode(405),
            "method_not_allowed",
            "GET /api/v1/mods 仅支持 GET".to_string(),
        ));
    }

    // /api/v1/mods/{id}/{action}
    let rest = path.strip_prefix(MODS_PREFIX)?;
    let mut parts = rest.split('/');
    let id = parts.next()?;
    let action = parts.next()?;
    // id 不含 `/`、action 后不得再有额外段。
    if id.is_empty() || action.is_empty() || parts.next().is_some() {
        return Some(json_error(
            StatusCode(404),
            "not_found",
            "未知 mod 路由".to_string(),
        ));
    }

    // GET /api/v1/mods/{id}/config → 读取单个 Mod 配置（只读；secret 字段脱敏）。
    if action == "config" && *method == Method::Get {
        return Some(handle_mod_config_get(ctx, id));
    }

    // GET /api/v1/mods/{id}/state → 运行态快照（Wave 2）。
    if action == "state" && *method == Method::Get {
        return Some(handle_mod_state_get(ctx, id));
    }

    // 其余 action 仅 POST。
    if *method != Method::Post {
        return Some(json_error(
            StatusCode(405),
            "method_not_allowed",
            "POST /api/v1/mods/{id}/{action} 仅支持 POST".to_string(),
        ));
    }

    // Security：loopback Origin + application/json（**仅当携带 body 时**）。
    //
    // `enable` / `disable` 是**无 body 控制命令**：浏览器
    // `fetch('/api/v1/mods/{id}/enable', {method:'POST'})`（Flutter
    // `ModsApi.setEnabled` 正是这么发的）不带 Content-Type。旧实现无条件要
    // JSON CT → 前端「Mod 管理」的启停恒 415，等于整个 Mod 链路点不动。
    // 口径与 `security::check_mutating_request` 对齐：无 body 放行 CT，
    // Origin 才是真正的 CSRF 闸门（下一段仍严格校验）。
    if !body.is_empty() && !crate::web_api::security::is_json_content_type(content_type) {
        return Some(json_error(
            StatusCode(415),
            "unsupported_media_type",
            "Content-Type 必须为 application/json".to_string(),
        ));
    }
    if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) {
        let err = match origin {
            Some(o) => crate::web_api::security::MutatingCheckError::BadOrigin {
                got: Some(o.to_string()),
            },
            None => crate::web_api::security::MutatingCheckError::NoOrigin,
        };
        return Some(crate::web_api::security::mutating_check_error_response(
            &err,
        ));
    }

    // 执行操作：借出 id。
    // - "enable"：body 可选携带 `{"config":{...}}`，有则 enable_with_config。
    // - "config"：body 必须携带 `{"config":{...}}`，reload_config + (enabled 则 restart)。
    // enable/disable/restart 的 id 参数是 `&'static str`：Box::leak 转换。
    // 未知 id 在 registry 内部以 ModError::Other 返回，下文映射为 404。
    let id_static: &'static str = Box::leak(id.to_string().into_boxed_str());
    let mut registry = lock_registry(ctx);

    // "command" action（产品级加强波次）：一次性动作，不改配置、不重启 Mod。
    //
    // body：`{"command":"clear","args":{…}}`；`args` 可省（缺省 `{}`）。
    // 状态码：200 ok / 400 坏 body / 404 不在注册表 / 409 不支持或执行失败
    //（`unsupported_command` / `command_failed`）/ 503 未启用或正忙（可重试）。
    if action == "command" {
        if !registry.contains(id) {
            drop(registry);
            return Some(json_error(
                StatusCode(404),
                "not_found",
                format!("Mod {id} 不在注册表"),
            ));
        }
        let parsed = match serde_json::from_str::<serde_json::Value>(body) {
            Ok(v) => v,
            Err(e) => {
                drop(registry);
                return Some(json_error(
                    StatusCode(400),
                    "bad_request",
                    format!("无效 JSON body：{e}"),
                ));
            }
        };
        let command = parsed
            .get("command")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(command) = command else {
            drop(registry);
            return Some(json_error(
                StatusCode(400),
                "bad_request",
                "body 必须包含非空 \"command\" 字段".to_string(),
            ));
        };
        let args = parsed
            .get("args")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let result = registry.command(id, command, &args);
        drop(registry);
        return Some(match result {
            Ok(value) => ok_response(
                StatusCode(200),
                &serde_json::json!({"ok": true, "result": value}).to_string(),
            ),
            Err(ModCommandError::Unsupported(c)) => json_error(
                StatusCode(409),
                "unsupported_command",
                format!("Mod {id} 不支持命令：{c}"),
            ),
            Err(ModCommandError::Unavailable) => json_error(
                StatusCode(503),
                "command_unavailable",
                format!("Mod {id} 未启用或正忙，可重试"),
            ),
            Err(ModCommandError::Failed(m)) => json_error(
                StatusCode(409),
                "command_failed",
                format!("Mod {id} 命令失败：{m}"),
            ),
        });
    }

    // "config" action：**reload_config 仍然同步**（纯内存 + 一次原子写盘，
    // 毫秒级），只有随后的 restart 挪到后台线程——restart 会 stop + start，
    // 而 start 里那个「立刻退出」观察窗（以及首次建 venv）会拖住 tiny_http
    // 唯一的接受循环（2026-10-09 计划书 §4）。
    if action == "config" {
        // POST /api/v1/mods/{id}/config：body 必须 {"config":{...}}。
        let parsed = match serde_json::from_str::<serde_json::Value>(body) {
            Ok(v) => v,
            Err(e) => {
                drop(registry);
                return Some(json_error(
                    StatusCode(400),
                    "bad_request",
                    format!("无效 JSON body：{e}"),
                ));
            }
        };
        let Some(config) = parsed.get("config") else {
            drop(registry);
            return Some(json_error(
                StatusCode(400),
                "bad_request",
                "body 必须包含 \"config\" 字段".to_string(),
            ));
        };
        let enabled = registry.is_enabled(id_static);
        let reload_result = registry.reload_config(id_static, config.clone());
        drop(registry);
        match reload_result {
            Ok(()) => {
                if enabled {
                    // 配置已落内存 + 落盘；restart 交后台，立刻回执。
                    spawn_mod_op(ctx, id_static, ModOp::Restart);
                    return Some(accepted_response());
                }
                return Some(ok_response(
                    StatusCode(200),
                    r#"{"ok":true,"enabled":false}"#,
                ));
            }
            Err(live2d_ai_mod_system::ModError::Other(_)) => {
                return Some(json_error(
                    StatusCode(404),
                    "not_found",
                    format!("Mod {id} 不在注册表"),
                ));
            }
            Err(e) => {
                return Some(json_error(StatusCode(409), "mod_error", e.to_string()));
            }
        }
    }

    // enable / disable / restart：**全部异步**。
    //
    // 这三条都会碰 Mod 的生命周期（start_one / shutdown），而 local-tts 的
    // start 还要等一个「立刻退出」观察窗、首次甚至要装依赖。从前它们跑在
    // tiny_http 的单线程接受循环里 ⇒ 这段时间整个页面都不应答。现在：同步只做
    // **存在性**判定（快），动作丢后台线程，立刻回 202 + pending=true ——
    // **不把「还在启动」写成成功**：前端据此轮询 GET /api/v1/mods，看到
    // running / failed 才下结论；失败时 last_error 带脚本输出。
    let known = registry.contains(id_static);
    drop(registry);
    if !known {
        return Some(json_error(
            StatusCode(404),
            "not_found",
            format!("Mod {id} 不在注册表"),
        ));
    }
    let op = match action {
        "enable" => {
            // body 可选 {"config":{...}}。
            let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
            ModOp::Enable(parsed.as_ref().and_then(|v| v.get("config")).cloned())
        }
        "disable" => ModOp::Disable,
        "restart" => ModOp::Restart,
        _ => {
            return Some(json_error(
                StatusCode(404),
                "not_found",
                format!("未知 action：{action}"),
            ));
        }
    };
    spawn_mod_op(ctx, id_static, op);
    Some(accepted_response())
}

/// 一条要在**后台线程**里跑的 Mod 生命周期动作（2026-10-09）。
enum ModOp {
    /// enable，可带一份 config（= 先 reload_config 再 enable）。
    Enable(Option<serde_json::Value>),
    Disable,
    Restart,
}

/// 把一条 Mod 生命周期动作丢进后台线程，**不阻塞接受循环**。
///
/// 结果落在 ModRegistry 自己的状态机里（Starting → Running / Failed），
/// 失败原因在 last_error；本函数只额外留一行日志（否则「点了没反应」在
/// 排障时一片空白）。锁毒化照旧降级（into_inner），不 panic。
fn spawn_mod_op(ctx: &ServerContext, id: &'static str, op: ModOp) {
    let registry = std::sync::Arc::clone(&ctx.mod_registry);
    std::thread::spawn(move || {
        let mut reg = match registry.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let result = match op {
            ModOp::Enable(Some(config)) => reg.enable_with_config(id, config),
            ModOp::Enable(None) => reg.enable(id),
            ModOp::Disable => reg.disable(id),
            ModOp::Restart => reg.restart(id),
        };
        if let Err(e) = result {
            tracing::warn!(target: "mod", mod_id = id, "后台 Mod 操作失败：{e}");
        }
    });
}

/// 202 Accepted + pending=true：动作已受理、**还没生效**。
///
/// 刻意不是 200「成功」：enable 只是把 Mod 标成 Starting 并丢进后台，
/// 它到底起没起来要等轮询 GET /api/v1/mods。
fn accepted_response() -> Response<Cursor<Vec<u8>>> {
    ok_response(StatusCode(202), r#"{"ok":true,"pending":true}"#)
}

/// 取 `mod_registry` 锁：**锁毒化不 panic**。
///
/// 口径与 `external_routes` / `voice_routes` 的门禁读完全一致（F-0006-03）：
/// Mod 侧一次 panic 只应把这把锁降级成「拿到旧快照」，**不应**在
/// `run_request_loop`（裸 for 循环，无 catch_unwind）上再 panic 一次 ——
/// 那会把整个 HTTP 面**永久**打死，而不是退化成一次失败响应。
fn lock_registry(
    ctx: &ServerContext,
) -> std::sync::MutexGuard<'_, crate::mod_registry::ModRegistry> {
    match ctx.mod_registry.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(), // 锁中毒不 panic：读快照即可。
    }
}

/// 列举全部 Mod 状态 + 可编辑配置（rc.4 M2）：
/// `{"mods":[{"id","name","version","api_version","status","enabled","config","settings_spec"}]}`。
///
/// - `config`：该 Mod 当前 namespaced 配置；`secret=true` 的字段被剥掉（脱敏）；
/// - `settings_spec`：schema，无注册时为 `null`（旧 Mod 只显示开关）。
fn handle_mods_list(ctx: &ServerContext) -> Response<Cursor<Vec<u8>>> {
    let registry = lock_registry(ctx);
    let mods: Vec<serde_json::Value> = registry
        .list()
        .iter()
        .map(|(desc, status, enabled)| {
            let spec = registry.settings_spec(desc.id);
            let config = registry
                .config(desc.id)
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            serde_json::json!({
                "id": desc.id,
                "name": desc.name,
                "version": desc.version,
                "api_version": desc.api_version,
                "status": status.as_str(),
                // 2026-10-09：失败原因**带出来**（含脚本 stderr 的尾巴）。
                // 只给 status="failed" 的话，用户看不到脚本到底说了什么，
                // 而「立刻退出 = Failed」的整条口径就是为了让他看到那句话。
                "last_error": registry.last_error(desc.id),
                "enabled": enabled,
                "config": redacted_config(&config, spec),
                "settings_spec": spec.map(spec_json).unwrap_or(serde_json::Value::Null),
            })
        })
        .collect();
    let body = serde_json::json!({"mods": mods}).to_string();
    ok_response(StatusCode(200), &body)
}

/// `GET /api/v1/mods/{id}/config`：只读返回单个 Mod 配置（secret 字段脱敏）。
fn handle_mod_config_get(ctx: &ServerContext, id: &str) -> Response<Cursor<Vec<u8>>> {
    let registry = lock_registry(ctx);
    let Some(config) = registry.config(id) else {
        return json_error(StatusCode(404), "not_found", format!("Mod {id} 不在注册表"));
    };
    let redacted = redacted_config(config, registry.settings_spec(id));
    ok_response(
        StatusCode(200),
        &serde_json::json!({"id": id, "config": redacted}).to_string(),
    )
}

/// `GET /api/v1/mods/{id}/state`：Mod 的**只读运行态**（Wave 2）。
///
/// 在 `ModRuntime::state_json`（`live2d-ai-mod-system`）之前，Mod 的内部状态
/// 只能从日志里看——「Mod 里跑了一套状态机、界面上什么也看不见」。
/// 本路由把它变成可测面：`wallpaper` 的当前决策、`pet-desktop` 的口型 / 窗口态
/// 都从这里出去，前端与集成测试用同一个面。
///
/// 状态码语义（**刻意区分 404 与 503**）：
/// - 未知 id → `404 not_found`（这个 Mod 根本不在注册表）；
/// - 在册但取不到快照 → `503 state_unavailable`——三种成因都归它：未启用 /
///   本 Mod 没实现 `state_json` / Mod worker 正持 runtime 锁。
///   前端据此显示「暂时读不到」而**不是**「这个 Mod 不存在」。
///
/// 只读，故与 `config` GET 同口径**不校验 Origin**（GET 无副作用）；
/// 返回体里的 state 由各 Mod 自行脱敏（契约见 `ModRuntime::state_json` 头注）。
fn handle_mod_state_get(ctx: &ServerContext, id: &str) -> Response<Cursor<Vec<u8>>> {
    let registry = lock_registry(ctx);
    if !registry.contains(id) {
        return json_error(StatusCode(404), "not_found", format!("Mod {id} 不在注册表"));
    }
    let enabled = registry.is_enabled(id);
    match registry.runtime_state(id) {
        Some(state) => ok_response(
            StatusCode(200),
            &serde_json::json!({"id": id, "enabled": enabled, "state": state}).to_string(),
        ),
        None => json_error(
            StatusCode(503),
            "state_unavailable",
            format!("Mod {id} 运行态当前不可读（未启用 / 未实现 state_json / worker 正忙）"),
        ),
    }
}

/// 把 `ModSettingsSpec` 序列化成前端表单契约（`kind` 标签判类型）。
fn spec_json(spec: &ModSettingsSpec) -> serde_json::Value {
    serde_json::json!({
        "mod_id": spec.mod_id,
        "title": spec.title,
        "version": spec.version,
        "fields": spec.fields.iter().map(field_json).collect::<Vec<_>>(),
    })
}

/// 单字段 schema（`kind` ∈ bool/string/number/select）。
fn field_json(field: &ModSettingField) -> serde_json::Value {
    match field {
        ModSettingField::Bool {
            key,
            label,
            default,
        } => serde_json::json!({"kind":"bool","key":key,"label":label,"default":default}),
        // default 可选：前端把它用作 config 缺该键时的表单初值
        // （ModSettingField.fromJson 一律读 default，四种 kind 同款）。
        ModSettingField::String {
            key,
            label,
            secret,
            default,
        } => serde_json::json!({
            "kind": "string",
            "key": key,
            "label": label,
            "secret": secret,
            "default": default,
        }),
        ModSettingField::Number {
            key,
            label,
            min,
            max,
        } => serde_json::json!({"kind":"number","key":key,"label":label,"min":min,"max":max}),
        ModSettingField::Select {
            key,
            label,
            options,
            default,
        } => serde_json::json!({
            "kind": "select",
            "key": key,
            "label": label,
            "options": options
                .iter()
                .map(|o| serde_json::json!({"value": o.value, "label": o.label}))
                .collect::<Vec<_>>(),
            "default": default,
        }),
    }
}

/// 脱敏：按 schema 剥掉 `secret=true` 的 String 字段值（密钥不进 GET）。
fn redacted_config(
    config: &serde_json::Value,
    spec: Option<&ModSettingsSpec>,
) -> serde_json::Value {
    let mut obj = config.as_object().cloned().unwrap_or_default();
    if let Some(spec) = spec {
        for field in &spec.fields {
            if let ModSettingField::String {
                key, secret: true, ..
            } = field
            {
                obj.remove(key);
            }
        }
    }
    serde_json::Value::Object(obj)
}

#[cfg(test)]
#[cfg(test)]
#[path = "mods_routes_tests_support.rs"]
mod tests_support;

#[cfg(test)]
#[path = "mods_routes_tests.rs"]
mod tests;
