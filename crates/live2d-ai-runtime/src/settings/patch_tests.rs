//! `settings::patch` 模块的单元测试。
//!
//! **头注豁免**：本文件为 `patch.rs` 测试拆分载体。`patch.rs` 已承载三态补丁
//! 类型 + `apply_patch` + `plan_atomic_write` 的实现（现 630 行，见其头注里的
//! 行数豁免说明），再内嵌本文件的测试会直接撞 1000 行豁免上限，故按 settings
//! 模块「子模块拆分」约定把 tests 单独提到本文件；断言与原文件**一字不改**。
//! 入口在 `patch.rs::mod patch_tests;`（仅 `#[cfg(test)]` 下生效）——本文件
//! 作为 `patch` 的子模块，可直接 `use super::*;` 访问 `patch` 的私有项。
//!
//! **2026-09-26（阶段5 D40）**：追加 `[action.models.<id>]` 每模型覆盖的回归
//! （覆盖>全局逐键回落 / 无覆盖跟随全局 / 量程钳位 / 非法 id 拒绝 / 删除与空表
//! 清理 / TOML round-trip）后本文件超过 800 行测试上限——同 `patch.rs` 的理由：
//! 三态语义在这一份文件里成体系，拆开会让「改一半忘了另一半」成为默认风险。

use std::process;

use crate::settings::patch::{
    LlmPatch, PatchOutcome, PersonaPatch, SettingsPatch, TtsPatch, apply_patch, plan_atomic_write,
};
use crate::settings::view::{
    settings_to_view, settings_to_view_with_keys, settings_to_view_with_keys_and_model,
};
use crate::settings::{AppSettings, LlmSettings, PersonaSettings, TtsSettings};

fn sample() -> AppSettings {
    AppSettings {
        // 2026-09-16：action 段默认（幅度倍率）。
        action: Default::default(),
        // 2026-09-22：表演层默认关（[performance] 段，客户端在 host 侧构造）。
        performance: Default::default(),
        llm: LlmSettings {
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen2.5:7b".into(),
            api_key_env: Some("LLM_KEY".into()),
            max_tokens: None,
            show_reasoning: None,
        },
        tts: TtsSettings {
            base_url: "http://127.0.0.1:8000/v1".into(),
            model: Some("tts-1".into()),
            voice: "alloy".into(),
            response_format: "pcm".into(),
            api_key_env: Some("TTS_KEY".into()),
            sample_rate: 24_000,
            channels: 1,
        },
        persona: PersonaSettings {
            system_prompt: "你是桌宠".into(),
            max_history_pairs: 2,
        },
        dev_mode: false,
    }
}

#[test]
fn settings_to_view_with_keys_drops_key_env_name_and_derives_from_lookup() {
    let s = sample();

    // (a) lookup 拿不到任何值 → 三段 has_api_key 全 false；序列化里既没有
    //     环境变量名，也没有任何明文密钥。
    let no_value = |_: &str| None;
    let v = settings_to_view_with_keys(&s, &no_value);
    let json = serde_json::to_string(&v).unwrap();
    assert!(!json.contains("LLM_KEY"), "view 仍含 env 名：{json}");
    assert!(!json.contains("TTS_KEY"), "view 仍含 env 名：{json}");
    assert!(
        !json.contains("api_key_env"),
        "view 暴露 api_key_env 字段：{json}"
    );
    assert_eq!(json.matches("\"has_api_key\":false").count(), 3, "{json}");

    // (b) 有非空值 → llm / tts / performance 三段走**同一**私有派生函数，
    //     一起翻 true；值本身永不出现在视图里。
    let mut s2 = s.clone();
    s2.performance.api_key_env = Some("PERF_KEY".into());
    let lookup = |name: &str| match name {
        "LLM_KEY" => Some("sk-llm".to_string()),
        "TTS_KEY" => Some("sk-tts".to_string()),
        "PERF_KEY" => Some("sk-perf".to_string()),
        _ => None,
    };
    let v2 = settings_to_view_with_keys(&s2, &lookup);
    assert!(
        v2.llm.has_api_key && v2.tts.has_api_key && v2.performance.has_api_key,
        "有值 → 三段都应 true"
    );
    let json2 = serde_json::to_string(&v2).unwrap();
    for leaked in [
        "sk-llm", "sk-tts", "sk-perf", "LLM_KEY", "TTS_KEY", "PERF_KEY",
    ] {
        assert!(!json2.contains(leaked), "view 泄露 {leaked}：{json2}");
    }

    // (c) 空串值 = 未设置（与 secrets::lookup 的「空串一律视为未设置」同口径）。
    let empty = |_: &str| Some(String::new());
    assert!(!settings_to_view_with_keys(&s, &empty).llm.has_api_key);

    // (d) 名字未声明（api_key_env = None）→ false，且**不以空名调用** lookup
    //     （空名不是合法变量名，不该被当成一个可能的键去查）。
    let mut s3 = s.clone();
    s3.llm.api_key_env = None;
    let seen = std::cell::RefCell::new(Vec::<String>::new());
    let record = |name: &str| {
        seen.borrow_mut().push(name.to_string());
        None
    };
    assert!(!settings_to_view_with_keys(&s3, &record).llm.has_api_key);
    assert!(
        !seen.borrow().iter().any(|n| n.is_empty()),
        "不得以空名调用 lookup：{:?}",
        seen.borrow()
    );
}

/// `settings_to_view` 的语义**冻结**：`has_api_key` = 「声明了变量名」，
/// 不读环境、不接受 lookup。egui / 纯展示口径；Web API 不得用它。
#[test]
fn settings_to_view_still_reports_declared_key_name_only() {
    let s = sample();
    let v = settings_to_view(&s);
    assert!(
        v.llm.has_api_key && v.tts.has_api_key,
        "声明了键名即为 true"
    );
    let json = serde_json::to_string(&v).unwrap();
    assert!(!json.contains("LLM_KEY") && !json.contains("TTS_KEY"));

    // 空串 / None = 未声明。
    let mut s2 = s.clone();
    s2.llm.api_key_env = Some(String::new());
    s2.tts.api_key_env = None;
    let v2 = settings_to_view(&s2);
    let json2 = serde_json::to_string(&v2).unwrap();
    assert!(json2.contains("\"llm\":{") && json2.contains("\"has_api_key\":false"));
    assert!(json2.contains("\"tts\":{") && json2.contains("\"has_api_key\":false"));
}

#[test]
fn patch_none_keeps_old_base_url() {
    let s = sample();
    // 段级 None = 不动 LLM 段。
    let patch = SettingsPatch {
        llm: None,
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::NoChange);
    assert_eq!(next.llm.base_url, s.llm.base_url);
    assert_eq!(next.llm.model, s.llm.model);
    // 字段级 None = 也不动。
    let patch2 = SettingsPatch {
        llm: Some(Some(LlmPatch {
            base_url: None,
            model: Some(Some("gpt-4o-mini".into())),
            api_key_env: None,
            max_tokens: None,
            show_reasoning: None,
        })),
        ..Default::default()
    };
    let (next2, outcome2) = apply_patch(&s, &patch2).unwrap();
    assert_eq!(outcome2, PatchOutcome::Updated);
    assert_eq!(
        next2.llm.base_url, s.llm.base_url,
        "base_url 字段级 None = 保留"
    );
    assert_eq!(next2.llm.model, "gpt-4o-mini");
}

#[test]
fn patch_some_empty_string_clears_api_key_env() {
    let mut s = sample();
    s.llm.api_key_env = Some("LLM_KEY".into());
    s.tts.api_key_env = Some("TTS_KEY".into());
    // api_key_env 字段级 Some(None) = 显式清除。
    let patch = SettingsPatch {
        llm: Some(Some(LlmPatch {
            api_key_env: Some(None),
            ..Default::default()
        })),
        tts: Some(Some(TtsPatch {
            api_key_env: Some(None),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    assert!(next.llm.api_key_env.is_none());
    assert!(next.tts.api_key_env.is_none());
    // 视图层确认 has_api_key 翻成 false（llm / tts；performance 段也带一个
    // has_api_key，那份在下面单独断言——不把它混进「两段都翻 false」的口径里）。
    // 注入的 lookup 故意「任何键都有值」：若派生还只看键名而不是
    // 「键名 + 值」，清空之后这里会红。
    let always_value = |_: &str| Some("sk-any".to_string());
    let v = settings_to_view_with_keys(&next, &always_value);
    assert!(!v.llm.has_api_key);
    assert!(!v.tts.has_api_key);
    let json = serde_json::to_string(&v).unwrap();
    assert_eq!(json.matches("\"has_api_key\":false").count(), 3, "{json}");
    assert!(!json.contains("_KEY\""), "视图不得回环境变量名：{json}");
}

#[test]
fn patch_with_new_value_overrides() {
    let s = sample();
    // 字段级 Some(Some(v)) = 覆盖为新值。
    let patch = SettingsPatch {
        llm: Some(Some(LlmPatch {
            base_url: Some(Some("https://api.openai.com/v1".into())),
            api_key_env: Some(Some("NEW_LLM_KEY".into())),
            ..Default::default()
        })),
        tts: Some(Some(TtsPatch {
            voice: Some(Some("nova".into())),
            sample_rate: Some(Some(48_000)),
            ..Default::default()
        })),
        persona: Some(Some(PersonaPatch {
            system_prompt: Some(Some("新提示".into())),
            max_history_pairs: Some(Some(4)),
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.base_url, "https://api.openai.com/v1");
    assert_eq!(next.llm.api_key_env.as_deref(), Some("NEW_LLM_KEY"));
    assert_eq!(next.tts.voice, "nova");
    assert_eq!(next.tts.sample_rate, 48_000);
    assert_eq!(next.persona.system_prompt, "新提示");
    assert_eq!(next.persona.max_history_pairs, 4);
}

#[test]
fn invalid_patch_returns_err() {
    let s = sample();
    // base_url 字段给个非 http URL + 非空 → 拒绝。
    let patch = SettingsPatch {
        llm: Some(Some(LlmPatch {
            base_url: Some(Some("not a url".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert!(apply_patch(&s, &patch).is_err());

    // api_key_env 字段给非法 env 名 → 拒绝。
    let patch2 = SettingsPatch {
        tts: Some(Some(TtsPatch {
            api_key_env: Some(Some("9 BAD".into())),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert!(apply_patch(&s, &patch2).is_err());

    // sample_rate 0 → 拒绝。
    let patch3 = SettingsPatch {
        tts: Some(Some(TtsPatch {
            sample_rate: Some(Some(0)),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert!(apply_patch(&s, &patch3).is_err());

    // channels 0 → 拒绝。
    let patch4 = SettingsPatch {
        tts: Some(Some(TtsPatch {
            channels: Some(Some(0)),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert!(apply_patch(&s, &patch4).is_err());
}

#[test]
fn patch_clear_section_empties_all_fields() {
    let s = sample();
    // 段级 Some(None) = 整段显式清空。
    let patch = SettingsPatch {
        llm: Some(None),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&s, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.base_url, "");
    assert_eq!(next.llm.model, "");
    assert!(next.llm.api_key_env.is_none());
}

#[test]
fn plan_atomic_write_writes_tmp_and_returns_path() {
    // 用 tempdir 替代：自己清理。
    let dir = std::env::temp_dir().join(format!("l2d-ai-atomic-{}", process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("live2d-ai.toml");
    let tmp = plan_atomic_write(&target, "hello = 1\n").unwrap();
    // tmp 路径形状：完整文件名 + `.tmp.<pid>`（**不**吞掉 `.toml`）。
    let expected_name = format!("live2d-ai.toml.tmp.{}", process::id());
    assert_eq!(
        tmp.file_name().and_then(|n| n.to_str()),
        Some(expected_name.as_str()),
        "tmp 必须保留原扩展名 .toml（修复 W6-B tmp 路径形状）"
    );
    // tmp 文件已写好 + 内容落盘可读。
    assert!(tmp.exists());
    assert_eq!(std::fs::read_to_string(&tmp).unwrap(), "hello = 1\n");
    // 调用方 rename → 原位置出现内容。
    std::fs::rename(&tmp, &target).unwrap();
    assert!(target.exists());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello = 1\n");
    // 清理。
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plan_atomic_write_rejects_empty_content() {
    // 空内容 = 静默丢失 live2d-ai.toml，必须拒绝。
    let dir = std::env::temp_dir().join(format!("l2d-ai-atomic-empty-{}", process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let target = dir.join("live2d-ai.toml");
    let result = plan_atomic_write(&target, "");
    assert!(result.is_err(), "空 content 必须被拒绝，不能写空盘");
    // 同时确认 tmp 文件未被误写。
    let pid = process::id();
    let stray = dir.join(format!("live2d-ai.toml.tmp.{pid}"));
    assert!(
        !stray.exists(),
        "空内容被拒绝时不该创建 tmp 文件（实际={}）",
        stray.display()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plan_atomic_write_cleans_tmp_on_io_failure() {
    // 写 tmp 失败（如父目录不存在）必须不留半写文件。
    let target = std::path::PathBuf::from("/this/path/should/not/exist/live2d-ai.toml");
    let result = plan_atomic_write(&target, "hello = 1\n");
    assert!(result.is_err(), "父目录不存在时写 tmp 必须失败");
    // 父目录本身未创建 → tmp 不可能存在。
    let pid = process::id();
    let stray = std::path::PathBuf::from(format!(
        "/this/path/should/not/exist/live2d-ai.toml.tmp.{pid}"
    ));
    assert!(
        !stray.exists(),
        "写失败时不应留半写 tmp（实际={}）",
        stray.display()
    );
}

// ============================================================================
// 修复点 W6-A：SettingsPatch 的「缺省 / null / 显式值」三态在 JSON 端
// 必须真实可区分。旧实现里 `Option<Option<T>>` 的 serde 默认行为把「字段
// 缺省」与「显式 null」都映射成 `None`，导致 HTTP 端无法表达「显式清除」。
// 改用 `serde_with::rust::double_option` 后三态可分；下列测试 **直接走
// serde_json::from_str**（不再只 Rust 构造），把 P0-2 防误删的契约测试
// 锁在 deserializer 层。
// ============================================================================

#[test]
fn json_three_state_explicit_null_clears_field() {
    // `{"llm":{"api_key_env":null}}` → api_key_env = Some(None) = 显式清除。
    let json = r#"{"llm":{"api_key_env":null}}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("合法 JSON");
    let llm = patch.llm.expect("llm 段在场");
    assert!(
        llm.is_some(),
        "llm 段是对象（不是 null）→ Some(Some(_))，不能用 is_none() 漏掉"
    );
    let inner = llm.expect("显式 Some(Some(_))");
    assert!(
        matches!(inner.api_key_env, Some(None)),
        "显式 null → Some(None)（清除），实际 = {:?}",
        inner.api_key_env
    );
    // 没动到的字段保持 None（缺省 = 不修改）。
    assert!(inner.base_url.is_none());
    assert!(inner.model.is_none());
}

#[test]
fn json_three_state_absent_field_is_no_change() {
    // 字段缺省 = 不修改；缺省与显式 null 不能再塌缩成同一状态。
    let json = r#"{"llm":{"base_url":"x"}}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("合法 JSON");
    let inner = patch.llm.expect("llm 在场").expect("对象形态");
    // base_url 显式 = Some(Some("x"))。
    assert!(matches!(&inner.base_url, Some(Some(v)) if v == "x"));
    // api_key_env 字段缺省 = None（**不是** Some(None)）——这是修复点核心。
    assert!(
        inner.api_key_env.is_none(),
        "缺省字段必须是 None（不修改），不能用 Some(None) 误表达为清除"
    );
    assert!(inner.model.is_none(), "缺省字段必须是 None（不修改）");
}

#[test]
fn json_three_state_explicit_value_overrides() {
    // 显式值 → Some(Some(v))；与 Some(None) 必须类型可分。
    let json = r#"{"llm":{"base_url":"https://api/x","model":"y"}}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("合法 JSON");
    let inner = patch.llm.expect("llm 在场").expect("对象形态");
    assert!(matches!(&inner.base_url, Some(Some(v)) if v == "https://api/x"));
    assert!(matches!(&inner.model, Some(Some(v)) if v == "y"));
    // 没填的字段保持缺省 None。
    assert!(inner.api_key_env.is_none());
}

#[test]
fn json_three_state_segment_null_clears_whole_section() {
    // 段级 null = Some(None) = 整段清空（与段缺省 = None 必须可分）。
    let json = r#"{"tts":null}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("合法 JSON");
    // llm 缺省 = None（不动）。
    assert!(patch.llm.is_none(), "llm 缺省 = None（不修改）");
    // tts:null = Some(None)（清空）。
    assert!(
        matches!(patch.tts, Some(None)),
        "tts:null 必须解为 Some(None)（清空整段），实际 = {:?}",
        patch.tts
    );
    // 与段级对象 / 段缺省三者**两两不等**——这是双 Option 在段级也起作用的证据。
    let empty_obj: SettingsPatch = serde_json::from_str(r#"{"tts":{}}"#).expect("合法 JSON");
    let absent: SettingsPatch = serde_json::from_str("{}").expect("合法 JSON");
    assert_ne!(patch.tts, empty_obj.tts, "null vs 空对象必须可分");
    assert_ne!(patch.tts, absent.tts, "null vs 缺省必须可分");
    assert_ne!(empty_obj.tts, absent.tts, "空对象 vs 缺省必须可分");
}

#[test]
fn json_three_state_empty_object_is_field_level_no_op() {
    // `{"llm":{}}` = Some(Some(LlmPatch{全 None})) = 字段级 no-op（任何字段都没动）。
    // 与段级 `{"llm":null}` (清空) 和段级缺省（不动）三者**都不同**：
    // - `{}` 整体：None（不动）
    // - `{"llm":{}}`：Some(Some(全 None))（段在场，字段级三态合并 → 不修改）
    // - `{"llm":null}`：Some(None)（段在场，**显式清空**整段）
    let json = r#"{"llm":{}}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("合法 JSON");
    assert!(
        matches!(&patch.llm, Some(Some(_))),
        "空对象必须是 Some(Some(_))，实际 = {:?}",
        patch.llm
    );
    let inner = patch.llm.as_ref().unwrap().as_ref().unwrap();
    assert!(inner.base_url.is_none());
    assert!(inner.model.is_none());
    assert!(inner.api_key_env.is_none());

    // 端到端：`{"llm":{}}` 经 apply_patch 后是 NoChange，**不**触发段清空。
    let s = sample();
    let (next, outcome) = apply_patch(&s, &patch).unwrap();
    assert_eq!(
        outcome,
        PatchOutcome::NoChange,
        "段级空对象 = 字段级 no-op（不修改任何字段），不能误清空整段"
    );
    assert_eq!(next.llm, s.llm);

    // 对照：`{"llm":null}` 走段级清空路径。
    let null_patch: SettingsPatch = serde_json::from_str(r#"{"llm":null}"#).expect("合法 JSON");
    let (next_null, outcome_null) = apply_patch(&s, &null_patch).unwrap();
    assert_eq!(outcome_null, PatchOutcome::Updated);
    assert_eq!(next_null.llm.base_url, "");
    assert_eq!(next_null.llm.model, "");
    assert!(next_null.llm.api_key_env.is_none());
}

#[test]
fn json_three_state_field_null_after_value_clears_only_that_field() {
    // `{"llm":{"api_key_env":null}}` 经 apply_patch 后只清 api_key_env，
    // **不**影响 base_url / model。
    let json = r#"{"llm":{"api_key_env":null}}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("合法 JSON");
    let s = sample();
    let (next, outcome) = apply_patch(&s, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.api_key_env, None, "api_key_env 被显式清除");
    // base_url / model **没动**（缺省 = 不修改）。
    assert_eq!(next.llm.base_url, s.llm.base_url);
    assert_eq!(next.llm.model, s.llm.model);
}

#[test]
fn json_three_state_invalid_payload_is_rejected() {
    // `base_url` 非法值走 `apply_patch` 错误路径——三态拆开后非法值的拒绝
    // 仍要在 deserializer + apply_patch 双层都生效。
    let json = r#"{"llm":{"base_url":"not a url"}}"#;
    let patch: SettingsPatch = serde_json::from_str(json).expect("JSON 形状合法");
    let s = sample();
    assert!(apply_patch(&s, &patch).is_err(), "非法 base_url 必须拒绝");

    // 非法 sample_rate（0）——数值类型可解，但 apply_patch 拒绝。
    let json2 = r#"{"tts":{"sample_rate":0}}"#;
    let patch2: SettingsPatch = serde_json::from_str(json2).expect("JSON 形状合法");
    assert!(apply_patch(&s, &patch2).is_err());

    // 真正的 JSON 语法错（缺右括号）—— deserializer 拒绝。
    let bad = r#"{"llm":{"base_url":"x""#;
    let parse_err = serde_json::from_str::<SettingsPatch>(bad);
    assert!(parse_err.is_err(), "JSON 语法错必须被 deserializer 拒绝");
}

#[test]
fn json_three_state_serialize_round_trip_preserves_three_states() {
    // 三态在 serialize 端也要保留——回环不能让「显式 null」塌缩成「缺省」。
    //
    // 实现细节：字段同时挂 `double_option` (deserialize/serialize) +
    // `skip_serializing_if = "Option::is_none"`——`double_option::serialize`
    // 默认会把「外层 None」也写成 `null`（破坏 round-trip 不可塌缩性），
    // 加 skip 后 outer None = 字段不出现，deserialize 走 `#[serde(default)]`
    // 拿回 `None`，三态两两不塌缩。
    let src = SettingsPatch {
        llm: Some(Some(LlmPatch {
            base_url: Some(None),            // 显式清空 → 序列化为 null
            model: Some(Some("gpt".into())), // 显式设值
            api_key_env: None,               // 缺省 → skip，不出现
            max_tokens: None,                // 同上
            show_reasoning: None,            // 同上
        })),
        ..Default::default()
    };
    let json = serde_json::to_string(&src).unwrap();
    // 显式 null / 显式值出现在 JSON；outer None 不出现。
    assert!(
        json.contains("\"base_url\":null"),
        "显式清除必须被序列化为 null，实际 = {json}"
    );
    assert!(json.contains("\"model\":\"gpt\""), "{json}");
    assert!(
        !json.contains("api_key_env"),
        "outer None 字段必须 skip（不出现），实际 = {json}"
    );

    // 回环后三态两两不塌缩：
    //   base_url:  Some(None)        (显式 null → 解为 Some(None))
    //   model:     Some(Some("gpt")) (显式值 → 解为 Some(Some("gpt")))
    //   api_key_env: None            (字段缺省 → 走 default → None)
    let back: SettingsPatch = serde_json::from_str(&json).unwrap();
    assert!(matches!(&back.llm, Some(Some(_))));
    let inner = back.llm.unwrap().unwrap();
    assert!(
        matches!(inner.base_url, Some(None)),
        "Some(None) 字段回环后仍为 Some(None)，实际 = {:?}",
        inner.base_url
    );
    assert!(
        matches!(&inner.model, Some(Some(v)) if v == "gpt"),
        "显式值字段回环后仍为 Some(Some(v))"
    );
    assert!(
        inner.api_key_env.is_none(),
        "outer None 字段回环后仍为 None（不塌缩为 Some(None)）"
    );
}

// ============================================================================
// llm.max_tokens 三态：缺省=不动 / null=回落默认(512) / 数字=硬设（0 = 不限制）
//
// 三态在这里是**必需**的，不是仪式：`0`（不限制）与 `省略`（用默认 512）
// 语义相反，若用 `Option<u32>` 表达就会塌缩成同一个 `None`，
// 「把上限清掉」和「设为不限制」将无法分别表达。
// ============================================================================

#[test]
fn max_tokens_patch_tri_state_does_not_collapse_zero_and_absent() {
    let s = sample();
    assert_eq!(s.llm.max_tokens, None);

    // 1) 字段缺省（outer None）= 不动：即便原值是 Some(64) 也保持。
    let mut with_value = s.clone();
    with_value.llm.max_tokens = Some(64);
    let untouched = SettingsPatch {
        llm: Some(Some(LlmPatch {
            base_url: None,
            model: None,
            api_key_env: None,
            max_tokens: None,
            show_reasoning: None,
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&with_value, &untouched).expect("apply");
    assert_eq!(outcome, PatchOutcome::NoChange);
    assert_eq!(next.llm.max_tokens, Some(64), "缺省 = 不动");

    // 2) 显式设值：含 0（= 不限制，必须原样存下去而不是被当成 None）。
    for value in [0u32, 1024] {
        let set = SettingsPatch {
            llm: Some(Some(LlmPatch {
                base_url: None,
                model: None,
                api_key_env: None,
                max_tokens: Some(Some(value)),
                show_reasoning: None,
            })),
            ..Default::default()
        };
        let (next, outcome) = apply_patch(&with_value, &set).expect("apply set");
        assert_eq!(outcome, PatchOutcome::Updated);
        assert_eq!(next.llm.max_tokens, Some(value));
    }

    // 3) 显式 null = 清除 → 回落默认（视图报 512），不是「不限制」。
    let clear = SettingsPatch {
        llm: Some(Some(LlmPatch {
            base_url: None,
            model: None,
            api_key_env: None,
            max_tokens: Some(None),
            show_reasoning: None,
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&with_value, &clear).expect("apply clear");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.llm.max_tokens, None);
    assert_eq!(
        settings_to_view(&next).llm.max_tokens,
        crate::settings::DEFAULT_MAX_TOKENS,
        "null = 回落默认，绝不能等于「不限制」"
    );

    // 4) 整段清空（llm: Some(None)）也要把 max_tokens 一并清回默认。
    let (next, _) = apply_patch(
        &with_value,
        &SettingsPatch {
            llm: Some(None),
            ..Default::default()
        },
    )
    .expect("apply section clear");
    assert_eq!(next.llm.max_tokens, None);
}

/// JSON 线上形态：`0` 与 `null` 必须是**两个不同的** JSON 字面量。
#[test]
fn max_tokens_patch_json_keeps_zero_and_null_distinct() {
    let set_zero: SettingsPatch = serde_json::from_str(r#"{"llm":{"max_tokens":0}}"#).unwrap();
    assert_eq!(
        set_zero.llm.as_ref().unwrap().as_ref().unwrap().max_tokens,
        Some(Some(0))
    );

    let clear: SettingsPatch = serde_json::from_str(r#"{"llm":{"max_tokens":null}}"#).unwrap();
    assert_eq!(
        clear.llm.as_ref().unwrap().as_ref().unwrap().max_tokens,
        Some(None)
    );

    let absent: SettingsPatch = serde_json::from_str(r#"{"llm":{"model":"m"}}"#).unwrap();
    assert_eq!(
        absent.llm.as_ref().unwrap().as_ref().unwrap().max_tokens,
        None
    );

    // 写回也不塌缩。
    assert!(
        serde_json::to_string(&set_zero)
            .unwrap()
            .contains(r#""max_tokens":0"#)
    );
    assert!(
        serde_json::to_string(&clear)
            .unwrap()
            .contains(r#""max_tokens":null"#)
    );
    assert!(
        !serde_json::to_string(&absent)
            .unwrap()
            .contains("max_tokens")
    );
}

// ===== [action] 幅度倍率（2026-09-16） =====

/// 段缺省 → 用出厂默认（0.75 / 0.80 / 1.0），不是 0；序列化也带上该段。
#[test]
fn action_section_defaults_are_product_scales() {
    let s = AppSettings::from_toml_str("").unwrap();
    assert_eq!(s.action.head_scale, 0.75);
    assert_eq!(s.action.body_scale, 0.80);
    assert_eq!(s.action.expression_scale, 1.0);
    let toml = s.to_toml_string();
    assert!(toml.contains("[action]"), "{toml}");
}

/// 显式值覆盖默认；视图回**归一化后**的生效值（越界钳进 [0.2, 2.2]）。
#[test]
fn action_section_overrides_and_view_normalizes() {
    let s = AppSettings::from_toml_str(
        "[action]\nhead_scale = 1.25\nbody_scale = 9.0\nexpression_scale = 0.0\n",
    )
    .unwrap();
    let v = settings_to_view(&s);
    assert_eq!(v.action.head_scale, 1.25);
    // 9.0 -> 钳 2.2；0.0 -> 钳 0.2。
    assert_eq!(v.action.body_scale, 2.2);
    assert_eq!(v.action.expression_scale, 0.2);
}

/// 未知键仍拒绝解析（deny_unknown_fields 对 [action] 一样生效）。
#[test]
fn action_section_rejects_unknown_keys() {
    assert!(AppSettings::from_toml_str("[action]\nhead = 1.0\n").is_err());
}

/// PATCH：三态（不改 / 设为值并钳 / null 回落默认）。
#[test]
fn action_patch_sets_clamps_and_resets() {
    use crate::settings::patch::ActionPatch;
    let base = AppSettings::from_toml_str("").unwrap();

    let patch = SettingsPatch {
        action: Some(Some(ActionPatch {
            head_scale: Some(Some(9.0)),
            body_scale: Some(Some(0.0)),
            expression_scale: None,
            models: None,
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&base, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.action.head_scale, 2.2);
    assert_eq!(next.action.body_scale, 0.2);
    assert_eq!(next.action.expression_scale, 1.0, "不修改的字段保持原值");

    let patch2 = SettingsPatch {
        action: Some(Some(ActionPatch {
            head_scale: Some(None),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next2, _) = apply_patch(&next, &patch2).unwrap();
    assert_eq!(next2.action.head_scale, 0.75, "null -> 回落出厂默认");
    assert_eq!(next2.action.body_scale, 0.2, "其余字段不动");
}

/// 三项都等于默认时是 NoChange（浮点噪声不该点亮界面 dirty）。
#[test]
fn action_patch_identical_is_no_change() {
    use crate::settings::patch::ActionPatch;
    let base = AppSettings::from_toml_str("").unwrap();
    let patch = SettingsPatch {
        action: Some(Some(ActionPatch {
            head_scale: Some(Some(0.75)),
            body_scale: Some(Some(0.80)),
            expression_scale: Some(Some(1.0)),
            models: None,
        })),
        ..Default::default()
    };
    let (_, outcome) = apply_patch(&base, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::NoChange);
}
// ============================================================================
// `[action.models.<id>]` 每模型覆盖（阶段5 D40，2026-09-26）
//
// 判据①覆盖>全局且逐键回落 / ②无覆盖跟随全局 / ④超量程钳 / ⑤非法 id 不落表。
// 判据③（PATCH 写回保注释、无 .tmp 残留）在 settings_routes 的 e2e 测试里
// （它需要真实写盘 + handle_patch 全链路）。
// ============================================================================

/// 判据①：覆盖 > 全局，且**逐键**回落（bai 只覆盖 head_scale）。
#[test]
fn action_models_override_beats_global_per_key() {
    let s = AppSettings::from_toml_str(
        "[action]\nhead_scale = 0.75\nbody_scale = 0.80\nexpression_scale = 1.0\n\n[action.models.bai]\nhead_scale = 1.5\n",
    )
    .expect("parse");
    let e = s.action.effective_for("bai");
    assert_eq!(e.head_scale, 1.5, "head 用覆盖值");
    assert_eq!(e.body_scale, 0.80, "body 逐键回落全局");
    assert_eq!(e.expression_scale, 1.0, "expression 逐键回落全局");
    // 全局三键仍在（覆盖表不污染全局）。
    let g = s.action.effective_for("no-such-model");
    assert_eq!(
        (g.head_scale, g.body_scale, g.expression_scale),
        (0.75, 0.80, 1.0)
    );
    // 视图口径同源（归一化后）。
    let v = settings_to_view(&s);
    assert_eq!(v.action.models["bai"].head_scale, Some(1.5));
    assert_eq!(v.action.models["bai"].body_scale, None);
}

/// 判据②：没有覆盖的模型（含空表、含空串 id）完整跟随全局。
#[test]
fn action_models_absent_model_follows_global() {
    let s = AppSettings::from_toml_str("[action]\nhead_scale = 1.1\n").expect("parse");
    assert!(s.action.models.is_empty(), "没写 models 就是空表");
    for id in ["bai", "hiyori", ""] {
        let e = s.action.effective_for(id);
        assert_eq!(
            (e.head_scale, e.body_scale, e.expression_scale),
            (1.1, 0.80, 1.0),
            "id={id:?}"
        );
    }
}

/// 判据④：越界钳 [0.2, 2.2]；NaN / 无穷按既有 `set_scale` 口径回落默认。
#[test]
fn action_models_clamp_out_of_range_and_nan_falls_back() {
    use crate::settings::patch::{ActionModelOverridePatch, ActionPatch};
    let base = AppSettings::from_toml_str("").expect("parse");
    let patch = SettingsPatch {
        action: Some(Some(ActionPatch {
            models: Some(std::collections::BTreeMap::from([(
                "bai".to_string(),
                Some(ActionModelOverridePatch {
                    head_scale: Some(Some(9.0)),
                    body_scale: Some(Some(-1.0)),
                    expression_scale: Some(Some(f32::NAN)),
                }),
            )])),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&base, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    let ov = next.action.models.get("bai").expect("bai 落表");
    assert_eq!(ov.head_scale, Some(2.2), "9.0 钳到上限");
    assert_eq!(ov.body_scale, Some(0.2), "-1.0 钳到下限");
    assert_eq!(ov.expression_scale, Some(1.0), "NaN 回落出厂默认 1.0");
    // 全局三键不受覆盖影响；全局自己的 NaN/无穷也沿用同一口径。
    assert_eq!(next.action.head_scale, 0.75);
    let gpatch = SettingsPatch {
        action: Some(Some(ActionPatch {
            head_scale: Some(Some(f32::INFINITY)),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (gnext, _) = apply_patch(&base, &gpatch).unwrap();
    assert_eq!(gnext.action.head_scale, crate::settings::DEFAULT_HEAD_SCALE);
}

/// 判据⑤：非法模型 id（空串 / 空格 / 斜杠 / 中文 / 超 64 / `..`）**不落表**，
/// `apply_patch` 返回 Err（上层映射 400 invalid_payload）；合法 id 正常写入。
#[test]
fn action_models_invalid_id_is_rejected_not_written() {
    use crate::settings::patch::{ActionModelOverridePatch, ActionPatch};
    let base = AppSettings::from_toml_str("").expect("parse");
    let long = "a".repeat(crate::settings::MAX_MODEL_ID_LEN + 1);
    let over = "a".repeat(crate::settings::MAX_MODEL_ID_LEN);
    let bad = [
        "",
        " ",
        "with space",
        "a/b",
        "中文",
        "a\tb",
        long.as_str(),
        "..",
        "a..b",
    ];
    for id in bad {
        let patch = SettingsPatch {
            action: Some(Some(ActionPatch {
                models: Some(std::collections::BTreeMap::from([(
                    id.to_string(),
                    Some(ActionModelOverridePatch {
                        head_scale: Some(Some(1.0)),
                        ..Default::default()
                    }),
                )])),
                ..Default::default()
            })),
            ..Default::default()
        };
        let err = apply_patch(&base, &patch).expect_err("非法 id 必须 Err");
        assert!(err.contains("非法模型 id"), "id={id:?} 错误={err}");
    }
    // 边界：恰好 64 的合法 id 正常落表。
    let patch = SettingsPatch {
        action: Some(Some(ActionPatch {
            models: Some(std::collections::BTreeMap::from([(
                over.clone(),
                Some(ActionModelOverridePatch {
                    head_scale: Some(Some(1.0)),
                    ..Default::default()
                }),
            )])),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&base, &patch).expect("合法 id 应通过");
    assert_eq!(outcome, PatchOutcome::Updated);
    assert_eq!(next.action.models[&over].head_scale, Some(1.0));
}

/// 删除语义：`null` 值删条目；对象里逐键 `null` 删到全 `None` 也删条目（不留空表）。
#[test]
fn action_models_delete_and_empty_entry_pruned() {
    use crate::settings::patch::{ActionModelOverridePatch, ActionPatch};
    let base = AppSettings::from_toml_str(
        "[action.models.bai]\nhead_scale = 1.5\n\n[action.models.follow]\nbody_scale = 1.1\n",
    )
    .expect("parse");
    assert_eq!(base.action.models.len(), 2);
    let patch = SettingsPatch {
        action: Some(Some(ActionPatch {
            models: Some(std::collections::BTreeMap::from([
                ("bai".to_string(), None),
                (
                    "follow".to_string(),
                    Some(ActionModelOverridePatch {
                        body_scale: Some(None),
                        ..Default::default()
                    }),
                ),
            ])),
            ..Default::default()
        })),
        ..Default::default()
    };
    let (next, outcome) = apply_patch(&base, &patch).unwrap();
    assert_eq!(outcome, PatchOutcome::Updated);
    assert!(
        next.action.models.is_empty(),
        "两条都该消失：{:?}",
        next.action.models
    );
    // 再打同一补丁 → NoChange（条目已不存在，删除是 no-op）。
    let (_, outcome2) = apply_patch(&next, &patch).unwrap();
    assert_eq!(outcome2, PatchOutcome::NoChange);
}

/// 序列化：`[action.models.<id>]` 真写成 TOML 子表并可 round-trip；空表不写出。
#[test]
fn action_models_round_trip_through_toml() {
    let s = AppSettings::from_toml_str(
        "[action]\nhead_scale = 1.1\n\n[action.models.bai]\nhead_scale = 1.5\n\n[action.models.follow]\nbody_scale = 1.1\n",
    )
    .expect("parse");
    let toml = s.to_toml_string();
    assert!(toml.contains("[action.models.bai]"), "{toml}");
    assert!(toml.contains("[action.models.follow]"), "{toml}");
    let back = AppSettings::from_toml_str(&toml).expect("re-parse");
    assert_eq!(back.action, s.action, "round-trip 不丢覆盖");
    // 空 models 整体省略（skip_serializing_if）。
    let empty = AppSettings::from_toml_str("").unwrap().to_toml_string();
    assert!(!empty.contains("models"), "{empty}");
    // deny_unknown_fields 对覆盖表内部一样生效。
    assert!(AppSettings::from_toml_str("[action.models.bai]\nhead = 1.0\n").is_err());
}

/// 视图：`models` 的值归一化（9.0 → 2.2）；未覆盖键为 `null`（三键始终出现）。
#[test]
fn action_view_models_normalized_and_null_keys() {
    let s = AppSettings::from_toml_str(
        "[action.models.bai]\nhead_scale = 9.0\n\n[action.models.follow]\nbody_scale = 1.1\n",
    )
    .expect("parse");
    let v = settings_to_view(&s);
    let bai = v.action.models.get("bai").expect("bai");
    assert_eq!(bai.head_scale, Some(2.2), "视图值必须归一化");
    assert_eq!(bai.body_scale, None);
    assert_eq!(bai.expression_scale, None);
    assert_eq!(v.action.models["follow"].body_scale, Some(1.1));
    let json = serde_json::to_string(&v.action).unwrap();
    assert!(
        json.contains("\"bai\":{\"head_scale\":2.2,\"body_scale\":null,\"expression_scale\":null}"),
        "未覆盖键必须序列化为 null 且三键都在：{json}"
    );
}

/// `SettingsView.active_model_id`：新函数透传；两个旧口径冻结为空串。
#[test]
fn settings_view_active_model_id_transport_and_freeze() {
    let s = sample();
    let no_value = |_: &str| None;
    assert_eq!(settings_to_view(&s).active_model_id, "", "旧口径冻结为空串");
    assert_eq!(
        settings_to_view_with_keys(&s, &no_value).active_model_id,
        "",
        "旧口径冻结为空串"
    );
    let v = settings_to_view_with_keys_and_model(&s, &no_value, "bai");
    assert_eq!(v.active_model_id, "bai");
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.contains("\"active_model_id\":\"bai\""), "{json}");
}
