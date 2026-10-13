//! `performance_wiring` 的内联单测（经 `#[path = "performance_wiring_tests.rs"]` 挂回，保持父文件 ≤500 行）。

use super::*;

fn llm() -> LlmSettings {
    LlmSettings {
        base_url: "http://127.0.0.1:11434/v1".to_string(),
        model: "chat-model".to_string(),
        api_key_env: Some("CHAT_KEY".to_string()),
        ..LlmSettings::default()
    }
}

fn director(enabled: bool, base: &str, model: &str) -> DirectorTakeover {
    DirectorTakeover {
        enabled,
        staging_base_url: base.to_string(),
        staging_model: model.to_string(),
        staging_api_key_env: String::new(),
    }
}

/// **缺省不装配**（2026-10-08 产品口径）：`[performance].enabled=false` 时
/// 即使导演启用也回 `None`——主链走二路按句清洗。
#[test]
fn disabled_by_default_even_when_the_director_is_enabled() {
    let perf = PerformanceSettings::default();
    assert!(!perf.enabled, "前提：[performance].enabled 缺省关");
    assert!(
        build_performance_runtime(&llm(), &perf, &director(true, "", "")).is_none(),
        "缺省必须直送（二路清洗路径），导演启用不改变这件事"
    );
}

/// **四条装配（显式打开后）**：导演关 → 用 [performance] 自己的端点（缺端点 → None）；
/// 两项空 → 模型名等于主 LLM；两项都有 → 用导演的模型名；只填一项 → None。
#[test]
fn explicit_opt_in_decides_by_the_pair_rule() {
    let perf = PerformanceSettings {
        enabled: true,
        ..PerformanceSettings::default()
    };
    assert!(
        build_performance_runtime(&llm(), &perf, &director(false, "", "")).is_none(),
        "导演关 + [performance] 端点空 → 不装配"
    );

    let reuse = build_performance_runtime(&llm(), &perf, &director(true, "", ""))
        .expect("两项都空 = 复用对话模型");
    assert!(reuse.enabled());
    assert_eq!(reuse.model(), "chat-model", "复用的就是主 LLM 的模型名");

    let own = build_performance_runtime(
        &llm(),
        &perf,
        &director(true, "http://127.0.0.1:59999/v1", "director-model"),
    )
    .expect("两项都有 = 用导演的端点");
    assert!(own.enabled());
    assert_eq!(own.model(), "director-model");

    for (base, model) in [("http://127.0.0.1:59999/v1", ""), ("", "director-model")] {
        assert!(
            build_performance_runtime(&llm(), &perf, &director(true, base, model)).is_none(),
            "只填一项（base={base:?}, model={model:?}）必须保持直送"
        );
    }

    // 导演关 + [performance] 端点齐 → 用 [performance] 自己那一份（老口径）。
    let standalone = PerformanceSettings {
        enabled: true,
        base_url: "http://127.0.0.1:59999/v1".to_string(),
        model: "perf-own".to_string(),
        ..PerformanceSettings::default()
    };
    let rt = build_performance_runtime(&llm(), &standalone, &director(false, "", ""))
        .expect("显式打开 + 自己有端点 → 装配");
    assert_eq!(rt.model(), "perf-own");
}

/// 接管不吃 `[performance]` 的端点：base_url / model 只贡献 timeout / structured。
#[test]
fn takeover_ignores_the_performance_section_endpoint() {
    let settings = PerformanceSettings {
        enabled: true,
        base_url: "http://127.0.0.1:1/v1".to_string(),
        model: "should-not-be-used".to_string(),
        timeout_ms: 2_500,
        structured: "prompt".to_string(),
        ..PerformanceSettings::default()
    };
    let rt = build_performance_runtime(&llm(), &settings, &director(true, "", ""))
        .expect("接管与 [performance].enabled 无关");
    assert_eq!(
        rt.model(),
        "chat-model",
        "端点来自对话模型，不是 [performance]"
    );
    assert_eq!(
        rt.mode(),
        live2d_ai_runtime::performance::StructuredMode::Prompt,
        "structured 仍读 [performance]"
    );
}

/// 能力集 / 规则回退仍是 director 的单一真源（装配面不另抄一份）。
#[test]
fn allow_list_comes_from_the_director() {
    let rt = build_performance_runtime(
        &llm(),
        &PerformanceSettings {
            enabled: true,
            ..PerformanceSettings::default()
        },
        &director(true, "", ""),
    )
    .expect("装配");
    assert_eq!(
        rt.allow(),
        live2d_ai_mod_director::presets::accepted_preset_ids(),
        "能力集必须来自 accepted_preset_ids（= 主 allowlist）"
    );
    let allow_def = live2d_ai_mod_director::presets::PRESET_IDS;
    assert!(
        !rt.allow()
            .iter()
            .any(|id| !allow_def.contains(&id.as_str())),
        "旧 id 不得再进能力集：{:?}",
        rt.allow()
    );
    for id in ["none", "smile", "unhappy", "surprised", "nod", "shake"] {
        assert!(
            rt.allow().iter().any(|x| x == id),
            "主 allowlist 的 {id} 必须在能力集里"
        );
    }
}

/// `mods.json` 读法：缺文件 / 坏 JSON / 缺 director 段 = 未启用，绝不失败。
#[test]
fn manifest_reading_is_defensive() {
    let manifest = serde_json::json!({
        "mods": {
            "director": {
                "enabled": true,
                "config": {
                    "staging_base_url": "http://127.0.0.1:1/v1",
                    "staging_model": "m"
                }
            }
        }
    });
    let parsed = DirectorTakeover::from_manifest(&manifest);
    assert!(parsed.enabled);
    assert_eq!(parsed.takeover(), Takeover::Own);

    for bad in [
        serde_json::json!({}),
        serde_json::json!({"mods": {}}),
        serde_json::json!({"mods": {"director": {}}}),
    ] {
        let parsed = DirectorTakeover::from_manifest(&bad);
        assert!(!parsed.enabled, "缺段必须按未启用：{bad}");
        assert_eq!(parsed.takeover(), Takeover::ReuseConversation);
    }

    let partial = serde_json::json!({
        "mods": {"director": {"enabled": true, "config": {"staging_base_url": "http://x/v1"}}}
    });
    assert_eq!(
        DirectorTakeover::from_manifest(&partial).takeover(),
        Takeover::Incomplete,
        "只填一项 = 不接管"
    );
}

/// 动作门：`enabled=false` → 关；两项都空且 enabled → 开；只填一项 → 关。
#[test]
fn motion_on_requires_enabled_and_active_takeover() {
    assert!(!motion_on(&director(false, "", "")));
    assert!(
        motion_on(&director(true, "", "")),
        "两项都空 + enabled = 开"
    );
    assert!(
        !motion_on(&director(true, "http://127.0.0.1:1/v1", "")),
        "只填一项 = 关"
    );
    assert!(motion_on(&director(true, "http://127.0.0.1:1/v1", "m")));
}

/// T16 / T17-9：`[performance].enabled` 缺省 false；动作门开着时不转 `SentenceReady`
/// 给 Mod（= 不产生第二份 cue 集合）。
#[test]
fn performance_default_off_does_not_emit_a_second_cue_set() {
    assert!(
        !PerformanceSettings::default().enabled,
        "前提：[performance].enabled 缺省仍是 false"
    );
    // 动作门开、表演层关（缺省）→ 不转（Mod 规则层不出第二份 cue）。
    assert!(!should_forward_sentence_ready(false, true));
    // 两路都关 → 照转（既有规则导演行为不变）。
    assert!(should_forward_sentence_ready(false, false));
    assert!(!should_forward_sentence_ready(true, false));
    assert!(!should_forward_sentence_ready(true, true));
}

/// 端点：只有「动作门开 + 两项都有」才用 staging，其余用对话模型。
#[test]
fn own_endpoint_is_used_only_when_both_staging_fields_are_set() {
    assert_eq!(
        clean_endpoint(&director(true, "http://127.0.0.1:1/v1", "m")),
        CleanEndpoint::Staging
    );
    assert_eq!(
        clean_endpoint(&director(true, "http://127.0.0.1:1/v1", "")),
        CleanEndpoint::Conversation,
        "只填 base_url = 不接管"
    );
    assert_eq!(
        clean_endpoint(&director(true, "", "m")),
        CleanEndpoint::Conversation,
        "只填 model = 不接管"
    );
    assert_eq!(
        clean_endpoint(&director(true, "", "")),
        CleanEndpoint::Conversation,
        "两项都空 = 复用对话模型"
    );
    assert_eq!(
        clean_endpoint(&director(false, "http://127.0.0.1:1/v1", "m")),
        CleanEndpoint::Conversation,
        "导演没启用 = 不接管"
    );
}

/// 文件 IO：只有 `live2d-ai.toml` **同目录**的 `mods.json` 才会被读到。
#[test]
fn load_reads_the_sibling_manifest() {
    let dir = std::env::temp_dir().join(format!(
        "l2d-wiring-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(&toml, "[llm]\n").expect("write toml");
    let toml_path = toml.display().to_string();

    assert!(
        !load_director_takeover(&toml_path).enabled,
        "没有 mods.json = 未启用"
    );
    std::fs::write(dir.join("mods.json"), "not json").expect("write bad json");
    assert!(
        !load_director_takeover(&toml_path).enabled,
        "坏 JSON = 未启用"
    );
    std::fs::write(
            dir.join("mods.json"),
            r#"{"mods":{"director":{"enabled":true,"config":{"staging_model":"m","staging_base_url":"http://x/v1"}}}}"#,
        )
        .expect("write manifest");
    let loaded = load_director_takeover(&toml_path);
    assert!(loaded.enabled);
    assert_eq!(loaded.takeover(), Takeover::Own);
    let _ = std::fs::remove_dir_all(&dir);
}
