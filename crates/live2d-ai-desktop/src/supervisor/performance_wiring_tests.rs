//! `performance_wiring` 的内联单测（经 `#[path = "performance_wiring_tests.rs"]` 挂回，保持父文件 ≤500 行）。

use super::*;

/// 临时目录（每个用例独立；`live2d-ai.toml` 与同目录 `mods.json` 成对）。
fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "l2d-wiring-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
}

/// 缺省不装配（2026-10-08 产品口径）：`[performance].enabled=false` → `None`。
#[test]
fn disabled_by_default() {
    let perf = PerformanceSettings::default();
    assert!(!perf.enabled, "前提：[performance].enabled 缺省关");
    assert!(
        build_performance_runtime(&perf).is_none(),
        "缺省关 = 主链走常驻二路按句清洗"
    );
}

/// 显式打开后**只读 `[performance]` 自己**的端点：缺一项 → None；齐了 → 用自己那份。
#[test]
fn explicit_opt_in_reads_only_the_performance_section() {
    let off = PerformanceSettings {
        enabled: true,
        ..PerformanceSettings::default()
    };
    assert!(
        build_performance_runtime(&off).is_none(),
        "enabled=true 但 base_url/model 缺 → 不装配"
    );
    for (base, model) in [("http://127.0.0.1:59999/v1", ""), ("", "perf-own")] {
        let partial = PerformanceSettings {
            enabled: true,
            base_url: base.to_string(),
            model: model.to_string(),
            ..PerformanceSettings::default()
        };
        assert!(
            build_performance_runtime(&partial).is_none(),
            "只填一项（base={base:?}, model={model:?}）必须保持直送"
        );
    }
    let own = PerformanceSettings {
        enabled: true,
        base_url: "http://127.0.0.1:59999/v1".to_string(),
        model: "perf-own".to_string(),
        ..PerformanceSettings::default()
    };
    let rt = build_performance_runtime(&own).expect("显式打开 + 自己有端点 → 装配");
    assert!(rt.enabled());
    assert_eq!(rt.model(), "perf-own", "端点来自 [performance] 自己");
}

/// 能力集 / 规则回退来自 runtime `performance::rules`（单一真源，装配面不另抄）。
#[test]
fn allow_list_comes_from_runtime_rules() {
    let perf = PerformanceSettings {
        enabled: true,
        base_url: "http://127.0.0.1:59999/v1".to_string(),
        model: "m".to_string(),
        ..PerformanceSettings::default()
    };
    let rt = build_performance_runtime(&perf).expect("装配");
    assert_eq!(
        rt.allow(),
        rules::accepted_preset_ids(),
        "能力集必须来自 runtime 规则表"
    );
    for id in ["none", "smile", "unhappy", "surprised", "nod", "shake"] {
        assert!(
            rt.allow().iter().any(|x| x == id),
            "主 allowlist 的 {id} 必须在能力集里"
        );
    }
    // 规则回退同源：整段规则表是公开函数（引擎失败时用同一张表）。
    assert!(
        rules::accepted_preset_ids()
            .iter()
            .all(|id| live2d_ai_runtime::performance::rules::is_known_preset(id)),
        "allowlist 里不得出现规则表不认识的 id"
    );
}

/// `motion_bias` 只读 `mods.director.config.motion_bias`：缺文件 / 坏 JSON / 缺段 = 平常；
/// **不读** `enabled` / 导演自己的 staging 端点（导演 Mod 已退役，两者不再是开关）。
#[test]
fn motion_bias_reading_is_defensive_and_ignores_enabled_and_staging() {
    let dir = temp_dir("bias");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(&toml, "[llm]\n").expect("write toml");
    let toml_path = toml.display().to_string();

    assert_eq!(
        load_motion_bias(&toml_path),
        MotionBias::Normal,
        "没有 mods.json = 平常"
    );
    std::fs::write(dir.join("mods.json"), "not json").expect("write bad json");
    assert_eq!(
        load_motion_bias(&toml_path),
        MotionBias::Normal,
        "坏 JSON = 平常"
    );

    let write = |value: serde_json::Value| {
        std::fs::write(dir.join("mods.json"), value.to_string()).expect("write manifest");
    };
    write(serde_json::json!({"mods": {"external-input": {"enabled": true}}}));
    assert_eq!(
        load_motion_bias(&toml_path),
        MotionBias::Normal,
        "没有 director 段 = 平常"
    );

    // 关键：`enabled=false` 也照样读 motion_bias（导演启用状态不是开关）；
    // staging 键存在也**不影响**读数（已删除，host 不读它们）。
    write(serde_json::json!({
        "mods": {
            "director": {
                "enabled": false,
                "config": {
                    "motion_bias": "eager"
                }
            }
        }
    }));
    assert_eq!(
        load_motion_bias(&toml_path),
        MotionBias::Eager,
        "motion_bias 与导演 enabled 状态无关"
    );
    write(serde_json::json!({"mods": {"director": {"config": {"motion_bias": "restrained"}}}}));
    assert_eq!(load_motion_bias(&toml_path), MotionBias::Restrained);
    write(serde_json::json!({"mods": {"director": {"config": {"motion_bias": "垃圾"}}}}));
    assert_eq!(
        load_motion_bias(&toml_path),
        MotionBias::Normal,
        "未知档位 = 平常"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 只有 `live2d-ai.toml` **同目录**的 `mods.json` 才会被读到。
#[test]
fn load_reads_the_sibling_manifest_only() {
    let dir = temp_dir("sibling");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(&toml, "[llm]\n").expect("write toml");
    let toml_path = toml.display().to_string();
    assert_eq!(
        mods_manifest_path(&toml_path),
        dir.join("mods.json"),
        "manifest 必须与配置同目录"
    );
    std::fs::write(
        dir.join("mods.json"),
        r#"{"mods":{"director":{"config":{"motion_bias":"eager"}}}}"#,
    )
    .expect("write manifest");
    assert_eq!(load_motion_bias(&toml_path), MotionBias::Eager);
    let _ = std::fs::remove_dir_all(&dir);
}
