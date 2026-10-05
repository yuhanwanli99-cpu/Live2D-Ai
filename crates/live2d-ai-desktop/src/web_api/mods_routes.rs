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
    let mut registry = ctx
        .mod_registry
        .lock()
        .expect("mod_registry mutex poisoned");

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

    // "config" action：提前处理（需要解析 body + 可能二次锁定 registry）。
    if action == "config" {
        // POST /api/v1/mods/{id}/config：body 必须 `{"config":{...}}`。
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
                    // 立即 restart 使 config 生效。
                    let mut reg2 = ctx
                        .mod_registry
                        .lock()
                        .expect("mod_registry mutex poisoned");
                    match reg2.restart(id_static) {
                        Ok(()) => {
                            return Some(ok_response(
                                StatusCode(200),
                                r#"{"ok":true,"restarted":true}"#,
                            ));
                        }
                        Err(e) => {
                            return Some(json_error(
                                StatusCode(409),
                                "mod_error",
                                format!("reload 成功但 restart 失败：{e}"),
                            ));
                        }
                    }
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

    let result = match action {
        "enable" => {
            // body 可选 `{"config":{...}}`。
            let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
            match parsed.as_ref().and_then(|v| v.get("config")) {
                Some(config) => registry.enable_with_config(id_static, config.clone()),
                None => registry.enable(id_static),
            }
        }
        "disable" => registry.disable(id_static),
        "restart" => registry.restart(id_static),
        _ => {
            drop(registry);
            return Some(json_error(
                StatusCode(404),
                "not_found",
                format!("未知 action：{action}"),
            ));
        }
    };
    drop(registry);

    match result {
        Ok(()) => Some(ok_response(StatusCode(200), r#"{"ok":true}"#)),
        Err(live2d_ai_mod_system::ModError::Other(_)) => {
            // registry.enable/disable/restart 返回 ModError::Other 表示 id 不存在。
            Some(json_error(
                StatusCode(404),
                "not_found",
                format!("Mod {id} 不在注册表"),
            ))
        }
        Err(e) => Some(json_error(StatusCode(409), "mod_error", e.to_string())),
    }
}

/// 列举全部 Mod 状态 + 可编辑配置（rc.4 M2）：
/// `{"mods":[{"id","name","version","api_version","status","enabled","config","settings_spec"}]}`。
///
/// - `config`：该 Mod 当前 namespaced 配置；`secret=true` 的字段被剥掉（脱敏）；
/// - `settings_spec`：schema，无注册时为 `null`（旧 Mod 只显示开关）。
fn handle_mods_list(ctx: &ServerContext) -> Response<Cursor<Vec<u8>>> {
    let registry = ctx
        .mod_registry
        .lock()
        .expect("mod_registry mutex poisoned");
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
    let registry = ctx
        .mod_registry
        .lock()
        .expect("mod_registry mutex poisoned");
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
    let registry = ctx
        .mod_registry
        .lock()
        .expect("mod_registry mutex poisoned");
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
mod tests {
    use super::*;
    use crate::web_api::models_routes;
    use crate::web_api::security::SecurityContext;
    use std::io::Read;

    /// 工具：构造带 mod_registry 的 ServerContext（空 registry）。
    fn dummy_ctx(sec: SecurityContext) -> ServerContext {
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
            models: models_routes::default_store(),
            mod_registry: std::sync::Arc::new(std::sync::Mutex::new(
                crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({})),
            )),
        }
    }

    /// 带 schema 的测试 Mod：注册 Bool + secret String + 普通 String。
    struct SpecMod;

    impl live2d_ai_mod_system::ModFactory for SpecMod {
        fn descriptor(&self) -> &'static live2d_ai_mod_system::ModDescriptor {
            static D: live2d_ai_mod_system::ModDescriptor = live2d_ai_mod_system::ModDescriptor {
                id: "specmod",
                name: "SpecMod",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn create(
            &self,
            _: live2d_ai_mod_system::ModServices,
            _: serde_json::Value,
        ) -> Result<Box<dyn live2d_ai_mod_system::ModRuntime>, live2d_ai_mod_system::ModError>
        {
            Ok(Box::new(SpecRuntime))
        }
    }

    struct SpecRuntime;

    impl live2d_ai_mod_system::ModRuntime for SpecRuntime {
        fn start(
            &mut self,
            registrar: &mut dyn live2d_ai_mod_system::ModRegistrar,
        ) -> Result<(), live2d_ai_mod_system::ModError> {
            registrar.register_settings(ModSettingsSpec {
                mod_id: "specmod".to_string(),
                title: "规格".to_string(),
                version: 1,
                fields: vec![
                    ModSettingField::Bool {
                        key: "on".to_string(),
                        label: "开关".to_string(),
                        default: true,
                    },
                    ModSettingField::String {
                        key: "token".to_string(),
                        label: "密钥".to_string(),
                        secret: true,
                        default: None,
                    },
                    ModSettingField::String {
                        key: "path".to_string(),
                        label: "路径".to_string(),
                        secret: false,
                        default: None,
                    },
                ],
            })
        }
    }

    static SPEC_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[&SpecMod];

    /// 工具：带指定 factories + manifest 的 ServerContext（Mod 已 start_all）。
    fn ctx_with_mod(manifest: serde_json::Value) -> ServerContext {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let mut registry = crate::mod_registry::ModRegistry::new(SPEC_FACTORIES, &manifest);
        registry.start_all();
        *ctx.mod_registry.lock().unwrap() = registry;
        ctx
    }

    fn body_string(resp: Response<Cursor<Vec<u8>>>) -> String {
        let mut reader = resp.into_reader();
        let mut s = String::new();
        let _ = reader.read_to_string(&mut s);
        s
    }

    /// M2：`GET /api/v1/mods` 带出 `settings_spec` + `config`，且 secret 字段脱敏。
    #[test]
    fn mods_list_includes_spec_and_redacts_secret() {
        let ctx = ctx_with_mod(serde_json::json!({
            "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
        }));
        let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
        assert_eq!(resp.status_code(), StatusCode(200));
        let body = body_string(resp);
        assert!(body.contains("\"settings_spec\""), "应带 schema: {body}");
        assert!(
            body.contains("\"kind\":\"string\""),
            "字段种类应可见: {body}"
        );
        assert!(body.contains("\"kind\":\"bool\""), "字段种类应可见: {body}");
        assert!(
            body.contains("\"secret\":true"),
            "secret 标记应在 schema 里: {body}"
        );
        assert!(!body.contains("SECRET"), "secret 字段值不得进 GET: {body}");
        assert!(body.contains("\"/x\""), "非 secret 字段应回值: {body}");
    }

    /// M2：`GET /api/v1/mods/{id}/config` 只读返回；未知 id → 404；secret 不回值。
    #[test]
    fn mod_config_get_returns_redacted_config() {
        let ctx = ctx_with_mod(serde_json::json!({
            "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
        }));
        let ok = handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/mods/specmod/config",
            "",
            None,
            None,
        )
        .unwrap();
        assert_eq!(ok.status_code(), StatusCode(200));
        let body = body_string(ok);
        assert!(body.contains("\"/x\""), "body = {body}");
        assert!(!body.contains("SECRET"), "secret 不得回值: {body}");

        let missing = handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/mods/nope/config",
            "",
            None,
            None,
        )
        .unwrap();
        assert_eq!(missing.status_code(), StatusCode(404));
    }

    /// **F-0062-01（路由层）**：POST `…/config` 省略声明的 secret 字段（= 前端从 GET
    /// 的**脱敏**配置出发的形状）时**保留**旧密钥。显式清除（空串）见
    /// `mod_registry::tests_secret`；端到端「密钥还在 ⇒ 端点仍 401」见
    /// `external_routes::tests::mod_config_save_without_secret_keeps_endpoint_auth`。
    #[test]
    fn config_save_preserves_omitted_secret() {
        let ctx = ctx_with_mod(serde_json::json!({
            "mods": {"specmod": {"enabled": true, "config": {"on": true, "token": "SECRET", "path": "/x"}}}
        }));
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/specmod/config",
            r#"{"config":{"path":"/y"}}"#,
            Some("http://127.0.0.1:18099"),
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(200));
        let reg = ctx.mod_registry.lock().unwrap();
        let cfg = reg.config("specmod").unwrap();
        assert_eq!(
            cfg["token"],
            serde_json::json!("SECRET"),
            "省略 secret 必须保留旧值：{cfg}"
        );
        assert_eq!(cfg["path"], serde_json::json!("/y"));
    }

    /// 非本族路径 → None。
    #[test]
    fn non_mods_path_returns_none() {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        assert!(
            handle_mods_route(
                &ctx,
                &Method::Get,
                "/api/v1/chat",
                "",
                None,
                Some("application/json"),
            )
            .is_none()
        );
    }

    /// 空 registry，enable 未知 id → 404。
    #[test]
    fn enable_unknown_id_returns_404() {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/nope/enable",
            "{}",
            Some("http://127.0.0.1:18099"),
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(404));
    }

    /// **回归（stabilize）**：无 body 的 enable/disable **不要求 Content-Type**。
    ///
    /// 触发场景（无头浏览器实测）：Flutter `ModsApi.setEnabled` 发的是
    /// `fetch(POST)` **不带 body、不带 Content-Type**；旧实现在此处无条件要
    /// `application/json` → 恒 415，前端「Mod 管理」的启停整个点不动。
    /// 口径与 `security::check_mutating_request` 一致：只有带 body 才校验 CT。
    #[test]
    fn bodyless_enable_disable_skip_content_type_check() {
        let ctx = ctx_with_mod(serde_json::json!({
            "mods": {"specmod": {"enabled": true, "config": {}}}
        }));
        let origin = Some("http://127.0.0.1:18099");
        for action in ["disable", "enable"] {
            let path = format!("/api/v1/mods/specmod/{action}");
            let resp = handle_mods_route(&ctx, &Method::Post, &path, "", origin, None)
                .expect("mods 路由应命中");
            assert_eq!(
                resp.status_code(),
                StatusCode(200),
                "无 body 的 {action} 必须 200（旧实现会 415）"
            );
        }
    }

    /// 带 body 的 mutating 仍必须 `application/json`（防表单 CSRF 不回退）。
    #[test]
    fn body_with_non_json_content_type_is_415() {
        let ctx = ctx_with_mod(serde_json::json!({
            "mods": {"specmod": {"enabled": true, "config": {}}}
        }));
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/specmod/enable",
            r#"{"config":{"on":true}}"#,
            Some("http://127.0.0.1:18099"),
            Some("text/plain"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(415));
        assert!(body_string(resp).contains("unsupported_media_type"));
    }

    /// 无 body + 缺 Origin + 不允许无 Origin → 403（**不是** 415）：
    /// Origin 才是闸门，错误面要给对，工具客户端才知道去开 allow_no_origin。
    #[test]
    fn bodyless_without_origin_is_403_not_415() {
        let sec = SecurityContext::new(18099, false);
        let ctx = dummy_ctx(sec);
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/specmod/enable",
            "",
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            resp.status_code(),
            StatusCode(403),
            "缺 Origin 应回 403（origin_required），不能是 415"
        );
    }

    /// GET /api/v1/mods 在空 registry → 200 + `{"mods":[]}`。
    #[test]
    fn get_mods_list_empty() {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_mods_route(&ctx, &Method::Get, "/api/v1/mods", "", None, None).unwrap();
        assert_eq!(resp.status_code(), StatusCode(200));
        let mut reader = resp.into_reader();
        let mut s = String::new();
        let _ = reader.read_to_string(&mut s);
        assert!(s.contains("\"mods\""), "body = {s}");
        assert!(s.contains("[]"), "body = {s}");
    }

    /// POST /api/v1/mods/{id}/config 路由匹配测试。
    /// - 空 registry 时 reload_config 返回 Other → 404。
    #[test]
    fn config_route_match_unknown_id_returns_404() {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/nope/config",
            r#"{"config":{"a":1}}"#,
            Some("http://127.0.0.1:18099"),
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(404));
    }

    /// POST /api/v1/mods/{id}/config：无效 JSON → 400。
    #[test]
    fn config_route_bad_json_returns_400() {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/nope/config",
            r#"not json"#,
            Some("http://127.0.0.1:18099"),
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(400));
    }

    /// POST /api/v1/mods/{id}/config：缺少 config 字段 → 400。
    #[test]
    fn config_route_missing_config_returns_400() {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let resp = handle_mods_route(
            &ctx,
            &Method::Post,
            "/api/v1/mods/nope/config",
            r#"{"other":"value"}"#,
            Some("http://127.0.0.1:18099"),
            Some("application/json"),
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(400));
    }

    // ------------------------------------------- Wave 2：GET {id}/state

    /// 带运行态快照的测试 Mod（`state_json` 有一份可断言的内容）。
    struct StateModFactory;

    impl live2d_ai_mod_system::ModFactory for StateModFactory {
        fn descriptor(&self) -> &'static live2d_ai_mod_system::ModDescriptor {
            static D: live2d_ai_mod_system::ModDescriptor = live2d_ai_mod_system::ModDescriptor {
                id: "stateful",
                name: "Stateful",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn create(
            &self,
            _: live2d_ai_mod_system::ModServices,
            _: serde_json::Value,
        ) -> Result<Box<dyn live2d_ai_mod_system::ModRuntime>, live2d_ai_mod_system::ModError>
        {
            Ok(Box::new(StatefulRuntime))
        }
    }

    struct StatefulRuntime;

    impl live2d_ai_mod_system::ModRuntime for StatefulRuntime {
        fn start(
            &mut self,
            _: &mut dyn live2d_ai_mod_system::ModRegistrar,
        ) -> Result<(), live2d_ai_mod_system::ModError> {
            Ok(())
        }
        fn state_json(&mut self) -> Option<serde_json::Value> {
            Some(serde_json::json!({"counter": 7, "note": "ok"}))
        }
    }

    static STATE_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[&StateModFactory];

    fn ctx_with_state_mod(enabled: bool) -> ServerContext {
        let sec = SecurityContext::new(18099, true);
        let ctx = dummy_ctx(sec);
        let mut registry = crate::mod_registry::ModRegistry::new(
            STATE_FACTORIES,
            &serde_json::json!({"mods": {"stateful": {"enabled": enabled}}}),
        );
        registry.start_all();
        *ctx.mod_registry.lock().unwrap() = registry;
        ctx
    }

    /// 启用中的 Mod：200 + `{id, enabled, state}`。
    #[test]
    fn state_route_returns_runtime_snapshot() {
        let ctx = ctx_with_state_mod(true);
        let resp = handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/mods/stateful/state",
            "",
            None,
            None,
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(200));
        let body = body_string(resp);
        assert!(body.contains("\"id\":\"stateful\""), "{body}");
        assert!(body.contains("\"enabled\":true"), "{body}");
        assert!(body.contains("\"counter\":7"), "应带出运行态: {body}");
    }

    /// **404 vs 503 的分界**：不在注册表 = 404；在册但没在跑 = 503。
    #[test]
    fn state_route_distinguishes_unknown_from_unavailable() {
        let ctx = ctx_with_state_mod(true);
        let unknown = handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/mods/nope/state",
            "",
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            unknown.status_code(),
            StatusCode(404),
            "不存在的 Mod 必须 404（前端显示「没有这个 Mod」）"
        );
        let body = body_string(unknown);
        assert!(body.contains("not_found"), "{body}");

        // 同一个 Mod，停用 → 有 runtime 槽位为空 → 503。
        let mut registry = ctx.mod_registry.lock().unwrap();
        registry.disable("stateful").unwrap();
        drop(registry);
        let unavailable = handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/mods/stateful/state",
            "",
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            unavailable.status_code(),
            StatusCode(503),
            "在册但取不到快照必须 503（前端显示「暂时读不到」）"
        );
        assert!(body_string(unavailable).contains("state_unavailable"));
    }

    /// 在册、在跑、但**没实现** `state_json` → 也是 503（不是 500）。
    #[test]
    fn state_route_without_state_json_is_503() {
        let ctx = ctx_with_mod(serde_json::json!({"mods": {"specmod": {"enabled": true}}}));
        let resp = handle_mods_route(
            &ctx,
            &Method::Get,
            "/api/v1/mods/specmod/state",
            "",
            None,
            None,
        )
        .unwrap();
        assert_eq!(resp.status_code(), StatusCode(503));
        assert!(body_string(resp).contains("state_unavailable"));
    }
}
