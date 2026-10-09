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
//! - **注册面**：已装配进 `AVAILABLE_MOD_FACTORIES`（计数断言 `mod_count_is_six`，
//!   **缺省停用**；启停唯一真源 = manifest `enabled`，`default_mods_manifest` 未收录）；
//!   handler 复用本 crate 的 [`prepare_transcript`]（=`clean_transcript` +
//!   [`normalize_for_locale`]，**不重写**），契约见 `docs/voice-input.md`。
//! - **不引入**任何 ASR 依赖（whisper / onnx / 音频解码），也不开 socket：完整 ASR
//!   不进 Rust 核心。
//! - **不接**动作通道（`action_tx` 自 rc.2 起休眠）；v0 **不订阅** host 事件。
//!
//! # 一次性命令（产品级加强波次，2026-09-15）
//!
//! 实现 [`ModRuntime::command`]：`selftest` 返回「当前配置是否自洽」的**脱敏**结论
//!（生效 backend / locale 是否合法 / `token_set` / 本地路由 / `problems` / `notes`）。
//! 真源是纯函数 [`selftest::config_selftest`]，面板「检查配置」按钮经
//! `POST /api/v1/mods/voice-input/command` 调它；未知命令回
//! [`ModError::UnsupportedCommand`]（host 映射 409 `unsupported_command`）。
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

//! # L1 产品级：唤醒闸 + 手动闸 + 一条硬主路径（2026-09-15）
//!
//! - **总闸 = 唤醒短语**（`wake_phrase`）：空 = 总闸关，**拒绝一切转写**
//!   （`403 voice_gate_closed`）；非空 = 必须包含它，命中后从正文里**剥掉**。
//!   判定在 [`gate::evaluate`]（纯函数，四态）。
//! - **手动闸 `manual_enabled`**（缺省 `true`）：false → `403 voice_manual_off`。
//! - **硬主路径 = 面板一键拉起官方 sidecar**：命令 `run_sidecar`
//!   用 [`std::process::Command`] **逐参数** spawn
//!   `docs/examples/voice-sidecar/voice_sidecar.py`（**绝不 `sh -c`**），
//!   argv 由 [`sidecar::build_sidecar_argv`] 拼装；spawn 后立即返回，
//!   后台线程 `wait()` 并把退出码 / stderr 尾巴写进 [`sidecar::SidecarStatus`]
//!   （HTTP 服务器是单线程的：在这里等 sidecar 会死锁，因为 sidecar 要 POST 回来）。
//! - **命令 `inject`**：走与端点**同一条** gate + prepare + say 路径，
//!   总闸关时返回**被拒绝的可读结果**（不是抛错），面板据此做「验证闸门」。
//! - [`ModRuntime::state_json`]：零 IO 的运行态快照（总闸 / 手动闸 / sidecar 状态）。

pub mod commands;
pub mod gate;
pub mod normalize;
pub mod selftest;
pub mod sidecar;

pub use gate::GateOutcome;
pub use normalize::{DEFAULT_LOCALE, LocaleProfile, locale_profile, normalize_for_locale};
pub use selftest::config_selftest;

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
///
/// v3（2026-09-15 用户裁决：设置项过多要收）：
/// - 主区只有两个：`wake_phrase`（缺省 **小可爱**，空 = 总闸关）+ `manual_enabled`；
/// - `backend` / `locale` / `token` / `sidecar_*` **仍在协议里**（不是删功能），
///   但前端把它们收进「高级」折叠——见 `VoiceInputPanel.advancedKeys`。
/// - 标签一律短句，长解释搬到面板 / 文档（面板也不再复述契约全文）。
pub fn voice_input_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 3,
        fields: vec![
            ModSettingField::String {
                key: "wake_phrase".to_string(),
                // 2026-10-08：标签只留「唤醒词」；「听到它才开始听 / 留空 = 关闸」
                // 由前端那句说明承担（ModPanel.fieldHelp），不在产品面出现内部名。
                label: "唤醒词".to_string(),
                secret: false,
                default: Some(gate::DEFAULT_WAKE_PHRASE.to_string()),
            },
            ModSettingField::Bool {
                key: "manual_enabled".to_string(),
                label: "按住说话".to_string(),
                default: true,
            },
            ModSettingField::Select {
                key: "backend".to_string(),
                label: "识别后端".to_string(),
                options: vec![
                    SelectOption {
                        value: (VoiceBackend::Mock).as_str().to_string(),
                        label: "mock（缺省；由外部直接喂文本）".to_string(),
                    },
                    SelectOption {
                        value: (VoiceBackend::Sidecar).as_str().to_string(),
                        label: "sidecar（外部 ASR 推文本到端点）".to_string(),
                    },
                ],
                default: Some((VoiceBackend::Mock).as_str().to_string()),
            },
            ModSettingField::String {
                key: "locale".to_string(),
                label: "文本归一化语言（BCP-47）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "token".to_string(),
                label: "访问令牌（空 = 不鉴权）".to_string(),
                secret: true,
                default: None,
            },
            ModSettingField::String {
                key: "sidecar_script".to_string(),
                label: "sidecar 脚本路径（空 = 仓库默认）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "sidecar_url".to_string(),
                label: "sidecar POST 的 URL（空 = 命令参数给）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "sidecar_transcriber".to_string(),
                label: "ASR 命令（fake = 读同名 .txt）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "sidecar_python".to_string(),
                label: "Python 解释器（缺省 python3）".to_string(),
                secret: false,
                default: None,
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
    pub(crate) services: ModServices,
    pub(crate) config: serde_json::Value,
    /// settings schema 是否已注册（`start` 成功标志）。
    pub(crate) registered: bool,
    /// sidecar 运行态（spawn/wait 线程推进；`state_json` 只读它，零 IO）。
    pub(crate) sidecar: std::sync::Arc<std::sync::Mutex<sidecar::SidecarStatus>>,
}

impl VoiceInputRuntime {
    /// 用注入的 host services + namespaced config 构造。
    pub fn new(services: ModServices, config: serde_json::Value) -> Self {
        Self {
            services,
            config,
            registered: false,
            sidecar: std::sync::Arc::new(std::sync::Mutex::new(sidecar::SidecarStatus::default())),
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

    /// 配置自检快照（脱敏）：[`selftest::config_selftest_with_script`] 的
    /// runtime 便捷入口（带上宿主注入的 `config_path`，从而能解析缺省脚本路径）。
    pub fn selftest(&self) -> serde_json::Value {
        selftest::config_selftest_with_script(&self.config, &self.services.config_path)
    }

    // inject / run_sidecar / state_json 快照 / 命令分派都在 `commands.rs`
    //（impl 块可在 crate 内其它模块；子模块可见根模块的私有字段）。
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

    /// 零 IO 运行态快照（L1 产品级）：总闸 / 手动闸 / sidecar 状态。
    ///
    /// 契约见 [`ModRuntime::state_json`]：**只读内存**（config + SidecarStatus），
    /// 不读盘、不 spawn、不阻塞。真源是 [`crate::commands::VoiceInputRuntime::state_snapshot`]。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(self.state_snapshot())
    }

    /// 一次性命令（L1 产品级）：`selftest` / `inject` / `run_sidecar`。
    ///
    /// 分派实现见 [`crate::commands::VoiceInputRuntime::dispatch_command`]。
    fn command(
        &mut self,
        command: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        self.dispatch_command(command, args)
    }
}

/// 装配进 `AVAILABLE_MOD_FACTORIES` 的工厂单例（`0.2.0-rc.2` 集成起已注册，缺省停用）。
pub static FACTORY: VoiceInputFactory = VoiceInputFactory;

// 回归测试拆到同目录 `tests.rs`（源文件保持 ≤500 行，与 desktop 的
// `voice_routes_tests.rs` 同模式）。`#[path]` 让测试文件仍是本模块的子模块。
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
