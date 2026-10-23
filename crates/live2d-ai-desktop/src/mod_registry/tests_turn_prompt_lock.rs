//! `TurnPrompt` 抢 runtime 槽位锁的短重试 / 记丢回归（rc.3，2026-10-23）。
//!
//! 场景：host 正在 restart 而持有某 Mod 的 runtime 锁时，`TurnPrompt` 是
//! `dispatch_event_and_flush` 的话题——旧实现静默跳过 = 宿主回执「处理完了」
//! 但这一轮注入其实没送达。本文件守：先短重试 3 次（约 10ms/次），仍失败则
//! 计入 `dropped_events`、不调 `on_event`、不碰槽位、回执照发；其它话题维持
//! 原样（跳过、不计数）。
//!
//! 单列文件：`tests_lifecycle.rs` 已逼近 500 行棘轮（rc.3 收口）。
//! 挂法与 `tests_start_cause` 同一先例（`#[path]` 兄弟文件）。

use super::*;

// ---------------------------------- rc.3：TurnPrompt 抢锁重试 / 记丢（2026-10-23）

/// 本用例专属的 on_event 记录（**全局 static**：on_event 跑在 worker 线程，
/// thread_local 会让测试线程读到另一份）。不碰 `RECEIVED`，避免并行测试污染。
static LOCK_HOLD_SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn lock_hold_seen() -> Vec<String> {
    LOCK_HOLD_SEEN.lock().unwrap().clone()
}

/// 一个「抢不到锁就什么都收不到」的 Mod：on_event 只记话题。
struct LockHoldMod;

impl ModFactory for LockHoldMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "lock_hold",
            name: "LockHold",
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
        Ok(Box::new(LockHoldRuntime))
    }
}

struct LockHoldRuntime;

impl ModRuntime for LockHoldRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        Ok(())
    }
    fn on_event(&mut self, topic: ModEventTopic, _: &str) -> Result<(), ModError> {
        LOCK_HOLD_SEEN
            .lock()
            .unwrap()
            .push(topic.as_str().to_string());
        Ok(())
    }
}

static LOCK_HOLD_FACTORIES: &[&dyn ModFactory] = &[&LockHoldMod];

/// **rc.3 回归**：`TurnPrompt` 的 runtime 锁被占（host 正在 restart）时——
///
/// 1. 先短重试 3 次（约 10ms/次）；
/// 2. 仍失败 → 计入 `dropped_events`，**不**调 `on_event`、**不**碰槽位；
/// 3. `dispatch_event_and_flush` 照常回执（宿主继续这一轮，这一轮就是没注入）；
/// 4. 其它话题维持旧行为（锁被占时直接跳过、不计数）；释放锁后普通 topic 照常
///    送达且 dropped 不再增加。
#[test]
fn turn_prompt_lock_hold_retries_then_counts_dropped_without_on_event() {
    LOCK_HOLD_SEEN.lock().unwrap().clear();
    let mut reg = ModRegistry::new(
        LOCK_HOLD_FACTORIES,
        &serde_json::json!({"mods":{"lock_hold":{"enabled":true}}}),
    );
    reg.start_all();
    assert_eq!(reg.dropped_events(), 0, "起点：还没丢过");

    // 测试线程先持有 runtime 锁 = host restart 正持锁（worker 的 try_lock 必失败）。
    let slot = Arc::clone(reg.runtimes.get("lock_hold").expect("在册槽位"));
    let held = slot.lock().unwrap_or_else(|e| e.into_inner());

    // 1+2+3：回执照发（阻塞等 worker 处理完），但这一条对 Mod 没送达。
    let started = std::time::Instant::now();
    assert!(
        reg.dispatch_event_and_flush(ModEventTopic::TurnPrompt, "本轮正文", Some("sess-1")),
        "队列没满：TurnPrompt 必须入队并回执（宿主继续本轮）"
    );
    let elapsed = started.elapsed();
    assert!(
        reg.dropped_events() >= 1,
        "抢锁重试后仍失败必须计入 dropped_events，got {}",
        reg.dropped_events()
    );
    assert!(
        !lock_hold_seen().iter().any(|t| t == "turn_prompt"),
        "锁被占时绝不能调用 on_event（也不碰槽位）：{:?}",
        lock_hold_seen()
    );
    // 下界证明「先重试再记丢」而不是立即跳过（3×10ms 退避只会更长）。
    assert!(
        elapsed >= std::time::Duration::from_millis(20),
        "TurnPrompt 应先短重试再记丢，实际只用了 {elapsed:?}"
    );
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "重试不得变成新的长等待，实际 {elapsed:?}"
    );

    // 其它 topic 在**同一把锁被占**时：直接跳过、不计数（既有行为）。
    let before = reg.dropped_events();
    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnStarted, "{}", None));
    assert_eq!(
        reg.dropped_events(),
        before,
        "其它 topic 的 try_lock 失败是正常过滤，不得计数"
    );
    assert!(!lock_hold_seen().iter().any(|t| t == "turn_started"));

    // 槽位没被碰过：runtime 仍在（释放锁后可见）。
    drop(held);
    assert!(
        reg.runtimes
            .get("lock_hold")
            .expect("槽位")
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some(),
        "抢锁失败不得清空该 Mod 的 runtime 槽位"
    );

    // 4：释放锁后普通 topic 照常送达，dropped 不变。
    let before = reg.dropped_events();
    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnStarted, "{}", None));
    assert_eq!(
        reg.dropped_events(),
        before,
        "释放锁后普通 topic 不该增加 dropped"
    );
    assert!(
        lock_hold_seen().iter().any(|t| t == "turn_started"),
        "释放锁后事件应正常送达：{:?}",
        lock_hold_seen()
    );
}
