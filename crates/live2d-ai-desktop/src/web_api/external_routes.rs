//! 外部文本注入端点（**未挂载存根**，2026-10-19）。
//!
//! 2026-10-19「一 Mod 一本地仓库」：`external-input` Mod 的实现已整体移出主仓
//! （对应本地仓库，本机路径见 `docs/architecture/two-repo-mods.md`），主仓不再依赖该 crate。
//! 本文件因此只剩**路径常量 + 恒定 404 存根**：
//!
//! - 路径命中 `POST /api/v1/external/chat` → `404` +
//!   `{"error":{"code":"mod_disabled",...}}`（未挂载 = 功能未启用）；
//! - 路径不命中 → `None`（交还给后续 `dispatch` 继续派发）。
//!
//! 存根**刻意不读** supervisor / secrets / 请求 body / Mod 计数，也不 parse /
//! render：没挂载时这条路径不产生任何主链副作用（不打 LLM、不 `say`）。
//! 要挂回：维护者手动把 crate 加回依赖 + 工厂表 + manifest，并把本文件换回
//! 真实 handler（历史实现见 git 与该 Mod 的本地仓库）。
//!
//! # 历史（不删）
//! 节点 E5-T3（2026-08-30）起本端点把外部事件经 `supervisor.say` 注入主链路：
//! 方法 / Content-Type / Origin 校验、`external-input` 启停门禁（403 `mod_disabled`）、
//! token（env `EXTERNAL_INPUT_TOKEN` > Mod config > 不鉴权，env 走 `secrets::lookup`）、
//! 模板/前缀渲染（Mod crate 纯函数）、长度上限、2026-10-17 的忙时合并窗口。
//! 这些行为随 Mod 与合并窗口一起移出主仓（`web_api/external_coalesce.rs` 已删）。

use std::io::Cursor;

use tiny_http::{Header, Method, Response, StatusCode};

use crate::web_api::ServerContext;

/// 路由前缀（v1 固定一个外部文本端点）。
const EXTERNAL_CHAT_PATH: &str = "/api/v1/external/chat";

/// 未挂载存根（签名与历史 handler 同形，调用点 `run_request_loop` 不用改）。
///
/// 路径命中即 `404 mod_disabled`；不读 `ctx` / body / headers / auth。
pub fn handle_external_chat(
    _ctx: &ServerContext,
    _method: &Method,
    path: &str,
    _body: &str,
    _origin: Option<&str>,
    _content_type: Option<&str>,
    _auth_header: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    if path != EXTERNAL_CHAT_PATH {
        return None;
    }
    Some(json_error(
        StatusCode(404),
        "mod_disabled",
        "external-input Mod 未挂载：实现已移出主仓（一 Mod 一本地仓库）；维护者加回依赖 + 工厂表 + manifest 后本端点才可用",
    ))
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

// 回归测试：未挂载即 404 + 路径不命中回 None（同目录 `*_tests.rs`，保持源文件 <500 行）。
#[cfg(test)]
#[path = "external_routes_tests.rs"]
mod tests;
