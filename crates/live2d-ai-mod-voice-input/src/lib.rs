//! live2d-ai-mod-voice-input（Wave 1，2026-09-14）——**语音输入 Mod 骨架**。
//!
//! 链路定位：**语音 → 文本 → [`ModServices::say_tx`]**（与聊天框、external-input
//! 共用同一条 LLM/TTS 主链）。本 crate 只做「拿到一段 ASR 文本 → 清洗 → say」，
//! **不做**真正的语音识别：ASR 住在外部 sidecar（见 `docs/examples/voice-sidecar/`）
//! 或由集成方在进程内喂入文本。
//!
//! # 边界（Wave 1；半成品是刻意的）
//!
//! - **注册面**：Wave 1 分支不注册；`0.2.0-rc.2` 集成时已按 REGISTER 装配进
//!   `AVAILABLE_MOD_FACTORIES`（`mod_count_is_five`，**缺省停用**，ASR 未接线）；
//!   启停唯一真源仍是 manifest `enabled`（`default_mods_manifest` 未收录）。
//! - **不引入**任何 ASR 依赖（whisper / onnx / 音频解码）：完整 ASR 不进 Rust 核心。
//! - **不接**动作通道（`action_tx` 自 rc.2 起休眠）；v0 **不订阅** host 事件。
//!
//! # 配置字段（settings_spec v1）
//!
//! | key | 语义 |
//! |---|---|
//! | `backend` | `mock`（缺省）＝ 由集成方/测试喂文本；`sidecar` ＝ 外部识别进程（占位） |
//! | `locale` | 识别语言提示（缺省 `zh-CN`） |
//! | `token` | 可选访问令牌（secret）；空 = 不鉴权，且**永不**回读明文 |
//!
//! schema 里**没有**第二个 `enabled`：启停唯一真源是 Mod manifest 的 `enabled`
//!（与 external-input 同口径，见 `docs/architecture/mod-product-chain.md` §4）。
//! 与 external-input 都收敛到 `say_tx`，差别只在**输入侧**（弹幕 vs 语音转写）。

use live2d_ai_mod_system::*;

/// 缺省识别语言（settings 里 `locale` 为空/缺失时使用）。
pub const DEFAULT_LOCALE: &str = "zh-CN";

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
    /// 外部识别进程（sidecar）——**占位**，本 crate 不开 socket、不做 IPC。
    Sidecar,
}

impl VoiceBackend {
    /// 稳定字符串 id（配置值 / 日志用）。
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
                label: "识别后端（不引入 ASR 本体；mock = 外部喂文本）".to_string(),
                options: vec![
                    SelectOption {
                        value: (VoiceBackend::Mock).as_str().to_string(),
                        label: "mock（占位）".to_string(),
                    },
                    SelectOption {
                        value: (VoiceBackend::Sidecar).as_str().to_string(),
                        label: "sidecar（外部 ASR，占位）".to_string(),
                    },
                ],
            },
            ModSettingField::String {
                key: "locale".to_string(),
                label: "识别语言（BCP-47，如 zh-CN；空 = 缺省）".to_string(),
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

    /// **语音 → 文本 → say** 的 Rust 侧落点：清洗后送进主链路。
    ///
    /// 返回 `false` 的两种情况：
    /// 1. 清洗后为空（不发空回合，只记 warn）；
    /// 2. 主链忙碌 / 通道满（`SaySender` 既有语义，不被本 Mod 改写）。
    pub fn inject_transcript(&self, raw: &str) -> bool {
        let Some(text) = clean_transcript(raw) else {
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
            "voice-input Mod 已启动（backend={}, locale={}）",
            self.backend().as_str(),
            self.locale()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// 测试用文本记录通道（say_tx 收到的话 / Mod 日志）。
    type Recorded = Arc<Mutex<Vec<String>>>;

    /// 记录 say_tx 文本 + 日志的测试 services。
    fn recording_services() -> (ModServices, Recorded, Recorded) {
        let said: Recorded = Arc::new(Mutex::new(Vec::new()));
        let logs: Recorded = Arc::new(Mutex::new(Vec::new()));
        let (s, l) = (said.clone(), logs.clone());
        let services = ModServices::new(
            ModActionSender::new(|_| false),
            SaySender::new(move |text| {
                s.lock().unwrap().push(text);
                true
            }),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(move |_lvl, msg| l.lock().unwrap().push(msg.to_string())),
        );
        (services, said, logs)
    }

    fn noop_services() -> ModServices {
        ModServices::new(
            ModActionSender::new(|_| false),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        )
    }

    /// 只接受 settings 注册；**任何 subscribe 都 panic**（v0 不该订阅）。
    #[derive(Default)]
    struct MockRegistrar {
        specs: Vec<ModSettingsSpec>,
    }
    impl ModRegistrar for MockRegistrar {
        fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
            self.specs.push(spec);
            Ok(())
        }
        fn subscribe(&mut self, _t: ModEventTopic) -> Result<SubscriptionId, ModError> {
            panic!("v0 不应订阅任何 host 事件")
        }
        fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
            Ok(())
        }
    }

    #[test]
    fn descriptor_and_factory_identity() {
        assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
        assert_eq!(FACTORY.descriptor().id, "voice-input");
        let rt: Box<dyn ModRuntime> = FACTORY
            .create(noop_services(), serde_json::json!({}))
            .expect("create 应成功");
        drop(rt);
    }

    #[test]
    fn start_registers_settings_only() {
        let mut rt = VoiceInputRuntime::new(noop_services(), serde_json::json!({}));
        let mut reg = MockRegistrar::default();
        rt.start(&mut reg).expect("start 应成功");
        assert!(rt.is_ready());
        assert_eq!(reg.specs, vec![voice_input_settings_spec()]);
        rt.shutdown().unwrap();
        assert!(!rt.is_ready(), "shutdown 后不得再报 ready");
    }

    /// 静态 schema 字段齐全；**不含 `enabled`**（唯一开关是 manifest）。
    #[test]
    fn static_spec_fields_and_no_enabled() {
        let spec = FACTORY.settings_spec().expect("静态 schema");
        assert!(spec.validate().is_ok(), "字段 key 不得重复");
        let keys: Vec<&str> = spec.fields.iter().map(|f| f.key()).collect();
        assert_eq!(keys, vec!["backend", "locale", "token"]);
        assert!(!keys.contains(&"enabled"), "启停只由 Mod manifest 表达");
        assert!(matches!(
            spec.fields[2],
            ModSettingField::String { secret: true, .. }
        ));
        match &spec.fields[0] {
            ModSettingField::Select { options, .. } => {
                let values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
                assert_eq!(values, vec!["mock", "sidecar"]);
            }
            other => panic!("backend 应为 Select，实际 {other:?}"),
        }
    }

    // ---------------------------------------------------- 清洗（纯函数）

    #[test]
    fn clean_collapses_whitespace_and_trims() {
        let c = clean_transcript;
        assert_eq!(c("  你好  世界  ").as_deref(), Some("你好 世界"));
        assert_eq!(c("你好\n\t世界").as_deref(), Some("你好 世界"));
        // 全角空格同样折叠；句内标点原样保留。
        assert_eq!(
            c("\u{3000}你\u{3000}\u{3000}好\u{3000}").as_deref(),
            Some("你 好")
        );
        assert_eq!(c("走吧？好。").as_deref(), Some("走吧？好。"));
    }

    #[test]
    fn clean_drops_zero_width_and_control_chars() {
        assert_eq!(
            clean_transcript("\u{FEFF}你\u{200B}好\u{0007}").as_deref(),
            Some("你好")
        );
    }

    #[test]
    fn clean_returns_none_for_blank_only() {
        for blank in ["", "   ", "\n\t\u{3000}", "\u{200B}\u{FEFF}"] {
            assert_eq!(
                clean_transcript(blank),
                None,
                "纯空白必须回落 None: {blank:?}"
            );
        }
    }

    // ---------------------------------------------------- 配置读取（纯函数）

    #[test]
    fn backend_from_config_defaults_to_mock() {
        let b = |v: serde_json::Value| VoiceBackend::from_config(&v);
        assert_eq!(b(serde_json::json!({})), VoiceBackend::Mock);
        assert_eq!(
            b(serde_json::json!({"backend": "mock"})),
            VoiceBackend::Mock
        );
        assert_eq!(
            b(serde_json::json!({"backend": "sidecar"})),
            VoiceBackend::Sidecar
        );
        // 未知 / 非字符串 → mock（宽容，不让配置写错打死链路）。
        assert_eq!(
            b(serde_json::json!({"backend": "whisper"})),
            VoiceBackend::Mock
        );
        assert_eq!(b(serde_json::json!({"backend": 7})), VoiceBackend::Mock);
        assert_eq!((VoiceBackend::Mock).as_str(), "mock");
        assert_eq!((VoiceBackend::Sidecar).as_str(), "sidecar");
    }

    #[test]
    fn locale_from_config_falls_back_to_default() {
        let l = |v: serde_json::Value| locale_from_config(&v);
        assert_eq!(l(serde_json::json!({})), DEFAULT_LOCALE);
        assert_eq!(l(serde_json::json!({"locale": " en-US "})), "en-US");
        assert_eq!(l(serde_json::json!({"locale": "   "})), DEFAULT_LOCALE);
        assert_eq!(l(serde_json::json!({"locale": 42})), DEFAULT_LOCALE);
    }

    #[test]
    fn token_from_config_trims_and_ignores_blank() {
        let t = |v: serde_json::Value| token_from_config(&v);
        assert_eq!(
            t(serde_json::json!({"token": " s3cret "})),
            Some("s3cret".to_string())
        );
        assert_eq!(t(serde_json::json!({"token": ""})), None);
        assert_eq!(t(serde_json::json!({"token": "  "})), None);
        assert_eq!(t(serde_json::json!({})), None);
        assert_eq!(t(serde_json::json!({"token": 7})), None);
    }

    // ---------------------------------------------------- 语音 → say

    #[test]
    fn inject_transcript_cleans_before_sending() {
        let (services, said, _logs) = recording_services();
        let rt = VoiceInputRuntime::new(services, serde_json::json!({}));
        assert!(rt.inject_transcript("  打开\u{3000}空调  "));
        assert_eq!(said.lock().unwrap().as_slice(), ["打开 空调"]);
    }

    #[test]
    fn inject_transcript_drops_blank_without_touching_say() {
        let (services, said, logs) = recording_services();
        let rt = VoiceInputRuntime::new(services, serde_json::json!({}));
        assert!(!rt.inject_transcript("   "));
        assert!(!rt.inject_transcript("\u{200B}"));
        assert!(said.lock().unwrap().is_empty(), "空文本不得进 say_tx");
        assert!(
            logs.lock().unwrap().iter().any(|m| m.contains("为空")),
            "应留下一条 warn 日志"
        );
    }

    #[test]
    fn inject_reports_busy_and_mock_entry_shares_path() {
        let busy = VoiceInputRuntime::new(
            ModServices::new(
                ModActionSender::new(|_| false),
                SaySender::new(|_| false),
                ModEventSender::new(|_t, _p| true),
                ModLogger::new(|_l, _m| {}),
            ),
            serde_json::json!({}),
        );
        assert!(!busy.inject_transcript("你好"), "忙碌时返回 false");

        let (services, said, logs) = recording_services();
        let rt = VoiceInputRuntime::new(services, serde_json::json!({"backend": "mock"}));
        assert_eq!(rt.backend(), VoiceBackend::Mock);
        assert!(rt.inject_mock_transcript("  你好  "));
        assert_eq!(said.lock().unwrap().as_slice(), ["你好"]);
        assert!(logs.lock().unwrap().iter().any(|m| m.contains("mock 后端")));
    }

    #[test]
    fn runtime_reads_backend_and_locale_from_config() {
        let rt = VoiceInputRuntime::new(
            noop_services(),
            serde_json::json!({"backend": "sidecar", "locale": "ja-JP"}),
        );
        assert_eq!(rt.backend(), VoiceBackend::Sidecar);
        assert_eq!(rt.locale(), "ja-JP");
        assert_eq!(rt.config()["backend"], serde_json::json!("sidecar"));
    }
}
