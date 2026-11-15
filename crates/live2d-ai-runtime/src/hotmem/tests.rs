//! hotmem 纯函数层的单测：**公式与预算逐字钉住**。
//!
//! 这里的每一条断言都对应计划 §6 M1 的一行要求：
//! 保留率公式、种类 / 起始强度 / 加固 / 上限、热卡替换语义、三类行与 `R` 分档、
//! 200 token 裁剪、钉住块 400 token 且只收「清楚」、与最近原文重复的行丢掉。
//!
//! token 口径按模块头注：**1 token = 1 个 CJK 字符 = 1 个 ASCII 词**。

use super::*;

// ---------------------------------------------------------------- 保留率

#[test]
fn retention_matches_the_decay_formula() {
    // t = S → e^-1 ≈ 0.3679（计划 §4「t = S 时约剩 0.37」）。
    let r = retention(18.0, 18.0);
    assert!(
        (r - std::f64::consts::E.recip()).abs() < 1e-12,
        "t = S 应剩 1/e，实际 {r}"
    );
    assert!((r - 0.36787944117144233).abs() < 1e-12);

    // t = 0 → 1.0；单调下降；t → ∞ → 0。
    assert_eq!(retention(0.0, 18.0), 1.0);
    let a = retention(10.0, 18.0);
    let b = retention(20.0, 18.0);
    assert!(
        a > b && a < 1.0 && b > 0.0,
        "R 必须随 t 单调下降：{a} > {b}"
    );
    assert!(retention(10_000.0, 18.0) < 1e-100, "很久以后必须趋近 0");
}

#[test]
fn retention_is_total_on_broken_input() {
    // 强度坏了 → 0（当作忘光），不产出 NaN。
    assert_eq!(retention(5.0, 0.0), 0.0);
    assert_eq!(retention(5.0, -1.0), 0.0);
    assert_eq!(retention(5.0, f64::NAN), 0.0);
    assert_eq!(retention(f64::INFINITY, 18.0), 0.0);
    // 时钟回拨（t < 0）→ 按 0 处理，R = 1。
    assert_eq!(retention(-3.0, 18.0), 1.0);
    // 结果恒在 [0, 1]。
    for t in [0.0, 1.0, 18.0, 1e6] {
        let r = retention(t, 42.0);
        assert!((0.0..=1.0).contains(&r), "R 越界：{r}");
    }
}

// ---------------------------------------------------------------- 种类与加固

#[test]
fn kinds_carry_the_documented_strength_table() {
    // 计划 §4 的表：episode 起始 18 小时 / ×1.6 / 上限 60 天；
    // durable 起始 14 天 / ×1.8 / 上限约 2 年。
    assert_eq!(FactKind::Episode.initial_hours(), 18.0);
    assert_eq!(FactKind::Episode.growth(), 1.6);
    assert_eq!(FactKind::Episode.max_hours(), 60.0 * 24.0);
    assert_eq!(FactKind::Durable.initial_hours(), 14.0 * 24.0);
    assert_eq!(FactKind::Durable.growth(), 1.8);
    assert_eq!(FactKind::Durable.max_hours(), 730.0 * 24.0);

    // 解析：只认 durable，其余（含空 / 未知）回落 episode。
    assert_eq!(FactKind::parse(Some("durable")), FactKind::Durable);
    assert_eq!(FactKind::parse(Some(" episode ")), FactKind::Episode);
    assert_eq!(FactKind::parse(Some("nonsense")), FactKind::Episode);
    assert_eq!(FactKind::parse(None), FactKind::Episode);
    assert_eq!(FactKind::Episode.as_str(), "episode");
    assert_eq!(FactKind::Durable.as_str(), "durable");
}

#[test]
fn strengthening_multiplies_and_caps() {
    // ×1.6 / ×1.8，并且**封顶**。
    assert!((strengthen(FactKind::Episode, 18.0) - 28.8).abs() < 1e-9);
    assert!((strengthen(FactKind::Durable, 336.0) - 604.8).abs() < 1e-9);
    assert_eq!(
        strengthen(FactKind::Episode, FactKind::Episode.max_hours()),
        FactKind::Episode.max_hours(),
        "到上限就不再涨"
    );
    assert_eq!(
        strengthen(FactKind::Durable, 1e9),
        FactKind::Durable.max_hours()
    );
    // 强度坏了 → 从起始值重起算，不返回 0。
    assert_eq!(strengthen(FactKind::Episode, 0.0), 18.0 * 1.6);
    assert_eq!(strengthen(FactKind::Durable, f64::NAN), 336.0 * 1.8);
}

#[test]
fn facts_tier_by_retention_and_reinforce_resets_age() {
    let mut fact = HotFact::new(1, "用户喜欢薄荷", FactKind::Durable);
    assert_eq!(fact.tier(), Some(CardTier::Clear), "刚记下的必然清楚");
    assert_eq!(fact.retention(), 1.0);

    // durable 的 S = 14 天：过 14 天 → R ≈ 0.368（模糊档）。
    fact.age_hours = 14.0 * 24.0;
    assert_eq!(fact.tier(), Some(CardTier::Vague));

    // 加固后年龄归零、强度 ×1.8。
    fact.reinforce();
    assert_eq!(fact.age_hours, 0.0);
    assert!((fact.s_hours - 336.0 * 1.8).abs() < 1e-9);
    assert_eq!(fact.tier(), Some(CardTier::Clear));

    // episode 的遗忘判据：R < 0.05 且超过 180 天。
    let mut old = HotFact::new(2, "随便聊聊", FactKind::Episode);
    old.age_hours = 181.0 * 24.0;
    assert!(old.is_expired(), "180 天以上且 R 极低应过期");
    let mut young = HotFact::new(3, "随便聊聊", FactKind::Episode);
    young.age_hours = 179.0 * 24.0;
    assert!(!young.is_expired(), "不到 180 天不删");
    // durable **永不**因遗忘被删。
    let mut durable = HotFact::new(4, "用户叫星梦", FactKind::Durable);
    durable.age_hours = 1e6;
    assert!(!durable.is_expired(), "durable 只随用户清理删");
}

// ---------------------------------------------------------------- token 口径

#[test]
fn token_estimate_is_cjk_per_char_and_ascii_per_word() {
    assert_eq!(estimate_tokens(""), 0);
    // 1 CJK 字符 = 1 token。
    assert_eq!(estimate_tokens("你好"), 2);
    assert_eq!(estimate_tokens("你好吗"), 3);
    // 1 个 ASCII 词 = 1 token（连写只算一个）。
    assert_eq!(estimate_tokens("hello"), 1);
    assert_eq!(estimate_tokens("hello world"), 2);
    assert_eq!(estimate_tokens("hello-world"), 1, "连字符在词内不切");
    // 空白与 ASCII 标点不计；CJK 标点算 1。
    assert_eq!(estimate_tokens("  ,,, "), 0);
    assert_eq!(estimate_tokens("你好，hello"), 4, "你好，= 3 + hello = 1");
    // 与裁剪实现同口径。
    assert_eq!(trim_to_tokens("你好吗世界", 3), "你好吗");
    assert_eq!(trim_to_tokens("hello world", 1), "hello");
    assert_eq!(trim_to_tokens("短句", 10), "短句", "预算内原样返回");
}

// ---------------------------------------------------------------- 热卡

fn card_of(items: &[(CardTier, &str)]) -> Vec<CardLine> {
    items
        .iter()
        .map(|(tier, text)| CardLine::new(*tier, *text))
        .collect()
}

#[test]
fn hot_card_renders_exactly_three_line_kinds() {
    let card = HotCard::from_lines(
        card_of(&[
            (CardTier::Clear, "用户喜欢薄荷"),
            (CardTier::Vague, "好像提过一只猫"),
            (CardTier::Material, "薄荷是唇形科"),
        ]),
        &[],
    );
    assert_eq!(
        render_hot_card(Some(&card)),
        "- [清楚] 用户喜欢薄荷\n- [模糊] 好像提过一只猫\n- [资料] 薄荷是唇形科"
    );
    // 三档字面前缀就是这三种，没有第四种。
    assert_eq!(CardTier::Clear.prefix(), "- [清楚] ");
    assert_eq!(CardTier::Vague.prefix(), "- [模糊] ");
    assert_eq!(CardTier::Material.prefix(), "- [资料] ");
}

#[test]
fn retention_tiers_split_at_0_7_and_0_2() {
    assert_eq!(CardTier::from_retention(1.0), Some(CardTier::Clear));
    assert_eq!(
        CardTier::from_retention(0.7),
        Some(CardTier::Clear),
        "边界含 0.7"
    );
    assert_eq!(CardTier::from_retention(0.6999), Some(CardTier::Vague));
    assert_eq!(
        CardTier::from_retention(0.2),
        Some(CardTier::Vague),
        "边界含 0.2"
    );
    assert_eq!(CardTier::from_retention(0.1999), None, "R < 0.2 不进热卡");
    assert_eq!(CardTier::from_retention(f64::NAN), None);
}

#[test]
fn empty_card_renders_empty_and_writes_no_heading() {
    let empty = HotCard::empty();
    assert!(empty.is_empty());
    assert_eq!(render_hot_card(Some(&empty)), "");
    assert_eq!(render_hot_card(None), "");
    // 拼系统文本时：空热卡**不写标题**，只剩系统句。
    assert_eq!(compose_system_text("系统句", "", Some(&empty)), "系统句");
}

#[test]
fn hot_card_replaces_whole_block_and_never_appends() {
    // 「只替换、不追加」：第二张卡不包含第一张的任何残留。
    let first = HotCard::from_lines(card_of(&[(CardTier::Clear, "第一轮的命中")]), &[]);
    assert_eq!(render_hot_card(Some(&first)), "- [清楚] 第一轮的命中");
    let second = HotCard::from_lines(card_of(&[(CardTier::Clear, "第二轮的命中")]), &[]);
    assert_eq!(render_hot_card(Some(&second)), "- [清楚] 第二轮的命中");
    assert!(
        !render_hot_card(Some(&second)).contains("第一轮"),
        "新热卡必须是整块替换，不能带上一张的残留"
    );
    // 换成空卡 = 清空（不是「保留旧卡」）。
    assert_eq!(render_hot_card(Some(&HotCard::empty())), "");
}

#[test]
fn duplicate_and_already_spoken_lines_are_dropped() {
    let recent = vec!["用户喜欢薄荷".to_string(), "今天下雨".to_string()];
    let card = HotCard::from_lines(
        card_of(&[
            (CardTier::Clear, "用户喜欢薄荷"),
            (CardTier::Clear, "用户喜欢薄荷"),
            (CardTier::Vague, "今天下雨"),
            (CardTier::Clear, "用户住在杭州"),
        ]),
        &recent,
    );
    assert_eq!(
        render_hot_card(Some(&card)),
        "- [清楚] 用户住在杭州",
        "与最近原文重复的、以及卡内重复的行都要丢掉"
    );
}

#[test]
fn hot_card_fits_the_200_token_budget() {
    // 每条 10 个 CJK 字符 = 10 token，行前缀另计；造 30 条，断言不超预算且整行收。
    let long: Vec<String> = (0..30)
        .map(|i| format!("第{i:02}条足够长的记忆内容"))
        .collect();
    let items: Vec<CardLine> = long
        .iter()
        .map(|t| CardLine::new(CardTier::Clear, t.clone()))
        .collect();
    let card = HotCard::from_lines(items, &[]);
    let rendered = render_hot_card(Some(&card));
    assert!(
        card.tokens() <= HOT_CARD_BUDGET_TOKENS,
        "热卡必须落在 200 token 内"
    );
    assert!(!card.is_empty(), "至少要留下第一条");
    for line in rendered.lines() {
        assert!(line.starts_with("- [清楚] "), "只允许整行收：{line}");
        assert!(
            long.iter().any(|t| line == format!("- [清楚] {t}")),
            "不得截出半行：{line}"
        );
    }
}

// ---------------------------------------------------------------- 钉住块

#[test]
fn pinned_block_takes_only_clear_items_and_skips_spoken() {
    let items = vec![
        PinnedItem::new(1, "用户叫星梦", 0.95),
        PinnedItem::new(2, "用户住在杭州", 0.5), // 模糊 → 不钉
        PinnedItem::new(3, "用户养了一只猫", 0.8),
        PinnedItem::new(4, "上一轮已经说过", 0.9),
    ];
    assert_eq!(
        render_pinned(&items, &[]),
        "- 用户叫星梦\n- 用户养了一只猫\n- 上一轮已经说过"
    );
    // 说过的那条**跳过但仍在候选里**（这里只验渲染）。
    assert_eq!(
        render_pinned(&items, &[1]),
        "- 用户养了一只猫\n- 上一轮已经说过"
    );
    assert_eq!(
        render_pinned(&items, &[4]),
        "- 用户叫星梦\n- 用户养了一只猫"
    );
    assert_eq!(render_pinned(&items, &[1, 3, 4]), "");
    // 钉住块不带分档标记（与热卡不同）。
    assert!(!render_pinned(&items, &[]).contains("[清楚]"));
}

#[test]
fn pinned_block_fits_the_400_token_budget() {
    let items: Vec<PinnedItem> = (0..100)
        .map(|i| PinnedItem::new(i, format!("第{i:02}条很长的耐久事实内容啊"), 0.99))
        .collect();
    let rendered = render_pinned(&items, &[]);
    assert!(pinned_tokens(&items, &[]) <= PINNED_BUDGET_TOKENS);
    assert!(!rendered.is_empty());
    // 整行收：每一行都能在候选里逐字找到。
    for line in rendered.lines() {
        assert!(line.starts_with("- 第"), "整行收：{line}");
        assert!(!line.contains('\n'));
    }
}

// ---------------------------------------------------------------- 四块拼接

#[test]
fn system_text_is_sentence_plus_pinned_plus_hot_card() {
    let card = HotCard::from_lines(card_of(&[(CardTier::Material, "薄荷是唇形科")]), &[]);
    let text = compose_system_text("系统句", "- 用户叫星梦", Some(&card));
    assert_eq!(
        text,
        "系统句\n\n【钉住的事】\n- 用户叫星梦\n\n【本轮相关】\n- [资料] 薄荷是唇形科"
    );

    // 空块连同标题一起消失。
    assert_eq!(compose_system_text("系统句", "", None), "系统句");
    assert_eq!(
        compose_system_text("系统句", "- 一条", None),
        "系统句\n\n【钉住的事】\n- 一条"
    );
    // 系统句为空时也不留空段。
    assert_eq!(
        compose_system_text("", "- 一条", None),
        "【钉住的事】\n- 一条"
    );
    assert_eq!(compose_system_text("  ", "", None), "");
}

#[test]
fn factory_sentence_is_the_documented_text() {
    // 计划 §2 的出厂句**逐字**钉住（改文案必须改测试）。
    assert_eq!(
        SYSTEM_PROMPT_FACTORY,
        "你是长期陪伴的对方。只根据钉住的事、本轮热卡和最近几轮说话。\
清楚的可以自然提起；模糊的用不确定的说法，对方否认就放下。没有写在这两处的旧事不要编。"
    );
    // 约 80 字量级；按本模块 token 口径估一遍，防止悄悄写长。
    let tokens = estimate_tokens(SYSTEM_PROMPT_FACTORY);
    assert!(
        (60..=110).contains(&tokens),
        "出厂句应在 80 字量级，实际 {tokens} token"
    );
}

#[test]
fn prompt_budget_measures_the_three_blocks() {
    let card = HotCard::from_lines(card_of(&[(CardTier::Clear, "你好")]), &[]);
    let budget = PromptBudget::measure("系统句", "- 一条", Some(&card));
    assert_eq!(budget.system_sentence, 3);
    // 「- 一条」= 1（孤立的 ASCII `-` 算一个词）+ 2 个 CJK = 3 token。
    assert_eq!(budget.pinned, 3);
    assert!(budget.hot_card > 0);
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
