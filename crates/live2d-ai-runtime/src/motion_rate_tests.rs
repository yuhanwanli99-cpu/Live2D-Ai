//! `motion_rate` 的纯函数单测（经 `#[path = "motion_rate_tests.rs"]` 挂回，
//! 保持 `motion_rate.rs` 生产代码 ≤500 行）。
//!
//! 判据全部是**可失败**的：删掉任一分支、把 `take(1)` 改成整表、
//! 或让克制档也补 cue，都会有测试变红。

use super::*;

/// 一条最小的 legacy cue（只带本模块关心的字段）。
fn cue(id: &str, sentence_seq: u64) -> PerformanceCue {
    PerformanceCue {
        sentence_seq,
        preset_id: id.to_string(),
        intensity: 2,
        ttl_ms: 2_000,
        ..Default::default()
    }
}

fn ids(cues: &[PerformanceCue]) -> Vec<&str> {
    cues.iter().map(|c| c.preset_id.as_str()).collect()
}

/// 解析：缺省 / 垃圾 = 平常；三档稳定码往返。
#[test]
fn parse_defaults_to_normal_and_round_trips_codes() {
    for value in [None, Some(""), Some("normal"), Some("  "), Some("turbo")] {
        assert_eq!(MotionRate::parse(value), MotionRate::Normal, "{value:?}");
    }
    assert_eq!(MotionRate::parse(Some("sparse")), MotionRate::Sparse);
    assert_eq!(MotionRate::parse(Some("dense")), MotionRate::Dense);
    for rate in [MotionRate::Sparse, MotionRate::Normal, MotionRate::Dense] {
        assert_eq!(MotionRate::parse(Some(rate.as_str())), rate);
    }
    assert_eq!(MotionRate::default(), MotionRate::Normal, "缺省 = 平常");
}

/// 稀疏 + 无问号：这一句的 cue 全部清空（模型给的 / 回退的都不例外）。
#[test]
fn sparse_without_question_clears_every_cue() {
    let raw = "今天真开心。";
    assert!(!has_question_mark(raw), "前提：这一句没有问号");
    assert_eq!(
        filter_cues(MotionRate::Sparse, false, raw, 3, vec![cue("smile", 3)]),
        Vec::new(),
        "稀疏档无问号 = 这一句不动"
    );
}

/// 稀疏 + 有问号：最多留**第一条**（不追加、不改内容）。
#[test]
fn sparse_with_question_keeps_only_the_first_cue() {
    let raw = "你在想什么？";
    assert!(has_question_mark(raw), "前提：有问号");
    let kept = filter_cues(
        MotionRate::Sparse,
        false,
        raw,
        4,
        vec![cue("nod", 4), cue("smile", 4)],
    );
    assert_eq!(ids(&kept), ["nod"], "问句只留第一条");
    assert_eq!(kept[0], cue("nod", 4), "留下的那条逐字段不变（不重写 ttl）");

    // 空表 + 有问号 = 仍是空表（稀疏只删不补）。
    assert_eq!(
        filter_cues(MotionRate::Sparse, false, raw, 4, Vec::new()),
        Vec::new()
    );
}

/// 平常：不增不减（空表保持空表，非空表逐条原样）。
#[test]
fn normal_leaves_cues_untouched() {
    let raw = "今天真开心。";
    let given = vec![cue("smile", 5), cue("nod", 5)];
    assert_eq!(
        filter_cues(MotionRate::Normal, false, raw, 5, given.clone()),
        given
    );
    assert_eq!(
        filter_cues(MotionRate::Normal, true, raw, 5, given.clone()),
        given
    );
    assert_eq!(
        filter_cues(MotionRate::Normal, false, raw, 5, Vec::new()),
        Vec::new()
    );
}

/// 密集 + 平常档 + 本句空表 → 从规则表**只补第一条**（纯函数、不随机）。
#[test]
fn dense_under_normal_bias_fills_only_the_first_rule_cue() {
    let raw = "今天真开心。";
    let expected = rule_cues_for_sentence(raw, 7);
    assert_eq!(expected.len(), 1, "前提：规则表对这一句有动作");
    let filled = filter_cues(MotionRate::Dense, false, raw, 7, Vec::new());
    assert_eq!(filled, expected.into_iter().take(1).collect::<Vec<_>>());
    assert_eq!(filled[0].preset_id, "smile");
    assert_eq!(filled[0].sentence_seq, 7, "补出来的 cue 锚在这一句");
    assert_eq!(filled[0].ttl_ms, 2_000, "ttl 仍 2000（本模块不改 ttl）");
    // 恒等：同一输入两次输出一样（不随机）。
    assert_eq!(
        filter_cues(MotionRate::Dense, false, raw, 7, Vec::new()),
        filled
    );
}

/// 密集 + 克制档 → **不补**（克制就是宁可少一条）。
#[test]
fn dense_under_restrained_bias_does_not_fill() {
    let raw = "今天真开心。";
    assert!(
        !rule_cues_for_sentence(raw, 2).is_empty(),
        "前提：规则表有动作"
    );
    assert_eq!(
        filter_cues(MotionRate::Dense, true, raw, 2, Vec::new()),
        Vec::new(),
        "克制档不补"
    );
}

/// 密集 + 主动档上一步已填整张表（非空）→ **不再加**；也不清掉。
#[test]
fn dense_never_adds_on_top_of_existing_cues() {
    let raw = "今天真开心。";
    let already = rule_cues_for_sentence(raw, 9);
    assert!(!already.is_empty());
    let kept = filter_cues(MotionRate::Dense, false, raw, 9, already.clone());
    assert_eq!(kept, already, "非空表原样返回（主动档填表后不再补）");
}

/// 密集 + 规则表对这句原文没有动作 → 不补（不硬造、不新增预设）。
#[test]
fn dense_does_not_fill_when_the_rules_are_empty() {
    let raw = "嗯。";
    assert_eq!(rule_cues_for_sentence(raw, 6), Vec::new(), "前提：规则表空");
    assert_eq!(
        filter_cues(MotionRate::Dense, false, raw, 6, Vec::new()),
        Vec::new(),
        "规则表空 = 不补"
    );
}
