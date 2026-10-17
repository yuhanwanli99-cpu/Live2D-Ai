//! rc.6（2026-10-17）多厂商选择的回归：旧 toml 逐字 round-trip、快照→当前、
//! 未知 / 非法 id、melo 与自定义 TTS、非激活坏配置、无密钥泄露、单端点、
//! 手改当前不回写快照、悬空标签容忍。

use super::patch::{PatchOutcome, SettingsPatch, apply_patch};
use super::view::settings_to_view_with_keys;
use super::{AppSettings, MELO_VENDOR_ID, TtsMode};

fn parse(text: &str) -> AppSettings {
    AppSettings::from_toml_str(text).expect("测试配置应可解析")
}

fn no_keys(_: &str) -> Option<String> {
    None
}

/// 旧 toml（没有 vendor / vendors）解析后序列化必须逐字相同，且不得冒出
/// vendor 键；merge_into_toml 还要保留原注释。
#[test]
fn old_toml_without_vendor_roundtrips_verbatim() {
    let canonical = AppSettings::default().to_toml_string();
    let parsed = parse(&canonical);
    assert_eq!(
        parsed.to_toml_string(),
        canonical,
        "旧 toml 必须逐字 round-trip"
    );
    assert!(!canonical.contains("vendor"), "空快照不得写进文件");

    // 带注释的手写文件：并回写只改值，不新增 vendor 键、不抹注释。
    let handwritten = canonical.replace("[tts]", "# 出声端点（用户注释）\n[tts]");
    let mut edited = parse(&handwritten);
    edited.dev_mode = true;
    let merged = edited.merge_into_toml(&handwritten).expect("merge 应成功");
    assert!(merged.contains("# 出声端点（用户注释）"));
    assert!(!merged.contains("vendor"));
    assert!(parse(&merged).dev_mode);
}

/// 选择厂商 = 一次 PATCH 的「快照→当前」原子复制：标签、当前值与快照都不被改写，
/// 且 resolve 只用被复制后的当前段（vendors 不进 runtime）。
#[test]
fn selecting_llm_vendor_copies_snapshot_atomically() {
    let text = r#"[llm]
base_url = "http://127.0.0.1:11434/v1"
model = "qwen2.5:7b"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo-large"
api_key_env = "FOO_KEY"
max_tokens = 2048
show_reasoning = true
"#;
    let current = parse(text);
    let before_vendors = current.llm.vendors.clone();
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":"foo"}}"#).expect("补丁应可解析");
    let (next, outcome) = apply_patch(&current, &patch).expect("合法厂商应可选中");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.vendor.as_deref(), Some("foo"));
    assert_eq!(next.llm.base_url, "https://api.example.com/v1");
    assert_eq!(next.llm.model, "foo-large");
    assert_eq!(next.llm.api_key_env.as_deref(), Some("FOO_KEY"));
    assert_eq!(next.llm.max_tokens, Some(2048));
    assert_eq!(next.llm.show_reasoning, Some(true));
    assert_eq!(next.llm.vendors, before_vendors, "快照本身不得被改写");

    let resolved = next
        .resolve_with(|name| (name == "FOO_KEY").then(|| "sk-x".to_string()))
        .expect("选中后的当前值应可解析");
    assert_eq!(resolved.llm.base_url, "https://api.example.com/v1");
    assert_eq!(resolved.llm.model, "foo-large");
    // 单端点：ResolvedSettings 仍只有一份 llm。
    assert_eq!(resolved.tts.base_url, next.tts.base_url);
}

/// 切到不存在的 id / 非法 id → Err（路由层映射为 400），当前值不动。
#[test]
fn unknown_or_invalid_vendor_is_rejected() {
    let current = parse(&AppSettings::default().to_toml_string());
    let long_id = "a".repeat(33);
    for id in ["ghost", "Bad", "a b", "", long_id.as_str()] {
        let body = serde_json::json!({ "llm": { "vendor": id } }).to_string();
        let patch: SettingsPatch = serde_json::from_str(&body).expect("补丁应可解析");
        let err = apply_patch(&current, &patch).expect_err("不存在的厂商必须被拒");
        assert!(
            err.contains("vendor") && (err.contains("不存在") || err.contains("非法")),
            "错误消息应说明厂商问题：{err}"
        );
    }
}

/// 选 melo：apply_local_tts_defaults() + mode=local；vendor=melo；快照不进文件。
#[test]
fn tts_melo_selection_applies_local_defaults() {
    let text = r#"[tts]
mode = "cloud"
base_url = "https://tts.example.com/v1"
voice = "alloy"
api_key_env = "TTS_KEY"
response_format = "wav"
sample_rate = 24000
channels = 2
"#;
    let current = parse(text);
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"tts":{"vendor":"melo"}}"#).expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("melo 是内置快照，必须可选");
    assert_eq!(next.tts.vendor.as_deref(), Some(MELO_VENDOR_ID));
    assert_eq!(next.tts.mode, TtsMode::Local);
    assert_eq!(next.tts.base_url, super::DEFAULT_TTS_BASE_URL);
    assert_eq!(next.tts.voice, super::DEFAULT_TTS_VOICE);
    assert_eq!(next.tts.api_key_env, None);
    assert_eq!(next.tts.response_format, "pcm");
    assert_eq!(next.tts.sample_rate, super::DEFAULT_TTS_SAMPLE_RATE);
    assert_eq!(next.tts.channels, 1);
    // 内置快照不落盘。
    assert!(!next.to_toml_string().contains("vendors.melo"));
    assert!(next.to_toml_string().contains("vendor = \"melo\""));
}

/// 选文件里的自定义 TTS 厂商（含 localhost 第二引擎）→ 复制快照 + 强制 mode=cloud。
#[test]
fn tts_custom_vendor_forces_cloud_mode() {
    let text = r#"[tts]
mode = "local"
base_url = "http://127.0.0.1:8091/v1"
voice = "ZH"

[tts.vendors.second]
base_url = "http://127.0.0.1:9000/v1"
voice = "EN"
sample_rate = 24000
channels = 2
model = "custom-tts"
"#;
    let current = parse(text);
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"tts":{"vendor":"second"}}"#).expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("自定义厂商应可选中");
    assert_eq!(next.tts.mode, TtsMode::Cloud);
    assert_eq!(next.tts.vendor.as_deref(), Some("second"));
    assert_eq!(next.tts.base_url, "http://127.0.0.1:9000/v1");
    assert_eq!(next.tts.voice, "EN");
    assert_eq!(next.tts.model.as_deref(), Some("custom-tts"));
    assert_eq!(next.tts.sample_rate, 24000);
    assert_eq!(next.tts.channels, 2);
}

/// 非激活项不参与 resolve：坏 URL / 坏 env 名的厂商可以留在文件里，
/// 当前链路照常工作；只有「选中它」才 400。
#[test]
fn inactive_vendor_bad_config_does_not_brick() {
    let text = r#"[llm]
base_url = "http://127.0.0.1:11434/v1"
model = "ok"

[llm.vendors.broken]
base_url = "not-a-url"
model = "x"
api_key_env = "9BAD"
"#;
    let current = parse(text);
    current
        .resolve_with(no_keys)
        .expect("非激活坏配置不得 brick");
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":"broken"}}"#).expect("补丁应可解析");
    let err = apply_patch(&current, &patch).expect_err("选中坏配置必须被拒");
    assert!(
        err.contains("base_url") || err.contains("api_key_env"),
        "{err}"
    );
}

/// GET 视图：厂商只回 has_api_key，绝不回 api_key_env / 值；密钥值为空时 false。
#[test]
fn vendor_view_never_leaks_env_names_or_values() {
    let text = r#"[llm]
base_url = "http://127.0.0.1:11434/v1"
model = "ok"
api_key_env = "CURRENT_KEY"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo"
api_key_env = "FOO_SECRET_ENV"

[tts]
base_url = "http://127.0.0.1:8091/v1"
voice = "ZH"

[tts.vendors.bar]
base_url = "https://tts.example.com/v1"
voice = "alloy"
api_key_env = "BAR_SECRET_ENV"
"#;
    let current = parse(text);
    let view = settings_to_view_with_keys(&current, &|name| {
        matches!(name, "CURRENT_KEY" | "FOO_SECRET_ENV").then(|| "sk-live".to_string())
    });
    let json = serde_json::to_string(&view).expect("视图应可序列化");
    assert!(!json.contains("api_key_env"), "不得回变量名");
    assert!(!json.contains("FOO_SECRET_ENV"));
    assert!(!json.contains("BAR_SECRET_ENV"));
    assert!(!json.contains("CURRENT_KEY"));
    assert!(!json.contains("sk-live"), "不得回密钥值");
    assert!(view.llm.has_api_key);
    assert!(view.llm.vendors["foo"].has_api_key);
    assert!(!view.tts.vendors["bar"].has_api_key);
    // 内置 melo 始终在列表里。
    assert!(view.tts.vendors.contains_key(MELO_VENDOR_ID));
}

/// 手改当前值后 GET 给 vendor_modified=true，但**不回写**快照。
#[test]
fn hand_edit_reports_vendor_modified_without_rewriting_snapshot() {
    let text = r#"[llm]
base_url = "https://api.example.com/v1"
model = "foo"
vendor = "foo"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo"
"#;
    let mut current = parse(text);
    let view = settings_to_view_with_keys(&current, &no_keys);
    assert!(!view.llm.vendor_modified, "与快照一致时不得报 modified");
    current.llm.model = "hand-edited".to_string();
    let view = settings_to_view_with_keys(&current, &no_keys);
    assert!(view.llm.vendor_modified);
    assert_eq!(current.llm.vendors["foo"].model, "foo", "GET 不得回写快照");
}

/// 悬空标签容忍：vendor 指向不存在的 id 不 brick、不报 modified；
/// 但显式「切到它」仍 400。
#[test]
fn dangling_vendor_label_is_tolerated() {
    let text = r#"[llm]
base_url = "http://127.0.0.1:11434/v1"
model = "ok"
vendor = "ghost"
"#;
    let current = parse(text);
    current.resolve_with(no_keys).expect("悬空标签不得 brick");
    let view = settings_to_view_with_keys(&current, &no_keys);
    assert_eq!(view.llm.vendor.as_deref(), Some("ghost"));
    assert!(!view.llm.vendor_modified);
    // 同一个悬空标签随整卡保存回来 → 容忍（不复制、不报错、不 brick）。
    let same: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":"ghost"}}"#).expect("补丁应可解析");
    assert!(apply_patch(&current, &same).is_ok());
    // 「切到」另一个不存在的 id 才是 400。
    let other: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":"ghost2"}}"#).expect("补丁应可解析");
    assert!(apply_patch(&current, &other).is_err());
}

/// 显式 null = 只清标签，当前值不动（快照仍在）。
#[test]
fn clearing_vendor_label_keeps_current_values() {
    let text = r#"[llm]
base_url = "https://api.example.com/v1"
model = "foo"
vendor = "foo"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo"
"#;
    let current = parse(text);
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":null}}"#).expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("清标签应成功");
    assert_eq!(next.llm.vendor, None);
    assert_eq!(next.llm.base_url, current.llm.base_url);
    assert_eq!(next.llm.vendors, current.llm.vendors);
}

/// 切 TTS 厂商不得碰 LLM 段（local-engine 等 Mod 也不在配置里被写）。
#[test]
fn switching_tts_vendor_leaves_other_sections_untouched() {
    let text = r#"[llm]
base_url = "http://127.0.0.1:11434/v1"
model = "ok"

[tts]
base_url = "http://127.0.0.1:8091/v1"
voice = "ZH"

[tts.vendors.bar]
base_url = "https://tts.example.com/v1"
voice = "alloy"
"#;
    let current = parse(text);
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"tts":{"vendor":"bar"}}"#).expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("合法厂商应可选中");
    assert_eq!(next.llm, current.llm);
    assert_eq!(next.persona, current.persona);
    assert_eq!(next.action, current.action);
    assert_eq!(next.performance, current.performance);
    assert_eq!(next.dev_mode, current.dev_mode);
}

/// 一次补丁里 vendor 与显式字段同现、且标签**确实变了**：快照复制优先
/// （原子，不半新半旧）。标签未变时不得重放快照——见下一个用例。
#[test]
fn vendor_selection_wins_when_label_changes() {
    let text = r#"[llm]
base_url = "http://127.0.0.1:11434/v1"
model = "ok"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo"
"#;
    let current = parse(text);
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":"foo","model":"explicit"}}"#)
            .expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("应成功");
    assert_eq!(next.llm.base_url, "https://api.example.com/v1");
    assert_eq!(next.llm.model, "foo", "切换厂商 = 复制快照");
    assert_eq!(next.llm.vendor.as_deref(), Some("foo"));
}

/// 保存整张卡时前端会把当前 vendor 一并带上：标签未变 → 不重放快照，
/// 用户刚编辑的字段必须保留。
#[test]
fn same_vendor_label_does_not_replay_snapshot() {
    let text = r#"[llm]
base_url = "https://api.example.com/v1"
model = "foo"
vendor = "foo"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo"
"#;
    let current = parse(text);
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":"foo","model":"draft-edit"}}"#)
            .expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("应成功");
    assert_eq!(next.llm.model, "draft-edit");
    assert_eq!(next.llm.vendor.as_deref(), Some("foo"));
}

/// 下拉「重新选中同一厂商」= 只发 vendor 的一帧 → 重新套用快照（修复手改）。
#[test]
fn reselecting_same_vendor_reapplies_snapshot() {
    let text = r#"[llm]
base_url = "https://api.example.com/v1"
model = "hand-edited"
vendor = "foo"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo"
"#;
    let current = parse(text);
    let patch: SettingsPatch =
        serde_json::from_str(r#"{"llm":{"vendor":"foo"}}"#).expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("重选应成功");
    assert_eq!(next.llm.model, "foo", "只发 vendor 的一帧应重放快照");
}

/// 段级清空连标签与快照表一起清。
#[test]
fn clearing_llm_section_drops_vendors() {
    let text = r#"[llm]
base_url = "https://api.example.com/v1"
model = "foo"
vendor = "foo"

[llm.vendors.foo]
base_url = "https://api.example.com/v1"
model = "foo"
"#;
    let current = parse(text);
    let patch: SettingsPatch = serde_json::from_str(r#"{"llm":null}"#).expect("补丁应可解析");
    let (next, _) = apply_patch(&current, &patch).expect("清段应成功");
    assert_eq!(next.llm.vendor, None);
    assert!(next.llm.vendors.is_empty());
}
