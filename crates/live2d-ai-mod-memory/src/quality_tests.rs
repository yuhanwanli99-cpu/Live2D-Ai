//! 检索质量基线（Wave 3 C 轨）：固定语料 fixtures + 「查询 → 期望命中」断言。
//!
//! 语料是**固定 fixture**（[`tests/fixtures/quality_corpus.json`]，`include_str!`），
//! 不是凭手感调的阈值。覆盖：
//! - 中文 bigram 命中（`zh-weather-shares-bigrams` / `zh-movie-shares-one-bigram`）；
//! - **词面不重叠 → 0 命中 → 不注入**（`zh-disjoint-no-hit`，运行时级断言
//!   `last_hits=0` + 零 patch）；
//! - 同义改写不命中（`zh-paraphrase-no-lexical-overlap`）——这是 v0 的**明文边界**，
//!   写进 fixture note，不装作有语义检索；
//! - ASCII 大小写不敏感（`ascii-token-overlap`）。

use std::collections::BTreeSet;
use std::path::Path;

use super::*;
use crate::store::JsonlStore;
use crate::strategy::{rank_top_k, tokenize};
use crate::test_support::*;

const CORPUS_JSON: &str = include_str!("../tests/fixtures/quality_corpus.json");

fn fixture() -> serde_json::Value {
    serde_json::from_str(CORPUS_JSON).expect("quality_corpus.json 必须是合法 JSON")
}

fn corpus_records(value: &serde_json::Value) -> Vec<MemoryRecord> {
    value["records"]
        .as_array()
        .expect("records 必须是数组")
        .iter()
        .map(|r| {
            MemoryRecord::new(
                r["text"].as_str().expect("record.text"),
                r["ts"].as_i64().expect("record.ts"),
                r["turn"].as_u64().expect("record.turn"),
            )
        })
        .collect()
}

fn cases(value: &serde_json::Value) -> Vec<serde_json::Value> {
    value["cases"].as_array().expect("cases 必须是数组").clone()
}

fn expect_hits(case: &serde_json::Value) -> Vec<String> {
    case["expect_hits"]
        .as_array()
        .expect("expect_hits 必须是数组")
        .iter()
        .map(|t| t.as_str().expect("expect_hits 元素").to_string())
        .collect()
}

fn case_by_name(value: &serde_json::Value, name: &str) -> serde_json::Value {
    cases(value)
        .into_iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("fixture 缺少样例 {name}"))
}

fn rank_texts(query: &str, records: &[MemoryRecord], k: usize) -> Vec<String> {
    rank_top_k(query, records, k)
        .into_iter()
        .map(|hit| records[hit.index].text.clone())
        .collect()
}

fn seed_store(dir: &Path, records: &[MemoryRecord]) -> JsonlStore {
    let store = JsonlStore::new(dir.join("memory.jsonl"));
    for record in records {
        store.append(record).expect("fixture 语料必须能落盘");
    }
    store
}

// ---------------------------------------------------------------- fixture 形状

#[test]
fn fixture_shape_and_referential_integrity() {
    let fixture = fixture();
    assert_eq!(fixture["version"], 1);
    let records = corpus_records(&fixture);
    assert!(records.len() >= 5, "固定语料要够覆盖多组查询");
    let case_list = cases(&fixture);
    assert!(case_list.len() >= 5, "至少覆盖命中/不命中/改写/ASCII");

    let texts: BTreeSet<&str> = records.iter().map(|r| r.text.as_str()).collect();
    let mut names = BTreeSet::new();
    let mut has_cjk_query = false;
    let mut has_disjoint_case = false;
    for case in &case_list {
        let name = case["name"].as_str().expect("case.name");
        assert!(names.insert(name), "case 名不能重复: {name}");
        let query = case["query"].as_str().expect("case.query");
        assert!(!query.trim().is_empty());
        if query.chars().any(|c| ('一'..='鿿').contains(&c)) {
            has_cjk_query = true;
        }
        let hits = expect_hits(case);
        for hit in &hits {
            assert!(
                texts.contains(hit.as_str()),
                "期望命中必须来自语料（引用完整性）: {hit}"
            );
        }
        let note = case["note"].as_str().expect("case.note");
        if hits.is_empty() && note.contains("不重叠") {
            has_disjoint_case = true;
        }
    }
    assert!(has_cjk_query, "§3C 要求至少一组中文");
    assert!(has_disjoint_case, "必须有一组『词面不重叠 → 不命中』并写明");
}

// ------------------------------------------------------------ 纯检索层断言

#[test]
fn fixture_hit_cases_rank_expected_records_in_order() {
    let fixture = fixture();
    let records = corpus_records(&fixture);
    let mut checked = 0;
    for case in cases(&fixture) {
        let expected = expect_hits(&case);
        if expected.is_empty() {
            continue;
        }
        let query = case["query"].as_str().unwrap();
        let actual = rank_texts(query, &records, 10);
        assert_eq!(actual, expected, "case {} 的命中序列不符", case["name"]);
        checked += 1;
    }
    assert!(checked >= 3, "至少要有 3 组正向命中样例");
}

#[test]
fn fixture_no_hit_cases_rank_nothing() {
    let fixture = fixture();
    let records = corpus_records(&fixture);
    let mut checked = 0;
    for case in cases(&fixture) {
        if !expect_hits(&case).is_empty() {
            continue;
        }
        let query = case["query"].as_str().unwrap();
        assert!(
            rank_top_k(query, &records, 10).is_empty(),
            "case {} 不应命中任何语料",
            case["name"]
        );
        checked += 1;
    }
    assert!(checked >= 2, "至少 2 组负向样例（不重叠 + 同义改写）");
}

#[test]
fn fixture_paraphrase_is_a_documented_lexical_gap() {
    let fixture = fixture();
    let records = corpus_records(&fixture);
    let case = case_by_name(&fixture, "zh-paraphrase-no-lexical-overlap");
    let query = case["query"].as_str().unwrap();
    let query_tokens: BTreeSet<String> = tokenize(query).into_iter().collect();
    for record in &records {
        let record_tokens: BTreeSet<String> = tokenize(&record.text).into_iter().collect();
        assert!(
            query_tokens.is_disjoint(&record_tokens),
            "该样例必须与语料零共享词元，否则不叫『词面不重叠』: {}",
            record.text
        );
    }
    assert!(rank_top_k(query, &records, 10).is_empty());
    let note = case["note"].as_str().unwrap();
    assert!(
        note.contains("词面") || note.contains("非目标") || note.contains("边界"),
        "v0 的词面边界必须写进 fixture note，不能只藏在实现里: {note}"
    );
}

#[test]
fn fixture_ascii_query_is_case_insensitive() {
    let fixture = fixture();
    let records = corpus_records(&fixture);
    assert_eq!(
        rank_texts("REMEMBER the API2 TOKEN Rotation", &records, 10),
        vec!["API2 token rotation policy is documented".to_string()]
    );
}

// ------------------------------------------------------------ 运行时级断言

#[test]
fn fixture_chinese_disjoint_turn_is_not_injected() {
    let fixture = fixture();
    let records = corpus_records(&fixture);
    let case = case_by_name(&fixture, "zh-disjoint-no-hit");
    let query = case["query"].as_str().unwrap().to_string();

    let dir = temp_dir("quality-disjoint");
    let host = FakeHost::new("基础人设");
    seed_store(&dir, &records);
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, &query).unwrap();

    let snap = rt.state_json().unwrap();
    assert_eq!(snap["last_hits"], 0, "词面不重叠 → 0 命中");
    assert_eq!(snap["hits"], 0, "累计命中仍为 0");
    assert_eq!(snap["injects"], 0, "不命中 → 不注入");
    assert!(host.patches().is_empty(), "不得写 persona.system_prompt");
    assert_eq!(host.main_prompt(), "基础人设");
    assert_eq!(snap["writes"], 1, "查询本身仍然要记下来");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn fixture_relevant_turn_injects_expected_memory() {
    let fixture = fixture();
    let records = corpus_records(&fixture);
    let case = case_by_name(&fixture, "zh-movie-shares-one-bigram");
    let query = case["query"].as_str().unwrap().to_string();
    let expected = expect_hits(&case);

    let dir = temp_dir("quality-hit");
    let host = FakeHost::new("基础人设");
    // P1-5：有 conversation 时检索/注入的都是**会话桶**。
    std::fs::create_dir_all(dir.join("sessions")).unwrap();
    let session_store = JsonlStore::new(dir.join("sessions").join("s1.memory.jsonl"));
    for record in &records {
        session_store
            .append(record)
            .expect("fixture 语料必须能落盘");
    }
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, &query, Some("s1"))
        .unwrap();

    let block = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
        .expect("应注入会话槽");
    for hit in &expected {
        assert!(
            block.contains(&format!("- [用户] {hit}")),
            "缺少期望记忆 {hit}: {block}"
        );
    }
    assert_eq!(block.matches(MEMORY_MARKER_BEGIN).count(), 1);
    assert_eq!(block.matches(MEMORY_MARKER_END).count(), 1);
    // 绝不写全局 persona.system_prompt。
    assert!(host.patches().is_empty());
    assert_eq!(host.main_prompt(), "基础人设");
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["writes"], 1, "只写了本轮查询这一条");
    assert_eq!(snap["injects"], 1);
    assert_eq!(snap["hits"], expected.len() as u64);
    let _ = std::fs::remove_dir_all(dir);
}
