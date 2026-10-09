//! 单元测试（三）：「子进程立刻退出 → start Err / Failed」（2026-10-09 新口径）。
//!
//! 计划书口径：第一次启动可以按 tag 装 venv；**装失败或进程马上以非 0 退出时，
//! `start` 必须返回 Err，Mod 状态是 Failed，错误里带脚本印出的原因**。
//! 不要显示「已启用」而其实没在听端口。
//!
//! 共享假件在 tests_support；这里只用「一出生就已退出」的假句柄 +
//! 显式观察窗口（`runtime_with_grace`）。

use std::sync::Arc;
use std::time::Duration;

use super::*;
use crate::tests_support::*;

/// 观察窗口（用例专用；生产是 [IMMEDIATE_EXIT_GRACE]，1 秒）。
const TEST_GRACE: Duration = Duration::from_millis(200);

#[test]
fn immediate_nonzero_exit_fails_start_with_the_script_reason() {
    let log = Arc::new(SpawnerLog::default());
    log.child_exits_on_spawn(
        Some(2),
        "[melo] 载入模型失败：缺 bert-base-multilingual-uncased\n",
    );
    let (mut rt, _) = runtime_with_grace(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
        TEST_GRACE,
    );

    let err = start(&mut rt).expect_err("子进程立刻非 0 退出时 start 必须 Err（不许说「已启用」）");
    let msg = err.to_string();
    assert!(msg.contains("退出码 2"), "错误里要有退出码：{msg}");
    assert!(
        msg.contains("bert-base-multilingual-uncased"),
        "错误里要带**脚本印出的原因**（stderr 尾巴）：{msg}"
    );
    assert_eq!(log.calls(), 1, "失败的重试不得再起第二个进程");
}

#[test]
fn immediate_zero_exit_also_fails_start() {
    // 退出码 0 也算失败：立刻退出的进程根本没在听端口。
    let log = Arc::new(SpawnerLog::default());
    log.child_exits_on_spawn(Some(0), "");
    let (mut rt, _) = runtime_with_grace(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
        TEST_GRACE,
    );

    let err = start(&mut rt).expect_err("立刻以 0 退出同样是「没在听端口」");
    assert!(err.to_string().contains("退出码 0"), "got {err}");
}

#[test]
fn signal_death_is_reported_as_signal_not_as_a_fake_code() {
    let log = Arc::new(SpawnerLog::default());
    log.child_exits_on_spawn(None, "");
    let (mut rt, _) = runtime_with_grace(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
        TEST_GRACE,
    );

    let err = start(&mut rt).expect_err("被信号终止也是「没起来」");
    let msg = err.to_string();
    assert!(msg.contains("被信号终止"), "got {msg}");
    assert!(!msg.contains("退出码"), "拿不到退出码就不许编一个：{msg}");
}

#[test]
fn failed_start_leaves_no_handle_and_shutdown_is_clean() {
    let log = Arc::new(SpawnerLog::default());
    log.child_exits_on_spawn(Some(3), "[melo] 创建 venv 失败\n");
    let (mut rt, _) = runtime_with_grace(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
        TEST_GRACE,
    );
    start(&mut rt).expect_err("应失败");

    let snap = rt.state_json().unwrap();
    assert_eq!(
        snap["child_running"],
        serde_json::json!(false),
        "失败的 start 不许留下一个「在跑」的句柄"
    );
    assert_eq!(snap["pid"], serde_json::json!(null));
    // 没有句柄 → shutdown 是 no-op；stop 命令则必须 Err（「没有子进程」）。
    rt.shutdown().expect("没有句柄时 shutdown 应成功");
    assert!(
        rt.command("stop", &serde_json::json!({})).is_err(),
        "没有子进程时 stop 必须 Err"
    );
}

/// 窗口内一直 Running = 正常路径（脚本正在装 venv / 载模型）。
#[test]
fn a_child_that_keeps_running_passes_the_window() {
    let log = Arc::new(SpawnerLog::default());
    let (mut rt, _) = runtime_with_grace(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
        Duration::from_millis(60),
    );
    start(&mut rt).expect("窗口内没退出就该判定为起来了");
    assert_eq!(
        rt.state_json().unwrap()["child_running"],
        serde_json::json!(true)
    );
}
