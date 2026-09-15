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
///
/// L1（2026-09-15）起为 **v2**：v1 的 backend / locale / token **一个不删**，
/// 后面追加总闸 / 手动闸 / sidecar 四项。
#[test]
fn static_spec_fields_and_no_enabled() {
    let spec = FACTORY.settings_spec().expect("静态 schema");
    assert!(spec.validate().is_ok(), "字段 key 不得重复");
    assert_eq!(spec.version, 2, "L1 起 schema 为 v2");
    let keys: Vec<&str> = spec.fields.iter().map(|f| f.key()).collect();
    assert_eq!(
        keys,
        vec![
            "backend",
            "locale",
            "token",
            "wake_phrase",
            "manual_enabled",
            "sidecar_script",
            "sidecar_url",
            "sidecar_transcriber",
            "sidecar_python",
        ],
        "v1 三字段一个不删，新字段追加在后"
    );
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
    let rt = VoiceInputRuntime::new(services, serde_json::json!({"wake_phrase": "小爱"}));
    assert!(rt.inject_transcript("  小爱 打开\u{3000}空调  "));
    assert_eq!(
        said.lock().unwrap().as_slice(),
        ["打开空调"],
        "唤醒短语被剥掉；缺省 zh-CN 归一化：CJK 词间空格被删"
    );
}

#[test]
fn inject_transcript_applies_config_locale() {
    let (services, said, _l, _a) = recording_services();
    let rt = VoiceInputRuntime::new(
        services,
        serde_json::json!({"locale": "en-US", "wake_phrase": "小爱"}),
    );
    assert!(rt.inject_transcript("小爱打开空调wifi"));
    assert_eq!(
        said.lock().unwrap().as_slice(),
        ["打开空调 wifi"],
        "en-US 档补中英边界（locale 真的影响注入文本）"
    );
}

/// 空输入 = 没听见唤醒词（`wake_phrase_required`）；整句只有唤醒词 =
/// `empty_transcript`。两者都**不碰 say**、都留可读 warn。
#[test]
fn inject_transcript_drops_blank_without_touching_say() {
    let (services, said, logs, _a) = recording_services();
    let rt = VoiceInputRuntime::new(services, serde_json::json!({"wake_phrase": "小爱"}));
    assert!(!rt.inject_transcript("   "));
    assert!(!rt.inject_transcript("\u{200B}"));
    assert!(!rt.inject_transcript(" 小爱 "));
    assert!(said.lock().unwrap().is_empty(), "空文本不得进 say_tx");
    let logs = logs.lock().unwrap();
    assert!(logs.iter().any(|m| m.contains("唤醒短语")), "{logs:?}");
    assert!(
        logs.iter().any(|m| m.contains("empty_transcript")),
        "{logs:?}"
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
        serde_json::json!({"wake_phrase": "小爱"}),
    );
    assert!(!busy.inject_transcript("小爱你好"), "忙碌时返回 false");

    let (services, said, logs, _a) = recording_services();
    let rt = VoiceInputRuntime::new(
        services,
        serde_json::json!({"backend": "mock", "wake_phrase": "小爱"}),
    );
    assert_eq!(rt.backend(), VoiceBackend::Mock);
    assert!(rt.inject_mock_transcript("  小爱 你好  "));
    assert_eq!(said.lock().unwrap().as_slice(), ["你好"]);
    assert!(logs.lock().unwrap().iter().any(|m| m.contains("mock 后端")));
}

/// 注入只碰 `say_tx`：`action_tx` 必须保持休眠（rc.2 裁决）。
#[test]
fn action_tx_stays_dormant_on_inject() {
    let (services, said, _logs, actions) = recording_services();
    let rt = VoiceInputRuntime::new(
        services,
        serde_json::json!({"backend": "sidecar", "wake_phrase": "小爱"}),
    );
    assert!(rt.inject_transcript("小爱你好"));
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

// ---------------------------------------------------- 自检（command("selftest")）

/// 缺省配置是**自洽**的：生效 mock / zh-CN，且两条「走了缺省」的提醒都在。
#[test]
fn selftest_default_config_is_self_consistent() {
    let v = config_selftest(&serde_json::json!({}));
    assert_eq!(v["ok"], serde_json::json!(true));
    assert_eq!(v["backend"], serde_json::json!("mock"));
    assert_eq!(v["backend_defaulted"], serde_json::json!(true));
    assert_eq!(v["locale"], serde_json::json!("zh-CN"));
    assert_eq!(v["locale_defaulted"], serde_json::json!(true));
    assert_eq!(v["locale_profile"], serde_json::json!("cjk"));
    assert_eq!(v["token_set"], serde_json::json!(false));
    assert_eq!(v["route"], serde_json::json!("local_inject"));
    assert_eq!(
        v["opens_network"],
        serde_json::json!(false),
        "自检也必须证明 Rust 不开 socket"
    );
    assert_eq!(v["problems"], serde_json::json!([]));
    // L1：总闸 / 手动闸 / sidecar 脚本一并在自检里可见（缺省总闸关）。
    assert_eq!(v["manual_enabled"], serde_json::json!(true));
    assert_eq!(
        v["wake_gate_open"],
        serde_json::json!(false),
        "缺省没配唤醒短语 = 总闸关: {v}"
    );
    assert_eq!(v["wake_phrase_set"], serde_json::json!(false));
    assert_eq!(
        v["sidecar_script_resolvable"],
        serde_json::json!(false),
        "config_path 为空时解析不出缺省脚本路径: {v}"
    );
    let notes = v["notes"].as_array().expect("notes 是数组");
    assert!(notes.iter().any(|n| n.as_str().unwrap().contains("mock")));
    assert!(notes.iter().any(|n| n.as_str().unwrap().contains("zh-CN")));
    assert!(
        notes.iter().any(|n| n.as_str().unwrap().contains("总闸")),
        "总闸关必须被点名: {v}"
    );
}

/// L1：配了唤醒短语 / 关闭手动闸 / 给了 config_path 时，自检如实反映。
#[test]
fn selftest_reports_lock_state_and_resolved_script() {
    let v = selftest::config_selftest_with_script(
        &serde_json::json!({"wake_phrase": "小爱", "manual_enabled": false}),
        "/repo/live2d-ai.toml",
    );
    assert_eq!(v["wake_gate_open"], serde_json::json!(true));
    assert_eq!(v["wake_phrase_set"], serde_json::json!(true));
    assert_eq!(v["manual_enabled"], serde_json::json!(false));
    assert_eq!(
        v["sidecar_script"],
        serde_json::json!("/repo/docs/examples/voice-sidecar/voice_sidecar.py"),
        "缺省脚本路径 = <config 目录>/docs/examples/voice-sidecar/voice_sidecar.py"
    );
    assert_eq!(v["sidecar_script_resolvable"], serde_json::json!(true));
    let notes = v["notes"].as_array().unwrap();
    assert!(
        notes.iter().any(|n| n.as_str().unwrap().contains("手动闸")),
        "手动闸关必须被点名: {v}"
    );
    // 明文绝不出现。
    assert!(!v.to_string().contains("小爱"), "自检不得回显唤醒短语: {v}");
}

/// backend 写错（未知字符串 / 非字符串）：链路宽容回落 mock，但自检必须点名。
#[test]
fn selftest_flags_unknown_backend_without_killing_the_link() {
    for bad in [
        serde_json::json!({"backend": "whisper"}),
        serde_json::json!({"backend": 7}),
    ] {
        let v = config_selftest(&bad);
        assert_eq!(v["ok"], serde_json::json!(false), "got: {v}");
        assert_eq!(
            v["backend"],
            serde_json::json!("mock"),
            "未知值仍回落 mock（宽容是行为，不是隐藏）: {v}"
        );
        assert_eq!(v["backend_valid"], serde_json::json!(false), "got: {v}");
        assert_eq!(v["route"], serde_json::json!("local_inject"));
        let problems = v["problems"].as_array().unwrap();
        assert!(
            problems
                .iter()
                .any(|p| p.as_str().unwrap().contains("backend")),
            "problems 必须点名 backend: {v}"
        );
    }
    // 显式写成 mock / sidecar 都算已知。
    for good in ["mock", "sidecar"] {
        let v = config_selftest(&serde_json::json!({"backend": good}));
        assert_eq!(v["backend_valid"], serde_json::json!(true), "got: {v}");
        assert_eq!(v["ok"], serde_json::json!(true), "got: {v}");
    }
}

/// locale 写错：回显生效值（只把「空」回落成 zh-CN），但标出它会落到拉丁档。
#[test]
fn selftest_flags_malformed_locale_and_reports_latin_profile() {
    let v = config_selftest(&serde_json::json!({"locale": "!!"}));
    assert_eq!(v["ok"], serde_json::json!(false));
    assert_eq!(
        v["locale"],
        serde_json::json!("!!"),
        "locale_from_config 只回落空值，自检如实回显"
    );
    assert_eq!(v["locale_valid"], serde_json::json!(false));
    assert_eq!(v["locale_profile"], serde_json::json!("latin"));
    assert_eq!(v["locale_defaulted"], serde_json::json!(false));
    assert!(
        v["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p.as_str().unwrap().contains("locale"))
    );

    // 合法 BCP-47（含区域与脚本子标签）不报问题。
    for good in ["en-US", "zh-Hans-CN", "ja_JP"] {
        assert_eq!(
            config_selftest(&serde_json::json!({"locale": good}))["ok"],
            serde_json::json!(true),
            "{good} 应自洽"
        );
    }
}

/// 自检结果**绝不含 token 明文**；sidecar + 已设 token 不再提醒「未设 token」。
#[test]
fn selftest_never_leaks_token() {
    let secret = "s3cret-token";
    let v = config_selftest(&serde_json::json!({"backend": "sidecar", "token": secret}));
    assert_eq!(v["token_set"], serde_json::json!(true));
    assert_eq!(v["backend"], serde_json::json!("sidecar"));
    assert_eq!(v["route"], serde_json::json!("accept_push"));
    assert_eq!(v["opens_network"], serde_json::json!(false));
    assert!(
        !v.to_string().contains(secret),
        "自检结果绝不得回显 token 明文: {v}"
    );
    assert!(
        v["notes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| !n.as_str().unwrap().contains("未设 token")),
        "已设 token 就不该再提醒: {v}"
    );
}

/// sidecar + 未设 token：只是**提醒**（本机任何进程可推），不是硬问题。
#[test]
fn selftest_reminds_when_sidecar_has_no_token() {
    let v = config_selftest(&serde_json::json!({"backend": "sidecar"}));
    assert_eq!(
        v["ok"],
        serde_json::json!(true),
        "缺 token 不该判不自洽: {v}"
    );
    assert!(
        v["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("token")),
        "应有一条 token 提醒: {v}"
    );
}

#[test]
fn locale_wellformed_accepts_bcp47_lite_and_rejects_garbage() {
    for ok in ["zh", "zh-CN", "en-US", "ja_JP", "zh-Hans-CN"] {
        assert!(selftest::locale_is_wellformed(ok), "{ok} 应合法");
    }
    for bad in [
        "",
        " ",
        "z",
        "中文",
        "!!",
        "zh CN",
        "zh-",
        "-CN",
        "zh-CN-verylongsubtag",
    ] {
        assert!(!selftest::locale_is_wellformed(bad), "{bad} 应判非法");
    }
}

/// 命令通道契约：认识 selftest、其它命令回 UnsupportedCommand。
#[test]
fn command_selftest_returns_result_unknown_is_unsupported() {
    let mut rt = VoiceInputRuntime::new(
        noop_services(),
        serde_json::json!({"backend": "sidecar", "locale": "ja-JP", "token": "t"}),
    );
    let out = rt
        .command("selftest", &serde_json::json!({}))
        .expect("selftest 应被认识");
    assert_eq!(out["backend"], serde_json::json!("sidecar"));
    assert_eq!(out["locale"], serde_json::json!("ja-JP"));
    assert_eq!(out["token_set"], serde_json::json!(true));
    assert_eq!(out["route"], serde_json::json!("accept_push"));

    let err = rt
        .command("nope", &serde_json::json!({}))
        .expect_err("未知命令必须报错");
    assert!(
        matches!(err, ModError::UnsupportedCommand { ref command } if command == "nope"),
        "got: {err:?}"
    );
    // 前后空白容忍（host 已 trim，Mod 侧再 trim 一次）。
    assert!(rt.command("  selftest  ", &serde_json::json!({})).is_ok());
}

#[test]
fn runtime_selftest_matches_pure_function() {
    let config = serde_json::json!({"backend": "mock", "locale": "en-US"});
    let rt = VoiceInputRuntime::new(noop_services(), config.clone());
    assert_eq!(rt.selftest(), config_selftest(&config));
}

// ---------------------------------------------------- state / 命令（L1）

/// `state_json` 契约：零 IO、字段齐全、**不回唤醒短语 / token 明文**。
#[test]
fn state_snapshot_is_redacted_and_zero_io() {
    let rt = VoiceInputRuntime::new(
        noop_services(),
        serde_json::json!({
            "wake_phrase": "小爱", "manual_enabled": true, "token": "s3cret",
            "sidecar_transcriber": "fake"
        }),
    );
    let v = rt.state_snapshot();
    assert_eq!(v["wake_gate_open"], serde_json::json!(true));
    assert_eq!(v["wake_phrase_set"], serde_json::json!(true));
    assert_eq!(v["manual_enabled"], serde_json::json!(true));
    assert_eq!(v["token_set"], serde_json::json!(true));
    assert_eq!(v["sidecar_status"]["state"], serde_json::json!("idle"));
    assert!(
        !v.to_string().contains("小爱"),
        "state 不得回显唤醒短语: {v}"
    );
    assert!(
        !v.to_string().contains("s3cret"),
        "state 不得回显 token: {v}"
    );
}

#[test]
fn state_json_trait_method_returns_snapshot() {
    let mut rt = VoiceInputRuntime::new(noop_services(), serde_json::json!({}));
    let v = rt.state_json().expect("voice-input 必须提供 state_json");
    assert_eq!(v["wake_gate_open"], serde_json::json!(false));
}

/// 命令 `inject`：总闸关时返回**可读拒绝**（不是抛错），且不碰 say。
#[test]
fn command_inject_rejects_readably_when_gate_closed() {
    let (services, said, _l, _a) = recording_services();
    let mut rt = VoiceInputRuntime::new(services, serde_json::json!({}));
    let out = rt
        .command("inject", &serde_json::json!({"text": "你好"}))
        .expect("被闸门拒绝不是错误");
    assert_eq!(out["accepted"], serde_json::json!(false));
    assert_eq!(out["ok"], serde_json::json!(false));
    assert_eq!(out["rejected_code"], serde_json::json!("voice_gate_closed"));
    assert!(out["message"].as_str().unwrap().contains("唤醒短语"));
    assert!(said.lock().unwrap().is_empty(), "被拒的注入不得进 say");
}

#[test]
fn command_inject_accepts_with_wake_phrase_and_strips_it() {
    let (services, said, _l, _a) = recording_services();
    let mut rt = VoiceInputRuntime::new(services, serde_json::json!({"wake_phrase": "小爱"}));
    let out = rt
        .command("inject", &serde_json::json!({"text": "小爱 关灯"}))
        .expect("命中唤醒短语");
    assert_eq!(out["accepted"], serde_json::json!(true));
    assert_eq!(out["text"], serde_json::json!("关灯"));
    assert_eq!(out["rejected_code"], serde_json::json!(null));
    assert_eq!(said.lock().unwrap().as_slice(), ["关灯"]);
}

#[test]
fn command_inject_manual_off_is_readable() {
    let (services, said, _l, _a) = recording_services();
    let mut rt = VoiceInputRuntime::new(
        services,
        serde_json::json!({"wake_phrase": "小爱", "manual_enabled": false}),
    );
    let out = rt
        .command("inject", &serde_json::json!({"text": "小爱关灯"}))
        .unwrap();
    assert_eq!(out["rejected_code"], serde_json::json!("voice_manual_off"));
    assert!(said.lock().unwrap().is_empty());
}

/// `run_sidecar` 的**参数校验**（在 spawn 之前就失败，故不会真起进程）。
#[test]
fn command_run_sidecar_reports_missing_audio_readably() {
    let mut rt =
        VoiceInputRuntime::new(noop_services(), serde_json::json!({"wake_phrase": "小爱"}));
    let err = rt
        .command(
            "run_sidecar",
            &serde_json::json!({"audio_path": "/no/such/file.wav"}),
        )
        .unwrap_err();
    assert!(err.to_string().contains("/no/such/file.wav"), "{err}");
}

#[test]
fn command_run_sidecar_requires_url_and_existing_script() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/voice-sidecar/fixtures/fake_zh.wav");
    let audio = fixture.to_string_lossy().into_owned();
    // 有 config_path → 缺省脚本可解析；但缺 URL → 可读错误。
    let services = noop_services().with_config_path("/repo/live2d-ai.toml");
    let mut rt = VoiceInputRuntime::new(services, serde_json::json!({"wake_phrase": "小爱"}));
    let err = rt
        .command("run_sidecar", &serde_json::json!({"audio_path": audio}))
        .unwrap_err();
    assert!(err.to_string().contains("URL"), "{err}");
    // 给了 URL 但脚本路径不存在 → 可读错误带脚本路径。
    let mut rt2 = VoiceInputRuntime::new(
        noop_services(),
        serde_json::json!({"wake_phrase": "小爱", "sidecar_script": "/no/such/sidecar.py"}),
    );
    let err2 = rt2
        .command(
            "run_sidecar",
            &serde_json::json!({
                "audio_path": audio,
                "url": "http://127.0.0.1:18080/api/v1/voice/transcript"
            }),
        )
        .unwrap_err();
    assert!(err2.to_string().contains("/no/such/sidecar.py"), "{err2}");
}
