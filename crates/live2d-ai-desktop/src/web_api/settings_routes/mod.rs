//! 设置管理路由（`/api/v1/settings` + `/api/v1/settings/test/{llm,tts}`，D1 §1.2）。
//!
//! 责任：
//! - `GET /api/v1/settings`：返回脱敏 [`SettingsView`]（**不**含密钥——
//!   P0-1）。直接走 [`live2d_ai_runtime::settings::view::settings_to_view`]，
//!   本路由不读 env、不显式派生 `has_api_key`（视图层已包办）。
//! - `PATCH /api/v1/settings`：body 解析为 [`PatchBody`] → 内部归并为
//!   [`SettingsPatch`] → [`apply_patch`] → 若 [`PatchOutcome::Updated`] 再经
//!   [`plan_atomic_write`] 写盘 + rename。`clear_api_key` 显式标志（§2 P0-2
//!   防误删，复审 D-P0A：改为 **段级** 字段）由本模块在 `apply_patch` **前**
//!   从 body 抽离：provider（llm/tts）独立，互不误伤。
//! - `POST /api/v1/settings/test/llm` / `test/tts`：用「暂存配置」发一次
//!   最小请求，**不**落盘——失败分类按 D1 错误码。
//!
//! 安全：
//! - P0-1：响应中**永不出密钥**——脱敏视图保证。
//! - P0-2：每 provider 独立 `clear_api_key: true` 显式删除；缺省视为"保持原值"。
//!   「无 clear 标志的 `api_key_env: null`」在 inject 阶段改写为字段缺省。
//!
//! D-P0C（2026-08-29）补充：PATCH 写盘成功（200）后调 caller 注入的
//! `patch_after_hook` 闭包，**让 dispatch 层**（拥有 supervisor 槽位）做
//! 「动态装配 / 触发 reload」决策，把 [`ApplyStatus`] 回填到响应 body
//! ——前端按状态显示正确文案，不再误报「已热重载」与「实际 503」错位。

use serde::Deserialize;
use tiny_http::{Response, StatusCode};

use live2d_ai_runtime::AppSettings;
use live2d_ai_runtime::settings::patch::{
    PatchOutcome, SettingsPatch, apply_patch, plan_atomic_write,
};
use live2d_ai_runtime::settings::view::settings_to_view;

use crate::web_api::app_routes::json_response;
use crate::web_api::dto::{ApplyStatus, ErrorDetail, ErrorResponse};

/// `GET /api/v1/settings` 处理器。
pub fn handle_get(current: &AppSettings) -> Response<std::io::Cursor<Vec<u8>>> {
    let view = settings_to_view(current);
    json_response(StatusCode(200), &view)
}

/// PATCH body 包装：在 [`SettingsPatch`] 之上加 **段级** `clear_api_key`
/// （D1 P0-2；D-P0A 复审：段级避免误伤另一 provider）。
///
/// 与 v0 顶层 `clear_api_key: bool` 的差异：
/// - v0 全局 true 会同时清 llm + tts 的 `api_key_env` —— 用户「只清 LLM + 改
///   TTS voice」会被误清 TTS key；
/// - v1 `clear_api_key` 段级独立；llm / tts 互不影响。
///
/// 段级 serde 三态（`PatchLlmBody` / `PatchTtsBody`）：
/// - 段缺省 → `None`（不参与）；
/// - 段是 `null` → `Some(None)`（整段清空，仍走 runtime 段级三态语义）；
/// - 段是对象 → `Some(Some(body))`（含 LlmPatch + 段级 clear_api_key）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatchBody {
    /// 见 [`SettingsPatch::llm`] + 段级 `clear_api_key`。
    /// 段级三态：缺省 = `None`（不参与）/ `null` = `Some(None)`（整段清空）/
    /// 对象 = `Some(Some(body))`（字段级三态合并）。
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub llm: Option<Option<PatchLlmBody>>,
    /// 见 [`SettingsPatch::tts`] + 段级 `clear_api_key`。
    /// 段级三态（同 `llm`）。
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub tts: Option<Option<PatchTtsBody>>,
    /// 见 [`SettingsPatch::persona`]。
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub persona: Option<Option<live2d_ai_runtime::settings::patch::PersonaPatch>>,
    /// W7 任务：顶层 dev_mode 三态补丁。语义同 SettingsPatch.dev_mode：
    /// - 缺省 = `None`（不修改）
    /// - `null` = `Some(None)`（显式关闭）
    /// - `true|false` = `Some(Some(b))`（显式设置）
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub dev_mode: Option<Option<bool>>,
}

/// LLM 段 PATCH body：runtime patch 字段 + 段级 `clear_api_key`。
///
/// serde 形态（HTTP body）：段级字段**扁平化**（用 `#[serde(flatten)]` 把
/// `LlmPatch` 的 `base_url` / `model` / `api_key_env` 直接展开在 `llm` 对象
/// 顶层，与 runtime 字段一一对应；`clear_api_key` 是额外顶层字段）。
///
/// 例如 `{"llm": {"base_url": "http://x/", "clear_api_key": true}}` 解析为
/// `PatchLlmBody { clear_api_key: Some(true), patch: LlmPatch { base_url:
/// Some(Some("http://x/")), .. } }`。
///
/// `clear_api_key: true` + 段在场但 body **未**给 `api_key_env` → 注入清除
/// 语义；与「body 给新 `api_key_env`」共存时新值优先。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatchLlmBody {
    /// 段级 P0-2 显式清除标志。
    /// - 缺省 = `None`（无显式意图，按 body 里的 `api_key_env` 字面解析）
    /// - `Some(true)` = 显式清除（apply 前 inject 阶段改写 `api_key_env`）
    /// - `Some(false)` = 显式否定（保留原值；用于「显式说 no」的可观测性）
    #[serde(default)]
    pub clear_api_key: Option<bool>,
    /// LLM 段字段级三态补丁（扁平化到段对象顶层）。
    #[serde(default, flatten)]
    pub patch: live2d_ai_runtime::settings::patch::LlmPatch,
}

/// TTS 段 PATCH body：runtime patch 字段 + 段级 `clear_api_key`。
///
/// 同 [`PatchLlmBody`]：用 `#[serde(flatten)]` 把 `TtsPatch` 字段展开到
/// `tts` 段顶层。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatchTtsBody {
    /// 段级 P0-2 显式清除标志（语义同 [`PatchLlmBody::clear_api_key`]）。
    #[serde(default)]
    pub clear_api_key: Option<bool>,
    /// TTS 段字段级三态补丁（扁平化到段对象顶层）。
    #[serde(default, flatten)]
    pub patch: live2d_ai_runtime::settings::patch::TtsPatch,
}

impl PatchBody {
    /// 解析 JSON body 字符串为 [`PatchBody`]。
    pub fn parse(body: &str) -> Result<Self, String> {
        serde_json::from_str(body).map_err(|e| format!("JSON 解析失败: {e}"))
    }
}

/// 把 PATCH body 的「段级 clear_api_key」与「无 clear 的 `api_key_env: null`」
/// 翻译进最终 `SettingsPatch`（D-P0A 复审修复点 1 + 2）。
///
/// 规则（对 llm / tts 段分别独立，互不误伤）：
/// 1. **clear=true 且段在场** 且 `patch.api_key_env == None`
///    （即 body 里 llm 段显式出现但**没**给 `api_key_env` 字段）→
///    注入 `Some(None)`，让 [`apply_patch`] 清空；
/// 2. **clear=true 且段在场** 且 `patch.api_key_env == Some(...)`（body 已
///    显式给出新值）→ **不**覆盖（用户意图优先：显式值生效）；
/// 3. **clear 缺省 / false** 且 `patch.api_key_env == Some(None)`（body 给
///    了 `api_key_env: null` 但**没**声明 clear）→ 改写为 `None`（字段缺省，
///    不表达清除，保持原值 —— P0-2 保护）；
/// 4. **clear=true** 但 `patch.api_key_env == Some(Some(v))`（body 给了新
///    `api_key_env` 值）→ 视情况 2：不覆盖。
/// 5. **clear=false** 显式否定 → 即使 body 给了 `api_key_env: null` 也走 3
///    改写为 `None`（用户明确表达「不清除」）。
///
/// 段级外层 `Option<Option<...>>`：
/// - `None` → 不参与；不动；
/// - `Some(None)` → 整段显式清空（透传给 runtime `apply_patch` 走段级清空路径）。
fn inject_clear_key_flag(patch: &mut SettingsPatch, body: &PatchBody) {
    // LLM 段
    if let Some(Some(llm_body)) = body.llm.as_ref() {
        // 段级 Some(None) 整段清空路径：unwrap 成 SettingsPatch.llm = Some(None)。
        if let Some(llm_outer) = patch.llm.as_mut() {
            // 走到这里 llm 段一定 Some(Some(_))（Some(None) 路径已由
            // `take` 处理），inject 字段级语义。
            if let Some(inner) = llm_outer.as_mut() {
                apply_inject_rules(inner, llm_body.clear_api_key);
            }
        }
    }
    // TTS 段
    if let Some(Some(tts_body)) = body.tts.as_ref()
        && let Some(tts_outer) = patch.tts.as_mut()
        && let Some(inner) = tts_outer.as_mut()
    {
        apply_inject_rules_tts(inner, tts_body.clear_api_key);
    }
}

/// 注入规则（LLM）：合并段级 `clear_api_key` 与 `api_key_env` 字面。
fn apply_inject_rules(
    inner: &mut live2d_ai_runtime::settings::patch::LlmPatch,
    clear: Option<bool>,
) {
    match clear {
        Some(true) => {
            // 显式清除：仅当 body 没显式给新值（inner.api_key_env 仍为 None）时
            // 注入 Some(None)。Some(Some(v)) 视为新值优先。
            if inner.api_key_env.is_none() {
                inner.api_key_env = Some(None);
            }
        }
        Some(false) | None => {
            // 无清除意图：若 body 给了 `api_key_env: null`（Some(None)）则
            // 改写为字段缺省（None = 保持原值）。Some(Some(v)) 保留新值。
            if matches!(inner.api_key_env, Some(None)) {
                inner.api_key_env = None;
            }
        }
    }
}

/// 注入规则（TTS）：同 [`apply_inject_rules`]，对 TtsPatch 字段。
fn apply_inject_rules_tts(
    inner: &mut live2d_ai_runtime::settings::patch::TtsPatch,
    clear: Option<bool>,
) {
    match clear {
        Some(true) => {
            if inner.api_key_env.is_none() {
                inner.api_key_env = Some(None);
            }
        }
        Some(false) | None => {
            if matches!(inner.api_key_env, Some(None)) {
                inner.api_key_env = None;
            }
        }
    }
}

/// PATCH 响应（写盘前/后端均用；指示是否落盘 + apply 状态）。
///
/// **D-P0C（2026-08-29）**：增加 `apply_status` 字段——dispatch 层在写盘
/// 成功后调用 `patch_after_hook` 拿到 [`ApplyStatus`]，回填到响应。前端
/// 按状态显示文案（**前端字符串锚点**：`"applied"` / `"restart_required"`
/// / `"queued"` / `"no_supervisor"`——前端按此分支显示文案；改动需同步）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PatchResponse {
    /// 是否真的写了盘（[`PatchOutcome::NoChange`] → `false`）。
    pub persisted: bool,
    /// 写盘后的脱敏视图（与 GET 响应同构）。
    pub settings: live2d_ai_runtime::settings::view::SettingsView,
    /// 写盘后 supervisor 装载/重载状态（D-P0C）。
    ///
    /// 缺省 `NoSupervisor`（旧调用方/未传 hook 时回退）。`Applied` =
    /// 动态装配成功 / reload 已触发；`RestartRequired` = 保存成功但
    /// supervisor 无法动态创建（如 LLM/TTS 配置不全）。前端按此字段
    /// 选择 toast / saveStatus 文案——避免「已热重载」与「实际 503」错位。
    #[serde(default = "default_apply_status")]
    pub apply_status: ApplyStatus,
}

/// `apply_status` 字段的 serde default（D-P0C）。
///
/// 用于 `#[serde(default = ...)]`——`PatchResponse` 序列化时若调用方
/// 显式未填（`apply_status` 字段缺失），回退 `NoSupervisor`。本路径
/// **只**在外部直接构造 `PatchResponse` 而非经 `apply_and_write` 时
/// 触发（生产路径总会显式填值）；保留作 serde 完整字段契约。
#[allow(dead_code)]
fn default_apply_status() -> ApplyStatus {
    ApplyStatus::NoSupervisor
}

/// PATCH 写盘成功后的「动态装配/重载」钩子（D-P0C）。
///
/// 由 `dispatch` 层在调用 [`handle_patch`] 时传入；handler 在写盘成功
/// 后**同步**调用本钩子拿 [`ApplyStatus`]，回填到响应 body。`Fn() -> ApplyStatus`
/// 而非 `FnMut`：每次 PATCH 调用一次，闭包捕获 `&ServerContext` 即可。
/// 钩子**只在** `PatchOutcome::Updated` 路径触发；`NoChange` 不调用
/// （盘上无变化 = 不会有新装配）。
#[allow(dead_code)] // 命名类型：dispatch.rs 用闭包字面量（`Box<dyn Fn() -> _>`），type alias 留作文档锚点。
pub type PatchAfterHook = Box<dyn Fn() -> ApplyStatus>;

/// `PATCH /api/v1/settings` 处理器（不写盘版本，仅返回 next settings）。
///
/// **写盘由调用方**通过 [`apply_and_write`] 触发（本模块函数是纯逻辑层）；
/// 路由层在 JSON 反序列化 / 校验后调用 [`apply_and_write`] 即可。
///
/// 写盘安全（D-P0A 修复点 3）：
/// - 写盘前 `create_dir_all` 父目录（首次配置 `~/.config/live2d-ai/` 不存在
///   也能保存）；
/// - `rename(tmp, target)` 失败时**清理 tmp 文件**（不留垃圾）。
///
/// D-P0C：`after_hook` 在写盘成功（`persisted=true`）后被调用，dispatch
/// 层负责把返回的 `apply_status` 写进 `PatchResponse`。
pub fn apply_and_write(
    current: &AppSettings,
    patch: &SettingsPatch,
    config_path: &str,
    after_hook: Option<&dyn Fn() -> ApplyStatus>,
) -> Result<PatchResponse, ErrorResponse> {
    let (next, outcome) = apply_patch(current, patch).map_err(|msg| {
        // apply_patch 返回的 String 形态错误码语义不细；按消息前缀判。
        if msg.contains("base_url 非法") {
            ErrorResponse::new(ErrorDetail::new("url_invalid", msg.clone()))
        } else if msg.contains("api_key_env 非法") {
            ErrorResponse::new(ErrorDetail::with_details(
                "invalid_env_name",
                msg.clone(),
                serde_json::json!({"section": if msg.starts_with("llm") {"llm"} else {"tts"}}),
            ))
        } else {
            ErrorResponse::new(ErrorDetail::new("invalid_payload", msg))
        }
    })?;
    let persisted = matches!(outcome, PatchOutcome::Updated);
    if persisted {
        // **合并写回**：保留现有文件里的注释与排版。
        // 直接 `to_toml_string()` 会重新生成整份文档、抹掉所有注释
        //（2026-09-11 实测发生：在界面上切一次 dev_mode，46 行注释全消失）。
        let toml_text = next.to_toml_string_merging(std::path::Path::new(config_path));
        // 写盘前创建父目录（P0-4 关联）：首次保存时 `~/.config/live2d-ai/`
        // 不存在也能成功。
        let target = std::path::Path::new(config_path);
        if let Some(parent) = target.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                ErrorResponse::new(ErrorDetail::new(
                    "persist_failed",
                    format!("create_dir_all {}: {e}", parent.display()),
                ))
            })?;
        }
        let tmp = plan_atomic_write(target, &toml_text)
            .map_err(|e| ErrorResponse::new(ErrorDetail::new("persist_failed", e)))?;
        if let Err(e) = std::fs::rename(&tmp, target) {
            // rename 失败：清理 tmp 不留垃圾（NotFound 静默忽略）。
            let _ = std::fs::remove_file(&tmp);
            return Err(ErrorResponse::new(ErrorDetail::new(
                "persist_failed",
                format!("rename {}: {e}", tmp.display()),
            )));
        }
        // D-P0C：写盘成功 → 调 hook 拿 apply_status。
        // 注：必须**写盘成功**之后才能保证 dispatch 看到的是新配置；
        // 旧版直接 `request_reload` 的 no-op 隐患在此闭合（hook 内部
        // 会做动态装配）。
        let apply_status = after_hook.map(|h| h()).unwrap_or(ApplyStatus::NoSupervisor);
        return Ok(PatchResponse {
            persisted,
            settings: settings_to_view(&next),
            apply_status,
        });
    }
    // NoChange：盘上无变化 → 不调 hook（不视为热重载机会），但仍返回
    // 当前视图。apply_status 用一个保守的「重启」状态——但前端只在
    // `persisted=true` 时才解析 apply_status 字段（写入未发生 = 文案
    // 无意义），所以这里选什么都安全。
    Ok(PatchResponse {
        persisted,
        settings: settings_to_view(&next),
        apply_status: ApplyStatus::NoSupervisor,
    })
}

/// 解析 PATCH body → 应用 → 写盘（路由层入口）。
///
/// D-P0C：增加 `after_hook` 参数（dispatch 层传入，封装
/// `ServerContext::ensure_supervisor_after_patch`）；写盘成功时调用并
/// 把返回的 `apply_status` 注入响应。`after_hook = None` 时（老调用方/
/// 纯逻辑测试）回退到 `NoSupervisor`。
pub fn handle_patch(
    current: &AppSettings,
    body_str: &str,
    config_path: &str,
    after_hook: Option<&dyn Fn() -> ApplyStatus>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match PatchBody::parse(body_str) {
        Ok(b) => b,
        Err(e) => {
            // 2026-09-11：解析失败也要留痕（过去只进 HTTP 响应体，日志里没有）。
            tracing::warn!(code = "invalid_payload", "PATCH body 解析失败：{e}");
            return error_response(StatusCode(400), "invalid_payload", e);
        }
    };
    // 段级三态归一：HTTP body 用 `Option<Option<PatchLlmBody>>`（段级
    // `Some(None)` = 整段清空 / `Some(Some(body))` = 字段级三态合并）；
    // runtime `SettingsPatch.llm` 同样为 `Option<Option<LlmPatch>>` —— 直接
    // 拆包 PatchLlmBody.patch 即可。段级 clear_api_key 在 inject 阶段合并。
    let mut patch = SettingsPatch {
        llm: body
            .llm
            .as_ref()
            .map(|outer| outer.as_ref().map(|b| b.patch.clone())),
        tts: body
            .tts
            .as_ref()
            .map(|outer| outer.as_ref().map(|b| b.patch.clone())),
        persona: body.persona.clone(),
        // W7 任务：dev_mode 三态由 HTTP body 顶层字段提供（不走段级）。
        dev_mode: body.dev_mode,
    };
    inject_clear_key_flag(&mut patch, &body);
    match apply_and_write(current, &patch, config_path, after_hook) {
        Ok(resp) => {
            // 审计轨迹：「我改过设置」这件事本身要留痕（persisted=false 表示
            // 服务端认为是 no-op）。写盘失败的分支在下面打 warn。
            tracing::info!(
                persisted = resp.persisted,
                apply_status = ?resp.apply_status,
                "设置已应用"
            );
            json_response(StatusCode(200), &resp)
        }
        // 400 类错误统一映射。
        Err(e)
            if e.error.code == "url_invalid"
                || e.error.code == "invalid_env_name"
                || e.error.code == "invalid_payload" =>
        {
            tracing::warn!(
                code = e.error.code,
                status = 400,
                "设置写入被拒：{}",
                e.error.message
            );
            error_response_from(StatusCode(400), e)
        }
        // 500 类。
        Err(e) => {
            tracing::error!(
                code = e.error.code,
                status = 500,
                "设置写入失败：{}",
                e.error.message
            );
            error_response_from(StatusCode(500), e)
        }
    }
}

/// `/api/v1/settings/test/{llm,tts}` 子模块（拆分以控制文件行数）。
pub mod test_endpoints;

// 把 test 端点 handler 重新导出，保留外部 `use settings_routes::handle_test_*`
// 路径。`run_llm_test` / `timeout_to_duration` 仅在 `#[cfg(test)]` 内通过
// `super::*` 引用（非测试 build 下不会用）；这里统一允许告警以保持单一
// re-export 入口。
#[allow(unused_imports)]
pub use test_endpoints::{TestBody, TestOutcome, handle_test_llm, handle_test_tts};
#[cfg(test)]
pub use test_endpoints::{run_llm_test, timeout_to_duration};

/// 构造错误响应（detail.code → HTTP status 已在调用方决定）。
pub fn error_response(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(status, &ErrorResponse::new(ErrorDetail::new(code, message)))
}

fn error_response_from(
    status: StatusCode,
    err: ErrorResponse,
) -> Response<std::io::Cursor<Vec<u8>>> {
    json_response(status, &err)
}

#[cfg(test)]
mod tests;
