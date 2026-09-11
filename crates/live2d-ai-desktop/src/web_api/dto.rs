//! Web API 数据传输对象（DTO）层（D1 契约第一批）。
//!
//! 边界：
//! - **DTO 形态与 §6 字段对齐表一致**（snake_case，字段名固定）；
//! - **P0-1 派生 `has_api_key`**：本模块**手工**生成 DTO，**不**直接
//!   `serde_json::to_value(&AppSettings)`（后者序列化 `api_key_env` 字段名；
//!   本模块用 [`runtime::settings::view::settings_to_view`] 转脱敏视图）。
//! - **错误响应统一**（D1 §5）：`{"error":{"code","message","details"}}`，
//!   `code` 是稳定契约字符串。
//!
//! DTO 字段稳定；新增字段需先在 §6 字段表登记再实现。

use serde::Serialize;

use live2d_ai_runtime::settings::view::SettingsView;

/// `GET /api/v1/app/capabilities` 响应。
///
/// 能力快照——前端用它决定按钮/页签是否显示，**不**依赖写盘/网络。
/// `schema_version` 是契约版本（加字段 = 递增；§6 字段表 = 宪法）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppInfo {
    /// 应用名（稳定字符串）。
    pub app: &'static str,
    /// 语义化版本（`env!("CARGO_PKG_VERSION")`，编译期固化）。
    pub version: &'static str,
    /// DTO schema 版本（v1 = 1；新增字段递增，便于前端 gating）。
    pub schema_version: u32,
    /// 可用动作列表（snake_case；与 [`live2d_ai_core::ActionId::name`] 对齐）。
    pub actions: Vec<&'static str>,
    /// 动作来源（snake_case；与 [`live2d_ai_core::ActionSource`] 对齐）。
    pub action_sources: Vec<&'static str>,
    /// 强度档位（恒为 `[1, 2, 3]`）。
    pub strength_levels: [u8; 3],
    /// 是否支持模型上传（v1 = `true`；D3 实现）。
    pub model_upload_supported: bool,
    /// 是否支持脚本调用（v1 = `true`；D4 实现）。
    pub script_invoke_supported: bool,
    /// 运行时 WebSocket 端点路径。
    pub runtime_ws: &'static str,
    /// 状态 WebSocket 端点路径。
    pub state_ws: &'static str,
    /// WebSocket 协议版本（D2 引入时填）。
    pub ws_protocol_version: u32,
}

/// `GET /api/v1/app/status` 响应。
///
/// 运行时状态摘要：LLM/TTS 配置存在性 + 进程 env 是否有非空密钥。
/// 字段全部**派生**——不持可变状态。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AppStatus {
    /// 进程启动时刻（ISO 8601 UTC，毫秒精度；启动时计算）。
    pub started_at: String,
    /// 启动至今秒数（`u64`）。
    pub uptime_s: u64,
    /// 当前生效的配置文件路径（`live2d-ai.toml` 解析路径；v1 = 启动时确定）。
    pub config_path: String,
    /// 当前激活的模型 id（v1 = "bai_001"；D3 实现可配置）。
    pub active_model_id: &'static str,
    /// 音频后端描述。
    pub audio: AudioStatus,
    /// LLM 配置摘要。
    pub llm: LlmStatus,
    /// TTS 配置摘要。
    pub tts: TtsStatus,
    /// dev-mode 开关（v1 = `false`；D4 命令台开启）。
    pub dev_mode: bool,
    /// 当前 epoch（v1 = 0，未接入 supervisor；D2 接入）。
    pub current_epoch: u64,
}

/// 音频后端状态（v1 占位：未启动 supervisor 时 `available=false`）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AudioStatus {
    /// 后端名（`"alsa"` / `"pulseaudio"` / `"none"`）。
    pub backend: &'static str,
    /// 是否可用。
    pub available: bool,
    /// 当前采样率。
    pub sample_rate: u32,
    /// 当前声道数。
    pub channels: u16,
}

/// LLM 配置摘要（has_api_key 派生，**不**携带密钥）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct LlmStatus {
    /// 是否已配置 base_url（空 = 未配置）。
    pub configured: bool,
    /// 当前 base_url。
    pub base_url: String,
    /// 当前 model 名。
    pub model: String,
    /// 进程 env 中密钥变量是否解析到非空值。
    pub has_api_key: bool,
}

/// TTS 配置摘要（has_api_key 派生）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtsStatus {
    pub configured: bool,
    pub base_url: String,
    pub model: Option<String>,
    pub voice: String,
    pub has_api_key: bool,
    pub sample_rate: u32,
    pub channels: u16,
}

/// 统一错误响应（§5）。
///
/// 形态：`{"error":{"code":"<stable_id>","message":"<human>","details":{...}}}`。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorResponse {
    /// 错误详情。
    pub error: ErrorDetail,
}

/// 错误详情（`code` / `message` / `details` 三段式）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorDetail {
    /// 稳定错误码（`invalid_payload` / `url_invalid` / `auth_failed` / ...）。
    pub code: &'static str,
    /// 人类可读描述（i18n 友好；仅 message 可演进，code 禁改动）。
    pub message: String,
    /// 附加字段（`field` / `value` / `id` 等；按 `code` 约定）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ErrorDetail {
    /// 构造错误详情（无 details）。
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    /// 构造错误详情（含 details JSON 对象）。
    pub fn with_details(
        code: &'static str,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            details: Some(details),
        }
    }
}

impl ErrorResponse {
    /// 构造错误响应包装。
    pub fn new(detail: ErrorDetail) -> Self {
        Self { error: detail }
    }
}

/// PATCH 写盘后 supervisor 装载/重载状态（D-P0C，2026-08-29）。
///
/// 与 [`crate::web_api::settings_routes::PatchResponse::apply_status`] 字段
/// 一一对应；前端按此字段决定文案（避免「已热重载并生效」与「实际 503」
/// 错位）。**前端字符串锚点**（app.js 会硬编码这些字面量；改动需同步）。
#[allow(dead_code)] // `Queued` 保留作未来异步 reload；其余变体由 settings_routes 序列化使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplyStatus {
    /// supervisor 动态创建成功（或已存在并 reload 已触发）——前端可立即
    /// 启用对话区（POST `/api/v1/chat` 会回 200 accepted）。
    Applied,
    /// 写盘成功，但 supervisor 无法动态创建（如 LLM/TTS 配置不全）；
    /// 需要重启进程或修复配置后重启。前端文案：「已保存，需重启生效」。
    RestartRequired,
    /// reload 已排队但尚未同步生效（异步生效中）。当前实现下
    /// 装配/重建均在请求线程内完成，**实际不会产生 Queued**；保留作
    /// 未来异步 reload 路径。
    Queued,
    /// 写盘成功但无 supervisor（如 dev-only 控制平面 / 关闭 chat 引擎
    /// 模式）；reload 路径 no-op。前端文案：「已保存，配置将在下次重启后生效」。
    NoSupervisor,
}

impl ApplyStatus {
    /// 稳定字符串（前端 / 日志共用）。
    #[allow(dead_code)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::RestartRequired => "restart_required",
            Self::Queued => "queued",
            Self::NoSupervisor => "no_supervisor",
        }
    }
}

/// 从 [`AppSettings`] 派生 LLM 状态（不读 env——`has_api_key` 由 env_lookup 决定）。
///
/// `env_lookup` 接收 `api_key_env` 名字，返回 `Some(value)` 表示进程 env
/// 已设置非空值。`AppSettings::api_key_env` 为 `None` 或空串时
/// `has_api_key` 一律 `false`（视图层口径与 §6.2 字段对齐表一致）。
pub fn llm_status_from(
    settings: &live2d_ai_runtime::AppSettings,
    env_lookup: &dyn Fn(&str) -> Option<String>,
) -> LlmStatus {
    let has_api_key = settings
        .llm
        .api_key_env
        .as_deref()
        .map(|n| !n.is_empty())
        .unwrap_or(false)
        && env_lookup(settings.llm.api_key_env.as_deref().unwrap_or(""))
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    LlmStatus {
        configured: !settings.llm.base_url.is_empty(),
        base_url: settings.llm.base_url.clone(),
        model: settings.llm.model.clone(),
        has_api_key,
    }
}

/// 从 [`AppSettings`] 派生 TTS 状态。
pub fn tts_status_from(
    settings: &live2d_ai_runtime::AppSettings,
    env_lookup: &dyn Fn(&str) -> Option<String>,
) -> TtsStatus {
    let has_api_key = settings
        .tts
        .api_key_env
        .as_deref()
        .map(|n| !n.is_empty())
        .unwrap_or(false)
        && env_lookup(settings.tts.api_key_env.as_deref().unwrap_or(""))
            .map(|v| !v.is_empty())
            .unwrap_or(false);
    TtsStatus {
        configured: !settings.tts.base_url.is_empty(),
        base_url: settings.tts.base_url.clone(),
        model: settings.tts.model.clone(),
        voice: settings.tts.voice.clone(),
        has_api_key,
        sample_rate: settings.tts.sample_rate,
        channels: settings.tts.channels,
    }
}

/// 把 [`SettingsView`] 复用为 DTO 字段（`GET /api/v1/settings` 响应）。
///
/// D1 契约：响应中**仅**含 LLM/TTS/Persona 三段，全部已脱敏（`has_api_key`
/// 派生，不含 `api_key_env` 字段名）。`SettingsView` 已是该形态——直接
/// 转发。本函数存在是为了**集中**「response 包装」层，便于 D2 引入
/// `stage` 段时单点扩展。
#[allow(dead_code)] // D2 引入 stage 段时单点扩展会启用。
pub fn settings_view_as_dto(view: SettingsView) -> SettingsView {
    view
}

pub fn capabilities_info() -> AppInfo {
    AppInfo {
        app: "live2d-ai-desktop",
        version: env!("CARGO_PKG_VERSION"),
        schema_version: 1,
        // 与 live2d_ai_core::ActionId::ALL 顺序一致（nod/shake_no/tilt/look_around/listen/surprise）。
        actions: vec![
            "nod",
            "shake_no",
            "tilt",
            "look_around",
            "listen",
            "surprise",
        ],
        // 与 live2d_ai_core::ActionSource 三个变体对应（user_command/llm_tool/rule_fallback）。
        action_sources: vec!["user_command", "llm_tool", "rule_fallback"],
        strength_levels: [1, 2, 3],
        model_upload_supported: true,
        script_invoke_supported: true,
        runtime_ws: "/ws/runtime",
        state_ws: "/ws/state",
        ws_protocol_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use live2d_ai_runtime::AppSettings;
    use live2d_ai_runtime::settings::view::settings_to_view;
    use live2d_ai_runtime::settings::{LlmSettings, PersonaSettings, TtsSettings};

    fn sample() -> AppSettings {
        AppSettings {
            llm: LlmSettings {
                base_url: "http://127.0.0.1:11434/v1".into(),
                model: "qwen2.5:7b".into(),
                api_key_env: Some("LIVE2D_AI_LLM_API_KEY".into()),
                max_tokens: None,
            },
            tts: TtsSettings {
                base_url: "http://127.0.0.1:8000/v1".into(),
                model: Some("tts-1".into()),
                voice: "alloy".into(),
                response_format: "pcm".into(),
                api_key_env: Some("LIVE2D_AI_TTS_API_KEY".into()),
                sample_rate: 24_000,
                channels: 1,
            },
            persona: PersonaSettings {
                system_prompt: "你是桌宠".into(),
                max_history_pairs: 2,
                name: "NEKO".into(),
                description: "一只会说话的猫娘桌宠。".into(),
                ..Default::default()
            },
            dev_mode: false,
        }
    }

    #[test]
    fn app_info_carries_version_and_stable_fields() {
        let info = capabilities_info();
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["app"], "live2d-ai-desktop");
        // version 来自编译期 CARGO_PKG_VERSION，非空字符串即可。
        assert!(json["version"].is_string());
        assert!(!json["version"].as_str().unwrap().is_empty());
        assert_eq!(json["schema_version"], 1);
        // 6 个动作（与 core ActionId::ALL 一致）。
        let actions = json["actions"].as_array().unwrap();
        assert_eq!(actions.len(), 6);
        assert!(actions.contains(&serde_json::json!("nod")));
        // 3 个 source。
        let sources = json["action_sources"].as_array().unwrap();
        assert_eq!(sources.len(), 3);
        // 强度档位。
        assert_eq!(json["strength_levels"], serde_json::json!([1, 2, 3]));
        // WS 路径。
        assert_eq!(json["runtime_ws"], "/ws/runtime");
        assert_eq!(json["state_ws"], "/ws/state");
    }

    #[test]
    fn settings_view_dto_never_leaks_key_name() {
        let s = sample();
        let view = settings_to_view(&s);
        let dto = settings_view_as_dto(view);
        let json = serde_json::to_string(&dto).unwrap();
        // P0-1：响应里既无 api_key_env 字段名，也无任何密钥明文。
        assert!(!json.contains("api_key_env"), "暴露 env 字段名：{json}");
        assert!(
            !json.contains("LIVE2D_AI_LLM_API_KEY"),
            "env 名泄漏：{json}"
        );
        assert!(
            !json.contains("LIVE2D_AI_TTS_API_KEY"),
            "env 名泄漏：{json}"
        );
        // has_api_key 字段存在（值为 true 因为 api_key_env 非空）。
        assert!(json.contains("\"has_api_key\":true"));
        // 验证 settings_view_as_dto 是恒等函数（透传 view）。
        let s2 = AppSettings::default();
        let v2 = settings_to_view(&s2);
        let j_default = serde_json::to_string(&settings_view_as_dto(v2)).unwrap();
        // 默认 settings（api_key_env=None）→ has_api_key=false。
        assert!(j_default.contains("\"has_api_key\":false"));
    }

    #[test]
    fn error_response_serializes_to_contract_shape() {
        let r = ErrorResponse::new(ErrorDetail::new(
            "invalid_payload",
            "字段 llm.base_url 不是合法 URL",
        ));
        let json: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(json["error"]["code"], "invalid_payload");
        assert!(json["error"]["message"].is_string());
        assert!(json["error"].get("details").is_none());
    }

    #[test]
    fn error_response_with_details_includes_field() {
        let r = ErrorResponse::new(ErrorDetail::with_details(
            "url_invalid",
            "字段 llm.base_url 不是合法 URL",
            serde_json::json!({"field":"llm.base_url","value":"not a url"}),
        ));
        let json: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(json["error"]["code"], "url_invalid");
        assert_eq!(json["error"]["details"]["field"], "llm.base_url");
    }

    #[test]
    fn llm_status_derives_has_api_key_from_env() {
        let s = sample();
        // 注入 env：`LIVE2D_AI_LLM_API_KEY=sk-LIVE2D_AI_LLM_API_KEY-INJECTED`。
        let st = llm_status_from(&s, &|name| {
            if name == "LIVE2D_AI_LLM_API_KEY" {
                Some("sk-LIVE2D_AI_LLM_API_KEY-INJECTED".into())
            } else {
                None
            }
        });
        assert!(st.configured);
        assert!(st.has_api_key);
        // 反向：无 env → false。
        let st = llm_status_from(&s, &|_| None);
        assert!(!st.has_api_key);
        // api_key_env 字段不在 DTO 出现。
        let json = serde_json::to_string(&st).unwrap();
        assert!(!json.contains("api_key_env"));
    }

    #[test]
    fn llm_status_unconfigured_when_base_url_empty() {
        let mut s = sample();
        s.llm.base_url.clear();
        let st = llm_status_from(&s, &|_| None);
        assert!(!st.configured);
        assert!(!st.has_api_key);
    }
}
