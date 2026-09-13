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
fn missing_tts_section_is_allowed_but_bad_url_still_errors() {
    // 段整体缺省是合法 TOML（容器级 default 兜底）。
    // **2026-09-10**：TTS 选型未定 —— `[tts]` 缺失 / 空 base_url 是**合法**状态
    // （引擎走空句路径：只出文本、不合成音频）；但非空非法 URL 仍然报错。
    let no_tts = "[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n";
    let resolved = AppSettings::from_toml_str(no_tts)
        .expect("段缺省可解析")
        .resolve_with(|_| None)
        .expect("TTS 未配置应可解析（空句路径）");
    assert!(!resolved.tts.is_configured(), "空 base_url = 未配置");

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

/// **rc.4 M5 主链收敛回归**：酒馆卡字段已不在 `[persona]`，出现即解析失败。
///
/// `PersonaSettings` 带 `deny_unknown_fields`——这条守住「卡字段不许回流主链」。
/// 它们属于标准 Mod `live2d-ai-mod-persona`（自己的 config 住 mods.json）。
#[test]
fn persona_card_fields_are_rejected_by_main_chain() {
    for key in ["name", "description", "personality", "scenario", "first"] {
        let text = format!("[persona]\n{key} = \"x\"\n");
        assert!(
            AppSettings::from_toml_str(&text).is_err(),
            "主链 `[persona]` 不得再接受卡字段 `{key}`：{text}"
        );
    }
    // 只留的两项可解析。
    let ok =
        AppSettings::from_toml_str("[persona]\nsystem_prompt = \"p\"\nmax_history_pairs = 2\n")
            .expect("parse");
    assert_eq!(ok.persona.system_prompt, "p");
    assert_eq!(ok.persona.max_history_pairs, 2);
}

/// **rc.4 M5**：`resolve()` 原样下发 `system_prompt`（不再拼名字/简介）。
#[test]
fn resolve_passes_system_prompt_verbatim() {
    let s = AppSettings::from_toml_str(
        "[llm]\nbase_url=\"http://127.0.0.1:11434/v1\"\n[tts]\nbase_url=\"http://127.0.0.1:8000/v1\"\n[persona]\nsystem_prompt=\"你是猫娘。\"\n",
    )
    .expect("parse");
    let resolved = s.resolve().expect("resolve");
    assert_eq!(resolved.conversation.system_prompt, "你是猫娘。");
}

/// `system_prompt` / `max_history_pairs` 的补丁三态（patch 级）。
#[test]
fn persona_prompt_patch_tri_state() {
    use crate::settings::patch::PersonaPatch;
    let s = AppSettings::from_toml_str("[persona]\nsystem_prompt=\"OLD\"\nmax_history_pairs=1\n")
        .expect("parse");
    // 写新值。
    let patch = SettingsPatch {
        persona: Some(Some(PersonaPatch {
            system_prompt: Some(Some("NEW".into())),
            max_history_pairs: Some(Some(3)),
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &patch).expect("apply");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.persona.system_prompt, "NEW");
    assert_eq!(next.persona.max_history_pairs, 3);

    // null = 清除（system_prompt 回空串 / 历史回 0）。
    let clear = SettingsPatch {
        persona: Some(Some(PersonaPatch {
            system_prompt: Some(None),
            max_history_pairs: Some(None),
        })),
        ..Default::default()
    };
    let (next, _) = apply_patch(&s, &clear).expect("apply clear");
    assert!(next.persona.system_prompt.is_empty());
    assert_eq!(next.persona.max_history_pairs, 0);
}

// ============================================================================
// llm.max_tokens：输出上限的「机制」而非提示词祈求
//   1) 省略 = 生效 512（DEFAULT_MAX_TOKENS）
//   2) 显式 0 = 不限制（与省略是两种不同状态，不能塌缩）
//   3) TOML round-trip 保留省略/显式两种形态
//   4) 视图回的是**生效值**（界面要显示「现在卡在多少」）
// ============================================================================

/// 最小 TOML + 可选的 `max_tokens` 行。
fn toml_with_max_tokens(line: &str) -> String {
    format!(
        "[llm]\nbase_url = \"http://127.0.0.1:11434/v1\"\nmodel = \"qwen2.5:7b\"\n{line}\n\
         \n[tts]\nbase_url = \"http://127.0.0.1:8000/v1\"\n"
    )
}

#[test]
fn max_tokens_omitted_falls_back_to_default() {
    let s = AppSettings::from_toml_str(&toml_with_max_tokens("")).expect("parse");
    assert_eq!(
        s.llm.max_tokens, None,
        "省略必须保持 None（不是 Some(默认值)）"
    );
    assert_eq!(s.llm.effective_max_tokens(), super::DEFAULT_MAX_TOKENS);
    // 默认值是**策略**，钉住它的具体数字只会让「改策略」看起来像「改坏了」。
    // 真正要守的是：它必须**留得下一次推理**。
    //
    // 2026-09-13 实测依据（改这个数之前请重跑一遍）：
    //   `deepseek-flash` 用同一份 `max_tokens` 同时容纳思考与正文。
    //   - 一个普通「你好」：思考 97 字、正文 23 字；
    //   - 一个要几步推理的提问：思考 1235–1602 字。
    //   512 时后者 `finish_reason=length`，正文只剩 25 字且以 `\sqrt{a` 收尾
    //   ——切不出完整句，前端**一个字都不上屏**（用户报「无模型返回」）。
    // 编译期断言（不是运行时 assert!：两边都是常量，clippy 会判
    // `assertions_on_constants`，而且编译期失败得更早）。
    const _: () = assert!(
        super::DEFAULT_MAX_TOKENS >= 2048,
        "默认上限必须给思考留出空间（实测长思考 1200–1600 token）"
    );
}

#[test]
fn max_tokens_zero_is_unlimited_and_distinct_from_omitted() {
    let s = AppSettings::from_toml_str(&toml_with_max_tokens("max_tokens = 0")).expect("parse");
    assert_eq!(s.llm.max_tokens, Some(0));
    assert_eq!(
        s.llm.effective_max_tokens(),
        0,
        "0 是合法显式值 = 不限制，不能回落成 512"
    );
    assert_ne!(
        s.llm.max_tokens,
        AppSettings::from_toml_str(&toml_with_max_tokens(""))
            .unwrap()
            .llm
            .max_tokens,
        "「不限制」与「用默认」必须可区分，否则就无法表达不限制"
    );
}

#[test]
fn max_tokens_round_trips_through_toml_and_view_echoes_effective() {
    // 省略态：写回也不出现该键；视图显示生效值 512。
    let omitted = AppSettings::from_toml_str(&toml_with_max_tokens("")).expect("parse");
    let written = omitted.to_toml_string();
    assert!(
        !written.contains("max_tokens"),
        "None 时不应写出该键（skip_serializing_if）:\n{written}"
    );
    assert_eq!(AppSettings::from_toml_str(&written).unwrap(), omitted);

    // 显式 0 与显式 4096：round-trip 逐字段相等，且视图回报生效值。
    for value in [0u32, 4096] {
        let s = AppSettings::from_toml_str(&toml_with_max_tokens(&format!("max_tokens = {value}")))
            .expect("parse");
        let back = AppSettings::from_toml_str(&s.to_toml_string()).expect("re-parse");
        assert_eq!(back.llm.max_tokens, Some(value));
        assert_eq!(
            settings_to_view(&back).llm.max_tokens,
            value,
            "视图必须回生效值（0 原样透传 = 不限制）"
        );
    }

    let view = settings_to_view(&omitted);
    assert_eq!(view.llm.max_tokens, super::DEFAULT_MAX_TOKENS);
}

// ─────────────────────────────────────────────────────────────────
// 合并写回：**不许吃掉注释**（2026-09-11 修的真实数据损失）
// ─────────────────────────────────────────────────────────────────

/// 一份**带注释**的配置（形状取自 `live2d-ai.toml.example`）。
const COMMENTED: &str = r#"# 顶部说明：这份配置是手写的，注释是给人看的
# 第二行注释

[llm]
# OpenAI 兼容服务基址（尾部 / 可省略）
base_url = "https://api.deepseek.com/v1"
# 模型名
model = "deepseek-flash"
api_key_env = "DEEPSEEK_API_KEY"   # 行尾注释也要留住
# max_tokens = 512

[tts]
base_url = "http://127.0.0.1:8080/v1"
voice = "skystar"
response_format = "pcm"
sample_rate = 24000
channels = 1

[persona]
# 人设：空 = 不注入
system_prompt = ""
"#;

#[test]
fn merge_keeps_every_comment_and_blank_line() {
    let mut s = AppSettings::default();
    s.llm.base_url = "https://api.deepseek.com/v1".to_string();
    s.llm.model = "deepseek-flash".to_string();
    s.llm.api_key_env = Some("DEEPSEEK_API_KEY".to_string());
    let merged = s.merge_into_toml(COMMENTED).expect("合并应成功");

    for needle in [
        "# 顶部说明",
        "# 第二行注释",
        "# OpenAI 兼容服务基址",
        "# 模型名",
        "# max_tokens = 512",
        "# 人设：空 = 不注入",
        "# 行尾注释也要留住",
    ] {
        assert!(
            merged.contains(needle),
            "合并后丢了注释：{needle}\n---\n{merged}"
        );
    }
    // 空行也要留着（不是「注释活着、排版死了」）。
    assert!(merged.contains("\n\n[tts]"), "段落之间的空行丢了");
}

/// 走**真实路径**（解析现有配置 → 改一个值 → 合并写回），
/// 而不是拿 `default()` 当期望值——后者会把「值为空的键」按省略语义删掉，
/// 那是另一条测试要覆盖的行为。
#[test]
fn merge_changes_only_the_value() {
    let mut s = AppSettings::from_toml_str(COMMENTED).expect("现有配置应可解析");
    s.tts.voice = "another_voice".to_string();
    let merged = s.merge_into_toml(COMMENTED).expect("合并应成功");
    assert!(merged.contains(r#"voice = "another_voice""#));
    assert!(!merged.contains(r#"voice = "skystar""#));
    // 另一个段里的**行尾注释**当然还在（我们没碰那个键）。
    assert!(
        merged.contains("# 行尾注释也要留住"),
        "改一个值不该动到别处的行尾注释：\n{merged}"
    );
}

/// **省略 = 删除**：这是与 `to_toml_string` 一致的语义，必须显式记下来。
///
/// 为什么值得单独测：`merge_into_toml` 会删掉「期望里没有的键」，
/// 所以调用方**必须**传完整快照（`apply_patch` 返回的就是完整快照），
/// 而不是一个「只想改一个字段」的部分对象——传部分对象会把其余键清空。
#[test]
fn merge_treats_omitted_keys_as_deletion() {
    // `default()` 的 `api_key_env` 是 `None`（序列化时省略）。
    let s = AppSettings::default();
    let merged = s.merge_into_toml(COMMENTED).expect("合并应成功");
    assert!(
        !merged.contains("api_key_env"),
        "省略的键应被删掉（与 to_toml_string 一致）：\n{merged}"
    );
    // 而完整快照（从现有文本解析而来）会把它留住。
    let full = AppSettings::from_toml_str(COMMENTED).expect("解析");
    assert!(
        full.merge_into_toml(COMMENTED)
            .expect("合并")
            .contains("api_key_env")
    );
}

#[test]
fn merge_removes_none_keys_but_keeps_the_rest() {
    // `api_key_env = None` → 该键应被删掉（与 `to_toml_string` 的「省略」一致），
    // 但**它的注释不许顺带把邻居带走**。
    let mut s = AppSettings::default();
    s.llm.model = "deepseek-flash".to_string();
    s.llm.api_key_env = None;
    let merged = s.merge_into_toml(COMMENTED).expect("合并应成功");
    assert!(
        !merged.contains("api_key_env"),
        "None 字段应被删掉：\n{merged}"
    );
    assert!(merged.contains("# 模型名"), "删一个键不该带走别的注释");
}

#[test]
fn merge_seeds_missing_keys_and_keeps_unknown_ones() {
    // 目标文件里**没有** [persona].system_prompt（键缺失）→ 追加；
    // 同时留一个我们不认识的键 → 保留（原实现会连带删掉）。
    let existing = "# 说明\n[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n";
    let mut s = AppSettings::default();
    s.llm.base_url = "http://a/v1".to_string();
    s.llm.model = "m".to_string();
    s.persona.system_prompt = "小明".to_string();
    let merged = s.merge_into_toml(existing).expect("合并应成功");
    assert!(merged.contains("# 说明"));
    assert!(
        merged.contains(r#"system_prompt = "小明""#),
        "缺的键要补上：\n{merged}"
    );
}

#[test]
fn merge_with_empty_existing_falls_back_to_plain_serialization() {
    let s = AppSettings::default();
    assert_eq!(s.merge_into_toml("").unwrap(), s.to_toml_string());
    assert_eq!(s.merge_into_toml("   \n  ").unwrap(), s.to_toml_string());
}

#[test]
fn merge_rejects_broken_existing_toml() {
    let s = AppSettings::default();
    assert!(s.merge_into_toml("[llm\nbase_url = ").is_err());
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
