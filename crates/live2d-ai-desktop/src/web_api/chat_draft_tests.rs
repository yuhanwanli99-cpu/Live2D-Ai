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

/// 带外挂资料的草稿请求（没有向量时，资料是唯一能确定进卡的来源）。
fn post_draft_with_material(
    ctx: &crate::web_api::ServerContext,
    handle: &crate::supervisor::SupervisorHandle,
    material: &[String],
    body: &str,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    handle_chat_draft(
        Some(handle.hot_slot()),
        material,
        &Method::Post,
        CHAT_DRAFT_PATH,
        body,
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
        &ctx.security,
    )
    .expect("路径匹配必须返回 Some")
}

/// 一次草稿 → 该会话热卡被整块换成「这次查出来的那一张」。
///
/// **注**：没有向量检索时事实库不出卡（见
/// [`non_empty_fact_store_without_embeddings_still_yields_an_empty_card`]），
/// 所以这里用**外挂资料**当「本轮命中的内容」——它是今天唯一能确定进卡的来源。
/// 事实库同时有两条清楚档 durable，正好反过来证明它们**不**进卡。
#[test]
fn draft_replaces_the_hot_card_for_that_session() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();
    slot.record(vec![live2d_ai_runtime::hotmem::Recollection::new(
        "用户喜欢薄荷",
        live2d_ai_runtime::hotmem::FactKind::Durable,
    )]);
    let material = vec!["薄荷是唇形科植物。".to_string(), "薄荷喜湿。".to_string()];

    let resp = post_draft_with_material(
        &ctx,
        &handle,
        &material,
        r#"{"session_id":"s1","text":"薄荷"}"#,
    );
    assert_eq!(resp.status_code().0, 200);
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("合法 JSON");
    assert!(parsed.accepted);
    assert_eq!(parsed.session_id.as_deref(), Some("s1"));
    assert_eq!(parsed.lines, 2, "资料两块应都进热卡：{}", parsed.card);
    assert!(
        parsed.card.contains("- [资料] 薄荷是唇形科植物。"),
        "{}",
        parsed.card
    );
    assert_eq!(parsed.material_lines, 2);
    // 事实库里那条清楚档 durable **不**借热卡出场（它走钉住块）。
    assert!(
        !parsed.card.contains("用户喜欢薄荷"),
        "没有检索命中时事实不得进热卡：{}",
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
    let first: DraftAccepted = serde_json::from_str(&read_body(post_draft_with_material(
        &ctx,
        &handle,
        &["第一种资料".to_string()],
        r#"{"session_id":"s1","text":"甲"}"#,
    )))
    .expect("JSON");
    assert_eq!(first.lines, 1);
    assert!(first.card.contains("第一种资料"), "{}", first.card);

    let second: DraftAccepted = serde_json::from_str(&read_body(post_draft_with_material(
        &ctx,
        &handle,
        &["第二种资料".to_string(), "第三种资料".to_string()],
        r#"{"session_id":"s1","text":"乙"}"#,
    )))
    .expect("JSON");
    assert!(
        second.card.contains("第二种资料"),
        "新一轮检索结果必须在卡里：{}",
        second.card
    );
    assert!(
        !second.card.contains("第一种资料"),
        "上一张卡的行不得残留（整块替换）：{}",
        second.card
    );
    // 整块重算：卡的行数 = 本次候选集大小，而不是 1 + 2。
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
    let first = post_draft_with_material(
        &ctx,
        &handle,
        &["一条资料".to_string()],
        r#"{"session_id":"s1","text":"有内容"}"#,
    );
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

/// **事实库非空、embedding 缺失 → 热卡为空、接口 200、「钉住的事」仍可出现**。
///
/// 这是 2026-11-16 维护者裁决第 1 条的判据（计划 §10.1）。与上面
/// [`empty_fact_store_yields_empty_card_and_still_200`] 的区别是**事实库里有东西**：
/// 库里躺着两条刚记下的、`R = 1.0`（清楚档）的 durable 事实，但本机没有向量检索，
/// 所以「查」给不出命中——热卡必须**仍然为空**。
///
/// 为什么这不是苛刻：保留率只说明「还记得」，不说明「和这句草稿有关」。按 `R`
/// 把这两条填进热卡，等于每轮都把不相关的旧事带进请求。而它们本来就会出现在
/// 「钉住的事」里（`R >= 0.7` 的 durable），不借热卡出场。
#[test]
fn non_empty_fact_store_without_embeddings_still_yields_an_empty_card() {
    let (ctx, handle) = ctx_with_supervisor();
    let slot = handle.hot_slot();
    // 两条刚写进去的 durable 事实：R = 1.0，落在「清楚」档。
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
    assert_eq!(slot.snapshot().facts, 2, "前提：事实库非空");

    let resp = post_draft(&ctx, &handle, r#"{"session_id":"s1","text":"薄荷"}"#);
    assert_eq!(resp.status_code().0, 200, "没有向量也必须 200");
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("JSON");
    assert_eq!(
        parsed.lines, 0,
        "没有检索命中 → 一行事实都不许进卡：{}",
        parsed.card
    );
    assert!(
        parsed.card.is_empty(),
        "事实库非空不等于热卡非空：{}",
        parsed.card
    );
    assert_eq!(parsed.tokens, 0);
    assert!(
        !slot.card_is_some(Some("s1")),
        "空卡 = 该会话没有热卡（不是存一张空卡）"
    );

    // 但「钉住的事」仍然可以出现：这两条就是钉住块的候选。
    let blocks = slot.compose("", Some("s1"));
    assert!(blocks.hot_card.is_empty(), "热卡块为空");
    assert!(
        blocks.pinned.contains("用户喜欢薄荷") && blocks.pinned.contains("用户住在杭州"),
        "清楚档的耐久事实走钉住块：{}",
        blocks.pinned
    );
    assert!(blocks.system.contains("【钉住的事】"));
    assert!(
        !blocks.system.contains("【本轮相关】"),
        "空热卡不写标题：{}",
        blocks.system
    );
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

/// **「记」是运行期开关，缺省关**（生产装配 `cli_entry` 显式打开）。
///
/// 为什么必须缺省关：记 = 每轮多一次 LLM 请求，而「一轮恰好 N 次请求」是主链
/// 多条回归的判据。测试装配默认带着第二条请求，只会逼着那些断言一起改大。
#[test]
fn hot_recall_is_off_by_default_and_opt_in() {
    let (_, handle) = ctx_with_supervisor();
    assert!(
        !handle.hot_recall_enabled(),
        "缺省关：测试装配不许多发一次请求"
    );
    handle.enable_hot_recall();
    assert!(handle.hot_recall_enabled(), "显式打开后为真");
    // 幂等：再打开一次不出错、也不翻转。
    handle.enable_hot_recall();
    assert!(handle.hot_recall_enabled());
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
    let resp = post_draft_with_material(
        &ctx,
        &handle,
        &["一条资料".to_string()],
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
    let _ = post_draft_with_material(
        &ctx,
        &handle,
        &["一条资料".to_string()],
        r#"{"session_id":"s1","text":"甲"}"#,
    );
    assert!(slot.card_is_some(Some("s1")));
    assert!(!slot.card_is_some(None), "全局桶不该被具名会话写入");

    let _ = post_draft_with_material(
        &ctx,
        &handle,
        &["另一条资料".to_string()],
        r#"{"text":"乙"}"#,
    );
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
