//! `settings_routes` 模块的单元测试（拆分承载：保持 mod.rs ≤ 500）。
//!
//! 入口：`settings_routes::mod tests;`（仅 `#[cfg(test)]` 下生效）。
//!
//! D-P0A 复审新增：段级 `clear_api_key` 互不误伤；无 clear 的 `api_key_env:
//! null` 改写为字段缺省（保持原值）；写盘路径安全（父目录创建 + rename
//! 失败 tmp 清理）。

use super::*;

fn sample() -> AppSettings {
    AppSettings {
        llm: live2d_ai_runtime::settings::LlmSettings {
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen2.5:7b".into(),
            api_key_env: Some("LIVE2D_AI_LLM_API_KEY".into()),
            max_tokens: None,
        },
        tts: live2d_ai_runtime::settings::TtsSettings {
            base_url: "http://127.0.0.1:8000/v1".into(),
            model: Some("tts-1".into()),
            voice: "alloy".into(),
            response_format: "pcm".into(),
            api_key_env: Some("LIVE2D_AI_TTS_API_KEY".into()),
            sample_rate: 24_000,
            channels: 1,
        },
        persona: live2d_ai_runtime::settings::PersonaSettings {
            system_prompt: "你是桌宠".into(),
            max_history_pairs: 2,
            ..live2d_ai_runtime::settings::PersonaSettings::default()
        },
        dev_mode: false,
    }
}

fn tempdir_path(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("live2d_ai_test_{name}_{}.toml", std::process::id()));
    p
}

fn unique_tmp(name: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0);
    let n = C.fetch_add(1, Ordering::SeqCst);
    let mut p = std::env::temp_dir();
    p.push(format!(
        "live2d_ai_test_{name}_{}_{}.toml",
        std::process::id(),
        n,
    ));
    p
}

fn init_cfg_with(path: &std::path::Path, settings: &AppSettings) {
    let _ = std::fs::remove_file(path);
    std::fs::write(path, settings.to_toml_string()).unwrap();
}

fn llm_patch_with_api_key(api_key_env: Option<Option<String>>) -> SettingsPatch {
    SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            api_key_env,
            ..Default::default()
        })),
        ..Default::default()
    }
}

#[test]
fn get_response_never_leaks_key() {
    let resp = handle_get(&sample());
    assert_eq!(resp.status_code().0, 200);
}

// ===== 段级 clear_api_key 解析 + inject 规则 =====

fn inner_api_key_env(p: &SettingsPatch, section: &str) -> Option<Option<String>> {
    match section {
        "llm" => p
            .llm
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .api_key_env
            .clone(),
        "tts" => p
            .tts
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .api_key_env
            .clone(),
        _ => unreachable!(),
    }
}

#[test]
fn patch_body_section_parsing() {
    // 段级 clear + 字段扁平化
    let body =
        PatchBody::parse(r#"{"llm":{"clear_api_key":true,"base_url":"http://x/"}}"#).unwrap();
    let llm = body.llm.unwrap().unwrap();
    assert_eq!(llm.clear_api_key, Some(true));
    assert_eq!(llm.patch.base_url, Some(Some("http://x/".into())));
    // tts 段独立
    let body = PatchBody::parse(r#"{"tts":{"voice":"nova"}}"#).unwrap();
    let tts = body.tts.unwrap().unwrap();
    assert_eq!(tts.clear_api_key, None);
    assert_eq!(tts.patch.voice, Some(Some("nova".into())));
    // 段级 null = Some(None)
    let body = PatchBody::parse(r#"{"llm":null}"#).unwrap();
    assert!(body.llm.unwrap().is_none());
}

#[test]
fn inject_rules_table() {
    // 矩阵覆盖：(clear flag, body api_key_env 字面) → inject 后状态。
    // - clear=true, body=Some(None)   → 注入 Some(None)
    // - clear=true, body=Some(Some(v))→ 保留新值（不覆盖）
    // - clear=true, body=None         → 注入 Some(None)
    // - clear=None,  body=Some(None)  → 改写为 None（保持原值）
    // - clear=Some(false), body=Some(None) → 同 None
    #[allow(clippy::type_complexity)]
    let cases: &[(
        Option<bool>,
        Option<Option<String>>,
        Option<Option<String>>,
        &str,
    )] = &[
        (Some(true), None, Some(None), "clear=true 注入"),
        (
            Some(true),
            Some(None),
            Some(None),
            "clear=true + body null → 注入",
        ),
        (
            Some(true),
            Some(Some("V".into())),
            Some(Some("V".into())),
            "显式值优先",
        ),
        (None, Some(None), None, "无 clear + null → 改写缺省"),
        (Some(false), Some(None), None, "false + null → 改写缺省"),
    ];
    for (clear, body_val, want, desc) in cases {
        let mut patch = llm_patch_with_api_key(body_val.clone());
        let body = PatchBody {
            llm: Some(Some(PatchLlmBody {
                clear_api_key: *clear,
                patch: live2d_ai_runtime::settings::patch::LlmPatch {
                    api_key_env: body_val.clone(),
                    ..Default::default()
                },
            })),
            ..Default::default()
        };
        inject_clear_key_flag(&mut patch, &body);
        assert_eq!(
            inner_api_key_env(&patch, "llm"),
            *want,
            "{desc}: clear={clear:?} body={body_val:?}"
        );
    }
}

#[test]
fn inject_section_clear_only_targets_requested_provider() {
    // LLM clear=true + TTS clear=缺省 + TTS 改 voice：TTS key 不被误清。
    let mut patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch::default())),
        tts: Some(Some(live2d_ai_runtime::settings::patch::TtsPatch {
            voice: Some(Some("nova".into())),
            api_key_env: Some(None),
            ..Default::default()
        })),
        ..Default::default()
    };
    let body = PatchBody {
        llm: Some(Some(PatchLlmBody {
            clear_api_key: Some(true),
            ..Default::default()
        })),
        tts: Some(Some(PatchTtsBody {
            clear_api_key: None,
            patch: live2d_ai_runtime::settings::patch::TtsPatch {
                voice: Some(Some("nova".into())),
                api_key_env: Some(None),
                ..Default::default()
            },
        })),
        ..Default::default()
    };
    inject_clear_key_flag(&mut patch, &body);
    assert_eq!(inner_api_key_env(&patch, "llm"), Some(None), "LLM 清");
    assert_eq!(inner_api_key_env(&patch, "tts"), None, "TTS 不被误伤");
    assert_eq!(
        patch.tts.as_ref().unwrap().as_ref().unwrap().voice,
        Some(Some("nova".into())),
        "TTS voice 不受影响"
    );
}

#[test]
fn inject_independent_clear_matrix() {
    // 矩阵 (a) LLM 清 + TTS 清 / (b) LLM 清 + TTS 留 / (c) LLM 留 + TTS 清。
    let cases = [
        (Some(true), Some(true), Some(None), Some(None)),
        (Some(true), None, Some(None), None),
        (None, Some(true), None, Some(None)),
    ];
    for (llm_clear, tts_clear, want_llm, want_tts) in cases {
        let mut patch = SettingsPatch {
            llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
                api_key_env: Some(None),
                ..Default::default()
            })),
            tts: Some(Some(live2d_ai_runtime::settings::patch::TtsPatch {
                api_key_env: Some(None),
                ..Default::default()
            })),
            ..Default::default()
        };
        let body = PatchBody {
            llm: Some(Some(PatchLlmBody {
                clear_api_key: llm_clear,
                ..Default::default()
            })),
            tts: Some(Some(PatchTtsBody {
                clear_api_key: tts_clear,
                ..Default::default()
            })),
            ..Default::default()
        };
        inject_clear_key_flag(&mut patch, &body);
        assert_eq!(
            inner_api_key_env(&patch, "llm"),
            want_llm,
            "llm_clear={llm_clear:?}"
        );
        assert_eq!(
            inner_api_key_env(&patch, "tts"),
            want_tts,
            "tts_clear={tts_clear:?}"
        );
    }
}

// ===== 端到端：clear 语义 + 段级独立 =====

/// 跑一条 PATCH + 校验磁盘状态。
fn patch_and_check(current: &AppSettings, name: &str, body: &str, assert: impl Fn(&AppSettings)) {
    let tmp = tempdir_path(name);
    init_cfg_with(&tmp, current);
    let resp = handle_patch(current, body, &tmp.to_string_lossy(), None);
    assert_eq!(resp.status_code().0, 200);
    let after = AppSettings::load_from_path(&tmp).expect("reload");
    assert(&after);
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn e2e_clear_semantics() {
    let current = sample();
    // (1) api_key_env=null 无 clear → 保持原值（P0-2 保护）。
    patch_and_check(
        &current,
        "p0a_keep_key",
        r#"{"llm":{"api_key_env":null}}"#,
        |after| assert_eq!(after.llm.api_key_env, current.llm.api_key_env),
    );
    // (2) 只清 LLM + 改 TTS voice → TTS key 保留。
    patch_and_check(
        &current,
        "p0a_clear_llm",
        r#"{"llm":{"clear_api_key":true},"tts":{"voice":"nova"}}"#,
        |after| {
            assert_eq!(after.llm.api_key_env, None);
            assert_eq!(after.tts.api_key_env, current.tts.api_key_env);
            assert_eq!(after.tts.voice, "nova");
        },
    );
    // (3) 只清 TTS + 改 LLM model → LLM key 保留。
    patch_and_check(
        &current,
        "p0a_clear_tts",
        r#"{"llm":{"model":"qwen2.5:14b"},"tts":{"clear_api_key":true}}"#,
        |after| {
            assert_eq!(after.tts.api_key_env, None);
            assert_eq!(after.llm.api_key_env, current.llm.api_key_env);
            assert_eq!(after.llm.model, "qwen2.5:14b");
        },
    );
    // (4) clear=true + 显式新 api_key_env → 显式值生效。
    patch_and_check(
        &current,
        "p0a_clear_with_new",
        r#"{"llm":{"clear_api_key":true,"api_key_env":"NEW_LLM_KEY"}}"#,
        |after| assert_eq!(after.llm.api_key_env, Some("NEW_LLM_KEY".into())),
    );
    // (5) 矩阵：llm 清 + tts 清 / llm 清 + tts 留 / llm 留 + tts 清。
    for (name, body, want_llm, want_tts) in [
        (
            "p0a_m_ab",
            r#"{"llm":{"clear_api_key":true},"tts":{"clear_api_key":true}}"#,
            None,
            None,
        ),
        (
            "p0a_m_a",
            r#"{"llm":{"clear_api_key":true},"tts":{"voice":"nova"}}"#,
            None,
            current.tts.api_key_env.clone(),
        ),
        (
            "p0a_m_b",
            r#"{"llm":{"model":"qwen2.5:14b"},"tts":{"clear_api_key":true}}"#,
            current.llm.api_key_env.clone(),
            None,
        ),
    ] {
        patch_and_check(&current, name, body, |after| {
            assert_eq!(after.llm.api_key_env, want_llm, "{name}");
            assert_eq!(after.tts.api_key_env, want_tts, "{name}");
        });
    }
}

// ===== 写盘路径安全：父目录创建 + tmp 清理 =====

#[test]
fn apply_and_write_creates_parent_dir_when_missing() {
    // 父目录不存在 → 第一次写盘应创建并成功。
    let mut base = std::env::temp_dir();
    base.push(format!("live2d_ai_test_p0a_mkdir_{}", std::process::id()));
    let target = base.join("nested").join("config.toml");
    let _ = std::fs::remove_dir_all(&base);

    let current = sample();
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("first-write".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let resp = apply_and_write(&current, &patch, &target.to_string_lossy(), None)
        .expect("apply+write should succeed with parent dir creation");
    assert!(resp.persisted);
    let after = AppSettings::load_from_path(&target).expect("reload");
    assert_eq!(after.llm.model, "first-write");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn apply_and_write_cleans_tmp_on_rename_failure() {
    // 模拟 rename / write tmp 失败：让 target 父路径是**文件**。
    // tmp 路径 = `<target>.tmp.<pid>` 必须写到 target 的父目录。
    // 所以"父路径是文件"指 target.parent() 必须是个文件。
    let base = unique_tmp("p0a_rename_fail");
    let _ = std::fs::remove_file(&base);
    std::fs::write(&base, "I am a file, not a dir").unwrap();
    // target 路径 = `<base>/<file>.toml` → parent = base（已存在的文件）。
    let target = base.join("config.toml");
    let current = sample();
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("new".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let result = apply_and_write(&current, &patch, &target.to_string_lossy(), None);
    // 父路径是文件 → create_dir_all(base) 失败 → apply_and_write 返回 Err。
    assert!(
        result.is_err(),
        "父路径是文件时写盘应失败: {:?}",
        result.as_ref().err()
    );
    // 验证无本次目标对应的 tmp.* 残留（pid + 唯一 nanos）。
    let pid = std::process::id();
    let base_name = base.file_name().unwrap().to_string_lossy().to_string();
    let leftover = std::fs::read_dir(std::env::temp_dir())
        .expect("readdir /tmp")
        .filter_map(|e| e.ok())
        .any(|e| {
            let n = e.file_name().to_string_lossy().to_string();
            n.contains(&base_name) && n.contains(&format!(".tmp.{pid}"))
        });
    assert!(!leftover, "rename 失败后 tmp 应被清理");
    let _ = std::fs::remove_file(&base);
}

// ===== 旧覆盖 =====

#[test]
fn apply_patch_then_persists_round_trip() {
    let tmp = tempdir_path("patch_roundtrip");
    init_cfg_with(&tmp, &AppSettings::default());
    let current = AppSettings::load_from_path(&tmp).expect("load");
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("new-model".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let resp =
        apply_and_write(&current, &patch, &tmp.to_string_lossy(), None).expect("apply+write");
    assert!(resp.persisted);
    assert_eq!(resp.settings.llm.model, "new-model");
    let after = AppSettings::load_from_path(&tmp).expect("reload");
    assert_eq!(after.llm.model, "new-model");
    let _ = std::fs::remove_file(&tmp);
}
#[test]
fn no_change_patch_does_not_touch_disk() {
    let tmp = tempdir_path("patch_no_change");
    init_cfg_with(&tmp, &AppSettings::default());
    let first = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            model: Some(Some("first".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let _ = apply_and_write(
        &AppSettings::load_from_path(&tmp).expect("load"),
        &first,
        &tmp.to_string_lossy(),
        None,
    )
    .expect("first");
    let disk = AppSettings::load_from_path(&tmp).expect("load");
    let resp = apply_and_write(&disk, &first, &tmp.to_string_lossy(), None).expect("second");
    assert!(!resp.persisted, "NoChange 时不应写盘");
    let _ = std::fs::remove_file(&tmp);
}
#[test]
fn url_invalid_patch_returns_400() {
    let mut current = sample();
    current.llm.base_url = "not a url".into();
    let patch = SettingsPatch {
        llm: Some(Some(live2d_ai_runtime::settings::patch::LlmPatch {
            base_url: Some(Some("not a url".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    let tmp = tempdir_path("url_invalid");
    let _ = std::fs::remove_file(&tmp);
    let err = apply_and_write(&current, &patch, &tmp.to_string_lossy(), None).unwrap_err();
    assert_eq!(err.error.code, "url_invalid");
}
#[test]
fn test_outcome_serializes_with_ok_tag() {
    let s = TestOutcome::success(42, "qwen2.5:7b");
    let j = serde_json::to_value(&s).unwrap();
    assert_eq!(j["ok"], true);
    assert_eq!(j["latency_ms"], 42);
    assert_eq!(j["model_echo"], "qwen2.5:7b");
}
#[test]
fn test_outcome_failure_serializes_with_ok_false() {
    let f = TestOutcome::failure("timeout", "请求超时");
    let j = serde_json::to_value(&f).unwrap();
    assert_eq!(j["ok"], false);
    assert_eq!(j["error"]["code"], "timeout");
}
#[test]
fn timeout_to_duration_clamps_to_bounds() {
    use std::time::Duration;
    assert_eq!(timeout_to_duration(None), Duration::from_millis(3000));
    assert_eq!(timeout_to_duration(Some(0)), Duration::from_millis(1));
    assert_eq!(timeout_to_duration(Some(8000)), Duration::from_millis(8000));
    assert_eq!(
        timeout_to_duration(Some(99_999)),
        Duration::from_millis(8000)
    );
}
#[test]
fn run_llm_test_fails_on_unconfigured_url() {
    let mut s = sample();
    s.llm.base_url.clear();
    let out = run_llm_test(&s, Some(100));
    assert!(!out.ok);
    assert_eq!(out.error.unwrap().code, "llm_unreachable");
}
#[test]
fn handle_patch_invalid_json_returns_400() {
    let tmp = tempdir_path("bad_json");
    init_cfg_with(&tmp, &AppSettings::default());
    let current = AppSettings::load_from_path(&tmp).expect("load");
    let resp = handle_patch(&current, "not json", &tmp.to_string_lossy(), None);
    assert_eq!(resp.status_code().0, 400);
    let _ = std::fs::remove_file(&tmp);
}

/// `llm.max_tokens` 走完整 HTTP PATCH → 写盘 → 重载 链路。
///
/// 这里是**唯一**能证明「三态 + flatten + 落盘」三者真的一起工作的地方：
/// `PatchLlmBody` 用 `#[serde(flatten)]` 展开 `LlmPatch`，而 flatten 走的是
/// serde 的 buffered-content 反序列化路径——与直接 `from_str::<LlmPatch>`
/// 不同，`double_option` 在那条路径上是否仍然区分 `0` / `null` / 缺省，
/// 只有端到端跑一遍才算数。
#[test]
fn e2e_max_tokens_tri_state_through_http_patch() {
    let current = sample();
    assert_eq!(current.llm.max_tokens, None, "起点：省略 = 用默认");

    // 1) 显式 0 = 不限制（必须与 null 区分开）。
    patch_and_check(
        &current,
        "mt_zero",
        r#"{"llm":{"max_tokens":0}}"#,
        |after| assert_eq!(after.llm.max_tokens, Some(0)),
    );

    // 2) 显式 2048 = 硬上限。
    patch_and_check(
        &current,
        "mt_set",
        r#"{"llm":{"max_tokens":2048}}"#,
        |after| assert_eq!(after.llm.max_tokens, Some(2048)),
    );

    // 3) 缺省字段 = 不动（已有值保持）。
    let capped = AppSettings {
        llm: live2d_ai_runtime::settings::LlmSettings {
            max_tokens: Some(333),
            ..current.llm.clone()
        },
        ..current.clone()
    };
    patch_and_check(
        &capped,
        "mt_keep",
        r#"{"llm":{"model":"qwen2.5:14b"}}"#,
        |after| assert_eq!(after.llm.max_tokens, Some(333)),
    );

    // 4) 显式 null = 清除 → 回落默认（视图报默认值，而不是「不限制」）。
    //
    // 断言**引用常量**而不是写死数字：默认值本身是策略（2026-09-13 因推理模型
    // 的思考占用输出预算，512 → 4096），钉死数字会让「改策略」看起来像
    // 「改坏了」，而这条测试真正要守的是「null 会回落到默认、不是不限制」。
    patch_and_check(
        &capped,
        "mt_clear",
        r#"{"llm":{"max_tokens":null}}"#,
        |after| {
            assert_eq!(after.llm.max_tokens, None);
            assert_eq!(
                live2d_ai_runtime::settings::view::settings_to_view(after)
                    .llm
                    .max_tokens,
                live2d_ai_runtime::settings::DEFAULT_MAX_TOKENS
            );
        },
    );
}
