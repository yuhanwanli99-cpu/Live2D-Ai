//! `chat_routes` 会话 baseline 回归（停止回基准 / 取消不补帧 / 逐会话 / 校验）（自内联 `mod tests` 拆出）。
//!
//! 脚手架见 `tests_support`。

use super::tests_support::*;
use super::*;

// ------------------------------------------------ 会话 baseline（阶段4e / V10）

/// **E2（阶段4e 判据）**：停止 → 回**该会话** baseline，不是全局、不是别人会话。
///
/// 三件事一起断言：
/// 1. HTTP 响应（**同一个 stop 请求**）带该会话 baseline；
/// 2. WS 上广播了**一条**既有 `action_cue` 帧，内容就是该会话 baseline；
/// 3. **不补帧**（一帧之后没有第二帧）。
#[test]
fn stop_returns_to_the_session_baseline() {
    let (_ctx, handle) = ctx_with_supervisor();
    let (bc, rx) = broadcaster_with_rx();
    handle.session_scopes().set_baseline("A", BASELINE_A);
    handle.session_scopes().set_baseline("B", BASELINE_B);

    // 活动会话 = A → 回 A 的 baseline。
    handle.set_active_session(Some("A"));
    let resp =
        handle_stop_with_supervisor(Some(&handle), Some(&bc), &Method::Post, "/api/v1/chat/stop");
    assert_eq!(resp.status_code().0, 200);
    let b = body(resp);
    assert!(b.contains("\"session_id\":\"A\""), "got: {b}");
    assert!(b.contains("smile"), "响应必须带会话 A 的 baseline: {b}");
    assert!(
        !b.contains("\"field\":\"head\""),
        "不得回 B 的 baseline: {b}"
    );

    let frame = rx.try_recv().expect("停止必须广播一帧基线");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("帧必须合法 JSON");
    assert_eq!(v["type"], "action_cue", "复用既有帧型，V11");
    assert_eq!(v["data"]["baseline"], serde_json::json!(true));
    assert_eq!(v["data"]["reason"], serde_json::json!("stop"));
    assert_eq!(v["data"]["cues"][0]["id"], serde_json::json!("smile"));
    assert!(rx.try_recv().is_err(), "不得补帧");

    // 切到 B → 回 B 的 baseline（绑会话，不是全局）。
    handle.set_active_session(Some("B"));
    let _ =
        handle_stop_with_supervisor(Some(&handle), Some(&bc), &Method::Post, "/api/v1/chat/stop");
    let frame = rx.try_recv().expect("第二次停止的基线帧");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("合法 JSON");
    assert_eq!(v["data"]["cues"][0]["field"], serde_json::json!("head"));
    assert!(
        v["data"]["cues"][0].get("id").is_none(),
        "B 的基准是 head，不得混进 A 的表情 id"
    );
    assert!(rx.try_recv().is_err(), "不得补帧");

    // 缺省为空 = 待机：没写过 baseline 的会话回 null + cues: []。
    handle.set_active_session(Some("C"));
    let resp =
        handle_stop_with_supervisor(Some(&handle), Some(&bc), &Method::Post, "/api/v1/chat/stop");
    let b = body(resp);
    assert!(b.contains("\"baseline\":null"), "待机 = null: {b}");
    let frame = rx.try_recv().expect("待机同样广播一帧（清计划）");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("合法 JSON");
    assert_eq!(v["data"]["cues"], serde_json::json!([]));
    assert!(rx.try_recv().is_err(), "不得补帧");

    // 无 broadcaster 时仍要能拿到结论（幂等 stop 的控制面路径）。
    let bare = return_to_session_baseline(&handle, None, "stop");
    assert_eq!(bare.session.as_deref(), Some("C"));
    assert_eq!(bare.baseline, None);
    assert!(bare.frame.is_some(), "帧照样构造，只是没广播");
    assert!(!bare.backfilled, "host 侧没有待补帧队列");

    handle.quit();
}

/// **E3（阶段4e 判据）**：新消息 → 取消旧编排（清计划 + 回基准），**不补帧**。
///
/// 这里的「取消」= 用一条 `action_cue`（baseline / 空表）**整表替换**旧 cue
/// 计划（4d 的 `DirectorCuePlan.replace` 语义），并且这一条之后**不再有任何
/// 帧**——旧 cue 不会在下一段音频里被补播。
#[test]
fn new_message_cancels_orchestration_without_backfill() {
    let (_ctx, handle) = ctx_with_supervisor();
    let (bc, rx) = broadcaster_with_rx();
    handle.session_scopes().set_baseline("A", BASELINE_A);

    let resp = handle_chat_with_supervisor(
        Some(&handle),
        Some(&bc),
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"新的一条消息","session_id":"A"}"#,
        0,
    );
    assert_eq!(resp.status_code().0, 200, "新消息应被受理");
    let b = body(resp);
    assert!(b.contains("\"accepted\":true"), "got: {b}");
    assert!(b.contains("\"session_id\":\"A\""), "got: {b}");
    assert!(b.contains("smile"), "同一个请求里回该会话 baseline: {b}");

    // 取消 = 唯一一条回基准帧（整表替换），且不补帧。
    let frame = rx.try_recv().expect("新消息必须广播一次回基准");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("合法 JSON");
    assert_eq!(v["type"], "action_cue");
    assert_eq!(v["data"]["reason"], serde_json::json!("new-message"));
    assert_eq!(v["data"]["cues"][0]["id"], serde_json::json!("smile"));
    assert!(rx.try_recv().is_err(), "取消后不得补帧（旧 cue 不许重放）");

    // 缺省为空 = 待机：新消息同样清计划。
    let _ = handle_chat_with_supervisor(
        Some(&handle),
        Some(&bc),
        &Method::Post,
        "/api/v1/chat",
        r#"{"text":"不带 baseline 的会话","session_id":"Z"}"#,
        0,
    );
    // 第二次 say 可能撞「忙碌」（容量 1）——帧**无论 say 收不收**都要发：
    // 回基准是取消语义，不是「发送成功才回」。
    let frame = rx.try_recv().expect("待机会话也要发一帧");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("合法 JSON");
    assert_eq!(v["data"]["cues"], serde_json::json!([]));
    assert!(rx.try_recv().is_err(), "不得补帧");

    handle.quit();
}

/// session 路由：baseline **随会话设置写**、按会话读回；键缺席 = 不动。
#[test]
fn session_route_writes_and_reads_the_baseline_per_session() {
    let (ctx, handle) = ctx_with_supervisor();
    // 写 A 的 baseline（与 session_id 同发 = 「随会话设置写」，O8）。
    let resp = handle_chat_session(
        &ctx,
        &Method::Post,
        CHAT_SESSION_PATH,
        &format!(r#"{{"session_id":"A","baseline":{BASELINE_A}}}"#),
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
    )
    .expect("path matches");
    assert_eq!(resp.status_code().0, 200);
    let b = body(resp);
    assert!(b.contains("\"active_session\":\"A\""), "got: {b}");
    assert!(b.contains("\"active_baseline\""), "got: {b}");
    assert!(b.contains("smile"), "响应必须带回写入的 baseline: {b}");
    assert!(b.contains("\"baselines\":1"), "got: {b}");
    assert!(
        b.contains("\"baseline_sessions\":[\"A\"]"),
        "状态面必须列出有基准的会话: {b}"
    );
    // 存储是**紧凑 JSON**（键序由 serde_json 规范化），因此按 JSON 值比较，
    // 不按原文字节比较。
    let stored: serde_json::Value = serde_json::from_str(
        handle
            .session_scopes()
            .baseline_for(Some("A"))
            .as_deref()
            .expect("A 的 baseline 必须已写入"),
    )
    .expect("存量必须是合法 JSON");
    let written: serde_json::Value = serde_json::from_str(BASELINE_A).expect("样例必须合法 JSON");
    assert_eq!(
        stored, written,
        "存进去的就是写回来的（同一张表、同一个会话键）"
    );

    // GET 读回（同一张表、同一个会话键）。
    let read = handle_chat_session(&ctx, &Method::Get, CHAT_SESSION_PATH, "", None, None)
        .expect("path matches");
    assert!(body(read).contains("smile"));

    // baseline 键**缺席** = 本次不动：切到 B 不会清掉 A 的 baseline。
    let _ = handle_chat_session(
        &ctx,
        &Method::Post,
        CHAT_SESSION_PATH,
        r#"{"session_id":"B"}"#,
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
    );
    assert!(
        handle.session_scopes().baseline_for(Some("A")).is_some(),
        "只切会话不得动别人的 baseline"
    );
    assert_eq!(handle.session_scopes().baseline_for(Some("B")), None);

    // null = 显式撤销（缺省为空 = 待机）。
    let cleared = handle_chat_session(
        &ctx,
        &Method::Post,
        CHAT_SESSION_PATH,
        r#"{"session_id":"A","baseline":null}"#,
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
    )
    .expect("path matches");
    assert_eq!(cleared.status_code().0, 200);
    assert!(body(cleared).contains("\"active_baseline\":null"));
    assert_eq!(handle.session_scopes().baseline_for(Some("A")), None);

    // 有 baseline 但缺合法 session_id → 400（宿主 API 的写入方错误，不静默丢）。
    let bad = handle_chat_session(
        &ctx,
        &Method::Post,
        CHAT_SESSION_PATH,
        r#"{"baseline":{"field":"body","y":0.1}}"#,
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
    )
    .expect("path matches");
    assert_eq!(bad.status_code().0, 400);
    assert!(body(bad).contains("baseline"));

    handle.quit();
}

/// baseline 非法 id 与超长都不得进表（与 prompt 槽同一道闸 / 同一纪律）。
#[test]
fn baseline_write_rejects_bad_session_and_overlong_payload() {
    let (_ctx, handle) = ctx_with_supervisor();
    let store = handle.session_scopes();
    assert!(!store.set_baseline("../etc/passwd", "b"));
    assert!(!store.set_baseline(
        "A",
        &"x".repeat(crate::session_scope::MAX_SESSION_BASELINE_CHARS + 1)
    ));
    assert_eq!(store.baseline_count(), 0);
    assert_eq!(store.baseline_for(Some("A")), None);
    // 坏 JSON 的存量值广播时退化为空表（不把坏串塞上 wire）。
    assert!(store.set_baseline("A", "not json"));
    let rev = return_to_session_baseline(&handle, None, "stop");
    let frame = rev.frame.expect("帧");
    let v: serde_json::Value = serde_json::from_str(&frame).expect("帧必须合法 JSON");
    assert_eq!(
        v["data"]["cues"],
        serde_json::json!([]),
        "坏 baseline 退化成待机"
    );
    handle.quit();
}
