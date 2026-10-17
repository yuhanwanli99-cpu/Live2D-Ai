//! WS ack / 事件投影 + broadcaster 槽位簿记测试（自 `tests_ws.rs` 机械拆出）。
//!
//! 入口：`web_api::mod tests_ws_acks;`（仅 `#[cfg(test)]` 下生效）。

/// 客户端手工写 1 条文本 WS 帧（client→server 必 mask；mask key 4 字节）。
///
/// **仅支持短帧（len < 126）**。
#[allow(dead_code)]
fn write_client_text_frame(client: &mut std::net::TcpStream, text: &str) {
    use std::io::Write as _;
    let payload = text.as_bytes();
    let len = payload.len();
    assert!(len < 126, "this test only supports short frames (len<126)");
    // 简单 mask key: 0x00 0x00 0x00 0x01（仅测试用，不要求真随机）。
    let mask_key: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
    let masked: Vec<u8> = payload
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ mask_key[i & 3])
        .collect();
    let mut buf = Vec::with_capacity(2 + 4 + len);
    // FIN=1, opcode=1 (text), MASK=1, len byte
    buf.push(0x81);
    buf.push(0x80 | (len as u8));
    buf.extend_from_slice(&mask_key);
    buf.extend_from_slice(&masked);
    client.write_all(&buf).expect("client write frame");
    client.flush().expect("client flush");
}

// ============================================================================
// ws.rs 单元测试（2026-08-29 D5 修复后从 ws.rs `mod tests` 拆出，保持主体 ≤500）
// ============================================================================

#[cfg(test)]
mod ws_unit_tests {
    use crate::app_event::{AppEvent, ConversationUiEvent, RootFact};
    use crate::web_api::ws::{
        Broadcaster, WS_CONN_LIMIT, app_event_to_ws_frame, epoch_secs_to_ymdhms, iso8601_now_ms,
        should_broadcast,
    };

    #[test]
    fn epoch_to_ymdhms_matches_known_anchors() {
        assert_eq!(epoch_secs_to_ymdhms(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(epoch_secs_to_ymdhms(946_684_800), (2000, 1, 1, 0, 0, 0));
        let (y, _, _, _, _, _) = epoch_secs_to_ymdhms(1_787_990_400);
        assert_eq!(y, 2026);
    }

    #[test]
    fn iso8601_now_ms_shape() {
        let s = iso8601_now_ms();
        assert!(s.ends_with('Z'), "got: {s}");
        assert!(s.len() >= 24, "got: {s}");
    }

    #[test]
    fn turn_completed_maps_to_turn_state_completed() {
        let ev = AppEvent::RootAudit(RootFact::TurnCompleted {
            epoch: 7,
            outcome_completed: true,
        });
        let v = app_event_to_ws_frame(&ev).expect("must map");
        assert_eq!(v["type"], "turn_state");
        assert_eq!(v["data"]["epoch"], 7);
        assert_eq!(v["data"]["status"], "completed");
        // P1WS-2：seq/ts **不**再由 typed event 携带——由 actor 在
        // connection::wrap_preserialized_with_seq 处包装（每连接独立 seq）。
        assert!(v.get("seq").is_none(), "typed event 不应含 seq");
        assert!(v.get("ts").is_none(), "typed event 不应含 ts");
    }

    #[test]
    fn turn_completed_failed_maps_to_failed_status() {
        let ev = AppEvent::RootAudit(RootFact::TurnCompleted {
            epoch: 7,
            outcome_completed: false,
        });
        let v = app_event_to_ws_frame(&ev).expect("must map");
        assert_eq!(v["data"]["status"], "failed");
    }

    #[test]
    fn conversation_voice_events_map_to_runtime_status() {
        let s = app_event_to_ws_frame(&AppEvent::Conversation(ConversationUiEvent::VoiceStarted {
            epoch: 3,
        }))
        .expect("must map");
        assert_eq!(s["type"], "runtime_status");
        assert_eq!(s["data"]["event"], "voice_started");
        assert_eq!(s["data"]["epoch"], 3);

        let e = app_event_to_ws_frame(&AppEvent::Conversation(ConversationUiEvent::VoiceEnded {
            epoch: 3,
        }))
        .expect("must map");
        assert_eq!(e["data"]["event"], "voice_ended");
    }

    #[test]
    fn new_epoch_and_shutdown_map_to_runtime_status() {
        let ne = app_event_to_ws_frame(&AppEvent::Conversation(ConversationUiEvent::NewEpoch {
            epoch: 9,
        }))
        .expect("must map");
        assert_eq!(ne["data"]["event"], "new_epoch");
        assert_eq!(ne["data"]["epoch"], 9);

        let sd = app_event_to_ws_frame(&AppEvent::ShutdownReady).expect("must map");
        assert_eq!(sd["data"]["event"], "shutdown_ready");
    }

    #[test]
    fn generation_finished_maps_to_text_delta() {
        let ev = AppEvent::RootAudit(RootFact::GenerationFinished {
            epoch: 4,
            completed: true,
        });
        let v = app_event_to_ws_frame(&ev).expect("must map");
        assert_eq!(v["type"], "text_delta");
        assert_eq!(v["data"]["epoch"], 4);
        assert_eq!(v["data"]["completed"], true);
        assert!(v["data"].get("sentence_seq").is_none(), "完成锚点不带句号");
    }

    /// 2026-09-13：思考（`reasoning_content`）投影成**独立**帧类型
    /// `reasoning_delta`。
    ///
    /// 为什么不能复用 `text_delta`：`text_delta` 的正文与音频同拍、会被追加到
    /// 气泡正文；思考没有声音可对，且长得像内心独白——混进正文就等于
    /// 「把模型的自言自语当回复显示」（而且旧前端会把它当正文）。
    /// 独立 type 也让旧客户端自然忽略（`default: break`），是向后兼容的新增。
    #[test]
    fn conversation_reasoning_delta_maps_to_its_own_frame() {
        let ev = AppEvent::Conversation(ConversationUiEvent::ReasoningDelta {
            epoch: 3,
            ts_ms: 42,
            text: "先看题目…".to_string(),
        });
        let v = app_event_to_ws_frame(&ev).expect("must map");
        assert_eq!(v["type"], "reasoning_delta");
        assert_ne!(v["type"], "text_delta", "思考不得复用正文帧类型");
        assert_eq!(v["data"]["epoch"], 3);
        assert_eq!(v["data"]["ts_ms"], 42);
        assert_eq!(v["data"]["text"], "先看题目…");
    }

    /// **展示思考总闸**（2026-09-15，`llm.show_reasoning`，缺省 false）：
    /// 关 = 思考**不上屏**；开 = 照常出帧。
    ///
    /// 「上屏」的唯一通路就是这条投影，所以闸门断言就落在这里。
    /// 注意别把断言写成「关掉思考 = 引擎不解析」——解析层与展示层是两件事：
    /// 思考与正文共用 `max_tokens`，关掉展示**不省 token**。
    #[test]
    fn reasoning_frame_is_gated_by_show_reasoning() {
        let reasoning = AppEvent::Conversation(ConversationUiEvent::ReasoningDelta {
            epoch: 1,
            ts_ms: 1,
            text: "内心独白".to_string(),
        });
        let text = AppEvent::Conversation(ConversationUiEvent::TextDelta {
            epoch: 1,
            ts_ms: 2,
            sentence_seq: 1,
            text: "正文".to_string(),
        });
        let err = AppEvent::Error(crate::app_event::AppErrorEvent {
            code: "llm_upstream_401".to_string(),
            stage: "llm".to_string(),
            message: "unauthorized".to_string(),
            hint: None,
            epoch: 1,
            fatal: true,
        });

        // 默认（false）：思考不上屏。
        assert!(!should_broadcast(&reasoning, false), "关：思考帧不得下发");
        // 开：思考照常。
        assert!(should_broadcast(&reasoning, true), "开：思考帧应下发");
        // 其余事件不受开关影响——尤其**错误**帧，任何情况下都要能看见。
        for on in [false, true] {
            assert!(should_broadcast(&text, on), "正文不受思考开关影响");
            assert!(should_broadcast(&err, on), "错误帧不受思考开关影响");
        }
        // 投影本身**照旧**能产出 reasoning_delta 帧（闸门在调用方）；
        // 这条断言把两件事钉开：关掉展示 ≠ 关掉解析/投影。
        assert_eq!(
            app_event_to_ws_frame(&reasoning).expect("投影仍在")["type"],
            "reasoning_delta"
        );
    }

    /// **rc.3 N0（2026-09-13）**：失败轮的正文兜底投影成**独立**帧类型
    /// `text_fallback`。
    ///
    /// 为什么不能复用 `text_delta`：后者的消费语义是「**追加**到气泡正文」
    /// 且与音频同拍；兜底要求前端「**整段设置**」并标注「未收尾」——混用 type 会
    /// 让正文出现两遍。独立 type 也让旧客户端自然忽略（`default: break`）。
    ///
    /// 帧形状（与 `reasoning_delta` 同构，宁少字段）：
    /// `{"type":"text_fallback","data":{"epoch":N,"ts_ms":M,"text":"…"}}`
    #[test]
    fn conversation_text_fallback_maps_to_its_own_frame() {
        let ev = AppEvent::Conversation(ConversationUiEvent::TextFallback {
            epoch: 9,
            ts_ms: 880,
            text: "第一句。第二".to_string(),
        });
        let v = app_event_to_ws_frame(&ev).expect("must map");
        assert_eq!(v["type"], "text_fallback");
        assert_ne!(v["type"], "text_delta", "兜底不得复用正文帧类型");
        assert_ne!(v["type"], "error", "兜底不是错误帧（错误另有一条）");
        assert_eq!(v["data"]["epoch"], 9);
        assert_eq!(v["data"]["ts_ms"], 880);
        assert_eq!(v["data"]["text"], "第一句。第二");
        // 宁少字段：不发明 turn_id 之类的跨层新字段。
        assert!(v["data"].get("turn_id").is_none());
    }

    /// 2026-09-11 回归：链路错误**必须**投影成 `error` 帧，且带机器可读的
    /// `code`。此前 `AppEvent` 没有 Error 变体、投影表里也没有这一支——
    /// 前端因此永远收不到错误详情（用户原话：「前端无法知道错误信息」）。
    #[test]
    fn app_error_maps_to_error_frame_with_code() {
        use crate::app_event::AppErrorEvent;
        use live2d_ai_runtime::{Error as RtError, ErrorKind};

        // 直接构造变体：`Error::status` 是 runtime 内部构造器（pub(crate)），
        // 跨 crate 测试用公开字段即可。
        let kind = ErrorKind::Llm(RtError::Status {
            status: reqwest::StatusCode::UNAUTHORIZED,
            body: "Authentication Fails (governor)".to_string(),
        });
        let ev = AppEvent::Error(AppErrorEvent::from_kind(&kind, 3));
        let v = app_event_to_ws_frame(&ev).expect("error 必须投影（不能是 None）");
        assert_eq!(v["type"], "error");
        assert_eq!(v["data"]["code"], "llm_upstream_401");
        assert_eq!(v["data"]["stage"], "llm");
        assert_eq!(v["data"]["epoch"], 3);
        assert_eq!(v["data"]["fatal"], false, "LLM 失败不致命");
        let message = v["data"]["message"].as_str().expect("message 必为字符串");
        assert!(message.contains("401"), "message 要含状态码: {message}");
        let hint = v["data"]["hint"].as_str().expect("401 必须带处置提示");
        assert!(hint.contains("api_key_env"), "hint: {hint}");
    }

    /// 致命错误（TTS）在帧里必须标 `fatal: true`——前端据此不再等这一轮。
    #[test]
    fn fatal_error_frame_flags_fatal() {
        use crate::app_event::AppErrorEvent;
        use live2d_ai_runtime::{Error as RtError, ErrorKind};

        let kind = ErrorKind::Tts(RtError::Status {
            status: reqwest::StatusCode::BAD_REQUEST,
            body: "bad voice".to_string(),
        });
        let v = app_event_to_ws_frame(&AppEvent::Error(AppErrorEvent::from_kind(&kind, 8)))
            .expect("must map");
        assert_eq!(v["data"]["code"], "tts_upstream_400");
        assert_eq!(v["data"]["fatal"], true);
    }

    /// P1WS-1：Conversation(TextDelta) → text_delta + text payload。
    /// 真实 LLM 流式正文片段路径——前端按 epoch 追加到气泡。
    #[test]
    fn conversation_text_delta_maps_to_text_delta_with_text() {
        let ev = AppEvent::Conversation(ConversationUiEvent::TextDelta {
            epoch: 7,
            ts_ms: 123,
            sentence_seq: 2,
            text: "你好".to_string(),
        });
        let v = app_event_to_ws_frame(&ev).expect("must map");
        assert_eq!(v["type"], "text_delta");
        assert_eq!(v["data"]["epoch"], 7);
        assert_eq!(v["data"]["ts_ms"], 123);
        assert_eq!(v["data"]["sentence_seq"], 2);
        assert_eq!(v["data"]["text"], "你好");
        // 兼容兜底：text_delta 帧**不**带 completed（仅 GenerationFinished
        // 路径有；前端在 text_delta 命中 + 后续 turn_state 到达时收口）。
        assert!(v["data"].get("completed").is_none());
    }

    /// P1WS-1：空 text payload（极端：引擎发出空增量）→ text_delta 帧
    /// **不**丢失，但 text 字段为 ""。前端 appendAssistantDelta 守
    /// 卫为 falsy，跳过追加。
    #[test]
    fn conversation_text_delta_empty_text_still_emits_frame() {
        let ev = AppEvent::Conversation(ConversationUiEvent::TextDelta {
            epoch: 0,
            ts_ms: 0,
            sentence_seq: 1,
            text: String::new(),
        });
        let v = app_event_to_ws_frame(&ev).expect("must map");
        assert_eq!(v["type"], "text_delta");
        assert_eq!(v["data"]["text"], "");
    }

    #[test]
    fn p1_events_return_none() {
        // 仍 P1 pending 的事件返回 None（Dropped / TurnStages / Tray）。
        // `action_state` 的投影已随动作系统整体删除（2026-09-11 用户裁决），
        // `AppEvent::Render` 变体本身不复存在。
        assert!(app_event_to_ws_frame(&AppEvent::RootAudit(RootFact::Dropped)).is_none());
    }

    #[test]
    fn broadcaster_basic_subscribe_and_broadcast() {
        let bc = Broadcaster::new();
        assert_eq!(bc.subscriber_count(), 0);
        bc.broadcast(r#"{"type":"x","data":{}}"#);
        bc.broadcast(r#"{"type":"y","data":{}}"#);
        assert_eq!(bc.subscriber_count(), 0);
        bc.clear();
        assert_eq!(bc.subscriber_count(), 0);
    }

    /// D5：try_subscribe 超出 `WS_CONN_LIMIT` 时回 Err，计数不溢出。
    #[test]
    fn broadcaster_try_subscribe_respects_limit() {
        let bc = Broadcaster::new();
        // 先填满 WS_CONN_LIMIT。
        let mut txs = Vec::new();
        for _ in 0..WS_CONN_LIMIT {
            let (tx, _rx) = std::sync::mpsc::sync_channel::<String>(4);
            assert!(bc.try_subscribe(tx.clone()).is_ok(), "应允许");
            txs.push(tx);
        }
        assert_eq!(bc.subscriber_count(), WS_CONN_LIMIT);
        // 再来一个应被拒绝。
        let (tx_extra, _rx_extra) = std::sync::mpsc::sync_channel::<String>(4);
        assert!(bc.try_subscribe(tx_extra).is_err(), "应拒绝");
        assert_eq!(bc.subscriber_count(), WS_CONN_LIMIT, "拒绝路径不应改变计数");
    }

    /// P1WS-2：unsubscribe(id) 按稳定 conn_id 移除槽位（**不**依赖
    /// 下次 broadcast try_send 失败延迟回收）。
    #[test]
    fn broadcaster_unsubscribe_by_id() {
        let bc = Broadcaster::new();
        let (tx1, _rx1) = std::sync::mpsc::sync_channel::<String>(4);
        let (tx2, _rx2) = std::sync::mpsc::sync_channel::<String>(4);
        let id1 = bc.try_subscribe(tx1).expect("s1");
        let id2 = bc.try_subscribe(tx2).expect("s2");
        assert_eq!(bc.subscriber_count(), 2);
        // 卸 id1。
        assert!(bc.unsubscribe(id1), "应移除");
        assert_eq!(bc.subscriber_count(), 1);
        // 再次卸 id1 → false（已不存在）。
        assert!(!bc.unsubscribe(id1), "重复卸应 no-op");
        assert_eq!(bc.subscriber_count(), 1);
        // 卸 id2。
        assert!(bc.unsubscribe(id2), "应移除");
        assert_eq!(bc.subscriber_count(), 0);
    }
}

// =============================================================================
// 连接生命周期：broadcaster 槽位簿记测试（**非**真实 WS E2E）
// =============================================================================
//
// 验证：
// - 40 次 try_subscribe/unsubscribe 不耗尽槽位（subscriber_count 必能回 0）。
//
// **测试设计**（2026-08-31 复审改名）：不走真实 TCP/WS 连接——直接调
// `Broadcaster::try_subscribe` + `unsubscribe` 验证**簿记正确性**
// （unsubscribe 显式回收，不依赖下次 broadcast try_send 失败）。
// 真实 socket 的「客户端 close → heartbeat 写失败 → actor 退出 →
// unsubscriber 调用」链路由 `lifecycle_e2e::*`（真 WS 握手）覆盖。

#[cfg(test)]
mod lifecycle_tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use super::super::ws::{Broadcaster, WS_CONN_LIMIT};

    /// 40 次连接/断开循环：subscriber_count 必能回到 0（不耗尽槽位）。
    #[test]
    fn repeated_subscribe_unsubscribe_does_not_leak() {
        let bc = Broadcaster::new();
        for i in 0..40 {
            let (tx, _rx) = mpsc::sync_channel::<String>(4);
            let id = bc
                .try_subscribe(tx)
                .unwrap_or_else(|_| panic!("iter {i}: subscribe should succeed"));
            assert_eq!(bc.subscriber_count(), 1, "iter {i}: count after subscribe");
            assert!(bc.unsubscribe(id), "iter {i}: unsubscribe should succeed");
            assert_eq!(
                bc.subscriber_count(),
                0,
                "iter {i}: count after unsubscribe"
            );
        }
        assert_eq!(bc.subscriber_count(), 0, "最终应回到 0");
    }

    /// WS_CONN_LIMIT 并发：所有槽位占满后 unsubscribe 全部 → 再能 subscribe
    /// 满（**不**耗尽）。
    #[test]
    fn fill_to_limit_unsubscribe_refill() {
        let bc = Broadcaster::new();
        // 1) 填满。
        let mut ids = Vec::with_capacity(WS_CONN_LIMIT);
        for _ in 0..WS_CONN_LIMIT {
            let (tx, _rx) = mpsc::sync_channel::<String>(4);
            ids.push(bc.try_subscribe(tx).expect("subscribe"));
        }
        assert_eq!(bc.subscriber_count(), WS_CONN_LIMIT);
        // 2) 卸一半。
        for id in ids.iter().take(WS_CONN_LIMIT / 2) {
            assert!(bc.unsubscribe(*id));
        }
        assert_eq!(bc.subscriber_count(), WS_CONN_LIMIT - WS_CONN_LIMIT / 2);
        // 3) 重新填满到上限。
        for _ in 0..WS_CONN_LIMIT / 2 {
            let (tx, _rx) = mpsc::sync_channel::<String>(4);
            assert!(bc.try_subscribe(tx).is_ok());
        }
        assert_eq!(bc.subscriber_count(), WS_CONN_LIMIT);
        // 4) 全卸。
        for id in &ids[WS_CONN_LIMIT / 2..] {
            assert!(bc.unsubscribe(*id));
        }
        // 全部清空。
        bc.clear();
        assert_eq!(bc.subscriber_count(), 0);
    }

    /// subscriber_count 在 broadcast 期间不会被错误地"清零"——仅在
    /// unsubscribe 显式调用 / 慢订阅者 try_send 失败时回收。
    #[test]
    fn broadcast_does_not_artificially_drop_active_subscribers() {
        let bc = Broadcaster::new();
        let (tx, rx) = mpsc::sync_channel::<String>(8);
        let id = bc.try_subscribe(tx).expect("subscribe");
        assert_eq!(bc.subscriber_count(), 1);
        for i in 0..5 {
            bc.broadcast(&format!(r#"{{"n":{i}}}"#));
        }
        assert_eq!(bc.subscriber_count(), 1, "活跃订阅者不应被回收");
        // 显式 unsubscribe 才回收。
        assert!(bc.unsubscribe(id));
        assert_eq!(bc.subscriber_count(), 0);
        // drop rx 让发送的 try_send 失败（验证：即使如此，**新**广播不会
        // 误删其它订阅者）。
        drop(rx);
        bc.broadcast(r#"{"k":1}"#);
        assert_eq!(bc.subscriber_count(), 0, "无订阅者时应保持 0");
    }

    /// **HTTP 始终可响应** 验证（轻量版，**不**起 server）——证明
    /// broadcaster 槽位/conn_id 维护**不**与 HTTP 派发路径耦合。完整
    /// e2e 验证（HTTP 在 WS 已建立时仍 200）见 `ws_and_http_concurrent`。
    #[test]
    fn broadcaster_state_independent_of_http_dispatch() {
        use std::sync::Arc;
        // 模拟"HTTP 派发 + WS 广播同时进行"：两个独立线程分别调用
        // broadcaster.try_subscribe/unsubscribe 和 broadcaster.broadcast。
        let bc = Arc::new(Broadcaster::new());
        let bc_for_subscribe = Arc::clone(&bc);
        let bc_for_broadcast = Arc::clone(&bc);
        // 订阅线程：100ms 内尝试 20 次 subscribe/unsubscribe 循环。
        let sub_thread = thread::spawn(move || {
            for i in 0..20 {
                let (tx, _rx) = mpsc::sync_channel::<String>(4);
                if let Ok(id) = bc_for_subscribe.try_subscribe(tx) {
                    thread::sleep(Duration::from_millis(2));
                    bc_for_subscribe.unsubscribe(id);
                }
                let _ = i;
            }
        });
        // 广播线程：100ms 内尝试 100 次 broadcast（无订阅者 → 安全 noop）。
        let bc_thread = thread::spawn(move || {
            for i in 0..100 {
                bc_for_broadcast.broadcast(&format!(r#"{{"i":{i}}}"#));
            }
        });
        sub_thread.join().expect("sub thread");
        bc_thread.join().expect("bc thread");
        // 最终状态：两个线程都退出 → subscriber_count = 0。
        assert_eq!(bc.subscriber_count(), 0);
    }
}
