//! `live2d-ai-mod-voice-input` 回归（Wave 3 轨 A 重排；源文件保持 ≤500 行）。
//!
//! 经 `#[cfg(test)] #[path = "tests.rs"] mod tests;` 挂在 crate 根下，
//! 故 `use super::*` 可见 crate 根（含 `pub use normalize::*` 的再导出）。

use super::*;
use crate::normalize::{is_ascii_word, is_cjk};
use std::sync::{Arc, Mutex};

/// 测试用记录通道（say_tx 文本 / Mod 日志 / action 请求）。
type Recorded = Arc<Mutex<Vec<String>>>;

/// 记录 say_tx 文本 + 日志 + action 请求的测试 services。
fn recording_services() -> (ModServices, Recorded, Recorded, Recorded) {
    let said: Recorded = Arc::new(Mutex::new(Vec::new()));
    let logs: Recorded = Arc::new(Mutex::new(Vec::new()));
    let actions: Recorded = Arc::new(Mutex::new(Vec::new()));
    let (s, l, a) = (said.clone(), logs.clone(), actions.clone());
    let services = ModServices::new(
        ModActionSender::new(move |_req| {
            a.lock().unwrap().push("action-attempt".to_string());
            false
        }),
        SaySender::new(move |text| {
            s.lock().unwrap().push(text);
            true
        }),
        ModEventSender::new(|_t, _p| true),
        ModLogger::new(move |_lvl, msg| l.lock().unwrap().push(msg.to_string())),
    );
    (services, said, logs, actions)
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

// ---------------------------------------------------- backend（配置 + 结构性无网络）

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

/// Wave 3 A 轨的**结构性**保证：没有任何 backend 会让 Rust 主动发网络请求，
/// 且每个 backend 的路由都只有本地变体（mock = 集成方喂；sidecar = 推模式）。
#[test]
fn no_backend_opens_network() {
    for b in [VoiceBackend::Mock, VoiceBackend::Sidecar] {
        assert!(!b.opens_network(), "{b:?} 不得让 Rust 主动发起网络请求");
    }
    assert_eq!(VoiceBackend::Mock.rust_route(), RustRoute::LocalInject);
    assert_eq!(VoiceBackend::Sidecar.rust_route(), RustRoute::AcceptPush);
}

// ---------------------------------------------------- locale（配置 + 归一化）

#[test]
fn locale_from_config_falls_back_to_default() {
    let l = |v: serde_json::Value| locale_from_config(&v);
    assert_eq!(l(serde_json::json!({})), DEFAULT_LOCALE);
    assert_eq!(l(serde_json::json!({"locale": " en-US "})), "en-US");
    assert_eq!(l(serde_json::json!({"locale": "   "})), DEFAULT_LOCALE);
    assert_eq!(l(serde_json::json!({"locale": 42})), DEFAULT_LOCALE);
}

#[test]
fn locale_profile_classifies_language_families() {
    assert_eq!(locale_profile("zh-CN"), LocaleProfile::Cjk);
    assert_eq!(locale_profile("ZH"), LocaleProfile::Cjk);
    assert_eq!(locale_profile("zh-Hans-CN"), LocaleProfile::Cjk);
    assert_eq!(locale_profile("ja_JP"), LocaleProfile::Cjk);
    assert_eq!(locale_profile("ko"), LocaleProfile::Cjk);
    assert_eq!(locale_profile("en-US"), LocaleProfile::Latin);
    assert_eq!(locale_profile(""), LocaleProfile::Latin);
    assert_eq!(locale_profile("de-DE"), LocaleProfile::Latin);
}

/// `locale=zh-CN`：删掉两个 CJK 字符之间的空格（ASR 分词伪影），
/// 但 ASCII 词两侧的**词边界空格保留**。
#[test]
fn locale_cjk_removes_inter_cjk_spaces() {
    let n = |t: &str| normalize_for_locale(t, "zh-CN");
    assert_eq!(n("你好 世界"), "你好世界");
    assert_eq!(n("把 窗户 关小一点"), "把窗户关小一点");
    assert_eq!(n("打开 wifi 开关"), "打开 wifi 开关");
    // 标点不是 CJK：其后空格原样保留（本档只管 CJK↔CJK）。
    assert_eq!(n("你好， 世界。"), "你好， 世界。");
}

/// `locale=en-US`：保留空格，只在 CJK 与 ASCII 词字符直接相邻处补边界。
#[test]
fn locale_latin_keeps_spaces_and_spaces_mixed_boundaries() {
    let n = |t: &str| normalize_for_locale(t, "en-US");
    assert_eq!(n("打开空调wifi"), "打开空调 wifi");
    assert_eq!(n("wifi开关"), "wifi 开关");
    assert_eq!(n("打开空调 wifi"), "打开空调 wifi");
    assert_eq!(n("打开 空调"), "打开 空调");
    assert_eq!(n("hello world"), "hello world");
}

#[test]
fn locale_normalization_is_idempotent() {
    for (t, l) in [("你好 世界", "zh-CN"), ("打开空调wifi", "en-US")] {
        let once = normalize_for_locale(t, l);
        assert_eq!(normalize_for_locale(&once, l), once, "归一化必须幂等: {t}");
    }
    assert!(is_cjk('好') && is_cjk('あ') && is_cjk('한'));
    assert!(!is_cjk('a') && !is_cjk('，'));
    assert!(is_ascii_word('7') && !is_ascii_word('你'));
}

// ---------------------------------------------------- prepare（唯一入口）

#[test]
fn prepare_transcript_cleans_then_normalizes() {
    assert_eq!(
        prepare_transcript("  你好\u{3000}世界  ", "zh-CN").as_deref(),
        Some("你好世界")
    );
    assert_eq!(
        prepare_transcript("  打开\u{3000}空调wifi  ", "en-US").as_deref(),
        Some("打开 空调 wifi")
    );
    for blank in ["", "   ", "\u{200B}", "\u{FEFF}\u{0007}"] {
        assert_eq!(prepare_transcript(blank, "zh-CN"), None, "{blank:?}");
    }
}

// ---------------------------------------------------- token

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
fn inject_transcript_cleans_and_normalizes_before_sending() {
    let (services, said, _logs, _actions) = recording_services();
    let rt = VoiceInputRuntime::new(services, serde_json::json!({}));
    assert!(rt.inject_transcript("  打开\u{3000}空调  "));
    assert_eq!(
        said.lock().unwrap().as_slice(),
        ["打开空调"],
        "缺省 zh-CN 归一化：CJK 词间空格被删"
    );
}

#[test]
fn inject_transcript_applies_config_locale() {
    let (services, said, _l, _a) = recording_services();
    let rt = VoiceInputRuntime::new(services, serde_json::json!({"locale": "en-US"}));
    assert!(rt.inject_transcript("打开空调wifi"));
    assert_eq!(
        said.lock().unwrap().as_slice(),
        ["打开空调 wifi"],
        "en-US 档补中英边界（locale 真的影响注入文本）"
    );
}

#[test]
fn inject_transcript_drops_blank_without_touching_say() {
    let (services, said, logs, _a) = recording_services();
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

    let (services, said, logs, _a) = recording_services();
    let rt = VoiceInputRuntime::new(services, serde_json::json!({"backend": "mock"}));
    assert_eq!(rt.backend(), VoiceBackend::Mock);
    assert!(rt.inject_mock_transcript("  你好  "));
    assert_eq!(said.lock().unwrap().as_slice(), ["你好"]);
    assert!(logs.lock().unwrap().iter().any(|m| m.contains("mock 后端")));
}

/// 注入只碰 `say_tx`：`action_tx` 必须保持休眠（rc.2 裁决）。
#[test]
fn action_tx_stays_dormant_on_inject() {
    let (services, said, _logs, actions) = recording_services();
    let rt = VoiceInputRuntime::new(services, serde_json::json!({"backend": "sidecar"}));
    assert!(rt.inject_transcript("你好"));
    assert_eq!(said.lock().unwrap().len(), 1);
    assert!(actions.lock().unwrap().is_empty(), "action_tx 必须保持休眠");
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
