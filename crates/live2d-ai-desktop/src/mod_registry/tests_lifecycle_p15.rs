//! P1.5（0.7.0 收口）：Mod 生命周期回归——幂等 enable / 失败一致性 / 停用 teardown。
//!
//! 单列文件：`tests_lifecycle.rs` 已 477 行（`crates/*/src` 单文件 500 行硬门禁），
//! 挂法照 `tests_start_cause` 的 `#[path]` 先例（见 `mod_registry.rs` 末尾）。
//!
//! 覆盖（对应 P1.5 验收项 1/2/3）：
//! - 连续 enable 两次不泄漏（create / shutdown / Drop 计数）；
//! - 启动失败 ⇒ 槽位空 + `Failed` + `last_error`；旧实例先 shutdown 再丢弃；
//! - disable ⇒ 调 `shutdown`、runtime 被 drop、事件不再投递。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::*;

fn load(c: &AtomicU64) -> u64 {
    c.load(Ordering::Relaxed)
}

/// 可观测实例计数的桩工厂：create / shutdown / Drop / on_event 各一个计数器。
///
/// 本文件里每个用例用自己的 static 计数器（宏各自展开一份），`cargo test`
/// 并行跑时互不污染。
macro_rules! counting_mod {
    ($mod_ty:ident, $rt_ty:ident, $factory:ident, $id:literal, $creates:ident, $shutdowns:ident, $drops:ident, $events:ident) => {
        static $creates: AtomicU64 = AtomicU64::new(0);
        static $shutdowns: AtomicU64 = AtomicU64::new(0);
        static $drops: AtomicU64 = AtomicU64::new(0);
        static $events: AtomicU64 = AtomicU64::new(0);

        struct $mod_ty;

        impl ModFactory for $mod_ty {
            fn descriptor(&self) -> &'static ModDescriptor {
                static D: ModDescriptor = ModDescriptor {
                    id: $id,
                    name: $id,
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
                $creates.fetch_add(1, Ordering::Relaxed);
                Ok(Box::new($rt_ty))
            }
        }

        struct $rt_ty;

        impl ModRuntime for $rt_ty {
            fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
                // 订阅全部 topic 才能观测「停用后不再投递」。
                for topic in ModEventTopic::ALL {
                    registrar.subscribe(*topic)?;
                }
                Ok(())
            }

            fn on_event(&mut self, _: ModEventTopic, _: &str) -> Result<(), ModError> {
                $events.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }

            fn shutdown(&mut self) -> Result<(), ModError> {
                $shutdowns.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
        }

        impl Drop for $rt_ty {
            fn drop(&mut self) {
                $drops.fetch_add(1, Ordering::Relaxed);
            }
        }

        static $factory: &[&dyn ModFactory] = &[&$mod_ty];
    };
}

// ------------------------------------------ 1) 连续 enable 两次不泄漏

counting_mod!(
    LeakMod,
    LeakRuntime,
    LEAK_FACTORIES,
    "leak",
    LEAK_CREATES,
    LEAK_SHUTDOWNS,
    LEAK_DROPS,
    LEAK_EVENTS
);

fn reset_leak() {
    LEAK_CREATES.store(0, Ordering::Relaxed);
    LEAK_SHUTDOWNS.store(0, Ordering::Relaxed);
    LEAK_DROPS.store(0, Ordering::Relaxed);
    LEAK_EVENTS.store(0, Ordering::Relaxed);
}

#[test]
fn enable_twice_is_idempotent_and_does_not_leak() {
    reset_leak();
    let mut reg = ModRegistry::new(LEAK_FACTORIES, &serde_json::json!({}));
    reg.enable("leak").expect("首次启用应成功");
    assert_eq!(load(&LEAK_CREATES), 1, "首次启用创建一个实例");
    assert_eq!(load(&LEAK_DROPS), 0);

    // 再 enable 一次：按 P1.5 的语义是**幂等 no-op**（不重启、不换实例）。
    reg.enable("leak").expect("重复 enable 应幂等成功");
    assert_eq!(
        load(&LEAK_CREATES),
        1,
        "重复 enable 不得再 create 第二个实例"
    );
    assert_eq!(
        load(&LEAK_DROPS),
        0,
        "重复 enable 不得丢掉/覆盖正在跑的实例"
    );
    assert_eq!(load(&LEAK_SHUTDOWNS), 0, "重复 enable 不得停掉正在跑的实例");

    let (_, status, enabled) = reg.list()[0].clone();
    assert_eq!(status, ModStatus::Running);
    assert!(enabled);
    let slot = reg.runtimes.get("leak").expect("在册槽位");
    assert!(
        slot.lock().unwrap_or_else(|e| e.into_inner()).is_some(),
        "幂等 no-op 之后旧实例仍在槽位里"
    );

    // 收尾：停用会 shutdown + drop 一次（不污染别的用例——本文件各自计数）。
    reg.disable("leak").expect("收尾停用应成功");
    assert_eq!(load(&LEAK_DROPS), 1);
}

// ------------------------------------------ 2) 启动失败：槽位空 + Failed

static FLAKY_CREATES: AtomicU64 = AtomicU64::new(0);
static FLAKY_SHUTDOWNS: AtomicU64 = AtomicU64::new(0);
static FLAKY_DROPS: AtomicU64 = AtomicU64::new(0);

/// `start` 是否成功（测试切换：模拟「配置坏 / 依赖缺失」的初始化失败）。
static FLAKY_START_OK: AtomicBool = AtomicBool::new(true);

struct FlakyMod;

impl ModFactory for FlakyMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "flaky",
            name: "Flaky",
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
        FLAKY_CREATES.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FlakyRuntime))
    }
}

struct FlakyRuntime;

impl ModRuntime for FlakyRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        if FLAKY_START_OK.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(ModError::Init {
                mod_id: "flaky".to_string(),
                message: "坏配置（测试）".to_string(),
            })
        }
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        FLAKY_SHUTDOWNS.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

impl Drop for FlakyRuntime {
    fn drop(&mut self) {
        FLAKY_DROPS.fetch_add(1, Ordering::Relaxed);
    }
}

static FLAKY_FACTORIES: &[&dyn ModFactory] = &[&FlakyMod];

fn reset_flaky() {
    FLAKY_CREATES.store(0, Ordering::Relaxed);
    FLAKY_SHUTDOWNS.store(0, Ordering::Relaxed);
    FLAKY_DROPS.store(0, Ordering::Relaxed);
    FLAKY_START_OK.store(true, Ordering::SeqCst);
}

fn slot_is_empty(reg: &ModRegistry, id: &str) -> bool {
    reg.runtimes
        .get(id)
        .expect("槽位")
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .is_none()
}

#[test]
fn failed_start_clears_slot_and_keeps_state_consistent() {
    reset_flaky();
    let mut reg = ModRegistry::new(FLAKY_FACTORIES, &serde_json::json!({}));

    // 阶段 1：create 成功、start 失败 ⇒ 槽位 None + Failed + last_error。
    FLAKY_START_OK.store(false, Ordering::SeqCst);
    assert!(
        reg.enable("flaky").is_ok(),
        "start 失败只落状态，enable 不返回 Err（主链不崩：M3 既有口径）"
    );
    let (_, status, enabled) = reg.list()[0].clone();
    assert!(enabled, "manifest 的启用意图保留");
    let failed_message = match status {
        ModStatus::Failed { message } => message,
        other => panic!("start 失败应置 Failed，got {other:?}"),
    };
    assert!(
        failed_message.contains("启动失败"),
        "状态文案要写明原因：{failed_message}"
    );
    let last_error = reg.entries.get("flaky").and_then(|e| e.last_error.clone());
    assert!(
        last_error
            .as_deref()
            .is_some_and(|m| m.contains("初始化失败")),
        "last_error 要写明原因，got {last_error:?}"
    );
    assert_eq!(load(&FLAKY_CREATES), 1);
    assert_eq!(
        load(&FLAKY_DROPS),
        1,
        "启动失败的实例必须被 drop，不得留在槽位"
    );
    assert!(slot_is_empty(&reg, "flaky"), "失败后槽位必须是 None");

    // 阶段 2：修好配置再 enable（状态 Failed ≠ Running，不幂等）→ Running。
    FLAKY_START_OK.store(true, Ordering::SeqCst);
    reg.enable("flaky").expect("修复后启用应成功");
    assert_eq!(reg.list()[0].1, ModStatus::Running);
    assert_eq!(load(&FLAKY_CREATES), 2);
    assert!(!slot_is_empty(&reg, "flaky"));

    // 阶段 3：人为制造「状态 Failed、槽位里却还有旧 runtime」的历史不一致。
    // 再 enable 必须先 shutdown 旧实例再启动——旧实现直接覆盖槽位（泄漏子进程）。
    if let Some(e) = reg.entries.get_mut("flaky") {
        e.status = ModStatus::Failed {
            message: "人为制造的不一致".to_string(),
        };
    }
    FLAKY_START_OK.store(false, Ordering::SeqCst);
    assert!(reg.enable("flaky").is_ok());
    assert_eq!(
        load(&FLAKY_SHUTDOWNS),
        1,
        "旧实例必须先 shutdown（直接覆盖槽位会泄漏子进程）"
    );
    assert_eq!(
        load(&FLAKY_DROPS),
        3,
        "旧实例 shutdown 后 drop + 新失败实例 drop"
    );
    assert_eq!(load(&FLAKY_CREATES), 3);
    assert!(
        slot_is_empty(&reg, "flaky"),
        "第三次失败后槽位仍是 None（不得留下旧实例）"
    );
    let (_, status, _) = reg.list()[0].clone();
    assert!(matches!(status, ModStatus::Failed { .. }), "got {status:?}");
}

// ------------------------------------------ 3) disable：真 teardown + 不再投递

counting_mod!(
    TeardownMod,
    TeardownRuntime,
    TEARDOWN_FACTORIES,
    "teardown",
    TEARDOWN_CREATES,
    TEARDOWN_SHUTDOWNS,
    TEARDOWN_DROPS,
    TEARDOWN_EVENTS
);

#[test]
fn disable_tears_down_runtime_and_stops_event_delivery() {
    TEARDOWN_EVENTS.store(0, Ordering::Relaxed);
    TEARDOWN_SHUTDOWNS.store(0, Ordering::Relaxed);
    TEARDOWN_DROPS.store(0, Ordering::Relaxed);
    let mut reg = ModRegistry::new(TEARDOWN_FACTORIES, &serde_json::json!({}));
    reg.enable("teardown").expect("启用应成功");

    // 启用期间：订阅过的 topic 正常送达（对照，证明后面的「不送达」不是因为
    // worker 根本没跑）。
    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnPrompt, "本轮", Some("s1")));
    assert_eq!(load(&TEARDOWN_EVENTS), 1, "启用期间应收到已订阅 topic");
    assert!(reg.dispatch_event_and_flush(ModEventTopic::VoiceStarted, "{}", None));
    assert_eq!(load(&TEARDOWN_EVENTS), 2);

    reg.disable("teardown").expect("停用应成功");
    assert_eq!(
        load(&TEARDOWN_SHUTDOWNS),
        1,
        "disable 必须调用 trait 的 shutdown（真 teardown）"
    );
    assert_eq!(
        load(&TEARDOWN_DROPS),
        1,
        "shutdown 之后 runtime 必须被 drop"
    );
    assert!(slot_is_empty(&reg, "teardown"), "停用后槽位为 None");

    let before = load(&TEARDOWN_EVENTS);
    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnPrompt, "下一轮", Some("s1")));
    assert_eq!(
        load(&TEARDOWN_EVENTS),
        before,
        "停用后不得再投递事件（TurnPrompt）"
    );
    assert!(reg.dispatch_event_and_flush(ModEventTopic::VoiceStarted, "{}", None));
    assert_eq!(
        load(&TEARDOWN_EVENTS),
        before,
        "停用后不得再投递事件（VoiceStarted）"
    );
}
