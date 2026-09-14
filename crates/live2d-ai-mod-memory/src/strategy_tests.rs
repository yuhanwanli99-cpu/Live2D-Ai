//! `strategy` 的纯逻辑单测（拆出来是为了让 `strategy.rs` 保持 ≤500 行源码）。

use std::path::PathBuf;

use crate::strategy::*;

fn rec(text: &str, ts: i64, turn: u64) -> MemoryRecord {
    MemoryRecord {
        text: text.to_string(),
        ts,
        turn,
    }
}

// ---------------------------------------------------------------- 分词

#[test]
fn tokenize_mixes_cjk_bigrams_and_ascii_words() {
    let tokens = tokenize("今天 weather 很好 API2");
    assert!(tokens.contains(&"今天".to_string()), "{tokens:?}");
    assert!(tokens.contains(&"天气".to_string()) || tokens.contains(&"很好".to_string()));
    assert!(tokens.contains(&"weather".to_string()), "{tokens:?}");
    assert!(tokens.contains(&"api2".to_string()), "{tokens:?}");
    // 标点/空格不产出词元。
    assert!(!tokens.iter().any(|t| t.contains(' ')));
}

#[test]
fn tokenize_drops_stopwords_both_scripts() {
    let tokens = tokenize("the 的 了 memory");
    assert!(!tokens.contains(&"the".to_string()));
    assert!(!tokens.contains(&"的".to_string()));
    assert!(!tokens.contains(&"了".to_string()));
    assert!(tokens.contains(&"memory".to_string()));
}

#[test]
fn tokenize_single_cjk_char_survives_unless_stopword() {
    // 单字成段：非停用字保留（否则一个字的查询什么都检索不到）。
    assert_eq!(tokenize("猫"), vec!["猫".to_string()]);
    // 单字停用词丢弃。
    assert!(tokenize("的").is_empty());
}

#[test]
fn tokenize_empty_and_punctuation_only_is_empty() {
    assert!(tokenize("").is_empty());
    assert!(tokenize("   ").is_empty());
    assert!(tokenize("，。！？…—（）").is_empty());
    assert!(tokenize("a").is_empty(), "长度为 1 的 ASCII 词丢弃");
}

// ---------------------------------------------------------------- 打分

#[test]
fn overlap_score_is_normalized_jaccard() {
    let q = tokenize("今天天气");
    assert_eq!(overlap_score(&q, &q), 1.0, "自己与自己 = 1.0");
    assert_eq!(
        overlap_score(&q, &tokenize("完全不相干的外语")),
        0.0,
        "无交集 = 0.0"
    );
    let partial = overlap_score(&tokenize("今天天气很好"), &tokenize("今天天气不错"));
    assert!(
        partial > 0.0 && partial < 1.0,
        "部分重叠 ∈ (0,1): {partial}"
    );
}

#[test]
fn overlap_score_ignores_duplicates_and_empty_sides() {
    let q = tokenize("记忆");
    let dup = tokenize("记忆 记忆 记忆");
    assert_eq!(overlap_score(&q, &dup), 1.0, "重复词元不抬高分数");
    assert_eq!(overlap_score(&[], &dup), 0.0);
    assert_eq!(overlap_score(&q, &[]), 0.0);
}

// ---------------------------------------------------------------- top-k

#[test]
fn rank_top_k_orders_by_score_desc() {
    let records = vec![
        rec("今天天气很好", 1, 1),
        rec("今天天气不错，出门走走", 2, 2),
        rec("今天下雨", 3, 3),
    ];
    let hits = rank_top_k("今天天气怎么样", &records, 3);
    assert_eq!(hits.len(), 3, "三条都有重叠");
    assert!(hits[0].score >= hits[1].score && hits[1].score >= hits[2].score);
    for hit in &hits {
        assert!((0.0..=1.0).contains(&hit.score));
    }
}

#[test]
fn rank_top_k_excludes_zero_score_records() {
    let records = vec![rec("今天天气很好", 1, 1), rec("量子力学导论", 2, 2)];
    let hits = rank_top_k("今天天气", &records, 5);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].index, 0);
}

#[test]
fn rank_top_k_clamps_k_and_handles_empty_query() {
    let records = vec![rec("今天天气很好", 1, 1), rec("今天天气不错", 2, 2)];
    assert!(rank_top_k("今天天气", &records, 0).is_empty(), "k=0 → 空");
    assert_eq!(
        rank_top_k("今天天气", &records, 99).len(),
        2,
        "k > 命中数 → 全部命中"
    );
    assert!(
        rank_top_k("", &records, 3).is_empty(),
        "空查询不检索任何东西"
    );
    assert!(
        rank_top_k("，。！", &records, 3).is_empty(),
        "纯标点查询同样为空"
    );
}

#[test]
fn rank_top_k_tie_break_prefers_newer() {
    // 同一份正文 → 分数完全相同；平局按 ts 新→旧。
    let records = vec![rec("今天天气很好", 100, 1), rec("今天天气很好", 300, 2)];
    let hits = rank_top_k("今天天气", &records, 2);
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].score, hits[1].score);
    assert_eq!(hits[0].index, 1, "ts 更大的（更新）排前面");
    assert_eq!(hits[1].index, 0);
}

#[test]
fn rank_top_k_tie_break_falls_back_to_turn_then_index() {
    // ts 相同 → 比 turn（大→小）；ts/turn 都相同 → 比 index（大→小，稳定兜底）。
    let records = vec![
        rec("今天天气很好", 5, 1),
        rec("今天天气很好", 5, 9),
        rec("今天天气很好", 5, 9),
    ];
    let hits = rank_top_k("今天天气", &records, 3);
    assert_eq!(hits[0].index, 2, "turn 大者在前；turn 同则 index 大者在前");
    assert_eq!(hits[1].index, 1);
    assert_eq!(hits[2].index, 0);
}

// ---------------------------------------------------------------- marker

#[test]
fn compose_injection_shape_and_marker() {
    let out = compose_injection("基础人设", &["第一条记忆".to_string()]);
    assert!(out.starts_with("基础人设\n\n"));
    assert!(out.contains(MEMORY_MARKER_BEGIN));
    assert!(out.contains("- 第一条记忆"));
    assert!(out.ends_with(MEMORY_MARKER_END));
}

#[test]
fn compose_injection_with_empty_memories_has_no_marker() {
    let out = compose_injection("基础人设", &[]);
    assert_eq!(out, "基础人设");
    assert!(!contains_memory_block(&out));
    let whitespace = compose_injection("基础人设", &["   ".to_string(), "\n".to_string()]);
    assert_eq!(whitespace, "基础人设");
}

#[test]
fn compose_injection_is_idempotent_and_replaces_old_block() {
    let first = compose_injection("基础人设", &["旧记忆".to_string()]);
    let second = compose_injection(&first, &["新记忆".to_string()]);
    assert!(!second.contains("旧记忆"), "旧块必须先被剥掉");
    assert!(second.contains("- 新记忆"));
    assert_eq!(
        second.matches(MEMORY_MARKER_BEGIN).count(),
        1,
        "重拼后只有一个块"
    );
    // 同样输入重复拼 → 字节相同。
    assert_eq!(compose_injection(&second, &["新记忆".to_string()]), second);
}

#[test]
fn strip_memory_block_is_idempotent_and_tolerates_half_block() {
    let base = "基础人设\n\n第二段";
    let composed = compose_injection(base, &["一条".to_string()]);
    assert_eq!(strip_memory_block(&composed), base);
    assert_eq!(
        strip_memory_block(&strip_memory_block(&composed)),
        strip_memory_block(&composed)
    );
    // 缺结束标记的半截块：剥到末尾，而不是原样留下。
    let broken = format!("基础人设\n\n{MEMORY_MARKER_BEGIN}\n- 半截");
    assert_eq!(strip_memory_block(&broken), "基础人设");
    assert_eq!(strip_memory_block("没有块"), "没有块");
}

#[test]
fn compose_injection_dedupes_identical_lines() {
    let out = compose_injection(
        "基础人设",
        &[
            "同一条".to_string(),
            "另一条".to_string(),
            "同一条".to_string(),
        ],
    );
    assert_eq!(out.matches("- 同一条").count(), 1, "{out}");
    assert_eq!(out.matches("- 另一条").count(), 1);
}

#[test]
fn sanitize_memory_line_flattens_and_truncates() {
    assert_eq!(sanitize_memory_line("  多行\n\t记忆  "), "多行 记忆");
    assert_eq!(sanitize_memory_line("   "), "");
    let long = "字".repeat(MAX_MEMORY_LINE_CHARS + 10);
    let line = sanitize_memory_line(&long);
    assert_eq!(line.chars().count(), MAX_MEMORY_LINE_CHARS + 1);
    assert!(line.ends_with('…'));
}

// ------------------------------------------------------------ 路径解析

#[test]
fn resolve_store_path_defaults_to_config_dir() {
    let p = resolve_store_path("/etc/live2d-ai/live2d-ai.toml", "").unwrap();
    assert_eq!(p, PathBuf::from("/etc/live2d-ai/memory.jsonl"));
}

#[test]
fn resolve_store_path_relative_explicit_is_anchored_to_config_dir() {
    let p = resolve_store_path("/srv/live2d/live2d-ai.toml", "data/mem.jsonl").unwrap();
    assert_eq!(p, PathBuf::from("/srv/live2d/data/mem.jsonl"));
    let abs = resolve_store_path("/srv/live2d/live2d-ai.toml", "/tmp/mem.jsonl").unwrap();
    assert_eq!(abs, PathBuf::from("/tmp/mem.jsonl"));
}

#[test]
fn resolve_store_path_without_config_path_is_none_only_for_default() {
    assert_eq!(
        resolve_store_path("", ""),
        None,
        "缺省路径 + 无 config_path → 必须 no-op"
    );
    assert_eq!(
        resolve_store_path("live2d-ai.toml", ""),
        None,
        "配置文件名没有目录部分 → 同一条 no-op"
    );
    assert_eq!(
        resolve_store_path("", "mem.jsonl"),
        Some(PathBuf::from("mem.jsonl")),
        "显式相对路径在无 config_path 时相对 cwd"
    );
}
