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
/// - **已实现**：`turn_state` / `runtime_status` / `text_delta` /
///   `reasoning_delta` / `text_fallback` / **`error`**。
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
/// **思考上屏闸门**（2026-09-15 产品开关 `llm.show_reasoning`，缺省 `false`）。
///
/// `false` → [`ConversationUiEvent::ReasoningDelta`] **不投影成 WS 帧**，
/// 前端因此看不到思考区；其余事件（正文 / 音频 / 状态 / **错误**）一律照常。
///
/// 为什么闸门放在这里（投影层）而不是引擎里：
/// - 引擎的契约是「思考单列一类事件、绝不进句子装配器、绝不进 TTS」
///   （见 `live2d_ai_runtime` 的 `reasoning_never_reaches_the_sentence_assembler`），
///   那条纪律与「要不要展示」是两件事——本开关只决定**上不上屏**；
/// - 放在投影层，`AppEvent` 生产者与 core reducer 一行不动，
///   也不会让「关掉展示」变成「关掉解析」（思考与正文共用 `max_tokens`，
///   上游该发的一个 token 不少——这不是性能开关）。
pub fn should_broadcast(event: &AppEvent, show_reasoning: bool) -> bool {
    show_reasoning
        || !matches!(
            event,
            AppEvent::Conversation(ConversationUiEvent::ReasoningDelta { .. })
        )
}

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
        AppEvent::Conversation(ConversationUiEvent::TextDelta {
            epoch,
            ts_ms,
            sentence_seq,
            text,
        }) => {
            // 一句一单元的上屏正文。`sentence_seq` 与音频帧同源；完成锚点
            // （上面的 GenerationFinished）不带这个字段。
            frame.set_type("text_delta");
            frame.set_data(serde_json::json!({
                "epoch": epoch,
                "ts_ms": ts_ms,
                "sentence_seq": sentence_seq,
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
        AppEvent::Conversation(ConversationUiEvent::TextFallback { epoch, ts_ms, text }) => {
            // **失败路径的正文兜底**（rc.3 N0，2026-09-13）。独立帧类型：
            // `text_delta` 的消费语义是「追加」且与音频同拍，而本帧要求前端
            // **整段设置**并标注「未收尾」——混用 type 会让前端把兜底当增量追加，
            // 正文出现两遍。未知 type 在旧客户端被忽略（`default: break`），
            // 所以这是向后兼容的新增。
            frame.set_type("text_fallback");
            frame.set_data(serde_json::json!({
                "epoch": epoch,
                "ts_ms": ts_ms,
                "text": text,
            }));
        }
        AppEvent::Conversation(ConversationUiEvent::ActionCue {
            epoch,
            ts_ms,
            covers_upto_seq,
            cues,
        }) => {
            // **复用既有 `action_cue` 帧类型**（2026-09-22；v1 沿用，V11）：
            // 表演层的 cues 与 director Mod 的 cues 是同一个 wire 契约。
            // 单条 cue 的 JSON 形态由 runtime 的 `PerformanceCue::to_json` 给出：
            // **既有键** sentence_seq/preset_id/intensity/ttl_ms/priority 逐字保留，
            // v1 的**新键**（field/seq/x/y/z/id/at/hold）以可选键摊平（只增不改）。
            // 这里不重抄一遍字段名。
            //
            // 回归：`performance_cues_project_to_the_existing_action_cue_frame`。
            frame.set_type("action_cue");
            frame.set_data(serde_json::json!({
                "epoch": epoch,
                "ts_ms": ts_ms,
                "covers_upto_seq": covers_upto_seq,
                "cues": cues.iter().map(|c| c.to_json()).collect::<Vec<_>>(),
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
        | AppEvent::RootAudit(RootFact::Dropped) => {
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
/// P1-3（2026-09-16）：把导演 cue 的 payload 封成 WS action_cue 帧。
///
/// 与其余 WS 帧**同形**：只有 type + data（seq/ts 由 per-connection actor 在
/// wrap_preserialized_with_seq 里加）。**必须是 data 而不是 payload**——前端
/// parseWsFrame 只读 data；写错字段会让 cue 静默丢失（本函数有回归）。
pub fn action_cue_frame(cue: &Value) -> String {
    serde_json::json!({ "type": "action_cue", "data": cue }).to_string()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// action_cue 帧必须是 type + data（前端 parseWsFrame 只读 data）。
    #[test]
    fn action_cue_frame_uses_type_and_data() {
        let cue = serde_json::json!({
            "epoch": 7,
            "covers_upto_seq": 2,
            "cues": [{"sentence_seq": 1, "preset_id": "nod", "intensity": 2, "ttl_ms": 1800, "priority": 40}],
        });
        let raw = action_cue_frame(&cue);
        let value: Value = serde_json::from_str(&raw).expect("必须是合法 JSON");
        assert_eq!(value["type"], "action_cue");
        assert_eq!(value["data"]["epoch"], 7);
        assert_eq!(value["data"]["cues"][0]["preset_id"], "nod");
        assert!(
            value.get("payload").is_none(),
            "字段名必须是 data（不是 payload），否则前端读不到"
        );
    }

    /// **表演层 ActionCue → 既有 action_cue 帧**（2026-09-22）：帧型与字段与
    /// director Mod 那条**逐字同形**——前端 `ActionCueEvent` 一行不用改。
    #[test]
    fn performance_action_cue_projects_to_the_same_action_cue_frame() {
        let ev = AppEvent::Conversation(ConversationUiEvent::ActionCue {
            epoch: 9,
            ts_ms: 321,
            covers_upto_seq: 2,
            cues: vec![live2d_ai_runtime::performance::PerformanceCue {
                sentence_seq: 2,
                preset_id: "smile".to_string(),
                intensity: 2,
                ttl_ms: 1_500,
                ..Default::default()
            }],
        });
        let frame = app_event_to_ws_frame(&ev).expect("必须投影成帧");
        assert_eq!(frame["type"], "action_cue");
        assert_eq!(frame["data"]["epoch"], 9);
        assert_eq!(frame["data"]["covers_upto_seq"], 2);
        assert_eq!(frame["data"]["cues"][0]["sentence_seq"], 2);
        assert_eq!(frame["data"]["cues"][0]["preset_id"], "smile");
        assert_eq!(frame["data"]["cues"][0]["intensity"], 2);
        assert_eq!(frame["data"]["cues"][0]["ttl_ms"], 1_500);
        assert_eq!(
            frame["data"]["cues"][0]["priority"],
            live2d_ai_runtime::performance::PRIORITY_PERFORMANCE
        );
    }

    /// **B9（v1）**：v1 cue → **既有** `action_cue` 帧——不新增帧型、不改既有键，
    /// 只把 v1 新键（field/seq/x/y/z/id/at/hold）以**可选键**摊平进每条 cue JSON。
    #[test]
    fn performance_cues_project_to_the_existing_action_cue_frame() {
        let source = "嗯……我想到了。";
        let plan = live2d_ai_runtime::performance::parse_plan(
            r#"{"segments":["嗯……","我想到了。"],"cues":[
                {"field":"head","x":0.2,"y":-0.4,"z":0.1,"intensity":2,"at":"seg:2","hold":false,"ttl_ms":800},
                {"field":"expression","id":"smile","intensity":1,"at":"now","hold":true}
            ]}"#,
            &["smile".to_string()],
            source,
        )
        .expect("v1 plan 必须合法");
        let ev = AppEvent::Conversation(ConversationUiEvent::ActionCue {
            epoch: 12,
            ts_ms: 99,
            covers_upto_seq: 2,
            cues: plan.cues.clone(),
        });
        let frame = app_event_to_ws_frame(&ev).expect("必须投影成帧");
        assert_eq!(frame["type"], "action_cue");
        assert_eq!(frame["data"]["epoch"], 12);
        assert_eq!(frame["data"]["covers_upto_seq"], 2);
        // 第 1 条 head cue：既有键逐字保留 + v1 新键。
        let first = &frame["data"]["cues"][0];
        assert_eq!(first["sentence_seq"], 2);
        assert_eq!(first["preset_id"], "head");
        assert_eq!(first["intensity"], 2);
        assert_eq!(first["ttl_ms"], 800);
        assert_eq!(
            first["priority"],
            live2d_ai_runtime::performance::PRIORITY_PERFORMANCE
        );
        assert_eq!(first["field"], "head");
        assert_eq!(first["seq"], 1);
        assert_eq!(first["x"], 0.2);
        assert_eq!(first["y"], -0.4);
        assert_eq!(first["z"], 0.1);
        assert_eq!(first["at"], "seg:2");
        assert_eq!(first["hold"], false);
        // 第 2 条 expression cue：id 就是 preset_id（V11 兼容面），默认 ttl 生效。
        let second = &frame["data"]["cues"][1];
        assert_eq!(second["field"], "expression");
        assert_eq!(second["id"], "smile");
        assert_eq!(second["preset_id"], "smile");
        assert_eq!(second["sentence_seq"], 1);
        assert_eq!(second["at"], "now");
        assert_eq!(second["hold"], true);
        assert_eq!(
            second["ttl_ms"],
            live2d_ai_runtime::performance::DEFAULT_TTL_MS_EXPRESSION
        );
    }
}
