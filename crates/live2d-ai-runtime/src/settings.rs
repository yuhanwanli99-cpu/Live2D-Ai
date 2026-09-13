//! 应用级配置加载 [`AppSettings`]：把 `live2d-ai.toml`（+ 环境变量密钥）解析成可直接使用的 [`LlmConfig`] / [`TtsConfig`] / [`ConversationConfig`]。
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
#[cfg(test)]
mod settings_tests;
pub mod view;
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
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
    /// 开发者模式开关（默认 false）：打开后 `/api/v1/logs*` 端点放行
    /// （见 W3 任务约定）。可在 `live2d-ai.toml` 顶层显式写 `dev_mode = true`，
    /// 或由 CLI `--dev-mode` 覆盖，或在设置面板（egui）勾选后 PATCH 落盘。
    /// 来源优先级：CLI flag > settings 文件 > 默认 false。
    pub dev_mode: bool,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// [`LlmSettings::max_tokens`] 省略时的默认上限。
///
/// 取值依据：按「每次 1–5 句、每句 5–60 字」的目标，中文一字符大约 0.6–1 token，
/// 5 句 × 60 字 ≈ 300 字 ≈ 180–300 token。512 留了接近一倍余量：
/// **宁可偶尔长一点（多切几句，每句仍完整），也不要因为卡上限把句子截断。**
pub const DEFAULT_MAX_TOKENS: u32 = 512;

impl LlmSettings {
    /// 最终生效的输出上限；`0` 表示不限制。
    #[must_use]
    pub fn effective_max_tokens(&self) -> u32 {
        self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)
    }
}

/// `[tts]` 段：OpenAI-compatible `/audio/speech` 上游。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TtsSettings {
    /// 服务基址。
    #[serde(default)]
    pub base_url: String,
    /// 可选模型名；省略时请求体不带 `model` 字段。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 音色（请求体 `voice` 字段）。
    #[serde(default = "default_voice")]
    pub voice: String,
    /// 返回格式：`"pcm"`（默认，裸 s16le 流式）或 `"wav"`（RIFF 容器，一次性解析）。
    ///
    /// 2026-09-10：CosyVoice 3 的 OpenAI 兼容层（如 CosyVoice3-API）默认返回
    /// WAV，故此处需填 `"wav"`；其余保持默认 `"pcm"`。
    #[serde(default = "default_response_format")]
    pub response_format: String,
    /// 可选：存放 API key 的环境变量名。省略 = 不鉴权。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// 返回 PCM 流采样率；默认 24 kHz（[`AudioSpec::DEFAULT_SAMPLE_RATE`]）。
    /// 注意：默认值不能靠 `u32::default()`（那是 0），必须显式指定函数。
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
    /// 返回 PCM 流声道数；默认单声道。
    #[serde(default = "default_channels")]
    pub channels: u16,
}

fn default_voice() -> String {
    "alloy".to_string()
}

fn default_response_format() -> String {
    crate::audio::SUPPORTED_FORMAT.to_string()
}

fn default_sample_rate() -> u32 {
    AudioSpec::DEFAULT_SAMPLE_RATE
}

fn default_channels() -> u16 {
    AudioSpec::DEFAULT_CHANNELS
}

impl Default for TtsSettings {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            model: None,
            voice: default_voice(),
            response_format: default_response_format(),
            api_key_env: None,
            sample_rate: AudioSpec::DEFAULT_SAMPLE_RATE,
            channels: AudioSpec::DEFAULT_CHANNELS,
        }
    }
}

/// `[persona]` 段：对话人设与历史策略。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PersonaSettings {
    /// 固定注入每轮的 system 提示；空串 = 不发 system 消息。
    #[serde(default)]
    pub system_prompt: String,
    /// 保留的历史轮数上限；`0` = 每轮独立（默认，最小闭环先不做记忆）。
    #[serde(default)]
    pub max_history_pairs: usize,
    /// 角色名称（酒馆卡用；空 = 未设置）。
    #[serde(default)]
    pub name: String,
    /// 角色描述（identity · background；空 = 未设置）。
    #[serde(default)]
    pub description: String,
    /// 个性（personality · speech style；空 = 未设置）。
    #[serde(default)]
    pub personality: String,
    /// 场景（scenario · world；空 = 未设置）。
    #[serde(default)]
    pub scenario: String,
    /// 开场白（first message；空 = 未设置）。
    #[serde(default)]
    pub first: String,
}

/// 把角色卡 name/description 拼接到 system_prompt 前的纯函数（空字段跳过）。
///
/// 返回：`[角色卡] 名称/描述\n\n{base}`；两字段皆空时原样返回 `base`。
/// **不修改** `system_prompt` 原值，只在 resolve 时拼接产物进入会话。
pub fn build_effective_system_prompt(name: &str, description: &str, base: &str) -> String {
    if name.is_empty() && description.is_empty() {
        return base.to_string();
    }
    let mut card = String::from("[角色卡]");
    if !name.is_empty() {
        card.push_str(" 名称：");
        card.push_str(name);
    }
    if !description.is_empty() {
        card.push_str("\n描述：");
        card.push_str(description);
    }
    let mut out = card;
    if !base.is_empty() {
        out.push_str("\n\n");
        out.push_str(base);
    }
    out
}

/// 解析产物：三份可直接交给引擎/客户端使用的配置视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSettings {
    /// LLM 客户端配置（key 已从环境读出并包装）。
    pub llm: LlmConfig,
    /// TTS 客户端配置。
    pub tts: TtsConfig,
    /// 对话引擎配置（system 提示 / 历史 / 队列等沿用 crate 默认）。
    pub conversation: ConversationConfig,
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
    /// 生产路径 [`AppSettings::resolve`] 即 `lookup = |n| env::var(n).ok()`；
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

        Ok(ResolvedSettings {
            llm: LlmConfig {
                base_url: self.llm.base_url.clone(),
                model: self.llm.model.clone(),
                api_key: llm_key,
                max_tokens: self.llm.effective_max_tokens(),
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
                system_prompt: build_effective_system_prompt(
                    &self.persona.name,
                    &self.persona.description,
                    &self.persona.system_prompt,
                ),
                max_history_pairs: self.persona.max_history_pairs,
                ..ConversationConfig::default()
            },
        })
    }

    /// 内置配置模板（`live2d-ai.toml.example` 的内容源），供 CLI 输出与文档引用。
    pub fn example_toml() -> &'static str {
        include_str!("../../../live2d-ai.toml.example")
    }

    /// 序列化为 TOML 文本（与 [`Self::from_toml_str`] 对称；供设置面板写回）。
    ///
    /// **密钥安全语义**：`api_key_env` 字段本身**只持有环境变量名**（与解析
    /// 对称），真实密钥永不出现在此序列化结果中；写回 `live2d-ai.toml` 后
    /// 下次启动仍由 `resolve_with(|n| env::var(n).ok())` 从环境读出。Serialize
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
