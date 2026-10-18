//! `settings::patch` 的 max_tokens / `[action]` 回归（自 `patch_tests.rs` 机械拆出）。

use crate::settings::AppSettings;
use crate::settings::patch::{LlmPatch, PatchOutcome, SettingsPatch, apply_patch};
use crate::settings::view::{
    settings_to_view, settings_to_view_with_keys, settings_to_view_with_keys_and_model,
};

use super::patch_tests::sample;

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
            mode: None,                      // 缺省 → skip，不出现
            base_url: Some(None),            // 显式清空 → 序列化为 null
            model: Some(Some("gpt".into())), // 显式设值
            api_key_env: None,               // 缺省 → skip，不出现
            max_tokens: None,                // 同上
            show_reasoning: None,            // 同上
            vendor: None,                    // 同上
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
            mode: None,
            base_url: None,
            model: None,
            api_key_env: None,
            max_tokens: None,
            show_reasoning: None,
            vendor: None,
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
                mode: None,
                base_url: None,
                model: None,
                api_key_env: None,
                max_tokens: Some(Some(value)),
                show_reasoning: None,
                vendor: None,
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
            mode: None,
            base_url: None,
            model: None,
            api_key_env: None,
            max_tokens: Some(None),
            show_reasoning: None,
            vendor: None,
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
