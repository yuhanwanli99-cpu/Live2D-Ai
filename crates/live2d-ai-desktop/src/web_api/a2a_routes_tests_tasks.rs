//! a2a_routes 任务面回归（0.3.8 rc.2）：tasks/get + 任务表上限 + 范围外方法拒绝。
//!
//! 全部用注入假上游，不碰网络；真实 HTTP + mock 上游在 a2a_routes_tests_upstream.rs。
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use super::a2a_tasks::{TASK_STORE_CAP, TaskStore};
use super::tests_support::*;
use super::*;

/// 发一条 message/send，返回 result Task。
fn send(ctx: &ServerContext, tasks: &Mutex<TaskStore>, chat: &ChatFn<'_>, text: &str) -> Value {
    let (status, body) = call_in(
        ctx,
        tasks,
        &rpc_body("message/send", send_params(text)),
        chat,
    );
    assert_eq!(status, 200);
    assert!(body.get("error").is_none(), "send 不该失败：{body}");
    body["result"].clone()
}

// ---------------------------------------------------------------- tasks/get

/// send 之后能按 id 取回**同一份** completed Task（内存、不落盘）。
#[test]
fn tasks_get_returns_the_same_completed_task() {
    let ctx = strict_ctx();
    let tasks = store();
    let (chat, _) = counting_chat("你好，世界。");
    let task = send(&ctx, &tasks, &chat, "你好");
    let task_id = task["id"].as_str().expect("task id").to_string();

    let (status, got) = call_in(
        &ctx,
        &tasks,
        &rpc_body("tasks/get", json!({"id": task_id})),
        &chat,
    );
    assert_eq!(status, 200);
    assert_eq!(got["result"], task, "tasks/get 必须回同一份 Task");
    assert_eq!(got["result"]["status"]["state"], "completed");
    assert_eq!(
        got["result"]["artifacts"][0]["parts"][0]["text"],
        "你好，世界。"
    );
}

/// 未知 id → -32001 TaskNotFound（不是 invalid params）。
#[test]
fn tasks_get_unknown_id_is_task_not_found() {
    let ctx = strict_ctx();
    let tasks = store();
    let (chat, hits) = counting_chat("不会用到");
    let (status, body) = call_in(
        &ctx,
        &tasks,
        &rpc_body("tasks/get", json!({"id": "task-does-not-exist"})),
        &chat,
    );
    assert_eq!(status, 200);
    assert_eq!(body["error"]["code"], -32001);
    assert_eq!(hits.load(Ordering::Relaxed), 0, "tasks/get 不打上游");
}

/// 缺 id / 空 id → -32602 invalid params。
#[test]
fn tasks_get_missing_id_is_invalid_params() {
    let ctx = strict_ctx();
    let tasks = store();
    let (chat, _) = counting_chat("不会用到");
    for params in [json!({}), json!({"id": ""}), json!({"id": 7})] {
        let (_, body) = call_in(&ctx, &tasks, &rpc_body("tasks/get", params), &chat);
        assert_eq!(body["error"]["code"], -32602, "{body}");
    }
}

/// 上限钉死 32（执行单 §3），且第 33 条会淘汰最旧的一条。
#[test]
fn task_store_keeps_at_most_32_and_evicts_oldest() {
    assert_eq!(TASK_STORE_CAP, 32, "执行单 §3：上限 32 条");
    let ctx = strict_ctx();
    let tasks = store();
    let (chat, _) = counting_chat("好。");
    let mut ids = Vec::new();
    for i in 0..33 {
        let task = send(&ctx, &tasks, &chat, &format!("第 {i} 条"));
        ids.push(task["id"].as_str().expect("task id").to_string());
    }
    // 最新的 32 条读得到，最旧的第 1 条已被淘汰。
    for id in &ids[1..] {
        let (_, body) = call_in(
            &ctx,
            &tasks,
            &rpc_body("tasks/get", json!({"id": id})),
            &chat,
        );
        assert!(body["result"].is_object(), "应能取回 {id}：{body}");
    }
    let (_, gone) = call_in(
        &ctx,
        &tasks,
        &rpc_body("tasks/get", json!({"id": ids[0]})),
        &chat,
    );
    assert_eq!(gone["error"]["code"], -32001, "最旧任务应被淘汰：{gone}");
    assert_eq!(tasks.lock().expect("tasks lock").len(), TASK_STORE_CAP);
}

// ---------------------------------------------------------------- 范围外方法

/// 范围外方法一律回 JSON-RPC 错误、**不打上游**（也不出半截 SSE）。
#[test]
fn out_of_scope_methods_are_rejected_without_calling_upstream() {
    let ctx = strict_ctx();
    let tasks = store();
    let (chat, hits) = counting_chat("不会用到");
    let cases = [
        // 已知 A2A 方法，但本服务不实现（streaming=false / 不做取消 / 不做列表）。
        ("message/stream", -32004),
        ("tasks/cancel", -32004),
        ("tasks/resubscribe", -32004),
        ("tasks/list", -32004),
        ("agent/getAuthenticatedExtendedCard", -32004),
        // 推送配置整族不支持（pushNotifications=false）。
        ("tasks/pushNotificationConfig/set", -32003),
        ("tasks/pushNotificationConfig/get", -32003),
        ("tasks/pushNotificationConfig/list", -32003),
        ("tasks/pushNotificationConfig/delete", -32003),
        // 完全未知的方法。
        ("foo/bar", -32601),
    ];
    for (method, expected) in cases {
        let (status, body) = call_in(&ctx, &tasks, &rpc_body(method, json!({})), &chat);
        assert_eq!(status, 200, "{method}: JSON-RPC 错误是 HTTP 200");
        assert_eq!(body["error"]["code"], expected, "{method}: {body}");
    }
    assert_eq!(hits.load(Ordering::Relaxed), 0, "范围外方法不得打上游");
}
