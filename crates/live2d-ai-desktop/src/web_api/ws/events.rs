//! WS 帧投影（`AppEvent` → D1 §4.3 JSON 帧）+ 帧构造器。
//!
//! P1WS-1 拆分：ws.rs 主文件因为新增 `text_delta` 真实 text 投影超过 500
//! 行约束，本文件承接**纯函数投影**（`app_event_to_ws_frame`）+ `WsFrame`
//! 内部结构。ws.rs 留作 broadcaster / 升级 / 错误响应承载。
//!
//! 不变量：
//! - 不引入 IO（pure transform）；
//! - 帧 schema 与 ws.rs 顶部 doc 保持一致（type/seq/ts/data）；
//! - 测 `ws_unit_tests::text_delta_*` 仍通过 `super::ws::app_event_to_ws_frame`
//! 访问（ws.rs 保留 `pub use` 转发，调用方代码不变）。

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde_json::Value;

use crate::app_event::{AppEvent, ConversationUiEvent, RootFact};

/// 把 [`AppEvent`] 投影为 D1 §4.3 WS 帧 JSON（`None` = P1 暂不实现）。
///
/// **状态（2026-09-11 更新）**：
/// - **已实现**：`turn_state` / `runtime_status` / `text_delta` / **`error`**。
///   `error` 曾长期标为「P1 partial：依赖 `AppEvent` 扩展」——2026-09-11
///   补上 `AppEvent::Error` 变体后接线（用户报「后端出错无具体错误代码、
///   前端无法知道错误信息」，根因就是这里从来没有 `error` 帧）。
/// - **已移除**：`action_state`（来自 `AppEvent::Render` 投影）——2026-09-11
///   用户裁决「LLM 不暴露任何工具，只做对话」，动作系统整体拆除后
///   `AppEvent::Render` / `RenderCommand` 不再存在，本投影随之删除。
/// - **P1 pending（需要 supervisor 信号源）**：`audio_status` /
///   `mouth_level` / `model_status` 仍 P1。
///
/// v1: `turn_state` (TurnCompleted) / `runtime_status` (Playback* +
/// Conversation(Voice*|NewEpoch) + ShutdownReady) / `text_delta`
///（含 Conversation(TextDelta) 真实 text payload；保留
/// GenerationFinished 作为「完成锚点」兼容旧前端）。
/// 2026-09-11 新增：`error`（AppEvent::Error 投影；见下）。
pub fn app_event_to_ws_frame(event: &AppEvent) -> Option<Value> {
    let mut frame = WsFrame::new();
    match event {
        AppEvent::RootAudit(RootFact::TurnCompleted {
            epoch,
            outcome_completed,
        }) => {
            frame.set_type("turn_state");
            frame.set_data(serde_json::json!({
                "epoch": epoch,
                "status": if *outcome_completed { "completed" } else { "failed" },
            }));
        }
        AppEvent::RootAudit(RootFact::PlaybackStarted { epoch }) => {
            frame.set_type("runtime_status");
            frame.set_data(serde_json::json!({
                "event": "voice_started",
                "epoch": epoch,
            }));
        }
        AppEvent::RootAudit(RootFact::PlaybackDrained { epoch })
        | AppEvent::RootAudit(RootFact::PlaybackCleared { epoch }) => {
            frame.set_type("runtime_status");
            frame.set_data(serde_json::json!({
                "event": "voice_ended",
                "epoch": epoch,
            }));
        }
        AppEvent::RootAudit(RootFact::GenerationFinished { epoch, completed }) => {
            // v1：用 GenerationFinished 代理 text_delta 完成锚点。
            // text_delta 帧**现在**带真实 text payload（来自
            // supervisor 透传 `ConversationUiEvent::TextDelta`）。本分支仍保留
            // 作为「完成锚点」——`completed=true` 时前端可用作流式气泡收口。
            frame.set_type("text_delta");
            frame.set_data(serde_json::json!({
                "epoch": epoch,
                "completed": completed,
            }));
        }
        AppEvent::Conversation(ConversationUiEvent::TextDelta { epoch, ts_ms, text }) => {
            // 真实 LLM 流式文本投影。前端按 epoch 追加到气泡 body。
            frame.set_type("text_delta");
            frame.set_data(serde_json::json!({
                "epoch": epoch,
                "ts_ms": ts_ms,
                "text": text,
            }));
        }
        AppEvent::Conversation(ConversationUiEvent::ReasoningDelta { epoch, ts_ms, text }) => {
            // **独立帧类型**（不复用 `text_delta`）：两者的消费语义不同——
            // `text_delta` 与音频同拍、会追加到气泡正文；思考只进「思考」折叠区，
            // 且可能比正文长得多。用同一个 type 加标志位会让前端每条都要分支判断，
            // 而且旧前端会把思考当正文追加（那是**错的**，等于把内心独白当回复）。
            // 未知 type 在旧客户端被忽略（`default: break`），所以这是向后兼容的新增。
            frame.set_type("reasoning_delta");
            frame.set_data(serde_json::json!({
                "epoch": epoch,
                "ts_ms": ts_ms,
                "text": text,
            }));
        }
        AppEvent::Conversation(ConversationUiEvent::VoiceStarted { epoch }) => {
            frame.set_type("runtime_status");
            frame.set_data(serde_json::json!({
                "event": "voice_started",
                "epoch": epoch,
            }));
        }
        AppEvent::Conversation(ConversationUiEvent::VoiceEnded { epoch }) => {
            frame.set_type("runtime_status");
            frame.set_data(serde_json::json!({
                "event": "voice_ended",
                "epoch": epoch,
            }));
        }
        AppEvent::Conversation(ConversationUiEvent::NewEpoch { epoch }) => {
            frame.set_type("runtime_status");
            frame.set_data(serde_json::json!({
                "event": "new_epoch",
                "epoch": epoch,
            }));
        }
        AppEvent::ShutdownReady => {
            frame.set_type("runtime_status");
            frame.set_data(serde_json::json!({ "event": "shutdown_ready" }));
        }
        // 2026-09-11：链路错误 → `error` 帧。
        //
        // 帧形状（前端 `WsErrorEvent` 消费；`data.code` 是**契约**）：
        // `{"type":"error","data":{"code":"llm_upstream_401","stage":"llm",
        //   "message":"LLM 阶段错误: 上游非成功状态 401 …",
        //   "hint":"确认 [llm] api_key_env …","epoch":1,"fatal":false}}`
        //
        // `message` 与 `hint` 都可能是空串/null（旧服务端没有 hint 字段，
        // 前端按缺省处理）；`code` **永不**为空——它是前端唯一可靠的锚点。
        AppEvent::Error(err) => {
            frame.set_type("error");
            frame.set_data(serde_json::json!({
                "code": err.code,
                "stage": err.stage,
                "message": err.message,
                "hint": err.hint,
                "epoch": err.epoch,
                "fatal": err.fatal,
            }));
        }
        // P1 pending（需要 supervisor 信号源）：
        AppEvent::RootAudit(RootFact::TurnStages { .. })
        | AppEvent::RootAudit(RootFact::Dropped)
        | AppEvent::Tray(_) => {
            return None;
        }
    }
    Some(frame.into_value())
}

// ---------------------------------------------------------------- 内部辅助

/// WS 帧构造器：构造 **typed event**（`{"type":..., "data":{...}}`，**无**
/// seq/ts）。seq/ts 由 [`super::connection::wrap_typed_with_seq`] 在发送
/// 前包装（每连接独立 seq）。
///
/// **P1WS-2 变更**：原 v1 全局 `static SEQ: AtomicU64` 删除——seq 由
/// `ConnectionState::alloc_seq()` 维护。
#[derive(Debug, Serialize)]
pub(crate) struct WsFrame<'a> {
    r#type: &'a str,
    data: Value,
}

impl<'a> WsFrame<'a> {
    fn new() -> Self {
        Self {
            r#type: "unknown",
            data: Value::Null,
        }
    }
    fn set_type(&mut self, t: &'a str) {
        self.r#type = t;
    }
    fn set_data(&mut self, d: Value) {
        self.data = d;
    }
    fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

/// ISO 8601 UTC 毫秒精度（无 chrono 依赖）。
pub(crate) fn iso8601_now_ms() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let ms = now.subsec_millis();
    let (year, mon, day, h, m, s) = epoch_secs_to_ymdhms(secs);
    format!("{year:04}-{mon:02}-{day:02}T{h:02}:{m:02}:{s:02}.{ms:03}Z")
}

/// 1970-01-01 起累计秒 → (年, 月, 日, 时, 分, 秒)。Gregorian 公历换算。
pub(crate) fn epoch_secs_to_ymdhms(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let s = (secs % 60) as u32;
    let m = ((secs / 60) % 60) as u32;
    let h = ((secs / 3600) % 24) as u32;
    let days = (secs / 86_400) as i64;
    // Howard Hinnant 算法（公历天数 → ymd）。
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (year as u32, month as u32, d as u32, h, m, s)
}
