//! 密钥真源端点（rc.2 2026-09-12）：`GET /api/v1/env` + `PUT /api/v1/env`。
//!
//! # 契约
//!
//! | 端点 | 作用 | 回显 |
//! |---|---|---|
//! | `GET /api/v1/env` | 列出**配置里声明的**键名（`llm` / `tts` / `performance` 的 `api_key_env`）与「是否已设置」 | **只回键名 + 布尔，永不回值** |
//! | `PUT /api/v1/env` | 写 `.env` 的单个键（`{"key":"…","value":"…"}`），写完热重载 | 回 `{key, set, apply_status}`（`apply_status` = 热重载是否真的装上，2026-10-16） |
//!
//! # 为什么单开一个端点而不是塞进 `PATCH /api/v1/settings`
//!
//! settings 是**可导出、可展示、可备份**的配置（`live2d-ai.toml`）；
//! 密钥不是——它住在 `.env`（gitignored、`0600`）。把写密钥混进 settings 的
//! PATCH，就等于让「保存配置」这条日常路径顺带碰密钥文件，语义与风险都不对。
//!
//! # 安全
//!
//! - `PUT` 在 `is_mutating_route` 里 → 由 dispatch 入口做 Origin + Content-Type 校验；
//! - 审计日志只记 `key`，**不记 value**（`dispatch` 本来就不记 body；
//!   这里额外保证错误信息里也不出现 value）；
//! - `GET` 只回是否已设置（[`live2d_ai_runtime::secrets::is_set`]）。
//!
//! # 值从哪里来
//!
//! 键名**不是**前端随便填的：`GET` 返回的就是配置里 `api_key_env` 指向的那个名字。
//! 前端照着填/写，因此不存在「前端写了一个后端不读的变量名」这种静默失效。
//!
//! 键名清单的**唯一真源**是 [`AppSettings::declared_key_envs`]——本模块的
//! `GET` 与 `PUT`（白名单）都从那一份结果推导，**不得**再各自抄一遍段清单。
//! 以前两边各抄 `llm` + `tts`，于是 `[performance].api_key_env` 既列不出、
//! 也写不进（PUT 回 `400 unknown_key`）。

use std::io::Cursor;

use serde::Deserialize;
use tiny_http::{Header, Method, Response, StatusCode};

use live2d_ai_runtime::settings::AppSettings;

use crate::web_api::app_routes::json_response;
use crate::web_api::dto::ApplyStatus;

/// HTTP 响应类型别名：`tiny_http::Response<Cursor<Vec<u8>>>` 长到 clippy 会抱怨
/// `type_complexity`，而这个文件里每个 handler 都要写它两遍。
///
/// 2026-10-16：提到 `pub(crate)` 是为了让拆出去的 `env_routes_tests` 能用它
/// （glob 导入只带 pub 项，测试文件不再与实现同模块）。
pub(crate) type Resp = Response<Cursor<Vec<u8>>>;

/// `GET /api/v1/env` 的单条。
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct EnvKeyEntry {
    /// 归属段（`"llm"` / `"tts"` / `"performance"`）。
    pub section: &'static str,
    /// 环境变量名（来自配置的 `api_key_env`）。
    pub key: String,
    /// 该键**是否有非空值**（`.env` 快照或进程环境）。
    pub set: bool,
}

/// `GET /api/v1/env` 响应。
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct EnvListResponse {
    /// 配置里声明的键（没有 `api_key_env` 的段不出现）。
    pub keys: Vec<EnvKeyEntry>,
    /// `.env` 路径（**不是**密钥，是排障必需的：用户要知道该改哪个文件）。
    pub env_file: String,
}

/// `PUT /api/v1/env` 请求体。
#[derive(Debug, Clone, Deserialize)]
pub struct EnvWriteRequest {
    /// 环境变量名（必须是配置里声明的那个，见 [`handle_get`]）。
    pub key: String,
    /// 新值。**空串 = 删除该键**。
    #[serde(default)]
    pub value: String,
}

/// 写 `.env` 的可注入副作用（生产 = `live2d_ai_runtime::secrets::write_key`）。
///
/// 注入的理由与 `handle_patch` 的 `lookup` 同源：单测**不许碰仓库根的真 `.env`**，
/// 而又得把「写盘成功之后」那一段（`apply_status` 进响应）整条走通。
pub type EnvWriter<'a> = &'a dyn Fn(&str, &str) -> Result<(), String>;

/// `PUT /api/v1/env` 响应（**不回显 value**）。
///
/// 2026-10-16（FINDINGS.md S-002）：加 `apply_status`。**写盘成功 ≠ 引擎装上**：
/// 密钥写进 `.env` 是一件事，supervisor 是否真的重建/热重载了 LLM/TTS client
/// 是另一件事。从前响应里没有能表达后者的字段，前端只能写死「立即生效」。
#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub struct EnvWriteResponse {
    pub key: String,
    /// 写完之后该键是否已设置（空值写入 = 删除 = `false`）。
    pub set: bool,
    /// 热重载状态（同 `PATCH /api/v1/settings` 的 `apply_status` 取值）。
    ///
    /// **只有 `applied` 才等于「立即生效」**；`no_supervisor` /
    /// `restart_required` 都表示密钥已在 `.env` 里、引擎还没用上它。
    pub apply_status: ApplyStatus,
}

/// `GET /api/v1/env`：配置声明的键名 + 是否已设置。
pub fn handle_get(settings: &AppSettings) -> Resp {
    // 唯一真源：`AppSettings::declared_key_envs()`（顺序 llm → tts → performance）。
    // 这里**不**枚举段清单——多抄一遍就多一处「新增段忘了同步」的缺口。
    let keys: Vec<EnvKeyEntry> = settings
        .declared_key_envs()
        .into_iter()
        .map(|d| EnvKeyEntry {
            section: d.section,
            set: live2d_ai_runtime::secrets::is_set(&d.name),
            key: d.name,
        })
        .collect();
    json_response(
        StatusCode(200),
        &EnvListResponse {
            keys,
            env_file: live2d_ai_runtime::secrets::env_file_path()
                .display()
                .to_string(),
        },
    )
}

/// `PUT /api/v1/env`：写 `.env` 的单个键 → 热重载 → 把热重载状态一起回给调用方。
///
/// 返回 `Ok(response)` = 已落盘；`Err(response)` = 400/500，**错误信息里不含 value**。
///
/// `write`：真正落盘那一步（见 [`EnvWriter`]）；`after_hook`（2026-10-16，同 `settings_routes::handle_patch` 的形状）：**写盘成功之后**
/// 才调用，由 dispatch 注入「让 supervisor 重建/热重载」那一步，返回值原样进
/// `apply_status`。`None`（纯逻辑测试 / 老调用方）回落 `NoSupervisor`——**不谎报已生效**。
pub fn handle_put(
    settings: &AppSettings,
    method: &Method,
    body_str: &str,
    write: EnvWriter<'_>,
    after_hook: Option<&dyn Fn() -> ApplyStatus>,
) -> Result<Resp, Resp> {
    if *method != Method::Put {
        return Err(method_not_allowed());
    }
    let req: EnvWriteRequest = match serde_json::from_str(body_str) {
        Ok(r) => r,
        Err(e) => {
            return Err(bad_request(
                "invalid_payload",
                &format!("请求 body JSON 解析失败: {e}"),
            ));
        }
    };
    // 键名白名单 = 配置里声明的那些（**与 `GET` 同一份真源**：
    // `AppSettings::declared_key_envs`）。不校验的话，前端可以往 `.env` 里写任意
    // 变量名（无害但会让「写了却没生效」变成常态，且 .env 会慢慢长满垃圾）。
    let declared = settings.declared_key_envs();
    if !declared.iter().any(|d| d.name == req.key) {
        return Err(bad_request(
            "unknown_key",
            &format!(
                "配置里没有声明这个键名（可选：{}）；键名来自 [llm]/[tts]/[performance] 的 api_key_env",
                if declared.is_empty() {
                    "无".to_string()
                } else {
                    declared
                        .iter()
                        .map(|d| d.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
        ));
    }
    if let Err(e) = write(&req.key, &req.value) {
        // `e` 来自写盘/校验路径，**不含 value**（写盘失败信息只有路径与 io 错误）。
        return Err(json_error(StatusCode(500), "persist_failed", &e));
    }
    let set = live2d_ai_runtime::secrets::is_set(&req.key);
    // 写盘成功之后才谈「生效」：这一步是 `ServerContext::ensure_supervisor_after_patch`。
    let apply_status = after_hook
        .map(|hook| hook())
        .unwrap_or(ApplyStatus::NoSupervisor);
    tracing::info!(key = %req.key, set, apply_status = ?apply_status, "env 写入成功（值不记录）");
    Ok(json_response(
        StatusCode(200),
        &EnvWriteResponse {
            key: req.key,
            set,
            apply_status,
        },
    ))
}

/// 非 GET/PUT 的统一 405（dispatch 直接调用）。
pub(crate) fn method_not_allowed() -> Resp {
    json_error(
        StatusCode(405),
        "method_not_allowed",
        "`/api/v1/env` 只支持 GET（读键名）与 PUT（写入）",
    )
}

fn bad_request(code: &'static str, message: &str) -> Resp {
    json_error(StatusCode(400), code, message)
}

fn json_error(status: StatusCode, code: &'static str, message: &str) -> Resp {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    Response::from_data(body.into_bytes())
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type"),
        )
}
