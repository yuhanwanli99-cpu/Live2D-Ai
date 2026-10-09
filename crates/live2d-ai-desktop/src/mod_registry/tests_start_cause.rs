//! `StartCause` 接线回归（2026-10-09「两类 TTS」）。
//!
//! # 为什么值得单列一个文件
//!
//! `ModRuntime::start` 读的是**线程局部**的启动原因，而它只有在宿主「同一条
//! 同步调用栈上先设置、再 `start_one`」时才正确。这条契约没有类型系统兜底：
//! 漏设一次，`on_apply` 的 Mod 就会在开机时被悄悄拉起来（或反过来，在用户点
//! 「保存并应用」时不动）。所以这里用一个只记录 `start_cause()` 的桩工厂，
//! 把 Boot / Enable / Apply 三条路径各钉一条。
//!
//! 挂法见 `crates/live2d-ai-desktop/src/mod_registry.rs` 末尾：
//! `#[cfg(test)] #[path = "mod_registry/tests_start_cause.rs"] mod tests_start_cause;`。

use std::cell::RefCell;

use super::*;

// 记录栈用 **线程局部**：`cargo test` 的每个用例跑在自己的线程上，
// 共享一个 `static Mutex<Vec<_>>` 会让并行用例互相看见对方的记录（假红/假绿）。
thread_local! {
    /// 桩 runtime 在 `start` 里看到的原因（按调用顺序）。
    static SEEN: RefCell<Vec<StartCause>> = const { RefCell::new(Vec::new()) };
}

fn seen() -> Vec<StartCause> {
    SEEN.with(|s| s.borrow().clone())
}

fn clear_seen() {
    SEEN.with(|s| s.borrow_mut().clear());
}

/// 只记录启动原因的桩工厂。
struct CauseMod;

impl ModFactory for CauseMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "cause",
            name: "Cause",
            version: "0.1.0",
            api_version: live2d_ai_mod_system::MOD_API_VERSION,
        };
        &D
    }

    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(CauseRuntime))
    }
}

struct CauseRuntime;

impl ModRuntime for CauseRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        SEEN.with(|s| s.borrow_mut().push(start_cause()));
        Ok(())
    }
}

/// `shutdown` 一定失败的桩工厂（停不掉的子进程）。
struct StubbornMod;

impl ModFactory for StubbornMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "stubborn",
            name: "Stubborn",
            version: "0.1.0",
            api_version: live2d_ai_mod_system::MOD_API_VERSION,
        };
        &D
    }

    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(StubbornRuntime))
    }
}

struct StubbornRuntime;

impl ModRuntime for StubbornRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        SEEN.with(|s| s.borrow_mut().push(start_cause()));
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        Err(ModError::Other("停不掉（桩）".to_string()))
    }

    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(serde_json::json!({ "stubborn": true }))
    }
}

static CAUSE_FACTORIES: &[&dyn ModFactory] = &[&CauseMod];
static STUBBORN_FACTORIES: &[&dyn ModFactory] = &[&StubbornMod];

#[test]
fn start_all_sets_boot_cause() {
    let mut reg = ModRegistry::new(
        CAUSE_FACTORIES,
        &serde_json::json!({ "mods": { "cause": { "enabled": true } } }),
    );
    clear_seen();
    reg.start_all();
    assert_eq!(seen(), vec![StartCause::Boot]);
}

#[test]
fn enable_sets_enable_cause() {
    let mut reg = ModRegistry::new(CAUSE_FACTORIES, &serde_json::json!({}));
    clear_seen();
    reg.enable("cause").expect("启用应成功");
    assert_eq!(seen(), vec![StartCause::Enable]);
}

/// `enable_with_config` **继续走 `enable`** ⇒ 是 Enable，不是 Apply。
#[test]
fn enable_with_config_sets_enable_cause() {
    let mut reg = ModRegistry::new(CAUSE_FACTORIES, &serde_json::json!({}));
    clear_seen();
    reg.enable_with_config("cause", serde_json::json!({ "program": "/bin/echo" }))
        .expect("带配置启用应成功");
    assert_eq!(seen(), vec![StartCause::Enable]);
}

/// 保存并应用：先停，停成功后再以 `Apply` 启动——而且**只启动一次**。
#[test]
fn restart_sets_apply_cause() {
    let mut reg = ModRegistry::new(
        CAUSE_FACTORIES,
        &serde_json::json!({ "mods": { "cause": { "enabled": true } } }),
    );
    reg.start_all();
    clear_seen();
    reg.restart("cause").expect("重启应成功");
    assert_eq!(
        seen(),
        vec![StartCause::Apply],
        "公开 enable 不得把这次 Apply 覆盖成 Enable（否则 on_apply 的 Mod 不会拉起）"
    );
}

/// `shutdown` 失败 ⇒ `disable` 返回 `Err` ⇒ `restart` **不再** `start`。
#[test]
fn restart_does_not_start_when_shutdown_fails() {
    let mut reg = ModRegistry::new(
        STUBBORN_FACTORIES,
        &serde_json::json!({ "mods": { "stubborn": { "enabled": true } } }),
    );
    reg.start_all();
    clear_seen();
    let err = reg
        .restart("stubborn")
        .expect_err("shutdown 失败时 restart 必须返回 Err");
    assert!(err.to_string().contains("停不掉"), "got {err}");
    assert!(
        seen().is_empty(),
        "停不掉就绝不能再 start 一个：{:?}",
        seen()
    );
}

/// `disable` 失败路径**什么都不改**：`enabled` 仍为 true、runtime 放回槽位。
#[test]
fn disable_keeps_enabled_and_runtime_when_shutdown_fails() {
    let mut reg = ModRegistry::new(
        STUBBORN_FACTORIES,
        &serde_json::json!({ "mods": { "stubborn": { "enabled": true } } }),
    );
    reg.start_all();
    let err = reg.disable("stubborn").expect_err("停不掉必须返回 Err");
    assert!(err.to_string().contains("停不掉"), "got {err}");

    let (_d, status, enabled) = reg.list()[0].clone();
    assert!(
        enabled,
        "失败路径不得把 enabled 写成 false（下次启用会再拉一个）"
    );
    assert_eq!(status, ModStatus::Running, "状态也不该动");
    assert!(
        reg.runtime_state("stubborn").is_some(),
        "runtime 必须放回槽位（否则子进程成了没人 wait 的孤儿）"
    );
}
