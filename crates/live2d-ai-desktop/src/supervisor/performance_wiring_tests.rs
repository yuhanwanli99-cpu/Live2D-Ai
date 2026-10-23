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

/// `motion_rate` 只读 `mods.director.config.motion_rate`：缺文件 / 坏 JSON /
/// 缺键 / 未知值 = 平常（normal，不增不减）；**不读** `enabled` / `motion_bias`。
#[test]
fn motion_rate_reading_is_defensive_and_ignores_enabled_and_bias() {
    let dir = temp_dir("rate");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(&toml, "[llm]\n").expect("write toml");
    let toml_path = toml.display().to_string();
    let write = |value: serde_json::Value| {
        std::fs::write(dir.join("mods.json"), value.to_string()).expect("write manifest");
    };

    assert_eq!(
        load_motion_rate(&toml_path),
        MotionRate::Normal,
        "没有 mods.json = normal"
    );
    std::fs::write(dir.join("mods.json"), "not json").expect("write bad json");
    assert_eq!(
        load_motion_rate(&toml_path),
        MotionRate::Normal,
        "坏 JSON = normal"
    );
    write(serde_json::json!({"mods": {"external-input": {"enabled": true}}}));
    assert_eq!(
        load_motion_rate(&toml_path),
        MotionRate::Normal,
        "没有 director 段 = normal"
    );
    // 关键：enabled 与 motion_bias 都不影响频率读数（三键各自独立）。
    write(serde_json::json!({
        "mods": {
            "director": {
                "enabled": true,
                "config": {"motion_bias": "eager"}
            }
        }
    }));
    assert_eq!(
        load_motion_rate(&toml_path),
        MotionRate::Normal,
        "缺 motion_rate = normal（不读 motion_bias）"
    );
    write(serde_json::json!({"mods": {"director": {"config": {"motion_rate": "sparse"}}}}));
    assert_eq!(load_motion_rate(&toml_path), MotionRate::Sparse);
    write(serde_json::json!({"mods": {"director": {"config": {"motion_rate": "dense"}}}}));
    assert_eq!(load_motion_rate(&toml_path), MotionRate::Dense);
    write(serde_json::json!({"mods": {"director": {"config": {"motion_rate": "turbo"}}}}));
    assert_eq!(
        load_motion_rate(&toml_path),
        MotionRate::Normal,
        "未知值 = normal"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// 合并两键：其它顶层键 / 其它 mods 条目 / director 的其它 config 键与 enabled
/// 全部原样保留；**其余提交键忽略且不写**；结果能被两个 load 函数读回。
#[test]
fn merge_writes_only_the_two_keys_and_preserves_everything_else() {
    let dir = temp_dir("merge");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(&toml, "[llm]\n").expect("write toml");
    let toml_path = toml.display().to_string();
    let manifest = dir.join("mods.json");
    std::fs::write(
        &manifest,
        serde_json::json!({
            "schema": 7,
            "notes": ["keep", "me"],
            "mods": {
                "director": {
                    "enabled": true,
                    "config": {"motion_bias": "restrained", "keep_me": {"nested": 1}}
                },
                "external-input": {"enabled": true, "config": {"token": "t"}}
            }
        })
        .to_string(),
    )
    .expect("write manifest");

    let submitted = serde_json::json!({
        "motion_bias": "eager",
        "motion_rate": "sparse",
        "unknown_key": "ignored"
    });
    merge_director_motion_config(&toml_path, &submitted).expect("合并必须成功");

    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest).expect("read merged manifest"))
            .expect("merged manifest 必须是合法 JSON");
    assert_eq!(after["schema"], 7, "其它顶层键原样保留");
    assert_eq!(after["notes"], serde_json::json!(["keep", "me"]));
    assert_eq!(
        after["mods"]["director"]["enabled"], true,
        "enabled 原样保留"
    );
    assert_eq!(
        after["mods"]["director"]["config"]["keep_me"],
        serde_json::json!({"nested": 1}),
        "director 其它 config 键原样保留"
    );
    assert_eq!(after["mods"]["director"]["config"]["motion_bias"], "eager");
    assert_eq!(after["mods"]["director"]["config"]["motion_rate"], "sparse");
    assert!(
        after["mods"]["director"]["config"]
            .get("unknown_key")
            .is_none(),
        "其余提交键忽略且不写"
    );
    assert_eq!(after["mods"]["external-input"]["enabled"], true);
    assert_eq!(
        after["mods"]["external-input"]["config"]["token"], "t",
        "其它 mods 条目原样保留"
    );
    // 写回内容必须能被运行时读取路径读回（同一份磁盘口径）。
    assert_eq!(load_motion_bias(&toml_path), MotionBias::Eager);
    assert_eq!(load_motion_rate(&toml_path), MotionRate::Sparse);

    let _ = std::fs::remove_dir_all(&dir);
}

/// 白名单 / 字符串类型校验：未知值、非字符串、大小写不符都是 400 类错误，
/// 且**先全量校验再动磁盘**——文件一个字节都不变。
#[test]
fn merge_rejects_unknown_or_non_string_values_without_writing() {
    let dir = temp_dir("merge-reject");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(&toml, "[llm]\n").expect("write toml");
    let toml_path = toml.display().to_string();
    let manifest = dir.join("mods.json");
    std::fs::write(
        &manifest,
        r#"{"mods":{"director":{"enabled":false,"config":{"motion_bias":"restrained"}}}}"#,
    )
    .expect("write manifest");
    let before = std::fs::read_to_string(&manifest).expect("read before");

    for submitted in [
        serde_json::json!({"motion_bias": "turbo"}),
        serde_json::json!({"motion_rate": "EAGER"}),
        serde_json::json!({"motion_bias": 3}),
        // 前一键合法、后一键非法：也必须整份拒绝（不许写一半）。
        serde_json::json!({"motion_bias": "eager", "motion_rate": "turbo"}),
    ] {
        let err =
            merge_director_motion_config(&toml_path, &submitted).expect_err("非法值必须返回错误");
        assert!(
            matches!(err, MergeError::InvalidValue(_)),
            "必须是可判 400 的错误：{err:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&manifest).expect("read after"),
            before,
            "非法值不得写盘（{submitted}）"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

/// 坏 JSON = 409 类错误且**不覆盖**；只有无关键 = no-op 不写盘；
/// director 条目不存在时创建 `{"config":{...}}`（**不写 enabled**）。
#[test]
fn merge_storage_errors_and_entry_creation() {
    let dir = temp_dir("merge-storage");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(&toml, "[llm]\n").expect("write toml");
    let toml_path = toml.display().to_string();
    let manifest = dir.join("mods.json");

    // 坏 JSON → Storage，磁盘内容原样保留（绝不覆盖读不懂的配置）。
    std::fs::write(&manifest, "not json").expect("write bad json");
    let err =
        merge_director_motion_config(&toml_path, &serde_json::json!({"motion_bias": "normal"}))
            .expect_err("坏 JSON 必须报错");
    assert!(matches!(err, MergeError::Storage(_)), "{err:?}");
    assert_eq!(
        std::fs::read_to_string(&manifest).expect("read"),
        "not json",
        "坏 JSON 不得被覆盖"
    );

    // 只有无关键 = no-op：文件不存在时也不创建。
    std::fs::remove_file(&manifest).expect("remove manifest");
    merge_director_motion_config(&toml_path, &serde_json::json!({"tempo": "fast"}))
        .expect("无关键必须被忽略");
    assert!(!manifest.exists(), "只有无关键 = 不写盘");

    // 条目不存在 → 创建 {"config":{...}}，不写 enabled。
    merge_director_motion_config(&toml_path, &serde_json::json!({"motion_rate": "dense"}))
        .expect("创建 director 条目");
    let after: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest).expect("read created"))
            .expect("created manifest 必须是合法 JSON");
    assert_eq!(after["mods"]["director"]["config"]["motion_rate"], "dense");
    assert!(
        after["mods"]["director"].get("enabled").is_none(),
        "创建条目时不得写 enabled"
    );
    assert_eq!(load_motion_rate(&toml_path), MotionRate::Dense);

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
