//! HTTP 对话入口（W2 任务，2026-08-29）。
//!
//! # 端点（D1 §1.1 + W2 任务）
//!
//! - `POST /api/v1/chat` body `{"text": "...", "session_id"?: "..."}` → 调
//!   `SupervisorHandle::say_scoped`（容量 1 通道；满则 429 busy）。
//!   `session_id` 是 **L1 会话绑定**的入口：同一句话带上会话 id 后，本轮 LLM
//!   请求的 system_prompt 取该会话的覆盖（persona / memory 按会话写），没有覆盖
//!   则回落全局 `persona.system_prompt`。id 归一化规则见
//!   `live2d_ai_mod_system::sanitize_session_id`；**非法 id 视为不带会话**
//!   （不报错——那会让前端多一条只在特定字符下才出现的失败路径）。
//! - `POST /api/v1/chat/stop` → 调 `SupervisorHandle::stop`（幂等）。
//! - `GET|POST /api/v1/chat/session` → **会话 id 宿主能力**（L1 基座）：
//!   GET 读「当前活动会话 + 已绑定的会话数/列表 + 该会话的 baseline」，POST 设
//!   当前活动会话（`{"session_id": "..."}`；缺省/空 = 清掉）并**随会话设置**
//!   写 baseline（O8：`{"session_id":"s1","baseline": <json|null>}`；
//!   `baseline` 键**缺席 = 不动**，`null` = 撤销回待机）。
//!
//! # 会话 baseline 的 host 侧接线（V10 / O8，阶段4e）
//!
//! - **存储**：`session_scope.rs` 里 `prompt_for(...)` 的**兄弟字段**（同一张表、
//!   同一个会话键、同一道 `sanitize_session_id` 闸）——**不是**第二套会话表；
//! - **写入方 = 宿主 API**：只有上面那条 session 路由会写它（本轮不做 per-character
//!   profile 文件）；
//! - **缺省为空 = 待机**：没写过 = 回待机；`null` 显式撤销；
//! - **停止 / 新消息** → `cancel_and_return_to_session_baseline` /
//!   `return_to_session_baseline`：清动作 + 表情 + TTS 待播（supervisor 的
//!   stop 事务）后，把该会话 baseline 作为一条**既有 `action_cue` 帧**广播出去，
//!   并在**同一个 HTTP 响应**里回给调用方（前端不另发请求）；**不补帧**。
//!
//!   为什么切会话要**告诉服务端**：external-input（弹幕）/ voice-input（转写）
//!   这两条注入路径不带会话——它们的语义是「接在当前这段对话上」。宿主记下
//!   活动会话，注入才会落到**用户正在看的**那个会话，而不是上一次发消息的那个。
//!
//! # 状态码语义
//!
//! - 200 + `{"accepted": true, "epoch": N}`：say 被受理（pending）。epoch 是
//!   supervisor **当前**代次（`u64`，给前端在 WS 上等待 `turn_state.epoch` 关联）。
//! - 400 `invalid_payload`：body 非 JSON 或缺 `text` 字段。
//! - 405 `method_not_allowed`：method 非 POST（保留路由层校验）。
//! - 429 `busy`：say 通道满（已有 pending）。前端应等待 WS 上
//!   `turn_state`/`runtime_status.new_epoch` 后再发。
//! - 503 `no_supervisor`：web 模式未装配 supervisor（仅纯控制平面场景）。
//!
//! # 不变量
//!
//! - handler **不**触碰 `core::apply`/`AppEvent::Render`（D1 §7.3 严格仲裁路径）。
//! - 全文**不**读取文件（除 `request_reload` 由 PATCH 路径触发）。
//!
//! # 行数豁免说明（用户硬约束 ≤500；本文件允许 ≤1000 头注豁免）
//!
//! 本文件当前约 560 行：对话入口 + stop + **L1 会话 id 路由**三个 handler，
//! 以及它们的契约测试全在同一文件。之所以不拆测试：这三个 handler 共用同一套
//! 响应构造器（`json_response` / `invalid_payload` / `busy_response` /
//! `no_supervisor`）与同一份「路由把查询串当路径」的历史教训（见 commit 历史：
//! 那次缺陷正是从 handler 与路由的边界漏出来的），拆开会让读者为了改一条响应
//! 文案来回跳两个文件。其余路由文件（`voice_routes_tests.rs` 等）把测试分出去，
//! 是因为它们的**测试**本身就比 handler 大得多。

use std::io::Cursor;

use serde::{Deserialize, Serialize};
use tiny_http::{Method, Response, StatusCode};

use live2d_ai_mod_system::SessionPromptSink as _;

use crate::supervisor::SupervisorHandle;
use crate::web_api::dto::{ErrorDetail, ErrorResponse};

// 会话 baseline 段拆到 `chat_routes/baseline.rs`：本文件已接近 code-stats 的
// >500 行档，而那一段是独立的「停止 / 新消息」事务尾段。
mod baseline;

// `pub` 路径保持不变（原先从本文件导出）。
pub use self::baseline::{cancel_and_return_to_session_baseline, return_to_session_baseline};
// 父模块内部仍直接用这个私有 helper（`handle_stop_with_supervisor` / 会话路由）。
use self::baseline::baseline_value;

/// `POST /api/v1/chat` 解析体。
#[derive(Debug, Default, Deserialize)]
pub struct ChatBody {
    /// 用户输入文本。空串视为 `invalid_payload`（空消息没意义）。
    pub text: Option<String>,
    /// 会话 id（L1 基座）。缺省 = 不带会话（全局桶）。
    ///
    /// 容忍未知额外字段（serde 默认忽略），所以旧前端 / 第三方客户端照常可用。
    #[serde(default)]
    pub session_id: Option<String>,
}

/// `POST /api/v1/chat` 响应。
#[derive(Debug, Serialize)]
pub struct ChatAccepted {
    /// 是否已被 supervisor 接受。
    pub accepted: bool,
    /// 受理时 supervisor 的当前代次（前端在 WS 上等待此 epoch 的 turn_state）。
    pub epoch: u64,
    /// 通道容量 1；满时 429，否则 200。
    /// 保留字段，便于前端对账（不会暴露内部状态）。
    pub pending_cleared: bool,
    /// 本条新消息所属的会话（宿主活动会话；null = 不带会话 / 待机桶）。
    pub session_id: Option<String>,
    /// 该会话的 baseline（null = 缺省为空 = 待机）。**同一个请求**里回给前端，
    /// 前端不另发请求（V10 / O8）；形状 = 写入时的 JSON（对象或数组）。
    pub baseline: serde_json::Value,
}

/// `POST /api/v1/chat/stop` 响应。
#[derive(Debug, Serialize)]
pub struct StopAccepted {
    /// stop 命令已发出（不阻塞；supervisor 在 turn 边界处理）。
    pub accepted: bool,
    /// 被收口的会话（宿主活动会话；null = 不带会话 / 待机桶）。
    pub session_id: Option<String>,
    /// 该会话的 baseline（null = 缺省为空 = 待机）。
    pub baseline: serde_json::Value,
}

/// 停止 / 新消息的统一收口结果（V10 §9.3）。
///
/// 可测形态：`frame` 就是**发到 WS 的原文**（既有 `action_cue` 帧型，V11：
/// 不新增帧、不改既有键），`backfilled` 恒 `false`（V7 §6.6：时钟消失即同步
/// 取消，**不补帧**——host 侧没有任何队列可回放，这里把「没有补帧」写成结论字段
/// 而不是一句注释）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRevocation {
    /// 被收口的会话（宿主活动会话；`None` = 不带会话）。
    pub session: Option<String>,
    /// 该会话的 baseline（`None` = 缺省为空 = 待机）。
    pub baseline: Option<String>,
    /// 实际广播的 WS 帧原文（`None` = 调用方没给 broadcaster）。
    pub frame: Option<String>,
    /// 是否发生补帧；**恒 false**。
    pub backfilled: bool,
}

/// Chat handler（带 supervisor 句柄；dispatch 调用本版本）。
///
/// P1WS-1：`current_epoch` 参数**仅**在无 supervisor 句柄时作为占位（保持
/// 旧 `0` 回退）——有句柄时直接读 [`SupervisorHandle::current_epoch`]，使
/// 前端拿到的 epoch 与 WS 上 `turn_state` / `text_delta` 真正对齐。无句柄
/// 503 路径不读 epoch（不返回 body）。
pub fn handle_chat_with_supervisor(
    handle: Option<&std::sync::Arc<SupervisorHandle>>,
    broadcaster: Option<&crate::web_api::ws::Broadcaster>,
    method: &Method,
    path: &str,
    body: &str,
    current_epoch_unused: u64,
) -> Response<Cursor<Vec<u8>>> {
    if *method != Method::Post {
        return method_not_allowed(path, "POST");
    }
    let parsed: ChatBody = match serde_json::from_str(body) {
        Ok(p) => p,
        Err(_) => return invalid_payload("请求体不是合法 JSON 或缺字段"),
    };
    let text = match parsed.text {
        Some(t) if !t.is_empty() => t,
        _ => return invalid_payload("字段 text 必填且非空"),
    };
    let Some(handle) = handle else {
        return no_supervisor();
    };
    // V10 §9.3（阶段4e）：**新用户消息** = 取消上一轮编排 + 回该会话 baseline。
    //
    // 这里**不**注入 server 侧 Stop：`supervisor/turn.rs` 的 `do_stop!` 在收到
    // Stop 时会 `while say_rx.try_recv().is_ok() {}`（清 pending says）——那样这条
    // 新消息会被自己的 Stop 吞掉。新消息这条路径上的「清三样」由上一轮的取消事务
    // 与前端的 `StageCancellation` 承担；host 的职责是**回该会话 baseline**并
    // 保证**不补帧**（见报告未决）。
    //
    // 先把**本条消息**的会话落成活动会话（同一道 `sanitize_session_id` 闸；
    // `say_scoped` 内部也会再做一次，幂等），否则「回 baseline」读到的是上一条
    // 消息的会话——那正是「会话串台」。
    let session = parsed
        .session_id
        .as_deref()
        .and_then(live2d_ai_mod_system::sanitize_session_id);
    handle.set_active_session(session.as_deref());
    let revocation = return_to_session_baseline(handle, broadcaster, "new-message");
    // L1：非法 id 的结局是「这句话按全局桶处理」，而不是 400——前端的会话 id 是
    // 本地生成的，出现非法值属于「客户端版本不匹配」，不该让用户发不出消息。
    if handle.say_scoped(text, session) {
        // P1WS-1：从 supervisor 读取**真实**当前 epoch（root.epoch 镜像）。
        // `Acquire` 与 supervisor 写入的 `Release` 配对；这是无锁快速路径。
        let epoch = handle.current_epoch();
        let _ = current_epoch_unused; // 参数保留以兼容旧测试夹具签名
        let body = serde_json::to_vec(&ChatAccepted {
            accepted: true,
            epoch,
            pending_cleared: false,
            session_id: revocation.session,
            baseline: baseline_value(revocation.baseline.as_deref()),
        })
        .unwrap_or_default();
        json_response(StatusCode(200), body)
    } else {
        busy_response()
    }
}

/// Stop handler（`POST /api/v1/chat/stop`）。
pub fn handle_stop_with_supervisor(
    handle: Option<&std::sync::Arc<SupervisorHandle>>,
    broadcaster: Option<&crate::web_api::ws::Broadcaster>,
    method: &Method,
    path: &str,
) -> Response<Cursor<Vec<u8>>> {
    if *method != Method::Post {
        return method_not_allowed(path, "POST");
    }
    // V10 §9.3：停止键 = 清动作 + 表情 + TTS 待播 + 回该会话 baseline。
    // 前三样是 supervisor 的 stop 事务（epoch 推进 / 取消在飞轮 / 清 pending PCM /
    // StopPlayback，见 supervisor.rs）；这里只做第四样：把该会话 baseline 广播出去
    // 并在同一个响应里回给调用方。**不补帧**。
    let revocation = handle.map(|h| cancel_and_return_to_session_baseline(h, broadcaster, "stop"));
    // 无 supervisor 时按"幂等 stop"语义：返回 200（v1 不阻塞前端控制按钮）。
    let body = serde_json::to_vec(&StopAccepted {
        accepted: true,
        session_id: revocation.as_ref().and_then(|r| r.session.clone()),
        baseline: baseline_value(revocation.as_ref().and_then(|r| r.baseline.as_deref())),
    })
    .unwrap_or_default();
    json_response(StatusCode(200), body)
}

/// 会话路由路径（L1 基座）。
pub const CHAT_SESSION_PATH: &str = "/api/v1/chat/session";

/// `GET|POST /api/v1/chat/session`（L1 会话 id 宿主能力）。
///
/// - 路径不匹配 → `None`（交还 dispatch）。
/// - GET：只读，**不校验 Origin**（与其它只读 GET 一致）。
/// - POST：mutating，校验 `application/json` + loopback Origin（与
///   external/voice 注入端点同一条安全口径）。
/// - 无 supervisor → 503 `no_supervisor`（会话表住在 supervisor 里）。
pub fn handle_chat_session(
    ctx: &crate::web_api::ServerContext,
    method: &Method,
    path: &str,
    body: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
) -> Option<Response<Cursor<Vec<u8>>>> {
    if path != CHAT_SESSION_PATH {
        return None;
    }
    let Some(handle) = ctx.try_get_supervisor() else {
        return Some(no_supervisor());
    };
    let store = handle.session_scopes();
    match *method {
        Method::Get => {}
        Method::Post => {
            if !crate::web_api::security::is_json_content_type(content_type) {
                // 415（不是 400）：与 external/voice 注入端点同一条口径——
                // 「CT 不对」是协议层错误，前端据此能一眼分辨自己发错了什么。
                return Some(unsupported_media_type());
            }
            if !crate::web_api::security::is_allowed_origin(origin, &ctx.security) {
                let err = match origin {
                    Some(o) => crate::web_api::security::MutatingCheckError::BadOrigin {
                        got: Some(o.to_string()),
                    },
                    None => crate::web_api::security::MutatingCheckError::NoOrigin,
                };
                return Some(crate::web_api::security::mutating_check_error_response(
                    &err,
                ));
            }
            let parsed: SessionBody = match serde_json::from_str(body) {
                Ok(p) => p,
                Err(_) => return Some(invalid_payload("请求体不是合法 JSON")),
            };
            // 归一化后写入；非法 id 落成「清掉活动会话」而不是 400——
            // 与会话不该成为一条「只在特定字符下才失败」的路径（同 say_scoped）。
            let normalized = parsed
                .session_id
                .as_deref()
                .and_then(live2d_ai_mod_system::sanitize_session_id);
            handle.set_active_session(normalized.as_deref());
            // O8：baseline **随会话设置写**——必须与一个合法 session_id 同发。
            // 键缺席 = 本次不动 baseline；null = 撤销（缺省为空 = 待机）。
            if let Some(baseline) = parsed.baseline {
                let Some(id) = normalized.as_deref() else {
                    // 没有会话可绑：显式 400（写入方错误，不静默丢弃）。那条
                    // 「非法会话 id 不报错」的纪律属于**用户消息**路径；这是宿主 API。
                    return Some(invalid_payload(
                        "baseline 必须与合法的 session_id 同发（baseline 绑会话，没有全局 baseline）",
                    ));
                };
                match baseline {
                    // null = 显式撤销回待机（缺省为空 = 待机）。
                    serde_json::Value::Null => {
                        store.clear_baseline(id);
                    }
                    other => {
                        let text = serde_json::to_string(&other).unwrap_or_default();
                        if !store.set_baseline(id, &text) {
                            return Some(invalid_payload("baseline 被拒绝（单会话超长）"));
                        }
                    }
                }
            }
        }
        _ => {
            return Some(method_not_allowed(path, "GET/POST"));
        }
    }
    let active = store.active();
    let payload = ChatSessionState {
        ok: true,
        active_baseline: baseline_value(store.baseline_for(active.as_deref()).as_deref()),
        active_session: active,
        bound_sessions: store.len(),
        baselines: store.baseline_count(),
        baseline_sessions: store.baseline_sessions(),
        sessions: store.sessions(),
    };
    Some(json_response(
        StatusCode(200),
        serde_json::to_vec(&payload).unwrap_or_default(),
    ))
}

/// `POST /api/v1/chat/session` 请求体。
#[derive(Debug, Default, Deserialize)]
struct SessionBody {
    /// 目标会话 id；缺省 / 空 / 非法 = 清掉活动会话。
    #[serde(default)]
    session_id: Option<String>,
    /// 会话 baseline（V10 / O8）。**「键缺席」与「键为 null」必须可区分**：
    /// 缺席 = `None`（本次不动）；`null` = `Some(Value::Null)`（撤销回待机）。
    ///
    /// 为什么不能直接写 `Option<Value>`：serde 对 `Option` 的默认语义把
    /// `null` 也读成 `None`——那样「撤销」会被误解成「不动」，缺省为空的待机
    /// 就永远清不掉（这正是本测试抓到的那条）。
    #[serde(default, deserialize_with = "deserialize_present_value")]
    baseline: Option<serde_json::Value>,
}

/// 把**键存在**的任意 JSON（含 `null`）读成 `Some(..)`；键缺席由
/// `#[serde(default)]` 给出 `None`。见 [SessionBody::baseline]。
fn deserialize_present_value<'de, D>(de: D) -> Result<Option<serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde_json::Value::deserialize(de).map(Some)
}

/// `GET|POST /api/v1/chat/session` 响应。
#[derive(Debug, Serialize)]
struct ChatSessionState {
    ok: bool,
    /// 宿主记录的当前活动会话（None = 没设过）。
    active_session: Option<String>,
    /// 已绑定人设/记忆的会话条数（**prompt 槽**的会话数）。
    bound_sessions: usize,
    /// 已绑定的会话 id 列表（稳定排序）。
    sessions: Vec<String>,
    /// 活动会话的 baseline（null = 缺省为空 = 待机）。
    active_baseline: serde_json::Value,
    /// 有 baseline 的会话数（兄弟字段的独立计数）。
    baselines: usize,
    /// 有 baseline 的会话 id 列表（稳定排序）——面板可显示「哪些会话有基准」。
    baseline_sessions: Vec<String>,
}

// ---------------------------------------------------------------- 内部响应

fn json_response(status: StatusCode, body: Vec<u8>) -> Response<Cursor<Vec<u8>>> {
    Response::from_data(body)
        .with_status_code(status)
        .with_header(
            tiny_http::Header::from_bytes(
                &b"Content-Type"[..],
                &b"application/json; charset=utf-8"[..],
            )
            .expect("Content-Type header"),
        )
}

fn invalid_payload(msg: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new("invalid_payload", msg);
    json_response(
        StatusCode(400),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn method_not_allowed(path: &str, expected: &str) -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new("method_not_allowed", format!("{path:?} 需 {expected} 方法"));
    json_response(
        StatusCode(405),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn busy_response() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(
        "busy",
        "已有 pending 输入（Say 通道容量 1），请等待 turn_state 收口后再发",
    );
    json_response(
        StatusCode(429),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn unsupported_media_type() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(
        "unsupported_media_type",
        "Content-Type 必须为 application/json",
    );
    json_response(
        StatusCode(415),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

fn no_supervisor() -> Response<Cursor<Vec<u8>>> {
    let detail = ErrorDetail::new(
        "no_supervisor",
        "web 模式未装配 supervisor（纯控制平面）；请先在 live2d-ai.toml 配 LLM/TTS 后重启",
    );
    json_response(
        StatusCode(503),
        serde_json::to_vec(&ErrorResponse::new(detail))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}").into_bytes()),
    )
}

// ---------------------------------------------------------------- 单元测试

// 回归测试拆到同目录 `chat_routes_tests_*.rs`（保持本文件 ≤500 行；与
// `voice_routes_tests.rs` 同模式）。
#[cfg(test)]
#[path = "chat_routes_tests_baseline.rs"]
mod tests_baseline;
#[cfg(test)]
#[path = "chat_routes_tests_basic.rs"]
mod tests_basic;
#[cfg(test)]
#[path = "chat_routes_tests_session.rs"]
mod tests_session;
#[cfg(test)]
#[path = "chat_routes_tests_support.rs"]
mod tests_support;
