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

use super::{AppSettings, SettingsError, view::settings_to_view};
use crate::audio::AudioSpec;
use crate::secret::ApiSecret;
use crate::settings::patch::{LlmPatch, PatchOutcome, SettingsPatch, apply_patch};

const FULL: &str = r#"
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

    assert_eq!(resolved.tts.voice, "alloy");
    assert_eq!(resolved.tts.spec, AudioSpec::default());
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
fn missing_section_or_bad_url_surfaces_at_resolve_not_parse() {
    // 段整体缺省是合法 TOML（容器级 default 兜底），但空 base_url 在
    // resolve 阶段按段名报错——「语法」与「运行配置」分层报告。
    let no_tts = "[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n";
    let err = AppSettings::from_toml_str(no_tts)
        .expect("段缺省可解析")
        .resolve_with(|_| None)
        .unwrap_err();
    assert!(matches!(
        err,
        SettingsError::InvalidBaseUrl { section: "tts", .. }
    ));

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

// ----- 写回 / round-trip（W6 接线：设置面板「保存」端到端）-----
#[test]
fn to_toml_string_round_trips_via_from_toml_str() {
    let original = AppSettings::from_toml_str(FULL).expect("parse FULL");
    let written = original.to_toml_string();
    let parsed = AppSettings::from_toml_str(&written).expect("re-parse");
    assert_eq!(
        original, parsed,
        "round-trip 必须逐字段相等；写回:\n{written}"
    );
}

#[test]
fn to_toml_string_writes_only_env_var_names_and_skips_none() {
    // `api_key_env` 字段类型只持有 String——编译期杜绝 secret 字段。
    // 文本里 `api_key_env` 只能出现两次（llm + tts），多 = 泄漏字段；
    // None 时不写出（`skip_serializing_if`）与解析 `default` 对称。
    let mut s = AppSettings::from_toml_str(FULL).expect("parse FULL");
    s.llm.api_key_env = Some("MY_LLM_KEY".into());
    s.tts.api_key_env = Some("MY_TTS_KEY".into());
    let written = s.to_toml_string();
    assert!(written.contains("MY_LLM_KEY"));
    assert!(written.contains("MY_TTS_KEY"));
    assert_eq!(written.matches("api_key_env").count(), 2, "应只出现两次");
    let def = AppSettings::default();
    let def_written = def.to_toml_string();
    assert!(!def_written.contains("api_key_env"));
    assert_eq!(AppSettings::from_toml_str(&def_written).unwrap(), def);
}

// ============================================================================
// dev_mode：W7 任务「可配置运行时开关」—— 4 条新测试
//   1) 默认 false（不写 = 不开）
//   2) TOML 顶层显式 true/false 都进 AppSettings.dev_mode
//   3) round-trip（写回 = 解出，含 dev_mode）
//   4) 三态补丁：None/Some(None)/Some(Some(true)) 走 patch 三态正确合并
// ============================================================================

#[test]
fn dev_mode_defaults_to_false_when_field_absent() {
    // 最小 TOML 不含 dev_mode 字段 → 走容器级 default = false。
    let minimal = r#"
        [llm]
        base_url = "http://127.0.0.1:11434/v1"
        model = "qwen2.5:7b"

        [tts]
        base_url = "http://127.0.0.1:8000/v1"
    "#;
    let s = AppSettings::from_toml_str(minimal).expect("parse");
    assert!(
        !s.dev_mode,
        "dev_mode 缺省 = false（与最小 TOML 兼容 + 无 break change）"
    );
}

#[test]
fn dev_mode_top_level_field_parses_to_bool() {
    // 顶层 `dev_mode = true/false` 必须解析进 AppSettings.dev_mode。
    let on = "dev_mode = true\n\n[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n\n[tts]\nbase_url = \"http://b/v1\"\n";
    assert!(AppSettings::from_toml_str(on).expect("parse on").dev_mode);
    let off = "dev_mode = false\n\n[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n\n[tts]\nbase_url = \"http://b/v1\"\n";
    assert!(!AppSettings::from_toml_str(off).expect("parse off").dev_mode);
}

#[test]
fn dev_mode_round_trips_through_to_toml_string() {
    // dev_mode=true 写回后解析回来必须仍为 true——单源真相、含顶层字段。
    let mut s = AppSettings::from_toml_str(FULL).expect("parse FULL");
    s.dev_mode = true;
    let written = s.to_toml_string();
    assert!(
        written.contains("dev_mode = true"),
        "dev_mode=true 必须被序列化到顶层（写回={written}）"
    );
    let back = AppSettings::from_toml_str(&written).expect("re-parse");
    assert!(back.dev_mode, "dev_mode round-trip 必须保留 true");
    assert_eq!(s, back, "整份 AppSettings round-trip 仍须字段对齐");

    // false 写回：bool 类型序列化会显式写 `dev_mode = false`（不被 skip）。
    let s_false = AppSettings::from_toml_str(FULL).expect("parse FULL"); // dev_mode=false
    let written_false = s_false.to_toml_string();
    assert!(
        written_false.contains("dev_mode = false"),
        "dev_mode=false 显式写回（写回={written_false}）"
    );
}

#[test]
fn dev_mode_three_state_patch_round_trip() {
    // 走 `dev_mode_patch: Option<Option<bool>>` 三态：
    //   None            = 不修改
    //   Some(None)      = 显式清除（强制 false）
    //   Some(Some(true))= 显式开启
    // 配合 LLM 段也修改一个无关字段，确认 dev_mode 与其它字段互不干扰。
    let s0 = AppSettings::from_toml_str(FULL).expect("parse FULL");
    assert!(!s0.dev_mode);

    // ① None = 不修改。
    let patch = SettingsPatch {
        dev_mode: None,
        llm: Some(Some(LlmPatch {
            model: Some(Some("patched".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s0, &patch).expect("apply");
    assert_eq!(outcome, PatchOutcome::Updated, "model 改了 → Updated");
    assert!(!next.dev_mode, "dev_mode 缺省 = 不修改（仍为 false）");
    assert_eq!(next.llm.model, "patched");

    // ② Some(Some(true)) = 显式开启。
    let patch = SettingsPatch {
        dev_mode: Some(Some(true)),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s0, &patch).expect("apply");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert!(next.dev_mode, "dev_mode Some(Some(true)) = 开启");

    // ③ Some(None) = 显式关闭。
    let mut s_on = s0.clone();
    s_on.dev_mode = true;
    let patch = SettingsPatch {
        dev_mode: Some(None),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s_on, &patch).expect("apply");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert!(!next.dev_mode, "dev_mode Some(None) = 关闭");

    // ④ View 也要带 dev_mode 字段。
    let s = s0.clone();
    let view = settings_to_view(&s);
    let json = serde_json::to_string(&view).expect("view json");
    assert!(
        json.contains("\"dev_mode\":false"),
        "SettingsView 必须含 dev_mode 字段（实际={json}）"
    );

    // JSON 端三态可分（与 patch 模块 double_option 规则一致）。
    let absent: SettingsPatch = serde_json::from_str("{}").expect("parse absent");
    assert!(absent.dev_mode.is_none(), "缺省 = None");
    let nulled: SettingsPatch = serde_json::from_str(r#"{"dev_mode":null}"#).expect("parse null");
    assert!(
        matches!(nulled.dev_mode, Some(None)),
        "null = Some(None)，实际 = {:?}",
        nulled.dev_mode
    );
    let on: SettingsPatch = serde_json::from_str(r#"{"dev_mode":true}"#).expect("parse true");
    assert!(
        matches!(on.dev_mode, Some(Some(true))),
        "true = Some(Some(true))，实际 = {:?}",
        on.dev_mode
    );
}

/// 角色卡：persona name/description 写入 + 生效 system_prompt 拼接（5 分支）。
#[test]
fn persona_name_desc_round_trip_and_effective_prompt() {
    use super::build_effective_system_prompt;

    // ① TOML 读写 round-trip。
    let toml_text = r#"
[persona]
name = "NEKO"
description = "一只会说话的猫娘桌宠。"
system_prompt = "用简短口语回答。"
max_history_pairs = 2
"#;
    let s = AppSettings::from_toml_str(toml_text).expect("parse");
    assert_eq!(s.persona.name, "NEKO");
    assert_eq!(s.persona.description, "一只会说话的猫娘桌宠。");

    // ② 生效 prompt 拼接（5 分支）。
    let cases = [
        // (name, desc, base, expected)
        ("", "", "", ""),
        ("", "", "base提示", "base提示"),
        ("NEKO", "", "base提示", "[角色卡] 名称：NEKO\n\nbase提示"),
        ("", "猫娘", "base提示", "[角色卡]\n描述：猫娘\n\nbase提示"),
        (
            "NEKO",
            "猫娘",
            "base提示",
            "[角色卡] 名称：NEKO\n描述：猫娘\n\nbase提示",
        ),
    ];
    for (name, desc, base, want) in cases {
        assert_eq!(
            build_effective_system_prompt(name, desc, base),
            want,
            "build_effective_system_prompt(name={name:?}, desc={desc:?}, base={base:?})"
        );
    }

    // ③ 经 resolve() 注入会话（base 为空时仍拼角色卡）。
    let s2 = AppSettings::from_toml_str(
        "[llm]\nbase_url=\"http://127.0.0.1:11434/v1\"\n[tts]\nbase_url=\"http://127.0.0.1:8000/v1\"\n[persona]\nname=\"Neko\"\ndescription=\"猫娘\"\n",
    )
    .expect("parse");
    let resolved = s2.resolve().expect("resolve");
    assert_eq!(
        resolved.conversation.system_prompt,
        "[角色卡] 名称：Neko\n描述：猫娘"
    );
}

/// persona name/description 补丁三态（patch 级）。
#[test]
fn persona_name_desc_patch_tri_state() {
    use crate::settings::patch::PersonaPatch;
    let s = AppSettings::from_toml_str(
        "[persona]\nname=\"OLD\"\ndescription=\"OLD-DESC\"\nsystem_prompt=\"p\"\n",
    )
    .expect("parse");
    // 写新值。
    let patch = SettingsPatch {
        persona: Some(Some(PersonaPatch {
            system_prompt: None,
            max_history_pairs: None,
            name: Some(Some("NEW".into())),
            description: Some(Some("NEW-DESC".into())),
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &patch).expect("apply");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.persona.name, "NEW");
    assert_eq!(next.persona.description, "NEW-DESC");

    // null = 清除。
    let clear = SettingsPatch {
        persona: Some(Some(PersonaPatch {
            system_prompt: None,
            max_history_pairs: None,
            name: Some(None),
            description: Some(None),
        })),
        ..Default::default()
    };
    let (next, _) = apply_patch(&s, &clear).expect("apply clear");
    assert!(next.persona.name.is_empty());
    assert!(next.persona.description.is_empty());
}
