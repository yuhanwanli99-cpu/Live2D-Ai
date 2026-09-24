//! 设置管理路由（`/api/v1/settings` + `/api/v1/settings/test/{llm,tts}`，D1 §1.2）。
//!
//! 责任：
//! - `GET /api/v1/settings`：返回脱敏 [`SettingsView`]（**不**含密钥——
//!   P0-1）。走 [`live2d_ai_runtime::settings::view::settings_to_view_with_keys`]，
//!   `has_api_key` 由**调用方注入的 `lookup`** 决定（= 「值真的读得到」）；
//!   本路由自己不读 env（不放行 `std::env::var`，见 AGENTS「密钥真源 = .env」）。
//! - `PATCH /api/v1/settings`：body 解析为 [`PatchBody`] → 内部归并为
//!   [`SettingsPatch`] → [`apply_patch`] → 若 [`PatchOutcome::Updated`] 再经
//!   [`plan_atomic_write`] 写盘 + rename。
//! - `POST /api/v1/settings/test/llm` / `test/tts`：用「暂存配置」发一次
//!   最小请求，**不**落盘——失败分类按 D1 错误码。
//!
//! 安全：
//! - P0-1：响应中**永不出密钥**——脱敏视图保证。
//!
//! # 密钥绑定：单一三态规则（2026-09 P5 起）
//!
//! `{"llm":{"api_key_env":null}}`（`tts` 段同款）**就是**清除绑定：字段级
//! `Option<Option<String>>` 的显式 `null` 直接落进
//! [`live2d_ai_runtime::settings::patch::LlmPatch::api_key_env`]，由
//! [`apply_patch`] 清空并触发写盘。
//!
//! 这里**没有** `clear_api_key` 标志，也**没有**「无 clear 标志的
//! `api_key_env: null` 视为保持原值」的改写层：曾经的双层包装（桌面层
//! `PatchLlmBody`/`PatchTtsBody` + `inject_clear_key_flag`）已删除——
//! 一个字段有两种「清除」语法时前端与服务端各发一套，语义漂移时没人知道
//! 该信哪个；前端 `Tri.clear()`（序列化为 `null`）本来就能可靠表达「清空」。
//! 注：body 里再出现 `clear_api_key` 现在是**未知键**，按 serde 缺省被忽略。
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
use live2d_ai_runtime::settings::view::settings_to_view_with_keys;

use crate::web_api::app_routes::json_response;
use crate::web_api::dto::{ApplyStatus, ErrorDetail, ErrorResponse};

/// `GET /api/v1/settings` 处理器。
///
/// `lookup` 由 dispatch 层注入（生产路径 = `live2d_ai_runtime::secrets::lookup`），
/// 让响应里的 `has_api_key` 与 DTO 状态口径一致：**声明了键名且值真的读得到**。
pub fn handle_get(
    current: &AppSettings,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let view = settings_to_view_with_keys(current, lookup);
    json_response(StatusCode(200), &view)
}

/// PATCH body：与 runtime [`SettingsPatch`] **逐段同形**（P5 收敛）。
///
/// 各段都是段级三态 `Option<Option<…>>`：
/// - 段缺省 → `None`（不参与）；
/// - 段是 `null` → `Some(None)`（整段清空，走 runtime 段级清空语义）；
/// - 段是对象 → `Some(Some(patch))`（字段级三态逐字段合并）。
///
/// 段内**直接**持有 runtime 的 `LlmPatch` / `TtsPatch` / …：字段级三态
/// （缺省 / `null` / 值）由 runtime 类型自己的 `double_option` serde 属性
/// 负责。P5 前那层 `PatchLlmBody` / `PatchTtsBody` 包装（`#[serde(flatten)]`
/// + 段级 `clear_api_key`）已删除——见模块头「单一三态规则」。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PatchBody {
    /// 见 [`SettingsPatch::llm`]。字段级 `api_key_env: null` = 清除绑定。
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub llm: Option<Option<live2d_ai_runtime::settings::patch::LlmPatch>>,
    /// 见 [`SettingsPatch::tts`]（同 `llm`）。
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub tts: Option<Option<live2d_ai_runtime::settings::patch::TtsPatch>>,
    /// 见 [`SettingsPatch::persona`]。
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub persona: Option<Option<live2d_ai_runtime::settings::patch::PersonaPatch>>,
    /// 见 [`SettingsPatch::action`]（2026-09-16，动作幅度倍率）。
    #[serde(
        default,
        deserialize_with = "::serde_with::rust::double_option::deserialize"
    )]
    pub action: Option<Option<live2d_ai_runtime::settings::patch::ActionPatch>>,
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

impl PatchBody {
    /// 解析 JSON body 字符串为 [`PatchBody`]。
    pub fn parse(body: &str) -> Result<Self, String> {
        serde_json::from_str(body).map_err(|e| format!("JSON 解析失败: {e}"))
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
    lookup: &dyn Fn(&str) -> Option<String>,
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
            // PATCH 响应与 GET 同口径（都走 with_keys），否则「保存后
            // 界面显示已配置、但 GET 又说没有」这类两义会在同一面板里打架。
            settings: settings_to_view_with_keys(&next, lookup),
            apply_status,
        });
    }
    // NoChange：盘上无变化 → 不调 hook（不视为热重载机会），但仍返回
    // 当前视图。apply_status 用一个保守的「重启」状态——但前端只在
    // `persisted=true` 时才解析 apply_status 字段（写入未发生 = 文案
    // 无意义），所以这里选什么都安全。
    Ok(PatchResponse {
        persisted,
        settings: settings_to_view_with_keys(&next, lookup),
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
    lookup: &dyn Fn(&str) -> Option<String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let body = match PatchBody::parse(body_str) {
        Ok(b) => b,
        Err(e) => {
            // 2026-09-11：解析失败也要留痕（过去只进 HTTP 响应体，日志里没有）。
            tracing::warn!(code = "invalid_payload", "PATCH body 解析失败：{e}");
            return error_response(StatusCode(400), "invalid_payload", e);
        }
    };
    // 段级三态**直接透传**：PatchBody 的 llm/tts 已经是
    // `Option<Option<LlmPatch>>` / `Option<Option<TtsPatch>>`，与
    // SettingsPatch 逐段同形（P5 起不再有 PatchLlmBody/PatchTtsBody 包装，
    // 也没有 clear_api_key 注入阶段）。
    let patch = SettingsPatch {
        llm: body.llm,
        tts: body.tts,
        persona: body.persona,
        action: body.action,
        // W7 任务：dev_mode 三态由 HTTP body 顶层字段提供（不走段级）。
        dev_mode: body.dev_mode,
    };
    match apply_and_write(current, &patch, config_path, after_hook, lookup) {
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
