//! 本机 Pi 出站 HTTP 路由（0.4.5 rc.1）：`POST /api/v1/peers/{id}/message`。
//!
//! 契约真源：`docs/versions/0.4.md`「0.4.5 把一条对话发给本机 Pi」`出站。
//!
//! # 契约（逐条锁死）
//!
//! - 与其它改状态接口同一条安全纪律：loopback + `Origin` +
//!   `Content-Type: application/json`（复用 [`crate::web_api::security`]，不另造判据）。
//! - 请求体 `{"text":"…","session_id":"…"}`；`text` 空白 → 400（**不启进程**）；
//!   `session_id` 只允许 `[A-Za-z0-9_-]`、最长 64，否则 400；空串 = 这一条不记会话。
//! - 成功 200 `{"text":"<Pi 的那一句>"}`，**只放助手正文**（工具记录 / 思考 /
//!   原始 stdout 都不进这个字段）。
//! - 错误码集合**恰好**五个：`peer_missing` 404 / `peer_busy` 409 /
//!   `peer_timeout` 504 / `peer_failed` 502 / `peer_unavailable` 404。
//! - `id == "claude"` 在**找二进制之前**就回 404 `peer_unavailable`，绝不 spawn
//!   （单测断言没有子进程）。
//! - 同一时刻只有一个 Pi 子进程；已有在飞时第二个请求**立即** 409，不排队、
//!   不打断皮套、不进弹幕队列。
//!
//! # 为什么请求要搬到 worker 线程
//!
//! [`crate::web_api::run_request_loop`] 是**串行**的 tiny_http 循环——在这里同步等
//! 最长 120s 会把整个 HTTP 面冻住。所以本模块只做校验 + 抢 busy 槽位（都在毫秒级），
//! 成功抢到后把 **move 进来的 `tiny_http::Request`** 交给 `peer-pi` worker 线程，
//! 由它在子进程结束后自己 `respond`。busy 状态住在
//! [`crate::web_api::ServerContext`] 的
//! [`crate::web_api::peer_pi::PeerState`] 里，跨请求可见。
//!
//! # 不碰皮套
//!
//! 不调用 `say`、不写 `mods.json`、不发 `action_cue`、不动 TTS——Pi 的回答只回给
//! 这一条 HTTP 请求（界面侧放在「Pi」气泡里，刷新可消失）。

use std::io::Cursor;

use tiny_http::{Method, Response, StatusCode};

use crate::web_api::ServerContext;
use crate::web_api::dto::{ErrorDetail, ErrorResponse};
use crate::web_api::peer_pi::{self, BusyGuard, PiRunOutcome, RunConfig};
use crate::web_api::responses::json_error;

/// 路由稳定标签（请求级日志用；与其它前置路由的 route 字段同源）。
pub const ROUTE_LABEL: &str = "peers.pi";

/// 本机 Pi 的 peer id（路径里的 `{id}`）。
pub const PI_PEER_ID: &str = "pi";

/// 这一版只画出来、点不了的 peer（Claude Code）。
const CLAUDE_PEER_ID: &str = "claude";

const PATH_PREFIX: &str = "/api/v1/peers/";
const PATH_SUFFIX: &str = "/message";

/// 路由处理结果：立即回，或把 Request 交给 worker 线程。
pub enum PeerOutcome {
    /// 校验失败 / 宿主前置失败：本线程直接回（毫秒级）。
    Immediate(Response<Cursor<Vec<u8>>>),
    /// 已抢到单飞名额：由 worker 线程跑子进程并自己 respond。
    Deferred(PiJob),
}

impl PeerOutcome {
    /// 收尾：立即响应走本线程；异步响应派生 `peer-pi` 线程。
    ///
    /// 线程起不来（资源耗尽）时 `Request` 随闭包被丢弃——tiny_http 的
    /// `Drop` 会回 500，busy 随 `BusyGuard` 释放，服务继续跑。
    pub fn complete(self, request: tiny_http::Request, method: &Method, path: &str) {
        match self {
            Self::Immediate(resp) => finish(request, method, path, resp),
            Self::Deferred(job) => {
                let method = method.clone();
                let path = path.to_string();
                let spawned = std::thread::Builder::new()
                    .name("peer-pi".to_string())
                    .spawn(move || {
                        let resp = job.run();
                        finish(request, &method, &path, resp);
                    });
                if let Err(err) = spawned {
                    tracing::error!(
                        target: "peer_pi",
                        error = %err,
                        "无法启动 peer-pi worker 线程（请求由 tiny_http Drop 回 500）"
                    );
                }
            }
        }
    }
}

/// 一次已经在飞的本机 Pi 运行（持有单飞守卫；drop 即释放）。
pub struct PiJob {
    /// busy 闸门守卫：子进程结束（或 panic 展开）时释放。
    _guard: BusyGuard,
    config: RunConfig,
    text: String,
    session_id: String,
}

impl PiJob {
    /// 同步执行（worker 线程里就是这一句）：错误码映射见模块头。
    pub fn run(self) -> Response<Cursor<Vec<u8>>> {
        let outcome = peer_pi::run(&self.config, &self.text, &self.session_id);
        match outcome {
            PiRunOutcome::Reply(text) => json_response(
                StatusCode(200),
                serde_json::to_vec(&serde_json::json!({ "text": text })).unwrap_or_default(),
            ),
            PiRunOutcome::Missing => peer_missing(),
            PiRunOutcome::Timeout => peer_timeout(),
            PiRunOutcome::Failed(reason) => {
                // 只记原因码，不记 stdout / stderr / 原文。
                tracing::warn!(target: "peer_pi", code = "peer_failed", reason, "本机 Pi 出站失败");
                peer_failed()
            }
        }
    }
}

/// 入口（`run_request_loop` 在 dispatch 之前调用）。
///
/// 路径不匹配 → `None`（交还 dispatch 的 501 / 404 既有口径）。
pub fn handle_peer_route(
    ctx: &ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Option<PeerOutcome> {
    let peer_id = parse_peer_path(path)?;
    if *method != Method::Post {
        return Some(PeerOutcome::Immediate(method_not_allowed(path)));
    }
    // 与 dispatch 的 mutating 校验同一条判据（传进来的 Origin / CT 与 body 是否存在）。
    let body_present = !body.is_empty();
    if let Err(err) = crate::web_api::security::check_mutating_request(
        origin,
        content_type,
        &ctx.security,
        body_present,
    ) {
        tracing::warn!(
            code = err.as_code(),
            status = err.as_status(),
            path = %path,
            origin = ?origin,
            content_type = ?content_type,
            body_present,
            "peer 请求被安全校验拒绝"
        );
        return Some(PeerOutcome::Immediate(
            crate::web_api::security::mutating_check_error_response(&err),
        ));
    }
    // **claude（及其它未接的 id）在找二进制 / 读 body / 抢 busy 之前返回**：
    // 这一版绝不执行 claude，也绝不因为它在飞而 409。
    if peer_id != PI_PEER_ID {
        return Some(PeerOutcome::Immediate(peer_unavailable(peer_id)));
    }
    let parsed: PeerBody = match serde_json::from_str(body) {
        Ok(parsed) => parsed,
        Err(_) => {
            return Some(PeerOutcome::Immediate(invalid_payload(
                "请求体不是合法 JSON 或缺字段 text",
            )));
        }
    };
    // 空白 text → 400，且**不启进程**（在 try_acquire 之前判）。
    if parsed.text.trim().is_empty() {
        return Some(PeerOutcome::Immediate(invalid_payload(
            "字段 text 必填且非空",
        )));
    }
    if !peer_pi::validate_session_id(&parsed.session_id) {
        return Some(PeerOutcome::Immediate(invalid_payload(
            "session_id 只允许字母、数字、_、-，最长 64",
        )));
    }
    // 唯一单飞闸门：抢不到 = 已有 Pi 在飞 → 立即 409（不排队）。
    let Some(guard) = ctx.peer_pi.try_acquire() else {
        return Some(PeerOutcome::Immediate(peer_busy()));
    };
    let config = match ctx.peer_pi.resolve_config() {
        Ok(config) => config,
        Err(err) => {
            // 宿主自己准备不了会话目录：本机故障，不是 Pi 的回答问题。
            // 错误码集合锁死，仍归 peer_failed（日志留原因）。
            tracing::warn!(target: "peer_pi", reason = "runtime_dir", error = %err, "无法准备 Pi 运行时目录");
            return Some(PeerOutcome::Immediate(peer_failed()));
        }
    };
    Some(PeerOutcome::Deferred(PiJob {
        _guard: guard,
        config,
        text: parsed.text,
        session_id: parsed.session_id,
    }))
}

/// 从路径里取 peer id：只认 `/api/v1/peers/{id}/message`。
fn parse_peer_path(path: &str) -> Option<&str> {
    let rest = path.strip_prefix(PATH_PREFIX)?;
    let id = rest.strip_suffix(PATH_SUFFIX)?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some(id)
}

/// 请求体（`session_id` 缺省 = 空串 = 这一条不记会话；`text` 必填）。
#[derive(serde::Deserialize)]
struct PeerBody {
    text: String,
    #[serde(default)]
    session_id: String,
}

fn finish(
    request: tiny_http::Request,
    method: &Method,
    path: &str,
    resp: Response<Cursor<Vec<u8>>>,
) {
    crate::web_api::dispatch::log_request_outcome(method, path, ROUTE_LABEL, &resp);
    let _ = request.respond(resp);
}

// =====================================================================
// 响应构造（错误体形状与既有端点一致：{"error":{"code","message"}}）
// =====================================================================

fn json_response(status: StatusCode, body: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    json_error(status, body)
}

fn error_response(
    status: StatusCode,
    code: &'static str,
    message: impl Into<String>,
) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(code, message);
    json_response(
        status,
        serde_json::to_vec(&ErrorResponse::new(detail)).unwrap_or_default(),
    )
}

fn invalid_payload(message: &str) -> Response<Cursor<Vec<u8>>> {
    error_response(StatusCode(400), "invalid_payload", message)
}

fn method_not_allowed(path: &str) -> Response<Cursor<Vec<u8>>> {
    error_response(
        StatusCode(405),
        "method_not_allowed",
        format!("{path:?} 需 POST 方法"),
    )
}

fn peer_missing() -> Response<Cursor<Vec<u8>>> {
    error_response(StatusCode(404), "peer_missing", "本机没有 pi")
}

fn peer_busy() -> Response<Cursor<Vec<u8>>> {
    error_response(StatusCode(409), "peer_busy", "上一条还在等 Pi")
}

fn peer_timeout() -> Response<Cursor<Vec<u8>>> {
    error_response(StatusCode(504), "peer_timeout", "Pi 超时")
}

fn peer_failed() -> Response<Cursor<Vec<u8>>> {
    error_response(StatusCode(502), "peer_failed", "Pi 没有返回一句可读的话")
}

fn peer_unavailable(peer_id: &str) -> Response<Cursor<Vec<u8>>> {
    let message = if peer_id == CLAUDE_PEER_ID {
        "这一版不接 Claude Code".to_string()
    } else {
        format!("这一版只支持本机 Pi（{PI_PEER_ID}），没有 {peer_id:?}")
    };
    error_response(StatusCode(404), "peer_unavailable", message)
}

#[cfg(all(test, unix))]
#[path = "peer_routes_tests.rs"]
mod tests;
