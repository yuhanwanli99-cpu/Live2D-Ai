//! live2d-ai-mod-voice-input（Wave 1 骨架 / Wave 2 A 轨端点 / **Wave 3 A 轨闭环**，2026-09-14）——**语音输入 Mod**。
//!
//! 链路定位：**语音 → 文本 → [`ModServices::say_tx`]**（与聊天框、external-input
//! 共用同一条 LLM/TTS 主链）。本 crate 只做「拿到一段 ASR 文本 → 清洗 +
//! locale 归一化 → say」，**不做**真正的语音识别：ASR 住在外部 sidecar
//! （`docs/examples/voice-sidecar/`）；端点 = `POST /api/v1/voice/transcript`
//! （`web_api/voice_routes.rs`）。
//!
//! # Wave 3 A 轨：把两个死配置变成可测行为
//!
//! - **`backend`（`mock` | `sidecar`）**：handler 读 Mod config 并走**明确分支**，
//!   响应里回 `backend` 字段（可观察）。Rust **永远不开 socket**——
//!   [`VoiceBackend::opens_network`] 恒 `false`、[`RustRoute`] 只有本地变体，
//!   这是结构性保证（回归 `no_backend_opens_network`）。`mock` 由集成方/测试
//!   直接喂文本；`sidecar` 是**推模式**（sidecar 主动 POST，Rust 只收）。
//!   sidecar 的全部失败面（401 / 403 / busy / empty / timeout / transport）与
//!   逐条退避见 `docs/voice-input.md` §4.4 / `docs/examples/voice-sidecar/README.md` §5。
//! - **`locale`（BCP-47）**：真正影响 text 归一化（[`normalize`]）；
//!   **不**影响 ASR 引擎选型、**不**随请求发给 sidecar。回归 `locale_*` 断言
//!   `zh-CN` 删 CJK 词间空格、`en-US` 保留空格并补中英边界。
//!
//! # 边界
//!
//! - **注册面**：已装配进 `AVAILABLE_MOD_FACTORIES`（`mod_count_is_six`，
//!   **缺省停用**；启停唯一真源 = manifest `enabled`，`default_mods_manifest` 未收录）；
//!   handler 复用本 crate 的 [`prepare_transcript`]（=`clean_transcript` +
//!   [`normalize_for_locale`]，**不重写**），契约见 `docs/voice-input.md`。
//! - **不引入**任何 ASR 依赖（whisper / onnx / 音频解码），也不开 socket：完整 ASR
//!   不进 Rust 核心。
//! - **不接**动作通道（`action_tx` 自 rc.2 起休眠）；v0 **不订阅** host 事件。
//!
//! # 配置字段（settings_spec v1）
//!
//! | key | 语义 |
//! |---|---|
//! | `backend` | `mock`（缺省）＝ 由集成方/测试喂文本；`sidecar` ＝ 外部识别进程（推模式） |
//! | `locale` | 识别语言提示（缺省 `zh-CN`）→ **只**影响 text 归一化档 |
//! | `token` | 可选访问令牌（secret）；空 = 不鉴权，且**永不**回读明文 |
//!
//! schema 里**没有**第二个 `enabled`：启停唯一真源是 Mod manifest 的 `enabled`
//!（与 external-input 同口径，见 `docs/architecture/mod-product-chain.md` §4）。
//! 与 external-input 都收敛到 `say_tx`，差别只在**输入侧**（弹幕 vs 语音转写）。

pub mod normalize;

pub use normalize::{DEFAULT_LOCALE, LocaleProfile, locale_profile, normalize_for_locale};

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "voice-input",
    name: "语音输入",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// 识别后端（由 settings 的 `backend` 选择）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceBackend {
    /// 占位后端：没有 ASR，由集成方/测试直接喂转写文本。
    Mock,
    /// 外部识别进程（sidecar）——**推模式**：`POST /api/v1/voice/transcript`
    /// （本 crate 不开 socket、不主动请求）。
    Sidecar,
}

/// Rust 侧对某个 backend 的**本地**动作。
///
/// 这个枚举里**没有网络变体**——`mock` / `sidecar` 都不可能让 Rust 主动发起
/// ASR 请求（推模式）。见 [`VoiceBackend::opens_network`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustRoute {
    /// `mock`：由集成方/测试喂文本，本地清洗 + 归一化后 say。
    LocalInject,
    /// `sidecar`：sidecar 推文本进来，本地清洗 + 归一化后 say。
    AcceptPush,
}

impl VoiceBackend {
    /// 稳定字符串 id（配置值 / 日志 / 响应回显用）。
    pub const fn as_str(self) -> &'static str {
        match self {
            VoiceBackend::Mock => "mock",
            VoiceBackend::Sidecar => "sidecar",
        }
    }

    /// 从 Mod config JSON 读取；**未知值 / 非字符串 / 缺失一律回落 `Mock`**
    ///（宽容：配置写错不该让输入链路直接死掉）。
    pub fn from_config(config: &serde_json::Value) -> Self {
        match config.get("backend").and_then(|v| v.as_str()) {
            Some("sidecar") => VoiceBackend::Sidecar,
            _ => VoiceBackend::Mock,
        }
    }

    /// Rust 侧该走哪条**本地**路径（纯函数，可直接断言分支）。
    pub const fn rust_route(self) -> RustRoute {
        match self {
            VoiceBackend::Mock => RustRoute::LocalInject,
            VoiceBackend::Sidecar => RustRoute::AcceptPush,
        }
    }

    /// 恒 `false`：本项目在 Rust 侧**不开 socket / 不主动请求 ASR**
    /// （AGENTS「禁止」+ Wave 3 计划 §3A）。保留成显式函数，是为了让
    /// 「不碰网络」成为可断言的结构，而不是一句口头约定。
    pub const fn opens_network(self) -> bool {
        false
    }
}

/// 语音输入设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
pub fn voice_input_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::Select {
                key: "backend".to_string(),
                label: "识别后端（Rust 不开 socket；mock = 外部喂文本，sidecar = 推模式）"
                    .to_string(),
                options: vec![
                    SelectOption {
                        value: (VoiceBackend::Mock).as_str().to_string(),
                        label: "mock（缺省，集成方/测试喂文本）".to_string(),
                    },
                    SelectOption {
                        value: (VoiceBackend::Sidecar).as_str().to_string(),
                        label: "sidecar（外部 ASR 推文本到端点）".to_string(),
                    },
                ],
            },
            ModSettingField::String {
                key: "locale".to_string(),
                label: "识别语言（BCP-47，如 zh-CN；只影响 text 归一化）".to_string(),
                secret: false,
            },
            ModSettingField::String {
                key: "token".to_string(),
                label: "访问令牌（可空；空 = 不鉴权，回读只显示是否已设置）".to_string(),
                secret: true,
            },
        ],
    }
}

/// 从 Mod config JSON 读取 `locale`；空/纯空白/非字符串 → [`DEFAULT_LOCALE`]。
pub fn locale_from_config(config: &serde_json::Value) -> String {
    config
        .get("locale")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_LOCALE)
        .to_string()
}

/// 从 Mod config JSON 读取 `token`；空串/纯空白/缺省 → `None`。
///
/// 与 external-input 的 `token_from_config` 同款：**只判空，不回显明文**，
/// 空值语义是「不鉴权」而不是「用空 token 鉴权」。
pub fn token_from_config(config: &serde_json::Value) -> Option<String> {
    config
        .get("token")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// 清洗一段 ASR / 侧车文本（纯函数，本 crate 的核心可测部分）。
///
/// - 去掉零宽字符（`U+200B` 零宽空格、`U+FEFF` BOM、`U+200C`/`U+200D`）与其它控制字符；
/// - 把连续空白（含全角空格 `U+3000`、\t、\n、\r）折叠成一个半角空格；
/// - 去掉首尾空白；清洗后为空 → `None`。
///
/// **空结果不得喂给 `say`**：那会造出一次空输入回合，与「空句不进 TTS」
/// 是同一条纪律（见 AGENTS「语音输出约定」）。
/// locale 归一化是**下一步**，见 [`normalize_for_locale`] / [`prepare_transcript`]。
pub fn clean_transcript(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut pending_space = false;
    for ch in raw.chars() {
        if ch.is_whitespace() {
            // 前导空白落不下（out 为空），尾随空白在循环结束后自然丢弃。
            pending_space = !out.is_empty();
            continue;
        }
        if matches!(ch, '\u{200B}' | '\u{FEFF}' | '\u{200C}' | '\u{200D}') || ch.is_control() {
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    (!out.is_empty()).then_some(out)
}

/// **唯一的转写准备入口**：`clean_transcript` → [`normalize_for_locale`]。
///
/// handler 与 [`VoiceInputRuntime::inject_transcript`] 都走这里，保证
/// 「端点收到的文本」与「Mod 自己注入的文本」是同一套规则。清洗后为空 → `None`。
pub fn prepare_transcript(raw: &str, locale: &str) -> Option<String> {
    let cleaned = clean_transcript(raw)?;
    let normalized = normalize_for_locale(&cleaned, locale);
    (!normalized.is_empty()).then_some(normalized)
}

/// 语音输入的运行时状态。
pub struct VoiceInputRuntime {
    services: ModServices,
    config: serde_json::Value,
    /// settings schema 是否已注册（`start` 成功标志）。
    registered: bool,
}

impl VoiceInputRuntime {
    /// 用注入的 host services + namespaced config 构造。
    pub fn new(services: ModServices, config: serde_json::Value) -> Self {
        Self {
            services,
            config,
            registered: false,
        }
    }

    /// 是否已启用（`start` 成功）。
    pub fn is_ready(&self) -> bool {
        self.registered
    }

    /// 当前 namespaced 配置。
    pub fn config(&self) -> &serde_json::Value {
        &self.config
    }

    /// 当前生效的识别后端。
    pub fn backend(&self) -> VoiceBackend {
        VoiceBackend::from_config(&self.config)
    }

    /// 当前生效的识别语言。
    pub fn locale(&self) -> String {
        locale_from_config(&self.config)
    }

    /// **语音 → 文本 → say** 的 Rust 侧落点：清洗 + locale 归一化后送进主链路。
    ///
    /// 返回 `false` 的两种情况：
    /// 1. 清洗后为空（不发空回合，只记 warn）；
    /// 2. 主链忙碌 / 通道满（[`SaySender`] 既有语义，不被本 Mod 改写）。
    pub fn inject_transcript(&self, raw: &str) -> bool {
        let Some(text) = prepare_transcript(raw, &self.locale()) else {
            self.services
                .logger
                .warn("语音转写清洗后为空，已丢弃（不发空回合）");
            return false;
        };
        let accepted = self.services.say_tx.say(text);
        if !accepted {
            self.services
                .logger
                .warn("语音转写未被主链路接受（忙碌或通道满）");
        }
        accepted
    }

    /// mock 后端便捷入口：与 [`Self::inject_transcript`] 同路径，多一行日志。
    pub fn inject_mock_transcript(&self, raw: &str) -> bool {
        self.services.logger.info(&format!(
            "mock 后端收到转写（backend={}）",
            self.backend().as_str()
        ));
        self.inject_transcript(raw)
    }
}

/// 语音输入工厂。
pub struct VoiceInputFactory;

impl ModFactory for VoiceInputFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// 静态 schema：未启用也能渲染配置表单（rc.4 M2 语义）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(voice_input_settings_spec())
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(VoiceInputRuntime::new(services, config)))
    }
}

impl ModRuntime for VoiceInputRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        registrar.register_settings(voice_input_settings_spec())?;
        // v0 不订阅 host 事件：输入由外部推入（见模块头注）。
        self.registered = true;
        self.services.logger.info(&format!(
            "voice-input Mod 已启动（backend={}, locale={}, route={:?}）",
            self.backend().as_str(),
            self.locale(),
            self.backend().rust_route()
        ));
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.registered = false;
        self.services.logger.info("voice-input Mod 已关闭");
        Ok(())
    }
}

/// 装配进 `AVAILABLE_MOD_FACTORIES` 的工厂单例（`0.2.0-rc.2` 集成起已注册，缺省停用）。
pub static FACTORY: VoiceInputFactory = VoiceInputFactory;

// 回归测试拆到同目录 `tests.rs`（源文件保持 ≤500 行，与 desktop 的
// `voice_routes_tests.rs` 同模式）。`#[path]` 让测试文件仍是本模块的子模块。
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
