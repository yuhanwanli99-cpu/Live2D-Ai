//! `mod_registry` 生命周期场景回归（自内联 `mod tests` 拆出）。
//!
//! 场景：缺省停用 / api_version 不兼容 / 启停 / 有界事件 / 运行态 / manifest
//! 往返 / 静态 settings schema。脚手架见 `tests_support`。

use super::events::parse_mod_config;
use super::tests_support::*;
use super::*;

#[test]
fn default_disabled_until_enabled() {
    let mut reg = ModRegistry::new(FACTORIES, &serde_json::json!({}));
    reg.start_all();
    let (_, status, enabled) = reg.list()[0].clone();
    assert_eq!(status, ModStatus::Disabled);
    assert!(!enabled);
}

/// M3：`api_version` 不兼容 → Failed，且 factory.create 不被调用（主链不崩）。
#[test]
fn incompatible_api_version_fails_without_crashing() {
    struct BadApiMod;
    impl ModFactory for BadApiMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            static D: ModDescriptor = ModDescriptor {
                id: "bad_api",
                name: "BadApi",
                version: "0.1.0",
                api_version: 999,
            };
            &D
        }
        fn create(
            &self,
            _: ModServices,
            _: serde_json::Value,
        ) -> Result<Box<dyn ModRuntime>, ModError> {
            panic!("api_version 不兼容时不得 create");
        }
    }
    static BAD: &[&dyn ModFactory] = &[&BadApiMod];
    let mut reg = ModRegistry::new(
        BAD,
        &serde_json::json!({"mods":{"bad_api":{"enabled":true}}}),
    );
    reg.start_all();
    let (_, status, enabled) = reg.list()[0].clone();
    assert!(
        matches!(status, ModStatus::Failed { .. }),
        "应 Failed，got {status:?}"
    );
    assert!(enabled, "manifest 里的 enabled 意图保留");
}

#[test]
fn enable_runs_mod() {
    let mut reg = ModRegistry::new(
        FACTORIES,
        &serde_json::json!({"mods":{"test":{"enabled":true}}}),
    );
    reg.start_all();
    let (_d, status, enabled) = reg.list()[0].clone();
    assert_eq!(status, ModStatus::Running);
    assert!(enabled);
}

#[test]
fn disable_shuts_down() {
    let mut reg = ModRegistry::new(
        FACTORIES,
        &serde_json::json!({"mods":{"test":{"enabled":true}}}),
    );
    reg.start_all();
    reg.disable("test").unwrap();
    let (_, status, enabled) = reg.list()[0].clone();
    assert_eq!(status, ModStatus::Disabled);
    assert!(!enabled);
}

#[test]
fn dispatch_event_bounded_no_block() {
    let reg = ModRegistry::new(FACTORIES, &serde_json::json!({}));
    assert!(reg.dispatch_event(ModEventTopic::TurnStarted, "{}", None));
    // Wave 2 新主题同样可投递（有界 channel，不阻塞）。
    assert!(reg.dispatch_event(ModEventTopic::TurnPrompt, "你好", None));
}

// ------------------------------------------------- Wave 2：运行态快照面

/// 未启用 → runtime 槽位为空 → `runtime_state` 回 `None`；
/// 启用后由 runtime 提供快照。
#[test]
fn runtime_state_is_none_unless_running() {
    let mut reg = ModRegistry::new(FACTORIES, &serde_json::json!({}));
    assert!(reg.contains("test"));
    assert!(!reg.is_enabled("test"));
    assert!(
        reg.runtime_state("test").is_none(),
        "未启用时没有 runtime，快照必须是 None"
    );
    assert!(
        reg.runtime_state("nope").is_none(),
        "不存在的 Mod 也是 None（路由用 contains 区分 404）"
    );
    reg.enable("test").unwrap();
    let snap = reg.runtime_state("test").expect("启用后应有快照");
    assert_eq!(snap["test_state"], serde_json::json!("ready"));
}

#[test]
fn parse_config_from_manifest() {
    let manifest = serde_json::json!({"mods": {"x": {"enabled": true, "config": {"port": 1}}}});
    let (e, c) = parse_mod_config(&manifest, "x");
    assert!(e);
    assert_eq!(c["port"], 1);
}

// ------------------------------------------------------- M1 持久化（rc.4）

/// M1：enable/disable 原子写回 `mods.json`，重启（重新构造 registry）状态不丢。
#[test]
fn enable_disable_persist_manifest_and_round_trip() {
    let dir = std::env::temp_dir().join(format!("l2d-mod-test-enable-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mods.json");

    {
        let mut reg =
            ModRegistry::new(FACTORIES, &serde_json::json!({})).with_manifest_path(path.clone());
        reg.enable("test").unwrap();
        assert!(reg.is_enabled("test"));
    }
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(v["mods"]["test"]["enabled"], true, "enable 应写回磁盘: {v}");

    // 重启语义：用磁盘内容重建 registry，开关仍在。
    let reg2 = ModRegistry::new(FACTORIES, &v);
    assert!(reg2.is_enabled("test"), "重启后 enable 状态应保持");

    // disable 同样写回。
    let mut reg3 = ModRegistry::new(FACTORIES, &v).with_manifest_path(path.clone());
    reg3.disable("test").unwrap();
    let v3: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        v3["mods"]["test"]["enabled"], false,
        "disable 应写回磁盘: {v3}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// M1：reload_config（POST …/config 内核）也写回 config，且不丢其它 Mod。
#[test]
fn reload_config_persists_manifest() {
    let dir = std::env::temp_dir().join(format!("l2d-mod-test-config-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mods.json");

    let mut reg =
        ModRegistry::new(FACTORIES, &serde_json::json!({})).with_manifest_path(path.clone());
    reg.reload_config("test", serde_json::json!({"port": 1_234}))
        .unwrap();
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        v["mods"]["test"]["config"]["port"], 1_234,
        "config 应写回磁盘: {v}"
    );
    assert_eq!(
        v["mods"]["test"]["enabled"], false,
        "未启用者也要保留在 manifest 里"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// M2：**静态** settings schema——未启用的 Mod 也能拿到（前端先填再启用）。
#[test]
fn static_settings_spec_available_when_disabled() {
    struct StaticSpecMod;
    impl ModFactory for StaticSpecMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            static D: ModDescriptor = ModDescriptor {
                id: "static_spec",
                name: "StaticSpec",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn settings_spec(&self) -> Option<ModSettingsSpec> {
            Some(ModSettingsSpec {
                mod_id: "static_spec".to_string(),
                title: "静态".to_string(),
                version: 1,
                fields: vec![ModSettingField::Bool {
                    key: "on".to_string(),
                    label: "开".to_string(),
                    default: true,
                }],
            })
        }
        fn create(
            &self,
            _: ModServices,
            _: serde_json::Value,
        ) -> Result<Box<dyn ModRuntime>, ModError> {
            Ok(Box::new(TestRuntime))
        }
    }
    static F: &[&dyn ModFactory] = &[&StaticSpecMod];
    let reg = ModRegistry::new(F, &serde_json::json!({}));
    assert!(!reg.is_enabled("static_spec"), "缺省不启用");
    assert!(
        reg.settings_spec("static_spec").is_some(),
        "未启用也要有 schema（factory 静态提供）"
    );
}

/// F-0013-01 / F-0644-01：手写的「当前不存在的 Mod id」与其它顶层键不得被写回抹掉。
///
/// 文档明确邀请用户手写 mods.json（docs/external-input.md 的启用 / 停用两处），
/// 而旧 persist_manifest 整份从「在册 factory」重建 ⇒ 第一次配置保存就**静默**
/// 抹掉那些键。这里先在磁盘上放一份带未知 id 的文件，再触发一次写回，逐键核对。
#[test]
fn persist_manifest_preserves_unknown_ids_and_top_level_keys() {
    let dir = std::env::temp_dir().join(format!("l2d-mod-test-unknown-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mods.json");
    std::fs::write(
        &path,
        r#"{"schema":"hand-written","mods":{"test":{"enabled":false},"future-mod":{"enabled":true,"config":{"x":1}}}}"#,
    )
    .unwrap();

    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let mut reg = ModRegistry::new(FACTORIES, &on_disk).with_manifest_path(path.clone());
    reg.enable("test").unwrap(); // 触发一次原子写回

    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        v["mods"]["test"]["enabled"], true,
        "在册 id 必须被更新: {v}"
    );
    assert_eq!(
        v["mods"]["future-mod"]["enabled"], true,
        "当前不存在的 Mod id 不得被抹掉: {v}"
    );
    assert_eq!(
        v["mods"]["future-mod"]["config"]["x"], 1,
        "未知 id 的 config 也要原样保留: {v}"
    );
    assert_eq!(v["schema"], "hand-written", "其它顶层键不得被抹掉: {v}");

    let _ = std::fs::remove_dir_all(&dir);
}
