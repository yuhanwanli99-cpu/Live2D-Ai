//! 脱敏视图 [`SettingsView`]：把 [`AppSettings`](super::AppSettings) 转成可序列化
//! 的、**不含任何密钥明文**的扁平结构供 Web API / egui 设置面板共用。
//!
//! 字段命名与 D1 契约一致：LLM/TTS 的 `api_key_env` 字段在这里换成
//! `has_api_key: bool`。`SettingsView` 有**两个构造口径**，用途不同：
//!
//! - [`settings_to_view`]：`has_api_key` = **「配置里声明了变量名」**。
//!   纯展示 / egui 设置面用它；不读环境、不看 `.env`，因此「已配置」可以
//!   与真实 401 同屏——这是刻意的弱口径。
//! - [`settings_to_view_with_keys`]：`has_api_key` = **「声明了变量名且
//!   `lookup(name)` 拿得到非空值」**。Web API（GET/PATCH `/api/v1/settings`）
//!   用它，保证「已配置」与 401/503 不会同屏；`lookup` 由调用方注入
//!   （生产路径传 [`crate::secrets::lookup`]），本模块不直接读 env。

use serde::Serialize;

use super::{
    ActionSettings, AppSettings, LlmSettings, PerformanceSettings, PersonaSettings, TtsSettings,
};

/// `AppSettings` 的脱敏只读视图，序列化结果可直接交给 Web API。
///
/// 字段名与 `AppSettings` 对齐，**唯一例外**：
/// - `llm.api_key_env` / `tts.api_key_env` 字段被替换成 `has_api_key: bool`，
///   永远不暴露环境变量名（也永远不暴露任何明文密钥——`AppSettings` 本来
///   就只持环境变量**名**，不是密钥本身，但「变量名」也属于敏感元数据）。
/// - 顶层 `dev_mode: bool` 透传（不涉敏感元数据；供设置面板与 PATCH
///   反馈使用，与 `AppStatus.dev_mode` 对齐）。
// 含 f32（action 倍率）→ 只能 PartialEq。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingsView {
    /// LLM 段。
    pub llm: LlmView,
    /// TTS 段。
    pub tts: TtsView,
    /// persona 段。
    pub persona: PersonaView,
    /// 动作幅度段（`[action]`）：头 / 身 / 表情三条独立倍率。
    pub action: ActionView,
    /// 表演层段（`[performance]`，2026-09-22）。**只读展示**：启停与端点写在
    /// `live2d-ai.toml`（本波不提供 PATCH 入口），面板据此显示现状 +
    /// 「主模型不负责表演；表演层每轮 JSON」。
    pub performance: PerformanceView,
    /// 开发者模式开关（W7 任务：可配置运行时开关；与 `AppStatus.dev_mode`
    /// 共享唯一来源 `AppSettings.dev_mode`）。
    pub dev_mode: bool,
}

/// 视图中的 LLM 段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LlmView {
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    /// **最终生效**的输出 token 上限（`0` = 不限制）。
    ///
    /// 回的是生效值而不是原始 `Option`：界面要显示「现在到底卡在多少」，
    /// 显示 `null` 对用户没有意义。
    pub max_tokens: u32,
    /// 展示思考总闸的**生效值**（省略 = false）。
    ///
    /// 与 `max_tokens` 同口径：界面要看到「现在到底开没开」，
    /// 而不是看到 `null` 再自己猜缺省。
    pub show_reasoning: bool,
}

/// 视图中的 TTS 段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TtsView {
    pub base_url: String,
    pub model: Option<String>,
    pub voice: String,
    pub has_api_key: bool,
    pub sample_rate: u32,
    pub channels: u16,
}

/// 视图中的 persona 段（rc.4 M5：主链只留 `system_prompt` + `max_history_pairs`）。
///
/// 酒馆卡字段不再出现在主设置里——它们属于标准 Mod `live2d-ai-mod-persona`，
/// 经 `GET /api/v1/mods` 的 settings_spec 暴露。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersonaView {
    pub system_prompt: String,
    pub max_history_pairs: usize,
}

/// 视图中的 `[action]` 段（三项动作幅度倍率，2026-09-16）。
///
/// 回的是**归一化后**的生效值（逐个钳进 `[0.2, 2.2]`）——界面滑条要显示
/// 「现在到底是多少」，显示越界原值会让用户以为钳位没生效。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActionView {
    pub head_scale: f32,
    pub body_scale: f32,
    pub expression_scale: f32,
}

impl From<&ActionSettings> for ActionView {
    fn from(s: &ActionSettings) -> Self {
        let n = s.normalized();
        Self {
            head_scale: n.head_scale,
            body_scale: n.body_scale,
            expression_scale: n.expression_scale,
        }
    }
}

impl From<&LlmSettings> for LlmView {
    fn from(s: &LlmSettings) -> Self {
        Self {
            base_url: s.base_url.clone(),
            model: s.model.clone(),
            has_api_key: s.api_key_env.as_deref().is_some_and(|n| !n.is_empty()),
            max_tokens: s.effective_max_tokens(),
            show_reasoning: s.effective_show_reasoning(),
        }
    }
}

impl From<&TtsSettings> for TtsView {
    fn from(s: &TtsSettings) -> Self {
        Self {
            base_url: s.base_url.clone(),
            model: s.model.clone(),
            voice: s.voice.clone(),
            has_api_key: s.api_key_env.as_deref().is_some_and(|n| !n.is_empty()),
            sample_rate: s.sample_rate,
            channels: s.channels,
        }
    }
}

/// 视图中的 `[performance]` 段。
///
/// `effective_timeout_ms` 回的是**钳位后**的生效值；`api_key_env` 换成
/// `has_api_key`（与 llm/tts 同一条脱敏纪律：变量名也不出门）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PerformanceView {
    pub enabled: bool,
    /// 开了闸且端点/模型都配齐——**这一位才是「会发 HTTP」**。
    pub wired: bool,
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    pub timeout_ms: u64,
    pub structured: String,
}

impl From<&PerformanceSettings> for PerformanceView {
    fn from(s: &PerformanceSettings) -> Self {
        Self {
            enabled: s.enabled,
            wired: s.is_wired(),
            base_url: s.base_url.clone(),
            model: s.model.clone(),
            has_api_key: s
                .api_key_env
                .as_deref()
                .is_some_and(|n| !n.trim().is_empty()),
            timeout_ms: s.effective_timeout_ms(),
            structured: s.structured.clone(),
        }
    }
}

impl From<&PersonaSettings> for PersonaView {
    fn from(s: &PersonaSettings) -> Self {
        Self {
            system_prompt: s.system_prompt.clone(),
            max_history_pairs: s.max_history_pairs,
        }
    }
}

/// 把 [`AppSettings`] 转成脱敏的 [`SettingsView`]（**弱口径**：`has_api_key`
/// = 「配置里声明了非空变量名」，**不**验证值是否真读得到）。
///
/// 纯函数：不读文件、不读环境、不做 IO。
///
/// # 与 [`settings_to_view_with_keys`] 的差别（别混用）
///
/// 本函数是 **egui / 纯展示** 口径：它连 `.env` 都不看，所以回答的是
/// 「配置里有没有声明键名」。凡是把 `has_api_key` 与 401/503 放在同一屏
/// 的调用方（Web API），必须走 [`settings_to_view_with_keys`]——否则
/// 「已配置」会与链路鉴权失败同屏。签名与语义**冻结不变**。
pub fn settings_to_view(s: &AppSettings) -> SettingsView {
    SettingsView {
        llm: (&s.llm).into(),
        tts: (&s.tts).into(),
        persona: (&s.persona).into(),
        action: (&s.action).into(),
        performance: (&s.performance).into(),
        dev_mode: s.dev_mode,
    }
}

/// `has_api_key` 的**单一语义**派生（llm / tts / performance 三段共用）。
///
/// `true` 当且仅当「段里声明了非空变量名」**且** `lookup(name)` 返回非空值。
/// 名字未声明 / 为空时**不调用** `lookup`（空名不是合法变量名，也不该被
/// 当成一个可能的键去查）。
fn key_is_ready(env_name: Option<&str>, lookup: &dyn Fn(&str) -> Option<String>) -> bool {
    let Some(name) = env_name.filter(|n| !n.is_empty()) else {
        return false;
    };
    lookup(name).is_some_and(|v| !v.is_empty())
}

/// 与 [`settings_to_view`] 同构，但 `has_api_key` 收敛为**单一语义**：
/// 「声明了变量名」**且**「值真的读得到」。
///
/// # 为什么要有这个函数（P2 收敛）
///
/// 收敛前 Web API 用的也是 [`settings_to_view`]（=「声明了变量名」），
/// 而 `dto::llm_status_from` / `tts_status_from` 用的是「声明了且有值」——
/// **同名两义**，于是界面上「已配置」能与链路 401/503 同屏。Web API 一律
/// 改走本函数后，两处含义一致。
///
/// `lookup` 由调用方注入（生产路径传 [`crate::secrets::lookup`]）：本函数
/// 保持纯函数——不读文件、不读环境、不做 IO，也**不碰** `std::env::var`
/// （见 AGENTS「密钥真源 = .env」）。返回值里永远不含变量名与密钥明文。
pub fn settings_to_view_with_keys(
    s: &AppSettings,
    lookup: &dyn Fn(&str) -> Option<String>,
) -> SettingsView {
    let mut view = settings_to_view(s);
    view.llm.has_api_key = key_is_ready(s.llm.api_key_env.as_deref(), lookup);
    view.tts.has_api_key = key_is_ready(s.tts.api_key_env.as_deref(), lookup);
    view.performance.has_api_key = key_is_ready(s.performance.api_key_env.as_deref(), lookup);
    view
}
