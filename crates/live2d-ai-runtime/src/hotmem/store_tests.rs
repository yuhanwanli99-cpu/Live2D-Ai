//! 进程内事实库与每会话状态的单测（2026-11-16 热槽记忆 M1/M4）。
//!
//! 从 `tests.rs` 拆出：`tests.rs` 已到 520 行，而门禁是「`crates/*/src` 里
//! >500 行的文件不得增加」。拆的时候只搬不动——每条断言的语义一行未改。
//!
//! 这里守的是**库**这一层：记 / 加固 / 清理、每会话热卡与摘要的替换语义、
//! 已说索引跳读但不删条。纯公式与热卡渲染仍在 `tests.rs`。

use super::*;

fn card_of(items: &[(CardTier, &str)]) -> Vec<CardLine> {
    items
        .iter()
        .map(|(tier, text)| CardLine::new(*tier, *text))
        .collect()
}
// ---------------------------------------------------------------- 事实库

#[test]
fn hot_memory_records_reinforces_and_prunes() {
    let mut mem = HotMemory::new();
    // 时间基点 0；记两条。
    let a = mem.remember(Recollection::new("用户叫星梦", FactKind::Durable));
    let b = mem.remember(Recollection::new("今天聊了天气", FactKind::Episode));
    assert!(a > 0 && b > 0 && a != b);
    assert_eq!(mem.facts().len(), 2);

    // 推进 20 天：durable 仍在「清楚」附近（S=14 天 → R≈0.24 模糊），
    // episode（S=18 小时）早已掉出热卡，但 20 天 < 180 天**不删**。
    mem.advance_to(20.0 * 24.0);
    mem.prune_expired();
    assert_eq!(mem.facts().len(), 2, "不到 180 天不得删 episode");

    // 加固 episode：强度 ×1.6、时间基点回到现在（仍不回到清楚档，
    // 但年龄归零 → 从「刚加固」重新算）。
    assert!(mem.reinforce(b));
    let after = mem.facts().into_iter().find(|f| f.id == b).expect("还在");
    assert_eq!(after.age_hours, 0.0);
    assert!((after.s_hours - 18.0 * 1.6).abs() < 1e-9);

    // 再推进 200 天 → episode 被清掉，durable 留着。
    mem.advance_to(220.0 * 24.0);
    assert_eq!(mem.prune_expired(), 1, "只清 episode");
    assert_eq!(mem.facts().len(), 1);
    assert_eq!(mem.facts()[0].kind, FactKind::Durable);

    // 快照数字与库里一致。
    let snap = mem.snapshot();
    assert_eq!(snap.facts, 1);
    assert_eq!(snap.durable, 1);
}

#[test]
fn card_is_replaced_per_session_and_cleared() {
    let mut mem = HotMemory::new();
    let a = HotCard::from_lines(card_of(&[(CardTier::Clear, "A 的命中")]), &[]);
    mem.set_card(Some("s1"), a);
    assert!(mem.card(Some("s1")).is_some());
    assert!(mem.card(Some("s2")).is_none(), "会话之间不共享热卡");

    // 新草稿 → 整块替换。
    let b = HotCard::from_lines(card_of(&[(CardTier::Clear, "B 的命中")]), &[]);
    mem.set_card(Some("s1"), b);
    assert_eq!(
        render_hot_card(mem.card(Some("s1"))),
        "- [清楚] B 的命中",
        "只替换、不追加"
    );

    // 空卡 = 清掉条目；已经空了再清是幂等的 false。
    mem.set_card(Some("s1"), HotCard::empty());
    assert!(mem.card(Some("s1")).is_none());
    assert!(!mem.clear_card(Some("s1")), "已经空的会话再清是 no-op");
}

#[test]
fn summary_is_replaced_and_clipped() {
    let mut mem = HotMemory::new();
    assert_eq!(mem.summary(Some("s1")), "");
    mem.set_summary(Some("s1"), "第一轮摘要");
    assert_eq!(mem.summary(Some("s1")), "第一轮摘要");
    // 替换（不是追加）。
    mem.set_summary(Some("s1"), "第二轮摘要");
    assert_eq!(mem.summary(Some("s1")), "第二轮摘要");
    // 300 字上限。
    mem.set_summary(Some("s1"), "字".repeat(1000));
    assert_eq!(mem.summary(Some("s1")).chars().count(), SUMMARY_MAX_CHARS);
    // 空串 = 撤销。
    mem.set_summary(Some("s1"), "");
    assert_eq!(mem.summary(Some("s1")), "");
}

#[test]
fn spoken_ids_suppress_repeats_without_deleting_facts() {
    let mut mem = HotMemory::new();
    let id = mem.remember(Recollection::new("用户叫星梦", FactKind::Durable));
    let items = mem.pinned_items();
    assert_eq!(items.len(), 1);
    assert_eq!(
        render_pinned(&items, mem.spoken(Some("s1"))),
        "- 用户叫星梦"
    );
    // 记下「上一轮说过」→ 本轮渲染跳过。
    mem.note_spoken(Some("s1"), vec![id]);
    assert_eq!(
        render_pinned(&mem.pinned_items(), mem.spoken(Some("s1"))),
        ""
    );
    // **条目还在库里**（跳过 ≠ 删除）。
    assert_eq!(mem.facts().len(), 1);
    // 别的会话不受影响。
    assert_eq!(
        render_pinned(&mem.pinned_items(), mem.spoken(Some("s2"))),
        "- 用户叫星梦"
    );
}
