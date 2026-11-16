//! **M5 外挂资料**：只进热卡的 @@[资料]@@ 行，原文不出现在说话请求里。
//!
//! 计划 §6 M5 的验收判据逐条对应下面的测试：
//! 1. 配置一项路径，缺省空 = 不查资料；
//! 2. 只读 utf-8 文本，按块取候选，压成 @@[资料]@@，受热卡剩余预算限制；
//! 3. **一段 5000 字的文件，说话请求的 system 里只能看见预算内的 @@[资料]@@ 行，
//!    看不见文件里的特征句**（选一句不在热卡里的原文做断言）。

use std::io::Write as _;

use tiny_http::Method;

use super::chat_draft::{CHAT_DRAFT_PATH, DraftAccepted, handle_chat_draft};
use super::chat_routes::tests_support::{body, ctx_with_supervisor};
use super::hot_material::{MAX_MATERIAL_BYTES, load_material, resolve_material_path};
use crate::web_api::hot_material;

fn read_body(resp: tiny_http::Response<std::io::Cursor<Vec<u8>>>) -> String {
    body(resp)
}

fn post_draft_with_material(
    ctx: &crate::web_api::ServerContext,
    handle: &crate::supervisor::SupervisorHandle,
    material: &[String],
    text: &str,
) -> tiny_http::Response<std::io::Cursor<Vec<u8>>> {
    let payload = serde_json::json!({"session_id": "s1", "text": text}).to_string();
    handle_chat_draft(
        Some(handle.hot_slot()),
        material,
        &Method::Post,
        CHAT_DRAFT_PATH,
        &payload,
        Some("http://127.0.0.1:18080"),
        Some("application/json"),
        &ctx.security,
    )
    .expect("路径匹配")
}

fn quit(handle: std::sync::Arc<crate::supervisor::SupervisorHandle>) {
    handle.quit();
    if let Ok(inner) = std::sync::Arc::try_unwrap(handle) {
        inner.join();
    }
}

/// 写一个临时资料文件（测试自己清理）。
fn write_material(name: &str, text: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("l2d-hotmem-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let path = dir.join(name);
    let mut f = std::fs::File::create(&path).expect("create");
    f.write_all(text.as_bytes()).expect("write");
    path
}

// ------------------------------------------------------------------ 路径解析

#[test]
fn empty_material_path_means_no_material() {
    assert!(resolve_material_path("/etc/live2d-ai.toml", "").is_none());
    assert!(resolve_material_path("/etc/live2d-ai.toml", "   ").is_none());
    assert!(load_material("/etc/live2d-ai.toml", "").is_none());
}

#[test]
fn relative_material_path_anchors_on_the_config_directory() {
    let got = resolve_material_path("/home/u/.config/live2d-ai/live2d-ai.toml", "notes.md")
        .expect("相对路径应能解析");
    assert_eq!(
        got,
        std::path::PathBuf::from("/home/u/.config/live2d-ai/notes.md")
    );
    // 绝对路径原样用。
    let abs = resolve_material_path("/etc/live2d-ai.toml", "/tmp/notes.md").expect("绝对路径");
    assert_eq!(abs, std::path::PathBuf::from("/tmp/notes.md"));
}

#[test]
fn missing_or_oversized_material_file_is_ignored_not_an_error() {
    // 不存在。
    assert!(load_material("/etc/live2d-ai.toml", "/nonexistent/notes.md").is_none());
    // 超上限。
    let big = "字".repeat((MAX_MATERIAL_BYTES as usize / 3) + 100);
    let path = write_material("big.md", &big);
    assert!(
        load_material("/etc/live2d-ai.toml", path.to_str().expect("utf8")).is_none(),
        "超过上限的文件必须被忽略而不是读进来"
    );
    let _ = std::fs::remove_file(path);
}

// ------------------------------------------------------------------ 只进 [资料] 行

/// **核心判据**（计划 §6 M5 第三条）：5000 字资料 → 说话请求里只看得到预算内的
/// @@[资料]@@ 行，看不见文件里的特征句。
#[test]
fn large_material_only_contributes_budgeted_material_lines() {
    let (ctx, handle) = ctx_with_supervisor();
    // 200 段，每段 25 字 ≈ 5000 字；其中一段带特征句。
    let marker = "特征句：薄荷是唇形科植物";
    let mut text = String::new();
    for i in 0..200 {
        if i == 150 {
            text.push_str(marker);
        } else {
            text.push_str(&format!(
                "第{i:03}段无关资料内容占位文字，用来把语料撑到足够大，模拟真实的外挂笔记文件。"
            ));
        }
        text.push_str("\n\n");
    }
    assert!(
        text.chars().count() > 5_000,
        "语料应足够大：{} 字",
        text.chars().count()
    );
    let path = write_material("corpus.md", &text);
    let chunks =
        load_material("/etc/live2d-ai.toml", path.to_str().expect("utf8")).expect("资料应能读出来");
    assert!(chunks.len() > 6, "切块数量应远多于一次能取的数量");

    // 库里放两条清楚档 durable：它们**不**进热卡（没有向量检索就没有命中），
    // 于是这份语料能用到几乎整个 200 token 预算——正好压一压资料的上限。
    handle.hot_slot().record(vec![
        live2d_ai_runtime::hotmem::Recollection::new(
            "用户喜欢薄荷",
            live2d_ai_runtime::hotmem::FactKind::Durable,
        ),
        live2d_ai_runtime::hotmem::Recollection::new(
            "用户住在杭州",
            live2d_ai_runtime::hotmem::FactKind::Durable,
        ),
    ]);
    let resp = post_draft_with_material(&ctx, &handle, &chunks, "薄荷");
    assert_eq!(resp.status_code().0, 200);
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("JSON");

    // 卡里有资料行，且数量受预算限制（不会把 200 段全塞进来）。
    assert!(parsed.material_lines > 0, "应有资料行：{}", parsed.card);
    assert!(
        parsed.material_lines <= live2d_ai_runtime::hotmem::MATERIAL_MAX_CHUNKS,
        "资料行数必须有上限：{}",
        parsed.material_lines
    );
    assert!(
        parsed.tokens <= live2d_ai_runtime::hotmem::HOT_CARD_BUDGET_TOKENS,
        "整卡必须落在 200 token 内：{}",
        parsed.tokens
    );
    // 事实行一条都不在：没有向量检索 ⇒ 没有命中 ⇒ 热卡只有 `[资料]` 行。
    assert!(
        !parsed.card.contains("[清楚]") && !parsed.card.contains("[模糊]"),
        "没有命中时不许出现事实行：{}",
        parsed.card
    );
    assert!(
        parsed.card.lines().all(|l| l.starts_with("- [资料] ")),
        "卡里只能有资料行：{}",
        parsed.card
    );

    // **特征句不在卡里**：它在第 174 段，远超预算能容纳的行数。
    assert!(
        !parsed.card.contains(marker),
        "预算外的原文不得进热卡：{}",
        parsed.card
    );

    // 说话请求里的 system 文本同样只看得到热卡内容（资料原文不在其中）。
    let blocks = handle.hot_slot().compose("", Some("s1"));
    assert!(
        !blocks.system.contains(marker),
        "说话请求的 system 里不得出现预算外的资料原文"
    );
    assert!(blocks.system.contains("【本轮相关】"), "有热卡就要有标题");

    let _ = std::fs::remove_file(path);
    quit(handle);
}

/// 资料与保留率无关：**没有事实**时资料照样进卡（只有 @@[资料]@@ 行）。
#[test]
fn material_lines_appear_without_any_facts() {
    let (ctx, handle) = ctx_with_supervisor();
    let material = vec!["薄荷是一种唇形科植物。".to_string()];
    let resp = post_draft_with_material(&ctx, &handle, &material, "薄荷");
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("JSON");
    assert_eq!(parsed.material_lines, 1);
    assert_eq!(parsed.card, "- [资料] 薄荷是一种唇形科植物。");
    quit(handle);
}

/// 没配资料（空表）+ 没有检索命中 → 空卡（与 M3 的「查不到就是空」一致）。
///
/// 事实库里有清楚档 durable，但它走「钉住的事」，不进热卡——这是 2026-11-16
/// 裁决第 1 条的直接推论。
#[test]
fn empty_material_list_keeps_the_m3_behaviour() {
    let (ctx, handle) = ctx_with_supervisor();
    handle
        .hot_slot()
        .record(vec![live2d_ai_runtime::hotmem::Recollection::new(
            "用户喜欢薄荷",
            live2d_ai_runtime::hotmem::FactKind::Durable,
        )]);
    let resp = post_draft_with_material(&ctx, &handle, &[], "薄荷");
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("JSON");
    assert_eq!(parsed.material_lines, 0);
    assert_eq!(parsed.lines, 0, "没有命中也没有资料 → 空卡");
    assert!(parsed.card.is_empty(), "{}", parsed.card);
    // 那条 durable 仍在钉住块里出场。
    let blocks = handle.hot_slot().compose("", None);
    assert!(blocks.pinned.contains("用户喜欢薄荷"), "{}", blocks.pinned);
    quit(handle);
}

/// 草稿话题变了 → 含资料的旧卡**整块作废**（不是保留资料行）。
#[test]
fn new_draft_replaces_the_whole_card_including_material() {
    let (ctx, handle) = ctx_with_supervisor();
    let material = vec!["薄荷是唇形科。".to_string()];
    let _ = post_draft_with_material(&ctx, &handle, &material, "薄荷");
    // 第二次带空资料 + 新草稿：旧资料行必须消失。
    let resp = post_draft_with_material(&ctx, &handle, &[], "别的话题");
    let parsed: DraftAccepted = serde_json::from_str(&read_body(resp)).expect("JSON");
    assert!(
        !parsed.card.contains("唇形科"),
        "新一轮没有资料时，旧资料行不得残留：{}",
        parsed.card
    );
    quit(handle);
}

/// 切块口径：按空行分段、超长段硬切、空白段丢弃。
#[test]
fn material_is_split_by_blank_lines_and_hard_cut() {
    let long = "字".repeat(live2d_ai_runtime::hotmem::MATERIAL_CHUNK_CHARS + 50);
    let text = format!("第一段\n\n第二段\n\n\n\n{long}\n\n   \n");
    let chunks = live2d_ai_runtime::hotmem::split_material(&text);
    assert_eq!(chunks.len(), 4, "{chunks:?}");
    assert_eq!(chunks[0], "第一段");
    assert_eq!(chunks[1], "第二段");
    assert_eq!(
        chunks[2].chars().count(),
        live2d_ai_runtime::hotmem::MATERIAL_CHUNK_CHARS
    );
    assert_eq!(chunks[3].chars().count(), 50);
}

/// 上限常量的口径（1 MiB）。
#[test]
fn material_size_cap_is_one_mebibyte() {
    assert_eq!(MAX_MATERIAL_BYTES, 1024 * 1024);
    assert_eq!(hot_material::MAX_MATERIAL_BYTES, MAX_MATERIAL_BYTES);
}
