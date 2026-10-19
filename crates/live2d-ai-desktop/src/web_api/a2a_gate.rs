//! A2A 宿主门（**未挂载存根**，2026-10-19）。
//!
//! 2026-10-19「一 Mod 一本地仓库」：`a2a` Mod 的实现（协议核：agent card /
//! JSON-RPC 2.0 / 任务表 / 上游收集）已整体移出主仓（本地仓库
//! 对应本地仓库，本机路径见 `docs/architecture/two-repo-mods.md`），主仓不再依赖该
//! crate 与它的上游闭包。
//! 本文件只剩**两条路径常量 + 恒定 404 存根**：
//!
//! - `GET /.well-known/agent-card.json` 与 `POST /api/v1/a2a` **都**回
//!   `404` + `{"error":{"code":"mod_disabled",...}}`；
//! - `is_a2a_path` 仍由 `run_request_loop` 做前置判断（只认这两条路径）；
//! - POST **不再派生工作线程**：没有上游 LLM 可等，`mod.rs` 里直接 respond。
//!
//! 存根**刻意不读** supervisor / secrets / 注册表 / body——未挂载时不打上游、
//! 不 `say`。要挂回：维护者手动加回依赖 + 工厂表 + manifest，并把真实门与
//! 协议核从该 Mod 的本地仓库接回（历史实现见 git）。
//!
//! # 历史（不删）
//! 0.3.9 rc.3 起本文件是「宿主只留门」的落点：loopback / Origin / Content-Type
//! 校验、`a2a` 停用门禁（403 `mod_disabled`）、宿主注入上游闭包
//! （`a2a_gate/llm.rs`：读 `[llm]` 快照 + `secrets::lookup` + `OpenAiClient`）、
//! POST 派生工作线程（`message/send` 要等上游整段回复，不能冻住 HTTP 循环）。
//! `a2a_gate/llm.rs` 已随 Mod 移出主仓删除。

use std::io::Cursor;

use tiny_http::{Header, Response, StatusCode};

use crate::web_api::ServerContext;

/// 发现文档路径（A2A agent card）。
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";
/// JSON-RPC 端点路径。
pub const A2A_PATH: &str = "/api/v1/a2a";

/// true = 这条路径属于 A2A（发现文档或 JSON-RPC 端点）。
///
/// `run_request_loop` 用它把请求**前置于** dispatch；不命中就交还给后续路由。
pub fn is_a2a_path(path: &str) -> bool {
    path == AGENT_CARD_PATH || path == A2A_PATH
}

/// 两条 A2A 路径的未挂载存根：恒定 `404 mod_disabled`。
///
/// 不读 `ctx`（不碰 supervisor / secrets / 注册表），不派线程、不打上游。
pub fn handle_a2a(_ctx: &ServerContext) -> Response<Cursor<Vec<u8>>> {
    json_error(
        StatusCode(404),
        "mod_disabled",
        "a2a Mod 未挂载：实现已移出主仓（一 Mod 一本地仓库）；维护者加回依赖 + 工厂表 + manifest 后本端点才可用",
    )
}

/// 构造 JSON 响应（status + body；Content-Type application/json）。
fn ok_response(status: StatusCode, body: &str) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body.as_bytes().to_vec())
        .with_status_code(status)
        .with_header(
            Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
        .with_header(
            Header::from_bytes(&b"Cache-Control"[..], &b"no-cache"[..])
                .expect("Cache-Control header"),
        )
}

/// 通用 JSON 错误响应（status + body；Content-Type application/json）。
fn json_error(status: StatusCode, code: &str, message: &str) -> Response<Cursor<Vec<u8>>> {
    let body = serde_json::json!({"error": {"code": code, "message": message}}).to_string();
    ok_response(status, &body)
}

// 回归测试：两条路径都 404 + is_a2a_path 只认两条路径（同目录 `*_tests.rs`）。
#[cfg(test)]
#[path = "a2a_gate_tests.rs"]
mod tests;
