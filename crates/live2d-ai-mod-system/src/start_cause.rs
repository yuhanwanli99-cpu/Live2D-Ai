//! 启动原因（**线程局部**，2026-10-09）。
//!
//! # 为什么需要它
//!
//! `ModRuntime::start` 只有 `&mut self` 与 `registrar`，**分不清**这一次
//! 启动是「随应用开机」「用户按开关启用」还是「保存配置后重启」。多数 Mod
//! 不需要知道；首个需要它的是 `local-tts`——它按 `launch_mode` 决定要不要
//! `spawn` 外部进程，而 `on_apply`（缺省）**只能**在「保存并应用」时拉起。
//!
//! # 为什么是线程局部而不是改 `ModServices`
//!
//! `ModServices` 的构造函数是**所有 Mod 都在用**的公开 API（静态注册表里
//! 6 个工厂全走它），为一个 Mod 的需求加参数会让每个 Mod 都被迫改。
//! 线程局部的前提是「宿主在**同一条同步调用栈**上设置，再调 `start`」——
//! 这正是 `ModRegistry` 的做法（`start_all` / `enable` / `restart` 都在
//! 自己的线程上直接调 `start_one`）。**不要把 `start` 挪到别的线程**，
//! 否则读到的会是缺省值。
//!
//! # 缺省值
//!
//! `Boot`：漏设时 `on_apply` **不会**意外 `spawn`——宁可少拉一个进程，
//! 也不要在用户没点「保存并应用」时偷偷拉起来。

use std::cell::Cell;

/// 这一次 `ModRuntime::start` 的调用原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartCause {
    /// 随应用开机（`ModRegistry::start_all` 逐个启动 manifest 里 enabled 的 Mod）。
    Boot,
    /// 用户按下启用开关（公开 `enable` / `enable_with_config`）。
    Enable,
    /// 保存配置后重启（`restart` 成功停掉旧实例之后的那一次 `start`）。
    Apply,
}

thread_local! {
    /// 当前线程的启动原因；缺省 `Boot`（见模块头注）。
    static START_CAUSE: Cell<StartCause> = const { Cell::new(StartCause::Boot) };
}

/// 设置**当前线程**的启动原因（宿主在调 `start` 之前、同一条同步栈上调用）。
pub fn set_start_cause(cause: StartCause) {
    START_CAUSE.with(|c| c.set(cause));
}

/// 读**当前线程**的启动原因（`local-tts` 在 `start` 里读它）。
pub fn start_cause() -> StartCause {
    START_CAUSE.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 缺省是 `Boot`：在一条**新线程**上读（thread-local 的真实缺省），
    /// 不依赖测试线程的先后顺序。
    #[test]
    fn default_is_boot_on_a_fresh_thread() {
        let seen = std::thread::spawn(start_cause).join().unwrap();
        assert_eq!(seen, StartCause::Boot);
    }

    /// 设置后同线程可见；读回后还原，避免污染同线程后续用例。
    #[test]
    fn set_then_read_on_same_thread() {
        std::thread::spawn(|| {
            set_start_cause(StartCause::Apply);
            assert_eq!(start_cause(), StartCause::Apply);
            set_start_cause(StartCause::Boot);
        })
        .join()
        .unwrap();
    }
}
