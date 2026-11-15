//! `POST /api/v1/chat/draft` 的契约测试（2026-11-16 热槽记忆 M3）。
//!
//! 守四件事：
//! 1. **端点存在且能换热卡**：一次草稿 → 该会话热卡被整块替换；
//! 2. **连发以最后一次为准**：旧的检索结果不会追加在后面；
//! 3. **草稿不是命令**：没有 turn、没有 TTS、没有 WS 帧、不写 toml——
//!    用「epoch 不动」与「热卡是唯一副作用」两侧夹住；
//! 4. **安全口径与 `/api/v1/chat` 同一套**：CT 不对 415、Origin 不对 403。

use tiny_http::Method;

use super::chat_draft::{CHAT_DRAFT_PATH, DRAFT_DEBOUNCE_MS, DraftAccepted, handle_chat_draft};

/// 复用 `chat_routes` 的测试脚手架（同一个 `Response` / `Cursor` /
/// `SupervisorHandle` 语义，不在这里再抄一份装配）。
use super::chat_routes::tests_support::{body, ctx_with_supervisor};

/// 从响应里读 body。
fn read_body(resp: tiny_http::Response<std::io::Cursor<Vec<u8>>>) -> String {
    body(resp)
}

fn post_draft(
    ctx: &crate::web_api::ServerContext,
    handle: &crate::supervisor::SupervisorHandle,
    body: &str,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    handle_chat_draft(
        Some(handle.hot_slot()),
        &[],
        &Method::Post,
        CHAT_DRAFT_PATH,
        body,
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
        &ctx.security,
    )
    .expect("路径匹配必须返回 Some")
}

#[test]
fn draft_replaces_the_hot_card_for_that_session() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();
    // 先往事实库里放两条**清楚**档的耐久事实。
    slot.record(vec![
        live2d_ai_runtime::hotmem::Recollection::new(
            "用户喜欢薄荷",
            live2d_ai_runtime::hotmem::FactKind::Durable,
        ),
        live2d_ai_runtime::hotmem::Recollection::new(
            "用户住在杭州",
            live2d_ai_runtime::hotmem::FactKind::Durable,
        ),
    ]);

    let resp = post_draft(&ctx, &handle, r#"{"session_id":"s1","text":"薄荷"}"#);
    assert_eq!(resp.status_code().0, 200);
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("合法 JSON");
    assert!(parsed.accepted);
    assert_eq!(parsed.session_id.as_deref(), Some("s1"));
    assert_eq!(parsed.lines, 2, "两条清楚档事实应都进热卡");
    assert!(
        parsed.card.contains("- [清楚] 用户喜欢薄荷"),
        "{}",
        parsed.card
    );
    assert!(
        parsed.card.contains("- [清楚] 用户住在杭州"),
        "{}",
        parsed.card
    );
    // 热卡落在**该会话**上。
    assert!(slot.card_is_some(Some("s1")));
    assert!(!slot.card_is_some(Some("s2")), "别的会话不受影响");

    quit(handle);
}

/// **连发以最后一次为准**：第二张卡整块重算，不是追加。
#[test]
fn repeated_drafts_replace_instead_of_appending() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();
    slot.record(vec![live2d_ai_runtime::hotmem::Recollection::new(
        "第一条事实",
        live2d_ai_runtime::hotmem::FactKind::Durable,
    )]);
    let first: DraftAccepted = serde_json::from_str(&read_body(post_draft(
        &ctx,
        &handle,
        r#"{"session_id":"s1","text":"甲"}"#,
    )))
    .expect("JSON");
    assert_eq!(first.lines, 1);

    slot.record(vec![live2d_ai_runtime::hotmem::Recollection::new(
        "第二条事实",
        live2d_ai_runtime::hotmem::FactKind::Durable,
    )]);
    let second: DraftAccepted = serde_json::from_str(&read_body(post_draft(
        &ctx,
        &handle,
        r#"{"session_id":"s1","text":"乙"}"#,
    )))
    .expect("JSON");
    assert!(
        second.card.contains("第二条事实"),
        "新一轮检索结果必须在卡里：{}",
        second.card
    );
    // 整块重算：卡的行数 = 当前候选集大小，而不是 1 + 2。
    assert_eq!(second.lines, 2, "热卡整块重算：{}", second.card);

    quit(handle);
}

/// **草稿不是命令**：epoch 不动（没有新轮次被提交）。
#[test]
fn draft_does_not_start_a_turn() {
    let (ctx, handle) = ctx_with_supervisor();
    let before = handle.current_epoch();
    let resp = post_draft(&ctx, &handle, r#"{"session_id":"s1","text":"只是草稿"}"#);
    assert_eq!(resp.status_code().0, 200);
    assert_eq!(handle.current_epoch(), before, "草稿不得推进 epoch");
    quit(handle);
}

/// 空白草稿 = 清掉该会话的热卡（没有检索依据，不拿旧卡顶着）。
#[test]
fn empty_draft_clears_the_card() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();
    slot.record(vec![live2d_ai_runtime::hotmem::Recollection::new(
        "一条事实",
        live2d_ai_runtime::hotmem::FactKind::Durable,
    )]);
    let first = post_draft(&ctx, &handle, r#"{"session_id":"s1","text":"有内容"}"#);
    assert_eq!(first.status_code().0, 200);
    assert!(slot.card_is_some(Some("s1")));

    let cleared: DraftAccepted = serde_json::from_str(&read_body(post_draft(
        &ctx,
        &handle,
        r#"{"session_id":"s1","text":""}"#,
    )))
    .expect("JSON");
    assert_eq!(cleared.lines, 0, "空草稿 → 空热卡");
    assert!(cleared.card.is_empty());
    assert!(!slot.card_is_some(Some("s1")), "旧热卡必须被清掉");
    quit(handle);
}

/// 空事实库 → 空热卡，接口仍 200。
///
/// 这是计划 §3「本机 embedding 没配好或加载失败：查不做向量，热卡为空」的
/// **当前实现判据**：候选集为空 ⇒ 卡为空 ⇒ 说话请求里不出现热卡段。
#[test]
fn empty_fact_store_yields_empty_card_and_still_200() {
    let (ctx, handle) = ctx_with_supervisor();
    let resp = post_draft(
        &ctx,
        &handle,
        r#"{"session_id":"s-empty","text":"随便说点什么"}"#,
    );
    assert_eq!(resp.status_code().0, 200, "没有候选也必须 200");
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("JSON");
    assert_eq!(parsed.lines, 0);
    assert!(parsed.card.is_empty(), "热卡必须是空的：{}", parsed.card);
    quit(handle);
}

/// 安全口径与 `/api/v1/chat` 同一套：CT 不对 415、Origin 不对 403。
#[test]
fn draft_enforces_the_same_mutating_contract_as_chat() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();

    let bad_ct = handle_chat_draft(
        Some(slot),
        &[],
        &Method::Post,
        CHAT_DRAFT_PATH,
        r#"{"text":"x"}"#,
        Some("http://127.0.0.1:18080"),
        Some("text/plain"),
        &ctx.security,
    )
    .expect("路径匹配");
    assert_eq!(bad_ct.status_code().0, 415);

    let bad_origin = handle_chat_draft(
        Some(slot),
        &[],
        &Method::Post,
        CHAT_DRAFT_PATH,
        r#"{"text":"x"}"#,
        Some("http://evil.example"),
        Some("application/json"),
        &ctx.security,
    )
    .expect("路径匹配");
    assert_eq!(bad_origin.status_code().0, 403);

    let no_origin = handle_chat_draft(
        Some(slot),
        &[],
        &Method::Post,
        CHAT_DRAFT_PATH,
        r#"{"text":"x"}"#,
        None,
        Some("application/json"),
        &ctx.security,
    )
    .expect("路径匹配");
    assert_eq!(no_origin.status_code().0, 403);

    let get = handle_chat_draft(
        Some(slot),
        &[],
        &Method::Get,
        CHAT_DRAFT_PATH,
        "",
        None,
        None,
        &ctx.security,
    )
    .expect("路径匹配");
    assert_eq!(get.status_code().0, 405);

    assert!(
        handle_chat_draft(
            Some(slot),
            &[],
            &Method::Post,
            "/api/v1/chat",
            "",
            None,
            None,
            &ctx.security,
        )
        .is_none()
    );

    quit(handle);
}

/// 客户端防抖常量是契约的一部分（前端停顿时长）；这不是服务端行为。
#[test]
fn debounce_hint_is_the_documented_value() {
    assert_eq!(DRAFT_DEBOUNCE_MS, 400);
}

/// 非法会话 id → 按「不带会话」处理（**不是 400**），与 `/api/v1/chat` 同一条纪律。
#[test]
fn invalid_session_id_falls_back_to_the_global_bucket() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();
    slot.record(vec![live2d_ai_runtime::hotmem::Recollection::new(
        "一条事实",
        live2d_ai_runtime::hotmem::FactKind::Durable,
    )]);
    let resp = post_draft(
        &ctx,
        &handle,
        r#"{"session_id":"../etc/passwd","text":"x"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("JSON");
    assert_eq!(parsed.session_id, None, "非法 id 归一化成不带会话");
    assert!(slot.card_is_some(None), "落在全局桶");
    quit(handle);
}

/// 全局桶（不带 session_id）与具名会话互不覆盖。
#[test]
fn session_and_global_buckets_do_not_overwrite_each_other() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();
    slot.record(vec![live2d_ai_runtime::hotmem::Recollection::new(
        "一条事实",
        live2d_ai_runtime::hotmem::FactKind::Durable,
    )]);
    let _ = post_draft(&ctx, &handle, r#"{"session_id":"s1","text":"甲"}"#);
    assert!(slot.card_is_some(Some("s1")));
    assert!(!slot.card_is_some(None), "全局桶不该被具名会话写入");

    let _ = post_draft(&ctx, &handle, r#"{"text":"乙"}"#);
    assert!(slot.card_is_some(None), "全局桶自己写");
    assert!(slot.card_is_some(Some("s1")), "互不覆盖");

    quit(handle);
}

/// 收口：`join` 消费 `Arc<SupervisorHandle>`，先 `quit()` 再解引用。
fn quit(handle: std::sync::Arc<crate::supervisor::SupervisorHandle>) {
    handle.quit();
    if let Ok(inner) = std::sync::Arc::try_unwrap(handle) {
        inner.join();
    }
}
