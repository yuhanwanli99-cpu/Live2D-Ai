//! D4 命令路由测试（D4 任务，2026-08-29）。
//!
//! 与 `commands_routes.rs` 主体分离（≤500 行约束；本文件**仅**含
//! `#[cfg(test)]` 代码）。`commands_routes` 主体对外暴露的
//! `handle_list_commands` / `handle_invoke_command` / `extract_command_id`
//! 全部为 `pub` 或 `pub(crate)`，可在此直接调用。
//!
//! 覆盖：
//! 1. **GET 列表**：6 条命令齐备 + 含 `kind` / `target_action`。
//! 2. **POST invoke**：
//!    - 无 supervisor → 503；
//!    - 非法 strength / 未知字段 / 非 JSON body → 400；
//!    - 未知 / 空 / 非 snake_case id → 404；
//!    - 有 supervisor → 200 + accepted + epoch + steps=1；
//!    - 空 body → 走默认值（strength=2）。
//! 3. **extract_command_id** 单元：基本 / 空 / 非 snake / 错前缀。
//! 4. **不变量（grep 审计）**：`commands_routes.rs` 主体**不**直接
//!    调 `core::apply` / 构造 `Event::Action` / 构造 `AppEvent::Render`。
//!    全部经 `SupervisorHandle::trigger_action` 走 core 仲裁。

use crate::supervisor::SupervisorHandle;
use crate::web_api::commands_routes::{
    extract_command_id, handle_invoke_command, handle_list_commands,
};
use serde_json::Value;
use std::hash::Hasher;
use std::io::Read as _;
use tiny_http::{Method, Response};

fn body(resp: Response<std::io::Cursor<Vec<u8>>>) -> String {
    let mut s = String::new();
    let _ = resp.into_reader().read_to_string(&mut s);
    s
}

/// 拼一个不可达端点的 supervisor（只验证通道发送/接受）。
fn spawn_minimal_supervisor() -> (std::sync::Arc<SupervisorHandle>, std::path::PathBuf) {
    use std::hash::Hash;
    let thread_id = format!("{:?}", std::thread::current().id());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    thread_id.hash(&mut hasher);
    let tmp = std::env::temp_dir().join(format!(
        "live2d_ai_test_cmds_{}_{}.toml",
        std::process::id(),
        hasher.finish()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(
        &tmp,
        live2d_ai_runtime::AppSettings::default().to_toml_string(),
    )
    .expect("write tmp");
    let client = live2d_ai_runtime::OpenAiClient::new(
        live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
        live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
    )
    .expect("client");
    let sup = std::sync::Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client,
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: Some(tmp.to_string_lossy().into_owned()),
        },
        |_ev| {},
    ));
    (sup, tmp)
}
#[test]
fn list_commands_returns_six_entries() {
    let resp = handle_list_commands(&Method::Get, "/api/v1/commands");
    assert_eq!(resp.status_code().0, 200);
    let b = body(resp);
    let v: Value = serde_json::from_str(&b).unwrap();
    let arr = v["commands"].as_array().unwrap();
    assert_eq!(arr.len(), 6);
}

#[test]
fn list_commands_includes_kind_and_target_action() {
    let resp = handle_list_commands(&Method::Get, "/api/v1/commands");
    let v: Value = serde_json::from_str(&body(resp)).unwrap();
    let arr = v["commands"].as_array().unwrap();
    let nod = arr
        .iter()
        .find(|c| c["id"] == "live2d_perform_action_nod")
        .unwrap();
    assert_eq!(nod["kind"], "single");
    assert_eq!(nod["target_action"], "nod");
}

#[test]
fn list_commands_wrong_method_returns_405() {
    let resp = handle_list_commands(&Method::Post, "/api/v1/commands");
    assert_eq!(resp.status_code().0, 405);
}

// -------- POST：无 supervisor --------

#[test]
fn invoke_without_supervisor_returns_503() {
    let resp = handle_invoke_command(
        None,
        &Method::Post,
        "/api/v1/commands/live2d_perform_action_nod/invoke",
        r#"{"strength":2}"#,
    );
    assert_eq!(resp.status_code().0, 503);
    assert!(body(resp).contains("no_supervisor"));
}

// -------- POST：参数校验 --------

#[test]
fn invoke_wrong_method_returns_405() {
    let resp = handle_invoke_command(
        None,
        &Method::Get,
        "/api/v1/commands/live2d_perform_action_nod/invoke",
        "",
    );
    assert_eq!(resp.status_code().0, 405);
}

#[test]
fn invoke_unknown_command_returns_404() {
    let resp = handle_invoke_command(
        None,
        &Method::Post,
        "/api/v1/commands/no_such_cmd/invoke",
        "",
    );
    assert_eq!(resp.status_code().0, 404);
    assert!(body(resp).contains("command_not_found"));
}

#[test]
fn invoke_invalid_path_returns_404() {
    let resp = handle_invoke_command(None, &Method::Post, "/api/v1/commands//invoke", "");
    assert_eq!(resp.status_code().0, 404);
    // 路径里空 id 视作 not_found
}

#[test]
fn invoke_non_snake_case_id_returns_404() {
    let resp = handle_invoke_command(None, &Method::Post, "/api/v1/commands/Has-Bad/invoke", "");
    assert_eq!(resp.status_code().0, 404);
}

#[test]
fn invoke_illegal_strength_returns_400() {
    let resp = handle_invoke_command(
        None,
        &Method::Post,
        "/api/v1/commands/live2d_perform_action_nod/invoke",
        r#"{"strength":4}"#,
    );
    assert_eq!(resp.status_code().0, 400);
    let b = body(resp);
    assert!(b.contains("invalid_params"), "got: {b}");
    assert!(b.contains("strength"), "got: {b}");
}

/// v1 不支持 `repeat`：未知字段一律 400。
#[test]
fn invoke_illegal_unknown_field_replaces_repeat() {
    let resp = handle_invoke_command(
        None,
        &Method::Post,
        "/api/v1/commands/live2d_perform_action_nod/invoke",
        r#"{"repeat":0}"#,
    );
    assert_eq!(resp.status_code().0, 400);
    let b = body(resp);
    assert!(b.contains("invalid_params"), "got: {b}");
    assert!(b.contains("未知字段"), "got: {b}");
}

#[test]
fn invoke_unknown_field_returns_400() {
    let resp = handle_invoke_command(
        None,
        &Method::Post,
        "/api/v1/commands/live2d_perform_action_nod/invoke",
        r#"{"strength":2,"hax":true}"#,
    );
    assert_eq!(resp.status_code().0, 400);
}

#[test]
fn invoke_non_json_returns_400() {
    let resp = handle_invoke_command(
        None,
        &Method::Post,
        "/api/v1/commands/live2d_perform_action_nod/invoke",
        "not json",
    );
    assert_eq!(resp.status_code().0, 400);
}

// -------- POST：有 supervisor（接受入队） --------

#[test]
fn invoke_with_supervisor_returns_200_accepted() {
    let (sup, tmp) = spawn_minimal_supervisor();
    let resp = handle_invoke_command(
        Some(&sup),
        &Method::Post,
        "/api/v1/commands/live2d_perform_action_nod/invoke",
        r#"{"strength":3}"#,
    );
    assert_eq!(resp.status_code().0, 200, "invoke 应被接受");
    let b = body(resp);
    let v: Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["accepted"], true);
    assert_eq!(v["command_id"], "live2d_perform_action_nod");
    assert_eq!(v["kind"], "single");
    assert_eq!(v["steps"], 1); // v1 恒为 1
    assert_eq!(v["action"], "nod");
    // epoch 来自 supervisor 镜像
    assert!(v["epoch"].is_u64());

    sup.quit();
    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn invoke_empty_body_uses_defaults() {
    let (sup, tmp) = spawn_minimal_supervisor();
    let resp = handle_invoke_command(
        Some(&sup),
        &Method::Post,
        "/api/v1/commands/live2d_perform_action_tilt/invoke",
        "",
    );
    assert_eq!(resp.status_code().0, 200);
    let v: Value = serde_json::from_str(&body(resp)).unwrap();
    assert_eq!(v["steps"], 1);
    assert_eq!(v["action"], "tilt");
    sup.quit();
    let _ = std::fs::remove_file(&tmp);
}

// -------- extract_command_id --------

#[test]
fn extract_command_id_basic() {
    assert_eq!(
        extract_command_id("/api/v1/commands/live2d_perform_action_nod/invoke"),
        Some("live2d_perform_action_nod")
    );
}

#[test]
fn extract_command_id_empty() {
    assert_eq!(extract_command_id("/api/v1/commands//invoke"), None);
}

#[test]
fn extract_command_id_non_snake() {
    assert_eq!(extract_command_id("/api/v1/commands/HasBad/invoke"), None);
}

#[test]
fn extract_command_id_wrong_prefix() {
    assert_eq!(extract_command_id("/api/v1/scripts/x"), None);
}

// -------- handle_invoke_command 不触碰 core::apply（grep 审计锚点） --------
// 不变量：本模块**不** import `live2d_ai_core::apply` / `Event::Action` /
// `ActionCommand::Play`（除 `ActionId` 与 `SemanticAction` 的类型构造）；
// 全经 `SupervisorHandle::trigger_action`（supervisor 线程内完成 core 仲裁）。
// 本断言防止后续重构时把 core::apply 直接挪到 handler。
//
// 实现注意：注释里**可以**写 `Event::Action` / `AppEvent::Render` 解释
// 路径语义，所以扫描时**剔除** `//` 与 `///` 开头的行——只检查代码行。
#[test]
fn commands_routes_does_not_directly_call_core_apply() {
    // 不变量：本模块**不**直接调 `core::apply` / 构造 `Event::Action` /
    // 构造 `AppEvent::Render`；全经 `SupervisorHandle::trigger_action`
    // 走核心仲裁。
    //
    // 扫描策略：剔除本测试函数体本身的代码（测试函数内允许提到这些
    // 名字解释语义），用 `cfg(test)` 之外的代码做断言。
    let src = include_str!("commands_routes.rs");
    // 切到 "fn commands_routes_does_not_directly_call_core_apply" 之前
    // 的所有内容（不含本测试函数）。
    let cut = src
        .find("fn commands_routes_does_not_directly_call_core_apply")
        .unwrap_or(src.len());
    let before = &src[..cut];
    // 剔除 `//` / `///` / `//!` 开头的注释行（行内注释保留，因为
    // 我们要查的是 import / 函数体）。
    let code: String = before
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !code.contains("core::apply("),
        "handler 不应直接调 core::apply（必须经 SupervisorHandle::trigger_action → 走 core 仲裁）"
    );
    assert!(
        !code.contains("Event::Action"),
        "handler 不应直接构造 Event::Action"
    );
    assert!(
        !code.contains("AppEvent::Render"),
        "handler 不应直接构造 AppEvent::Render"
    );
}
