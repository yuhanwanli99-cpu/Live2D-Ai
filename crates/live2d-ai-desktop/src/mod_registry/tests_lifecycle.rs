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

// ------------------------------------------------- S-003 / S-006（2026-10-16）

/// S-003：`mods.json` 写盘失败**不得**被吞成成功。
///
/// 从前 `persist_manifest` 是 best-effort（只记 error 日志、返回 `()`），于是
/// 磁盘满 / 只读目录时 HTTP 200 + 界面「已启用」，而磁盘上什么都没变。
#[test]
fn enable_reports_a_failed_manifest_write() {
    let dir = std::env::temp_dir().join(format!("l2d-mod-s003-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // 让**父路径是个普通文件**：create_dir_all 必失败 ⇒ 写盘必失败。
    let blocker = dir.join("not-a-dir");
    std::fs::write(&blocker, b"x").unwrap();
    let path = blocker.join("mods.json");

    let mut reg = ModRegistry::new(FACTORIES, &serde_json::json!({})).with_manifest_path(path);
    let err = reg
        .enable("test")
        .expect_err("写盘失败必须上抛，不许回 Ok（那正是界面写「已启用」的来源）");
    assert!(err.to_string().contains("mods.json"), "got {err}");
    assert!(
        err.to_string().contains("重启后会回到磁盘上的旧值"),
        "文案要说明后果：{err}"
    );
    // 内存里确实启用了（这一进程是真的），但状态机必须报 Failed，界面才不会说成功。
    let (_, status, enabled) = reg.list()[0].clone();
    assert!(enabled, "内存里确实启用了");
    assert!(
        matches!(status, ModStatus::Failed { .. }),
        "写盘失败必须把状态标 Failed，got {status:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// S-006：丢掉的事件要**计数**，并且能在**现有** state_json 面读到。
///
/// 从前 `dropped_events` 从不自增、getter 是 `#[allow(dead_code)]`——「这一轮少了
/// 注入」在界面与 HTTP 上都看不出来。
#[test]
fn dropped_events_are_counted_and_visible_in_state_json() {
    let (tx, gate) = std::sync::mpsc::channel();
    *STALL_GATE.lock().unwrap() = Some(gate);
    let mut reg = ModRegistry::new(
        STALL_FACTORIES,
        &serde_json::json!({"mods":{"stall":{"enabled":true}}}),
    );
    reg.start_all();
    assert_eq!(reg.dropped_events(), 0, "还没丢过");

    // 第一条事件把 worker 卡在闸门上（它只卡第一条）；随后的事件把容量 256 的
    // 队列填满，再往后每条都 `try_send` 失败 = 真丢弃。
    let mut dropped = 0u64;
    for _ in 0..400 {
        if !reg.dispatch_event(ModEventTopic::TurnStarted, "{}", None) {
            dropped += 1;
        }
    }
    assert!(dropped > 0, "填满之后必须开始丢弃");
    assert_eq!(reg.dropped_events(), dropped, "计数必须与实际丢弃条数一致");

    // 放行 worker，让它把积压的队列排完，再读**现有** state_json。
    tx.send(()).unwrap();
    let mut state = None;
    for _ in 0..200 {
        state = reg.runtime_state("stall");
        if state.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let state = state.expect("worker 排空后 state_json 必须可读");
    assert_eq!(
        state["dropped_events"],
        serde_json::json!(dropped),
        "现有 state_json 面必须带出丢弃计数：{state}"
    );
    assert_eq!(state["test_state"], serde_json::json!("ready"));
}

/// worker 卡住用的闸门（[`StallingMod`] 在 `create` 时取走接收端）。
static STALL_GATE: Mutex<Option<std::sync::mpsc::Receiver<()>>> = Mutex::new(None);

struct StallingMod;
impl ModFactory for StallingMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "stall",
            name: "Stall",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        let gate = STALL_GATE
            .lock()
            .unwrap()
            .take()
            .expect("测试要先往 STALL_GATE 里放接收端");
        Ok(Box::new(StallingRuntime {
            gate: Some(gate),
            state: serde_json::json!({"test_state": "ready"}),
        }))
    }
}

struct StallingRuntime {
    /// **只卡第一条**：卡住之后测试才有机会把队列填满。
    gate: Option<std::sync::mpsc::Receiver<()>>,
    state: serde_json::Value,
}

impl ModRuntime for StallingRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        Ok(())
    }
    fn on_event(&mut self, _: ModEventTopic, _: &str) -> Result<(), ModError> {
        if let Some(gate) = self.gate.take() {
            let _ = gate.recv();
        }
        Ok(())
    }
    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(self.state.clone())
    }
}

static STALL_FACTORIES: &[&dyn ModFactory] = &[&StallingMod];
