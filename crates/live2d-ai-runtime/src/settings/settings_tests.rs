//! `settings` 模块的单元测试拆分载体。
//!
//! **头注豁免**：本文件为 `settings.rs` 测试拆分载体。`settings.rs` 已承载
//! `AppSettings` / `LlmSettings` / `TtsSettings` / `PersonaSettings` 类型 +
//! 解析/序列化/写回等核心实现（接近 500 行硬上限），本批为「dev_mode 可配置
//! 开关」新增 `AppSettings.dev_mode: bool` 字段（+ 整文件 round-trip / 三态
//! 补丁等测试）会让 `settings.rs` 撞 500 行上限，故按 patch 模块「子模块拆分」
//! 约定把 tests 单独提到本文件；原 8 条测试断言**一字未改**，新增 dev_mode
//! 测试 4 条。入口在 `settings.rs::mod settings_tests;`（仅 `#[cfg(test)]`
//! 下生效）——本文件作为 `settings` 的子模块，可直接 `use super::*;` 访问
//! 私有项。

use super::settings_tests_roundtrip::COMMENTED;
use super::{AppSettings, SettingsError, view::settings_to_view};
use crate::audio::AudioSpec;
use crate::secret::ApiSecret;
use crate::settings::patch::{LlmPatch, PatchOutcome, SettingsPatch, apply_patch};

pub(super) const FULL: &str = r#"
    [llm]
    base_url = "https://api.openai.com/v1"
    model = "gpt-4o-mini"
    api_key_env = "TEST_LLM_KEY"

    [tts]
    base_url = "https://tts.example/v1"
    model = "tts-1"
    voice = "nova"
    api_key_env = "TEST_TTS_KEY"
    sample_rate = 48000
    channels = 2

    [persona]
    system_prompt = "你是桌宠"
    max_history_pairs = 4
"#;

#[test]
fn full_file_parses_and_resolves_keys_from_lookup() {
    let settings = AppSettings::from_toml_str(FULL).expect("parse");
    let resolved = settings
        .resolve_with(|name| match name {
            "TEST_LLM_KEY" => Some("sk-llm".into()),
            _ => None,
        })
        .expect("resolve");

    assert_eq!(resolved.llm.base_url, "https://api.openai.com/v1");
    assert_eq!(resolved.llm.model, "gpt-4o-mini");
    assert_eq!(
        resolved.llm.api_key.as_ref().map(ApiSecret::expose_secret),
        Some("sk-llm")
    );
    assert_eq!(resolved.tts.voice, "nova");
    assert_eq!(resolved.tts.spec.sample_rate(), 48_000);
    assert_eq!(resolved.tts.spec.channels(), 2);
    // TTS 变量缺失 → 无鉴权，不是错误。
    assert!(resolved.tts.api_key.is_none());
    // persona 透传进对话配置。
    assert_eq!(resolved.conversation.system_prompt, "你是桌宠");
    assert_eq!(resolved.conversation.max_history_pairs, 4);
    // 默认仍请求 pcm。
    assert_eq!(resolved.tts.response_format.as_deref(), Some("pcm"));
}

#[test]
fn minimal_file_falls_back_to_defaults() {
    let minimal = r#"
        [llm]
        base_url = "http://127.0.0.1:11434/v1"
        model = "qwen2.5:7b"

        [tts]
        base_url = "http://127.0.0.1:8000/v1"
    "#;
    let resolved = AppSettings::from_toml_str(minimal)
        .expect("parse")
        .resolve_with(|_| None)
        .expect("resolve");

    // 2026-10-09（0.2.3-rc.1）：只写 base_url 时，其余键取**出厂缺省** ——
    // 音色 ZH、pcm、44.1 kHz 单声道（= 仓库自带 MeloTTS 垫片的
    // melo/weights/config.json 口径），不再是 OpenAI 的 "alloy" / 24 kHz。
    assert_eq!(resolved.tts.voice, crate::settings::DEFAULT_TTS_VOICE);
    assert_eq!(resolved.tts.voice, "ZH");
    assert_eq!(
        resolved.tts.spec,
        AudioSpec::new(crate::settings::DEFAULT_TTS_SAMPLE_RATE, 1).expect("非零")
    );
    assert_ne!(
        resolved.tts.spec,
        AudioSpec::default(),
        "出厂 TTS 采样率（44.1k）与播放侧默认（24k）是两个口径"
    );
    assert!(resolved.llm.api_key.is_none() && resolved.tts.api_key.is_none());
    // persona 缺省：不发 system 消息、不保留历史。
    assert_eq!(resolved.conversation.system_prompt, "");
    assert_eq!(resolved.conversation.max_history_pairs, 0);
}

#[test]
fn unknown_field_is_rejected_not_silently_ignored() {
    let typo = r#"
        [llm]
        base_url = "http://a/v1"
        model = "m"
        apikey = "typo-key-name"
    "#;
    let err = AppSettings::from_toml_str(typo).unwrap_err();
    assert!(err.to_string().contains("unknown field"), "{err}");
}

#[test]
fn missing_tts_section_is_allowed_but_bad_url_still_errors() {
    // 段整体缺省是合法 TOML（容器级 default 兜底）；非空非法 URL 仍然报错。
    // **2026-10-09（0.2.3-rc.1）**：出厂 `[tts]` 不再是「空」——缺段时用
    // `TtsSettings::default()`，它指向仓库自带的 MeloTTS 垫片
    // （8091 / ZH / pcm / 44100 / 单声道 / 无密钥），所以**开箱就有声**。
    let no_tts = "[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n";
    let resolved = AppSettings::from_toml_str(no_tts)
        .expect("段缺省可解析")
        .resolve_with(|_| None)
        .expect("TTS 缺段应可解析（用出厂缺省）");
    assert!(
        resolved.tts.is_configured(),
        "缺段 = 用出厂缺省（MeloTTS 垫片），不再是「未配置」"
    );
    assert_eq!(resolved.tts.base_url, crate::settings::DEFAULT_TTS_BASE_URL);
    assert_eq!(resolved.tts.voice, crate::settings::DEFAULT_TTS_VOICE);
    assert_eq!(
        resolved.tts.spec.sample_rate(),
        crate::settings::DEFAULT_TTS_SAMPLE_RATE
    );
    assert_eq!(resolved.tts.spec.channels(), 1);

    // **显式**写空 base_url 才是「本轮不接 TTS」（引擎走空句路径，不报错）。
    let blank_tts = "[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n\n[tts]\nbase_url = \"\"\n";
    let blank = AppSettings::from_toml_str(blank_tts)
        .expect("空 base_url 可解析")
        .resolve_with(|_| None)
        .expect("空 base_url = 空句路径，不是错误");
    assert!(!blank.tts.is_configured(), "显式空 base_url = 未配置");

    let bad_url =
        "[llm]\nbase_url = \"not a url\"\nmodel = \"m\"\n\n[tts]\nbase_url = \"http://b\"\n";
    let err = AppSettings::from_toml_str(bad_url)
        .expect("parse")
        .resolve_with(|_| None)
        .unwrap_err();
    assert!(matches!(
        err,
        SettingsError::InvalidBaseUrl { section: "llm", .. }
    ));

    let ftp = "[llm]\nbase_url = \"ftp://a\"\nmodel = \"m\"\n\n[tts]\nbase_url = \"http://b\"\n";
    assert!(matches!(
        AppSettings::from_toml_str(ftp)
            .expect("parse")
            .resolve_with(|_| None),
        Err(SettingsError::InvalidBaseUrl { .. })
    ));
}

#[test]
fn invalid_env_names_are_rejected_before_lookup() {
    let bad = r#"
        [llm]
        base_url = "http://a/v1"
        model = "m"
        api_key_env = "9BAD NAME"

        [tts]
        base_url = "http://b/v1"
    "#;
    let err = AppSettings::from_toml_str(bad)
        .expect("parse")
        .resolve_with(|_| None)
        .unwrap_err();
    assert!(matches!(
        err,
        SettingsError::InvalidEnvName { section: "llm", .. }
    ));
}

#[test]
fn zero_sample_rate_is_a_settings_error() {
    let bad = r#"
        [llm]
        base_url = "http://a/v1"
        model = "m"

        [tts]
        base_url = "http://b/v1"
        sample_rate = 0
    "#;
    let result = AppSettings::from_toml_str(bad)
        .expect("parse")
        .resolve_with(|_| None);
    assert!(result.is_err(), "sample_rate=0 必须报错而非静默回退");
}

#[test]
fn example_template_parses_and_resolves_without_env() {
    // 模板即文档：它本身必须永远可解析（密钥段留注释态）。
    let resolved = AppSettings::from_toml_str(AppSettings::example_toml())
        .expect("模板必须是合法 TOML")
        .resolve_with(|_| None)
        .expect("模板必须可无环境变量解析");
    assert!(!resolved.llm.base_url.is_empty());
}

// ------------------------------------------------- 展示思考总闸（2026-09-15）

/// **默认 false**：省略 `llm.show_reasoning` 时思考**不上屏**。
///
/// 这是产品口径（要不要把模型的内心独白摆给用户看），不是性能开关——
/// 与 `max_tokens` 无关，思考该生成多少还是多少。
#[test]
fn show_reasoning_defaults_to_false() {
    let s = AppSettings::default();
    assert_eq!(s.llm.show_reasoning, None, "缺省是「没写」而不是显式 false");
    assert!(!s.llm.effective_show_reasoning(), "生效值必须 false");
    assert!(!settings_to_view(&s).llm.show_reasoning, "视图报生效值");
    // 老 toml（没有这个键）解析出来同样是 false——升级不改变行为。
    let parsed = AppSettings::from_toml_str(FULL).expect("老配置应可解析");
    assert!(!parsed.llm.effective_show_reasoning());
}

/// 三态补丁：显式开 / 显式关 / `null` 清除（回落缺省 false）。
#[test]
fn show_reasoning_patch_is_three_state() {
    let s = AppSettings::default();

    // 缺省（字段不出现）= 不动。
    let untouched = SettingsPatch {
        llm: Some(Some(LlmPatch {
            show_reasoning: None,
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &untouched).expect("apply");
    assert_eq!(outcome, PatchOutcome::NoChange);
    assert_eq!(next.llm.show_reasoning, None);

    // 显式开。
    let on = SettingsPatch {
        llm: Some(Some(LlmPatch {
            show_reasoning: Some(Some(true)),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &on).expect("apply on");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.show_reasoning, Some(true));
    assert!(next.llm.effective_show_reasoning());
    assert!(settings_to_view(&next).llm.show_reasoning);

    // 显式 null = 清除 → 回落缺省 false。
    let clear = SettingsPatch {
        llm: Some(Some(LlmPatch {
            show_reasoning: Some(None),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&next, &clear).expect("apply clear");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.show_reasoning, None);
    assert!(!next.llm.effective_show_reasoning());
}

/// 落盘 round-trip：`show_reasoning = true` 写进 toml 再读回来仍是 true
///（缺省时该键**不出现**——老配置不受污染）。
#[test]
fn show_reasoning_survives_toml_round_trip() {
    let mut s = AppSettings::default();
    let omitted = s.to_toml_string();
    assert!(
        !omitted.contains("show_reasoning"),
        "缺省不写该键: {omitted}"
    );
    s.llm.show_reasoning = Some(true);
    let text = s.to_toml_string();
    assert!(text.contains("show_reasoning = true"), "{text}");
    let back = AppSettings::from_toml_str(&text).expect("round-trip");
    assert!(back.llm.effective_show_reasoning());
}

/// 合并结果必须**仍能被解析回等价的值**（否则「保住了注释、写坏了配置」）。
#[test]
fn merged_text_round_trips_to_the_same_settings() {
    let mut s = AppSettings::default();
    s.llm.base_url = "https://api.deepseek.com/v1".to_string();
    s.llm.model = "deepseek-flash".to_string();
    s.tts.voice = "skystar".to_string();
    s.persona.system_prompt = "小明".to_string();
    let merged = s.merge_into_toml(COMMENTED).expect("合并应成功");
    let back = AppSettings::from_toml_str(&merged).expect("合并结果应可解析");
    assert_eq!(back.llm.base_url, s.llm.base_url);
    assert_eq!(back.llm.model, s.llm.model);
    assert_eq!(back.tts.voice, s.tts.voice);
    assert_eq!(back.persona.system_prompt, s.persona.system_prompt);
}

// ------------------------- `GET/PUT /api/v1/env` 的键名真源（P1）

/// 顺序固定 `llm → tts → performance`（声明顺序 = 接口展示顺序）。
#[test]
fn declared_key_envs_lists_llm_tts_performance_in_order() {
    let mut s = AppSettings::default();
    s.llm.api_key_env = Some("L2D_LLM_KEY".into());
    s.tts.api_key_env = Some("L2D_TTS_KEY".into());
    s.performance.api_key_env = Some("L2D_PERF_KEY".into());
    let got: Vec<(&str, String)> = s
        .declared_key_envs()
        .into_iter()
        .map(|d| (d.section, d.name))
        .collect();
    assert_eq!(
        got,
        vec![
            ("llm", "L2D_LLM_KEY".to_string()),
            ("tts", "L2D_TTS_KEY".to_string()),
            ("performance", "L2D_PERF_KEY".to_string()),
        ]
    );
}

/// 空串 = 「没声明」：跳过，而不是产出一个空键名
///（空键名会让 `PUT` 白名单形同虚设，也会在错误提示里长出一对空引号）。
#[test]
fn declared_key_envs_skips_empty_names() {
    let mut s = AppSettings::default();
    s.llm.api_key_env = Some(String::new());
    s.tts.api_key_env = Some("L2D_TTS_ONLY".into());
    s.performance.api_key_env = Some(String::new());
    let got = s.declared_key_envs();
    assert_eq!(got.len(), 1, "空串不该出现: {got:?}");
    assert_eq!(got[0].section, "tts");
    assert_eq!(got[0].name, "L2D_TTS_ONLY");
}

/// 全空（全 `None` 或全空串）= 空列表 → `GET /api/v1/env` 回 `keys:[]`，
/// `PUT` 对任何键名都回 `400`。
#[test]
fn declared_key_envs_is_empty_when_nothing_declared() {
    assert!(AppSettings::default().declared_key_envs().is_empty());
    let mut s = AppSettings::default();
    s.llm.api_key_env = Some(String::new());
    s.tts.api_key_env = Some(String::new());
    s.performance.api_key_env = Some(String::new());
    assert!(s.declared_key_envs().is_empty());
}
