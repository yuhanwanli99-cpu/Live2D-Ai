//! 应用级配置加载 [`AppSettings`]：把 `live2d-ai.toml`（+ 环境变量密钥）解析成可直接使用的 [`LlmConfig`] / [`TtsConfig`] / [`ConversationConfig`]。
//!
//! # 行数（AGENTS.md「源码 ≤500 行，豁免 ≤1000 需头注理由」）
//!
//! 本文件 >500 行。理由：它是**唯一**一份「TOML 形状 + 解析 + 校验 + 保注释写回」
//! 的整体——每加一个配置段（[tts] / [persona] / [action] / [performance]）都要在
//! 「结构体 + resolve + 视图 + 序列化」四处同步，拆文件会让「改一半忘了另一半」
//! 变成默认风险（那正是 rc.5 注释被清空、快照与磁盘分叉那类缺陷的成因）。
//! 仍在 1000 行豁免上限内；测试已外提到 `settings_tests` / `patch_tests`。
//!
//! # 配置来源与边界
//!
//! - **文件**只放非敏感项：`base_url`、模型名、音色、persona 提示词；
//! - **密钥不进文件**：写环境变量**名**（`api_key_env`），真实 key 在运行环境里；
//!   解析经 `resolve`/`resolve_with` 读出并包成 [`ApiSecret`]；变量未设置或为空串一律视为「无鉴权」。
//! - 未知字段**拒绝解析**（`deny_unknown_fields`）：拼写错误在启动时暴露。
//!
//! # 文件示例
//!
//! ```toml
//! [llm]
//! base_url = "http://127.0.0.1:11434/v1"
//! model = "qwen2.5:7b"
//!
//! [tts]
//! base_url = "http://127.0.0.1:8000/v1"
//! voice = "alloy"
//!
//! [persona]
//! system_prompt = "你是桌宠，用简短口语回答。"
//! ```
//! 全部字段与默认值见 [`AppSettings::example_toml`]。视图/补丁/原子写回规划见子模块 [`view`] / [`patch`]（`patch_tests` 仅测试构建，避免 patch.rs 撞 500 行硬上限）。
pub mod patch;
#[cfg(test)]
mod patch_tests;
/// 2026-10-09：`[tts]` 语音来源二选一（local / cloud）的回归**单列**——
/// `patch_tests` 已贴着「`crates/*/src > 1000 行`」棘轮（上限 0）。
#[cfg(test)]
#[path = "settings/patch_tts_mode_tests.rs"]
mod patch_tts_mode_tests;
#[cfg(test)]
mod settings_tests;
pub mod view;
use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::audio::AudioSpec;
use crate::config::{LlmConfig, TtsConfig};
use crate::conversation::ConversationConfig;
use crate::secret::ApiSecret;

/// 配置解析错误：文件读取、TOML 语法/字段、URL 与环境变量名校验。
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// 读文件失败（不存在 / 无权限）。
    #[error("读取配置文件失败 ({path}): {source}")]
    Io {
        /// 配置文件路径。
        path: String,
        /// 底层 IO 错误。
        #[source]
        source: std::io::Error,
    },

    /// TOML 解析失败（语法错误、缺必填字段、未知字段）。
    #[error("配置文件解析失败: {0}")]
    Parse(#[from] toml::de::Error),

    /// `base_url` 非法：无法解析为绝对 URL 或 scheme 不是 http(s)。
    #[error("{section} base_url 非法: {url:?}（需要绝对 http/https URL）")]
    InvalidBaseUrl {
        /// 所属配置段名（`llm` / `tts`），便于定位。
        section: &'static str,
        /// 原始字符串。
        url: String,
    },

    /// `api_key_env` 不是合法的环境变量名。
    #[error(
        "{section} api_key_env 非法: {name:?}（仅允许 ASCII 字母/数字/下划线，且不以数字开头）"
    )]
    InvalidEnvName {
        /// 所属配置段名。
        section: &'static str,
        /// 原始字符串。
        name: String,
    },

    /// `[tts]` 的 PCM 规格非法（采样率/声道数为 0）。
    #[error("tts 音频规格非法: {0}")]
    InvalidAudioSpec(#[from] crate::error::Error),
}

/// `live2d-ai.toml` 的反序列化形态（字段全部有默认值的最小文件即可工作）。
///
/// 构造 [`Self`] 只做「形状」解析；URL / 环境变量名校验与密钥读取发生在
/// [`AppSettings::resolve`]，让「语法错误」和「运行环境问题」分开报告。
// 注意：`AppSettings` 含 f32（`[action]` 的幅度倍率），因此**不能** derive
// `Eq`——只有 `PartialEq`。需要 Eq 的旧断言都是对不含 action 的子段。
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
// 容器级 default：任一段整体缺省时回退到该段类型的 `Default`（deny_unknown_fields
// 与它独立——只拦「写了但拼错」的键，不拦「没写」的段）。
#[serde(default, deny_unknown_fields)]
pub struct AppSettings {
    /// LLM 段（`[llm]`）。
    pub llm: LlmSettings,
    /// TTS 段（`[tts]`）。
    pub tts: TtsSettings,
    /// persona 段（`[persona]`），可整体省略。
    pub persona: PersonaSettings,
    /// 动作幅度段（`[action]`，2026-09-16 用户可调）：头 / 身 / 表情三条独立倍率。
    pub action: ActionSettings,
    /// 表演层段（`[performance]`，2026-09-22）：主模型之外的「导演/大脑」。
    /// **缺省关**（`enabled=false`）——关掉时主链行为与没有本段时逐字一致。
    pub performance: PerformanceSettings,
    /// 开发者模式开关（默认 false）：打开后 `/api/v1/logs*` 端点放行
    /// （见 W3 任务约定）。可在 `live2d-ai.toml` 顶层显式写 `dev_mode = true`，
    /// 或由 CLI `--dev-mode` 覆盖，或在设置面板（egui）勾选后 PATCH 落盘。
    /// 来源优先级：CLI flag > settings 文件 > 默认 false。
    pub dev_mode: bool,
}

/// 配置里声明的一个密钥环境变量（`GET /api/v1/env` 的一条）。
///
/// 由 [`AppSettings::declared_key_envs`] 产出——**那份函数才是
/// `GET/PUT /api/v1/env` 的唯一真源**；路由层不得再各自抄一份
/// `llm` / `tts` / `performance` 段清单（抄两遍正是「`[performance]`
/// 的键既列不出也写不进」这个缺陷的成因）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredKeyEnv {
    /// 归属段：`"llm"` / `"tts"` / `"performance"`。
    pub section: &'static str,
    /// 环境变量名（**非空**，只持有变量名；值只住 `.env`）。
    pub name: String,
}

/// `[llm]` 段：OpenAI-compatible `/chat/completions` 上游。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LlmSettings {
    /// 服务基址（如 `http://127.0.0.1:11434/v1`）。
    #[serde(default)]
    pub base_url: String,
    /// 模型名，进入请求体 `model` 字段。
    #[serde(default)]
    pub model: String,
    /// 可选：存放 API key 的环境变量名。省略 = 不鉴权。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 输出 token 上限。**省略 = 用 [`DEFAULT_MAX_TOKENS`]**；
    /// 显式 `0` = 不限制（请求体里省略该字段）。
    ///
    /// 这是控制回复长度的**机制**：不靠提示词求模型守规矩，而是给出硬上限。
    /// **注意下限陷阱**：设得太小会把句子**截断在半句**，那本身就是
    /// 「断句」——正是本项目明确不能接受的现象。所以默认值取得偏宽。
    ///
    /// **2026-09-13 修正（实测）**：推理模型（如 `deepseek-flash`）的**思考与正文
    /// 共用这份预算**。512 时一个普通提问的思考就要 1200+ 字，正文被挤到只剩
    /// 25 字且以 `\sqrt{a` 这种半截收尾（`finish_reason=length`）——前端按
    /// 「句子必须以句读结尾」切不出完整句，于是**一个字都不上屏**，
    /// 用户看到的就是「模型没有返回」。所以默认值按「正文 + 思考」一起放宽。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// **思考展示总闸**（2026-09-15 产品开关）：推理模型除正文外的
    /// `reasoning_content` 要不要上屏（WS `reasoning_delta` 帧）。
    ///
    /// - 省略 / `false`（**缺省**）= 不展示：引擎照常解析（解析层单列
    ///   `LlmEvent::ReasoningDelta` 的契约不变，思考**永远不进**句子装配器与
    ///   TTS），但 host **不把它投影成 WS 帧**——用户看不见思考；
    /// - `true` = 展示：前端在气泡的「思考」折叠区照常渲染。
    ///
    /// 这是**产品开关**（要不要看思考），不是性能开关：无论开关如何，思考与
    /// 正文都共用 `max_tokens`，上游该发的 token 一个不少。不要把它写成
    /// 「省流量 / 加速」——那是对它的误读。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_reasoning: Option<bool>,
}

/// [`LlmSettings::max_tokens`] 省略时的默认上限。
///
/// 取值依据：目标形态是「每次 1–5 句、每句 5–60 字」，中文一字符约 0.6–1 token，
/// 即正文 180–300 token。**但推理模型的思考也吃这份预算**：实测一个需要几步
/// 推理的提问思考约 1200–1600 token（见 [`LlmSettings::max_tokens`] 的实测记录）。
/// 4096 ≈ 最坏情况的 2 倍余量。
///
/// 它是**上限**不是配额：正常短回复仍只花几百 token，放宽只影响「需要长思考时
/// 会不会被截断」这一件事。而截断的代价特别大——不是「回复短一点」，而是
/// 「切不出完整句 → 一个字都不上屏」。
pub const DEFAULT_MAX_TOKENS: u32 = 4096;

impl LlmSettings {
    /// 最终生效的输出上限；`0` 表示不限制。
    #[must_use]
    pub fn effective_max_tokens(&self) -> u32 {
        self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)
    }

    /// 最终生效的「展示思考」开关；省略 = **不展示**（见字段注）。
    ///
    /// 缺省必须是 `false`：思考是模型的内心独白，产品默认不把它摆到用户面前。
    #[must_use]
    pub fn effective_show_reasoning(&self) -> bool {
        self.show_reasoning.unwrap_or(false)
    }
}

/// 语音来源二选一（`[tts].mode`，2026-10-09）。
///
/// **缺省 `Local`**：出声走仓库自带的 MeloTTS 垫片（`127.0.0.1:8091`），
/// 音色 / 格式 / 采样率 / 声道由设置保存写死（普通层不给改端口）。
/// `Cloud`：地址 / 音色 / 模型名 / 密钥由用户自己填，本项目**不预填厂商**、
/// **不带 Key**。
///
/// 切换走同一次「保存并应用」；失败就是这一轮没生效（错误码照旧），
/// **不自动改回另一个**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TtsMode {
    /// 本地（缺省）。
    #[default]
    Local,
    /// 云端。
    Cloud,
}

impl TtsMode {
    /// 稳定值（写进 `[tts].mode`，也是 HTTP patch 的取值）。
    pub const LOCAL: &'static str = "local";
    /// 见 [Self::LOCAL]。
    pub const CLOUD: &'static str = "cloud";

    /// 稳定字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => Self::LOCAL,
            Self::Cloud => Self::CLOUD,
        }
    }
}

/// `[tts]` 段：OpenAI-compatible `/audio/speech` 上游。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TtsSettings {
    /// 语音来源（本地 / 云端）。缺省 `local`——出厂就是仓库自带的 MeloTTS 垫片。
    #[serde(default)]
    pub mode: TtsMode,
    /// 服务基址。**出厂缺省 = 仓库自带的 MeloTTS 垫片**（见
    /// [`DEFAULT_TTS_BASE_URL`]）：本地 TTS 不是 Mod，出声端点唯一权威仍是这一段。
    #[serde(default = "default_tts_base_url")]
    pub base_url: String,
    /// 可选模型名；省略时请求体不带 `model` 字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 音色（请求体 `voice` 字段）。**出厂缺省 `ZH`** =
    /// `melo/weights/config.json` 里 `spk2id` 的键。
    #[serde(default = "default_voice")]
    pub voice: String,
    /// 返回格式：`"pcm"`（出厂默认，裸 s16le 流式）或 `"wav"`（RIFF 容器，一次性解析）。
    ///
    /// 缺省随出厂端点（MeloTTS 垫片回 s16le 裸流）取 `"pcm"`。换了返回 WAV 的
    /// 服务端才需要写 `"wav"`——填错要么被 400 拒绝，要么按错容器解出杂音。
    #[serde(default = "default_response_format")]
    pub response_format: String,
    /// 可选：存放 API key 的环境变量名。省略 = 不鉴权。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 返回 PCM 流采样率；**出厂缺省 44.1 kHz**（[`DEFAULT_TTS_SAMPLE_RATE`] =
    /// `melo/weights/config.json` 的 `sampling_rate`）。
    /// 注意：默认值不能靠 `u32::default()`（那是 0），必须显式指定函数。
    /// 它与 [`AudioSpec::DEFAULT_SAMPLE_RATE`]（播放侧 24 kHz）**是两个口径**：
    /// 改这里不动播放默认，改播放默认也不动出厂 TTS 配置。
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// 返回 PCM 流声道数；默认单声道。
    #[serde(default = "default_channels")]
    pub channels: u16,
}

/// 出厂 TTS 服务地址（2026-10-09）：仓库自带的 MeloTTS 垫片。
///
/// 这是**开箱出声**的那一条：`local-tts-melo` Mod 随应用拉起 `melo/start.sh`
/// （缺省监听 8091），这一段指向它。Mod **不写**这一段（`tts-is-core.md`）——
/// 它只是恰好与缺省一致；用户改了地址或换了端点，链路照样跟着 `[tts]` 走。
pub const DEFAULT_TTS_BASE_URL: &str = "http://127.0.0.1:8091/v1";

/// 出厂音色：`melo/weights/config.json` 里 `spk2id` 的键（本例只有 `ZH`）。
pub const DEFAULT_TTS_VOICE: &str = "ZH";

/// 出厂 TTS 采样率 = `melo/weights/config.json` 的 `sampling_rate`。
///
/// **与播放侧 `AudioSpec::DEFAULT_SAMPLE_RATE`（24 kHz）是两个口径**：那个是
/// 「没有配置时音频调度按多少赫兹理解」，这个是「出厂 `[tts]` 那一行写什么」。
pub const DEFAULT_TTS_SAMPLE_RATE: u32 = 44_100;

fn default_tts_base_url() -> String {
    DEFAULT_TTS_BASE_URL.to_string()
}

fn default_voice() -> String {
    DEFAULT_TTS_VOICE.to_string()
}

fn default_response_format() -> String {
    crate::audio::SUPPORTED_FORMAT.to_string()
}

fn default_sample_rate() -> u32 {
    DEFAULT_TTS_SAMPLE_RATE
}

fn default_channels() -> u16 {
    AudioSpec::DEFAULT_CHANNELS
}

/// 本地模式的**冻结字段**（2026-10-09）：切到「本地」时由设置保存写进 `[tts]`。
///
/// 三条口径：端口固定 `127.0.0.1:8091`（普通层不给改）、音色 `ZH`、
/// 裸 PCM / 44100 / 单声道、**无密钥**。这是核心链路（不是 Mod）——
/// `local-tts-melo` 只负责把进程拉起来（`tts-is-core.md`）。
///
/// **不**碰 [`AudioSpec::DEFAULT_SAMPLE_RATE`]（那是没配音频时的调度常量，
/// 与「出厂 `[tts]` 那一行写什么」是两个口径）。
pub fn apply_local_tts_defaults(tts: &mut TtsSettings) {
    tts.mode = TtsMode::Local;
    tts.base_url = DEFAULT_TTS_BASE_URL.to_string();
    tts.model = None;
    tts.voice = DEFAULT_TTS_VOICE.to_string();
    tts.response_format = default_response_format();
    tts.api_key_env = None;
    tts.sample_rate = DEFAULT_TTS_SAMPLE_RATE;
    tts.channels = AudioSpec::DEFAULT_CHANNELS;
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            mode: TtsMode::default(),
            base_url: default_tts_base_url(),
            model: None,
            voice: default_voice(),
            response_format: default_response_format(),
            api_key_env: None,
            sample_rate: default_sample_rate(),
            channels: default_channels(),
        }
    }
}

/// `[persona]` 段：对话人设与历史策略（rc.4 M5 起主链**只留**这两项）。
///
/// 酒馆卡字段（name / description / personality / scenario / first）与导入 UI
/// 已**迁出主链**，由标准 Mod `live2d-ai-mod-persona` 承担——主链不再拼名字/简介，
/// 只把 `system_prompt` 原样交给引擎。见 `docs/architecture/mod-product-chain.md`。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaSettings {
    /// 固定注入每轮的 system 提示；空串 = 不发 system 消息。
    #[serde(default)]
    pub system_prompt: String,
    /// 保留的历史轮数上限；`0` = 每轮独立（默认，最小闭环先不做记忆）。
    #[serde(default)]
    pub max_history_pairs: usize,
}

/// 动作幅度倍率的钳位下限（2026-09-16）。
pub const MIN_ACTION_SCALE: f32 = 0.2;
/// 动作幅度倍率的钳位上限（2026-09-16；**2026-09-24 重标定 2.5 → 2.2**）。
///
/// 旧上限下 body 滑条对 `nod` / `shake` / `look_*` 在 1.43 以上完全无效
///（出厂 1.4 已吃掉 96–98% 行程，见 RESEARCH §2.1）。收到 2.2 后，
/// 渲染面表值（手势主轴头 12 / 身 3.9）使**每个旋钮单独走满都不触上限**
///（头 12 × 0.9285 × 2.2 ≤ 30 × 0.95；身 3.9 × 2.2 ≤ 10 × 0.95）。
/// 数值账与「两个旋钮同时拉满会钳位的组合」见 `l2d-wasm-demo/src/preset/scales.rs`。
pub const MAX_ACTION_SCALE: f32 = 2.2;

/// 出厂默认头摆倍率（相对渲染面预设表内的幅值）。
pub const DEFAULT_HEAD_SCALE: f32 = 0.75;
/// 出厂默认身摆倍率（相对表内幅值）。
///
/// **2026-09-24 重标定 1.4 → 0.80**：旧值把身摆幅顶到 ±9.8（上限 10），
/// 出厂身/头比从表内 0.325 被放大到 0.65（躯干比头还显眼）；0.80 后
/// 出厂身/头比 ≈ 0.347，回到设计口径 [0.30, 0.50]。
pub const DEFAULT_BODY_SCALE: f32 = 0.80;
/// 出厂默认表情倍率。
pub const DEFAULT_EXPRESSION_SCALE: f32 = 1.0;

/// 把任意 f32 归一化进 `[MIN_ACTION_SCALE, MAX_ACTION_SCALE]`。
///
/// NaN / 无穷 → 回落 1.0 再钳（配置出错不该把动作整体关掉）。
#[must_use]
pub fn clamp_action_scale(value: f32) -> f32 {
    if !value.is_finite() {
        return 1.0;
    }
    value.clamp(MIN_ACTION_SCALE, MAX_ACTION_SCALE)
}

fn default_head_scale() -> f32 {
    DEFAULT_HEAD_SCALE
}
fn default_body_scale() -> f32 {
    DEFAULT_BODY_SCALE
}
fn default_expression_scale() -> f32 {
    DEFAULT_EXPRESSION_SCALE
}

/// `[action]` 段：用户可调的三项动作幅度倍率（2026-09-16）。
///
/// **真源在本文件（`live2d-ai.toml`）**，经 `GET/PATCH /api/v1/settings` 暴露，
/// 由 Flutter 设置面板的滑条编辑；渲染面在写 preset 帧时按参数归属乘对应倍率。
/// 前端/调试面板的临时覆盖**不落盘**，只影响当前会话。
///
/// 基准（文档写清，避免「改完不知道为什么变成这样」）：
/// - 渲染面预设表内的幅值是**基准**（头 `ParamAngle*` ≤30、身 `ParamBodyAngle*` ≤10）；
/// - 实际写入 = 表值 × 峰值系数 × intensity × 对应倍率，再按通道红线钳位
///   （头 ≤30、身 ≤10、五官 ≤4）。**两个乘法旋钮**各自走满都不触上限，
///   但**同时**拉满会钳位——会钳位的组合表见
///   `crates/l2d-wasm-demo/src/preset/scales.rs` 顶部注释。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionSettings {
    /// 头角度（`ParamAngle*`）倍率。
    #[serde(default = "default_head_scale")]
    pub head_scale: f32,
    /// 身角度（`ParamBodyAngle*`）倍率。
    #[serde(default = "default_body_scale")]
    pub body_scale: f32,
    /// 表情（口 / 眉 / 眼）倍率。
    #[serde(default = "default_expression_scale")]
    pub expression_scale: f32,
    /// **单模型覆盖表**（`[action.models.<model_id>]`，阶段5 D40，2026-09-26）。
    ///
    /// 键 = 模型 id（`active_model_id` 的原样字符串），值 = 三键**各自可选**的
    /// 覆盖；未覆盖的键**逐键回落**全局（见 [`ActionSettings::effective_for`]）。
    /// 空表整体省略（`skip_serializing_if`），因此没配覆盖的 toml 与改动前逐字一致。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, ActionModelOverride>,
}

/// `[action.models.<model_id>]` 的单模型覆盖（阶段5 D40，2026-09-26）。
///
/// 三键都是 `Option<f32>`：`None` = **该键不覆盖**（回落全局），
/// `Some(v)` = 显式覆盖（写回前经 [`clamp_action_scale`] 钳进 `[0.2, 2.2]`）。
/// `deny_unknown_fields` 与 `[action]` 同口径——拼错的键启动即报错。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ActionModelOverride {
    /// 头角度覆盖；`None` = 跟随全局。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_scale: Option<f32>,
    /// 身角度覆盖；`None` = 跟随全局。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_scale: Option<f32>,
    /// 表情覆盖；`None` = 跟随全局。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression_scale: Option<f32>,
}

/// 某模型 id 的**最终生效**三项倍率（全局与覆盖逐键合并后；已在 `[0.2, 2.2]` 内）。
///
/// 这是前端 `ActionScalesSyncer` 下发的唯一数值来源；**渲染面零改动**。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EffectiveActionScales {
    /// 生效头摆倍率。
    pub head_scale: f32,
    /// 生效身摆倍率。
    pub body_scale: f32,
    /// 生效表情倍率。
    pub expression_scale: f32,
}

/// 模型 id 的最大长度（字节；id 只允许 ASCII，故字节 = 字符）。
pub const MAX_MODEL_ID_LEN: usize = 64;

/// `[action.models.<id>]` 的 id 合法性（**纯函数**，patch 层据此拒绝非法 id）。
///
/// 规则：非空、长度 ≤ [`MAX_MODEL_ID_LEN`]、仅 ASCII 字母/数字与 `.` `_` `-`；
/// 另外拒绝含 `..` 的 id（`".."` / `"a..b"`）——它们看着像路径，既不该成为
/// TOML 键，也不该被当成模型目录名。
#[must_use]
pub fn is_valid_model_id(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_MODEL_ID_LEN {
        return false;
    }
    if id.contains("..") {
        return false;
    }
    id.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

impl Default for ActionSettings {
    fn default() -> Self {
        Self {
            head_scale: DEFAULT_HEAD_SCALE,
            body_scale: DEFAULT_BODY_SCALE,
            expression_scale: DEFAULT_EXPRESSION_SCALE,
            models: BTreeMap::new(),
        }
    }
}

impl ActionSettings {
    /// 归一化后的三项倍率 + 每模型覆盖（全部钳进 `[0.2, 2.2]`）。
    ///
    /// 全局三键的语义**不变**；同时对 `models` 里每个 override 的 `Some` 值
    /// 逐键 `clamp_action_scale`（NaN / 无穷 → 1.0 再钳，与全局同一口径）。
    /// `None` 保持 `None`（= 不覆盖，继续回落全局）。
    #[must_use]
    pub fn normalized(&self) -> Self {
        Self {
            head_scale: clamp_action_scale(self.head_scale),
            body_scale: clamp_action_scale(self.body_scale),
            expression_scale: clamp_action_scale(self.expression_scale),
            models: self
                .models
                .iter()
                .map(|(id, ov)| {
                    (
                        id.clone(),
                        ActionModelOverride {
                            head_scale: ov.head_scale.map(clamp_action_scale),
                            body_scale: ov.body_scale.map(clamp_action_scale),
                            expression_scale: ov.expression_scale.map(clamp_action_scale),
                        },
                    )
                })
                .collect(),
        }
    }

    /// 某模型 id 的**最终生效**三项倍率：**逐键** `override.or(global)`。
    ///
    /// 内部先走 [`Self::normalized`]，所以返回的三个值一定在 `[0.2, 2.2]` 内；
    /// `model_id` 不在 `models` 里（或某键为 `None`）→ 该键跟随全局。
    #[must_use]
    pub fn effective_for(&self, model_id: &str) -> EffectiveActionScales {
        let g = self.normalized();
        let ov = g.models.get(model_id);
        EffectiveActionScales {
            head_scale: ov.and_then(|o| o.head_scale).unwrap_or(g.head_scale),
            body_scale: ov.and_then(|o| o.body_scale).unwrap_or(g.body_scale),
            expression_scale: ov
                .and_then(|o| o.expression_scale)
                .unwrap_or(g.expression_scale),
        }
    }
}

/// 表演层超时缺省值（毫秒）：给「非流式 JSON」留够预算，又不至于拖死一轮。
pub const DEFAULT_PERFORMANCE_TIMEOUT_MS: u64 = 4_000;
/// 表演层超时下限（与 runtime performance::MIN_TIMEOUT_MS 同口径）。
pub const MIN_PERFORMANCE_TIMEOUT_MS: u64 = 100;
/// 表演层超时上限（与 runtime performance::MAX_TIMEOUT_MS 同口径）。
pub const MAX_PERFORMANCE_TIMEOUT_MS: u64 = 30_000;

/// `[performance]` 段：表演层（导演 / 大脑）的独立端点。
///
/// **主模型不负责表演**：主模型只做酒馆式角色扮演（人设 + 剧情/记忆注入，
/// 无工具、无表演类预设）；表演层每轮收「用户输入 + 主模型原文」，交回**一份
/// 合法化 JSON**（`{"speak":…,"cues":[…]}`），其中 `speak` 是本轮 TTS / 上屏的
/// 唯一真源，`cues` 进既有 WS `action_cue` 帧。契约终稿与配置键见
/// `docs/architecture/performance-layer-v0.md`。
///
/// 与主模型 / TTS **完全独立**：自己的 `base_url` / `model` / `timeout_ms` /
/// `api_key_env`；表演层断了只影响表演，不影响主链与语音。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceSettings {
    /// 总闸。**缺省 `false`**——关掉时主链走既有的「边流边切句边送 TTS」，
    /// 规则导演（director Mod）照旧；打开后 `speak` 成为本轮 TTS / 上屏真源。
    #[serde(default)]
    pub enabled: bool,
    /// 表演层端点的 base_url（OpenAI 兼容；独立于 `[llm]`）。空 = 未配。
    #[serde(default)]
    pub base_url: String,
    /// 表演层模型名（空 = 未配）。
    #[serde(default)]
    pub model: String,
    /// 密钥的**环境变量名**（值只住 `.env`；省略 = 不鉴权）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 单次调用超时（毫秒；钳 `100..=30000`，缺省 4000）。
    ///
    /// 比主模型宽：表演层是**非流式**一次往返，且本轮开声要等它返回
    ///（用户已接受这份延迟）。
    #[serde(default = "default_performance_timeout_ms")]
    pub timeout_ms: u64,
    /// structured output 策略：`auto`（缺省，先 json_schema，4xx 降级 prompt）/
    /// `json_schema`（强制）/ `prompt`（只发「只输出 JSON」提示）。
    #[serde(default = "default_performance_structured")]
    pub structured: String,
}

fn default_performance_timeout_ms() -> u64 {
    DEFAULT_PERFORMANCE_TIMEOUT_MS
}

fn default_performance_structured() -> String {
    "auto".to_string()
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: String::new(),
            model: String::new(),
            api_key_env: None,
            timeout_ms: DEFAULT_PERFORMANCE_TIMEOUT_MS,
            structured: default_performance_structured(),
        }
    }
}

impl PerformanceSettings {
    /// 最终生效的超时（钳进 `[MIN, MAX]`）。
    #[must_use]
    pub fn effective_timeout_ms(&self) -> u64 {
        self.timeout_ms
            .clamp(MIN_PERFORMANCE_TIMEOUT_MS, MAX_PERFORMANCE_TIMEOUT_MS)
    }

    /// 表演层是否**真的会发 HTTP**（开了闸 + 端点与模型都配了）。
    ///
    /// 开了闸但缺端点 → 不算「真开」：每轮回退到确定性规则层（degraded）。
    #[must_use]
    pub fn is_wired(&self) -> bool {
        self.enabled && !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

/// 解析产物：可直接交给引擎/客户端使用的配置视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSettings {
    /// LLM 客户端配置（key 已从环境读出并包装）。
    pub llm: LlmConfig,
    /// TTS 客户端配置。
    pub tts: TtsConfig,
    /// 对话引擎配置（system 提示 / 历史 / 队列等沿用 crate 默认）。
    pub conversation: ConversationConfig,
    /// 表演层配置（**原样**交给 desktop 装配；客户端在 host 侧构造）。
    pub performance: PerformanceSettings,
}

impl AppSettings {
    /// 解析 TOML 文本。
    pub fn from_toml_str(text: &str) -> Result<Self, SettingsError> {
        Ok(toml::from_str(text)?)
    }

    /// 从磁盘读取并解析 `live2d-ai.toml`。
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| SettingsError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_toml_str(&text)
    }

    /// 用**密钥真源**（`.env` 快照 > 进程环境）解析成 [`ResolvedSettings`]。
    ///
    /// 未设置或为空串都视为「无鉴权」（CI 里常导出空变量，不应报错）。
    ///
    /// 2026-09-12（rc.2）：以前只读进程环境，于是「界面里能改模型名、却改不了
    /// key」——用户改完还是 401。现在密钥以 `.env` 为唯一真源
    /// （[`crate::secrets`]），进程环境仍然兼容（`.env` 优先）。
    pub fn resolve(&self) -> Result<ResolvedSettings, SettingsError> {
        self.resolve_with(crate::secrets::lookup)
    }

    /// 注入式解析：`lookup` 返回 `None` 表示该环境变量不存在。
    ///
    /// 生产路径 [`AppSettings::resolve`] 传的是 [`crate::secrets::lookup`]
    /// （密钥真源 = `.env` 快照 > 进程环境），**不是** `env::var`；
    /// 测试用它避免污染进程环境。
    pub fn resolve_with(
        &self,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<ResolvedSettings, SettingsError> {
        let llm_key = match &self.llm.api_key_env {
            Some(name) => {
                validate_env_name("llm", name)?;
                lookup(name).map(ApiSecret::new)
            }
            None => None,
        };
        let tts_key = match &self.tts.api_key_env {
            Some(name) => {
                validate_env_name("tts", name)?;
                lookup(name).map(ApiSecret::new)
            }
            None => None,
        };

        validate_base_url("llm", &self.llm.base_url)?;
        // **TTS 选型待定（2026-09-10）**：`base_url` 为空 = 明确「未配置」，
        // 不报错；引擎走空句路径（见 `conversation::worker`）。
        if !self.tts.base_url.trim().is_empty() {
            validate_base_url("tts", &self.tts.base_url)?;
            // 格式门禁：只接受 pcm / wav（mp3/flac 等明确报错，不假装支持）。
            crate::audio::ensure_supported_format(Some(&self.tts.response_format))?;
        }

        // sample_rate/channels 为 0 时经 AudioSpec::new 显式报配置错误
        //（serde 默认值非零，只有显式写 0 才会走到这里）。
        let spec = AudioSpec::new(self.tts.sample_rate, self.tts.channels)?;

        // 表演层：**关闸时不校验**（配了半截也不该让进程起不来）；开闸才校验端点
        // 与非空密钥变量名。空 base_url 由 host 转成 degraded（每轮回退规则层）。
        if self.performance.enabled && !self.performance.base_url.trim().is_empty() {
            validate_base_url("performance", &self.performance.base_url)?;
        }
        if let Some(name) = &self.performance.api_key_env
            && !name.trim().is_empty()
        {
            validate_env_name("performance", name)?;
        }

        Ok(ResolvedSettings {
            llm: LlmConfig {
                base_url: self.llm.base_url.clone(),
                model: self.llm.model.clone(),
                api_key: llm_key,
                max_tokens: self.llm.effective_max_tokens(),
                // 2026-10-08：请求体默认关思考；用户用「思考」开关打开时发 enabled。
                thinking: self.llm.effective_show_reasoning(),
                // 2026-10-08：**产品默认开**二路按句清洗（模型与一路同一份）。
                clean_tts: true,
            },
            tts: TtsConfig {
                base_url: self.tts.base_url.clone(),
                model: self.tts.model.clone(),
                api_key: tts_key,
                voice: self.tts.voice.clone(),
                response_format: Some(self.tts.response_format.clone()),
                spec,
            },
            conversation: ConversationConfig {
                // rc.4 M5：主链只发 `system_prompt` 原文，不再拼角色卡 name/description。
                system_prompt: self.persona.system_prompt.clone(),
                max_history_pairs: self.persona.max_history_pairs,
                ..ConversationConfig::default()
            },
            performance: self.performance.clone(),
        })
    }

    /// 内置配置模板（`live2d-ai.toml.example` 的内容源），供 CLI 输出与文档引用。
    pub fn example_toml() -> &'static str {
        include_str!("../../../live2d-ai.toml.example")
    }

    /// **`GET/PUT /api/v1/env` 的唯一真源**：配置里声明的密钥环境变量名。
    ///
    /// 顺序固定 `llm` → `tts` → `performance`（声明顺序即接口展示顺序，
    /// 前端不用排序）；只收**非空**的 `api_key_env`——空串与「没写」等价
    /// （与 [`Self::resolve_with`] 的判据一致）。
    ///
    /// # 边界（不要为了「列全」越界）
    ///
    /// 这里**只有** `AppSettings` 持有的核心链路密钥。**Mod 级密钥不住在
    /// `AppSettings` 里**：memory Mod 的 `summary_api_key_env`、director Mod 的
    /// `staging_api_key_env` 都写在 `mods.json` 的 Mod config 中，由 Mod 自己的
    /// `settings_spec` 暴露。把 Mod 级密钥塞进 `AppSettings` 会让核心配置依赖
    /// Mod 的存在性——那是错的，本函数**不包含**它们。
    #[must_use]
    pub fn declared_key_envs(&self) -> Vec<DeclaredKeyEnv> {
        [
            ("llm", self.llm.api_key_env.as_deref()),
            ("tts", self.tts.api_key_env.as_deref()),
            ("performance", self.performance.api_key_env.as_deref()),
        ]
        .into_iter()
        .filter_map(|(section, name)| {
            let name = name.filter(|n| !n.is_empty())?;
            Some(DeclaredKeyEnv {
                section,
                name: name.to_string(),
            })
        })
        .collect()
    }

    /// 序列化为 TOML 文本（与 [`Self::from_toml_str`] 对称；供设置面板写回）。
    ///
    /// **密钥安全语义**：`api_key_env` 字段本身**只持有环境变量名**（与解析
    /// 对称），真实密钥永不出现在此序列化结果中。密钥真源是 `.env`（快照优先，
    /// 回退进程环境），读取一律走 `crate::secrets::lookup`——`resolve_with` 只是
    /// 参数化的协议钩子，**不是**产品路径上的密钥读取入口。Serialize
    /// 走纯内存、无 IO / 无失败路径——返回 `String` 而非 `Result`。
    pub fn to_toml_string(&self) -> String {
        toml::to_string(self).expect("AppSettings 序列化 infallible（仅内存操作）")
    }

    /// 把自身的值**合并进**既有 TOML 文本，**保留其中的注释与排版**。
    ///
    /// # 为什么不能用 [`Self::to_toml_string`] 直接覆盖（2026-09-11 修）
    ///
    /// `toml::to_string` 是「重新生成一整份文档」——**注释、空行、键的顺序全丢**。
    /// 而 `live2d-ai.toml` 是**手写并带大量说明**的（模板里有 46 行注释，
    /// 解释每个字段怎么填、有哪些坑）。结果是：用户在界面上改一个开关并保存，
    /// 配置文件里的说明就被抹平了。这不是理论风险——**当天就真的发生了一次**
    ///（在界面上切 dev_mode → 46 行注释全部消失）。
    ///
    /// 依赖 `toml_edit`（本来就是 `toml` 的传递依赖，提为直接依赖不新增 crate）：
    /// 它是**保格式**的 TOML 编辑器。
    ///
    /// # 语义（与 `to_toml_string` 严格对齐）
    ///
    /// - 键已存在 → **就地改值**，保留该值的前后装饰（缩进、行尾注释）；
    /// - 键不存在 → 追加；
    /// - `None` 字段 → **删掉该键**（等价于「省略」，与 `to_toml_string` 一致）；
    /// - 表里**我们不认识的键** → 保留（原实现会连带删掉）。
    ///
    /// `existing` 为空串（文件不存在）时退化为 `to_toml_string`。
    pub fn merge_into_toml(&self, existing: &str) -> Result<String, String> {
        if existing.trim().is_empty() {
            return Ok(self.to_toml_string());
        }
        let mut doc: toml_edit::DocumentMut = existing
            .parse()
            .map_err(|e| format!("现有配置不是合法 TOML：{e}"))?;
        let desired: toml_edit::DocumentMut = self
            .to_toml_string()
            .parse()
            .map_err(|e| format!("自身序列化不是合法 TOML：{e}"))?;
        sync_item(doc.as_item_mut(), desired.as_item());
        Ok(doc.to_string())
    }

    /// 合并进**磁盘上现有**的 TOML；读不到就当作「没有现有文件」。
    ///
    /// 这是写盘点该用的入口——它把「读现有 → 合并」这一步收在一处，
    /// 避免三个写盘点各写一遍（漏一个就又是一次注释清空）。
    pub fn to_toml_string_merging(&self, path: &std::path::Path) -> String {
        let existing = std::fs::read_to_string(path).unwrap_or_default();
        self.merge_into_toml(&existing)
            .unwrap_or_else(|_| self.to_toml_string())
    }
}

/// 把 `desired` 的值**就地**同步进 `dst`，尽量保留 `dst` 的注释与排版。
///
/// 递归规则见 [`AppSettings::merge_into_toml`]。`Item::Value` 分支刻意**改值
/// 而不是换键**——`toml_edit` 里「键自身的前缀注释」挂在 key 上，
/// 换键会把它们一起换掉；就地改值只动值，键与它的注释都留着。
fn sync_item(dst: &mut toml_edit::Item, desired: &toml_edit::Item) {
    use toml_edit::Item;
    // `&mut *dst` 是**重借用**：直接写 `(dst, desired)` 会把 `&mut` 移动进元组，
    // 于是最后的 `_` 分支就没法再用 `dst`（借用检查器报 use of moved value）。
    match (&mut *dst, desired) {
        (Item::Table(dst_table), Item::Table(desired_table)) => {
            // ① 目标里有、期望里没有的键 → 删（`None` 字段的「省略」语义）。
            let stale: Vec<String> = dst_table
                .iter()
                .map(|(k, _)| k.to_string())
                .filter(|k| !desired_table.contains_key(k))
                .collect();
            for key in stale {
                dst_table.remove(&key);
            }
            // ② 逐个同步；新键直接插（没有旧注释可保）。
            for (key, want) in desired_table.iter() {
                match dst_table.get_mut(key) {
                    Some(got) => sync_item(got, want),
                    None => {
                        dst_table.insert(key, want.clone());
                    }
                }
            }
        }
        (Item::Value(got), Item::Value(want)) => {
            // 保留原值的装饰（行尾注释、前后空白）——只换值本身。
            let decor = got.decor().clone();
            let mut next = want.clone();
            *next.decor_mut() = decor;
            *got = next;
        }
        // 表数组（本项目没有）与「类型变了」（罕见）：整体替换——
        // 比「默默留下半新半旧」好。
        _ => {
            *dst = desired.clone();
        }
    }
}
/// 校验环境变量名：非空、ASCII、`[A-Za-z_][A-Za-z0-9_]*`。
fn validate_env_name(section: &'static str, name: &str) -> Result<(), SettingsError> {
    let valid = !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !name.as_bytes()[0].is_ascii_digit();
    if valid {
        Ok(())
    } else {
        Err(SettingsError::InvalidEnvName {
            section,
            name: name.to_string(),
        })
    }
}

/// 校验 `base_url` 是可解析的绝对 http(s) URL。
fn validate_base_url(section: &'static str, raw: &str) -> Result<(), SettingsError> {
    match Url::parse(raw) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => Ok(()),
        _ => Err(SettingsError::InvalidBaseUrl {
            section,
            url: raw.to_string(),
        }),
    }
}
