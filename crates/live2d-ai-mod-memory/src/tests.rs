//! 运行时级回归：Mod 生命周期 + `TurnPrompt` 一轮的记/检索/注入 + 物理淘汰 +
//! 计数 + 与 persona 共存（last-writer-wins）+ 停用清残留。
//!
//! 纯逻辑（分词 / 打分 / top-k / marker / 路径）的单测在 `strategy_tests.rs`，
//! JSONL 读写与物理淘汰的单测在 `store.rs`，检索质量基线在 `quality_tests.rs`；
//! 这里只测把它们接起来的行为。替身与脚手架在 `test_support.rs`（两个模块共用）。

use std::path::PathBuf;

use super::*;
use crate::test_support::*;

// ---------------------------------------------------------------- 生命周期

#[test]
fn start_registers_schema_and_subscribes_turn_prompt() {
    let dir = temp_dir("start");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    let mut reg = RecordingRegistrar::default();
    rt.start(&mut reg).expect("start 应成功");
    assert!(rt.is_registered());
    assert_eq!(reg.specs.len(), 1);
    assert_eq!(reg.specs[0].mod_id, "memory");
    assert!(reg.specs[0].validate().is_ok(), "字段 key 必须唯一");
    assert_eq!(
        reg.topics,
        vec![
            ModEventTopic::TurnPrompt,
            // Wave 3（2026-09-21）：助手侧正文。
            ModEventTopic::AssistantReplied,
        ]
    );
    rt.shutdown().unwrap();
    assert!(!rt.is_registered());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn descriptor_and_settings_spec_declare_no_second_enabled() {
    assert_eq!(DESCRIPTOR.id, "memory");
    assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
    let spec = memory_settings_spec();
    let keys: Vec<&str> = spec.fields.iter().map(ModSettingField::key).collect();
    assert_eq!(
        keys,
        vec![
            "store_path",
            "top_k",
            "max_records",
            "enabled_injection",
            // P1-5 真摘要：独立于主链 [llm] 的一组配置。
            "summary_enabled",
            "summary_base_url",
            "summary_model",
            "summary_api_key_env",
            "summary_timeout_ms",
            "summary_ratio",
            "summary_cooldown_turns",
            "summary_keep_recent_turns",
        ]
    );
    assert!(
        !keys.contains(&"enabled"),
        "启停唯一真源是 manifest，schema 不许有第二个 enabled"
    );
    assert!(
        !keys.contains(&"summary_api_key"),
        "密钥只存变量名（summary_api_key_env），值不进配置"
    );
    assert_eq!(spec.mod_id, "memory");
    assert_eq!(spec.version, 2, "P1-5 追加摘要字段：schema 版本 +1");
}

#[test]
fn factory_creates_runtime_with_static_spec() {
    let host = FakeHost::new("基础人设");
    assert_eq!(FACTORY.descriptor().id, "memory");
    assert!(
        FACTORY.settings_spec().is_some(),
        "静态 schema 必须可取自未启用状态"
    );
    let rt: Box<dyn ModRuntime> = FACTORY
        .create(host.services(""), serde_json::json!({}))
        .expect("create 应成功");
    drop(rt);
}

#[test]
fn non_turn_prompt_topics_are_ignored() {
    let dir = temp_dir("ignore");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnStarted, "7").unwrap();
    rt.on_event(ModEventTopic::TextDelta, "你好").unwrap();
    assert!(rt.retrieve("你好").is_empty(), "非 TurnPrompt 不记不注入");
    assert!(host.patches().is_empty());
    assert!(!dir.join("memory.jsonl").exists());
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 记 / 检索

#[test]
fn first_turn_is_remembered_but_not_injected() {
    let dir = temp_dir("first");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    let lines = std::fs::read_to_string(dir.join("memory.jsonl")).unwrap();
    assert_eq!(lines.lines().count(), 1, "正文必须落盘");
    assert!(lines.contains("今天天气很好"));
    assert!(
        host.patches().is_empty(),
        "刚记住的当轮正文不得自命中并注入（无历史 = no-op）"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn related_second_turn_injects_previous_memory_into_the_session_slot() {
    let dir = temp_dir("inject");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "今天天气很好", Some("s1"))
        .unwrap();
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "今天天气不错，出门走走",
        Some("s1"),
    )
    .unwrap();

    let block = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
        .expect("会话槽必须有记忆块");
    assert!(block.contains(MEMORY_MARKER_BEGIN), "{block}");
    assert!(block.contains("- [用户] 今天天气很好"), "{block}");
    assert!(block.ends_with(MEMORY_MARKER_END), "{block}");
    // P1-5：有 conversation 时**绝不**写全局 persona.system_prompt。
    assert!(
        host.patches().is_empty(),
        "会话注入不得写 persona.system_prompt"
    );
    assert_eq!(host.main_prompt(), "基础人设");
    assert_eq!(rt.state_json().unwrap()["injects"], 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unrelated_turn_does_not_inject() {
    let dir = temp_dir("unrelated");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "量子力学导论")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错")
        .unwrap();
    assert!(host.patches().is_empty(), "零相关 = no-op，不写盘");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn injection_disabled_still_remembers_but_never_writes_config() {
    let dir = temp_dir("noinject");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"enabled_injection": false}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.join("memory.jsonl"))
            .unwrap()
            .lines()
            .count(),
        2,
        "关掉注入不影响记忆"
    );
    assert!(host.patches().is_empty());
    assert_eq!(host.main_prompt(), "基础人设");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn identical_repeat_turn_is_idempotent_no_second_inject() {
    let dir = temp_dir("idem");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    for _ in 0..3 {
        rt.on_scoped_event(ModEventTopic::TurnPrompt, "今天天气很好", Some("s1"))
            .unwrap();
    }
    let block = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
        .unwrap_or_default();
    assert_eq!(
        block.matches("- [用户] 今天天气很好").count(),
        1,
        "同一句话不重复注入: {block}"
    );
    // 第 2 / 3 轮重拼结果与槽内一致 -> 不再计注入（幂等）。
    assert_eq!(rt.state_json().unwrap()["injects"], 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn max_records_is_a_physical_count_cap_not_just_a_window() {
    let dir = temp_dir("cap");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"max_records": 2}));
    for text in ["第一条主题", "第二条主题", "第三条主题"] {
        rt.on_event(ModEventTopic::TurnPrompt, text).unwrap();
    }
    // Wave 3：第 N+1 条写入后，**文件里**只剩 N 条（不是只把载入窗口调窄）。
    let raw = std::fs::read_to_string(dir.join("memory.jsonl")).unwrap();
    assert_eq!(raw.lines().count(), 2, "第 3 条写入后文件里只剩 N=2 条");
    assert!(!raw.contains("第一条主题"), "最旧的被物理删除: {raw}");
    assert!(raw.contains("第二条主题") && raw.contains("第三条主题"));
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["writes"], 3);
    assert_eq!(snap["evicted"], 1, "淘汰计数必须可观察");
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 边界 / no-op

#[test]
fn empty_prompt_is_noop() {
    let dir = temp_dir("empty");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "   \n\t ").unwrap();
    assert!(!dir.join("memory.jsonl").exists(), "空正文不落盘");
    assert!(host.patches().is_empty());
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["turns_seen"], 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn missing_store_path_is_warned_noop_not_panic() {
    let host = FakeHost::new("基础人设");
    // 无 config_path、store_path 空 → 路径不可解析：必须 no-op + warn。
    let mut rt = MemoryRuntime::new(
        host.services(""),
        MemoryConfig::from_value(&serde_json::json!({})),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    assert!(host.patches().is_empty());
    assert!(
        host.logs().iter().any(|l| l.contains("存储路径未配置")),
        "必须留下可排障的 warn: {:?}",
        host.logs()
    );
    assert_eq!(
        rt.state_json().unwrap()["store_path"],
        serde_json::Value::Null
    );
}

#[test]
fn missing_session_capability_is_warned_not_fatal() {
    let dir = temp_dir("reject");
    let host = FakeHost::new("基础人设");
    host.sessions().set_enabled(false);
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "今天天气很好", Some("s1"))
        .unwrap();
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "今天天气不错，出门走走",
        Some("s1"),
    )
    .unwrap();
    assert!(host.patches().is_empty());
    assert_eq!(host.main_prompt(), "基础人设");
    assert!(
        host.logs()
            .iter()
            .any(|l| l.contains("宿主未注入会话提示词能力")),
        "能力缺失必须可见: {:?}",
        host.logs()
    );
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["injected"], 0, "能力缺失不计数");
    assert_eq!(snap["remembered"], 2, "记忆仍然照记");
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 停用 / 状态

#[test]
fn shutdown_strips_legacy_global_block_and_own_session_slot() {
    let dir = temp_dir("shutdown");
    let host = FakeHost::new("基础人设");
    // 模拟**旧版本**留下的全局注入块（P1-5 起本 Mod 不再写它，但停用仍要清理）。
    let legacy = crate::strategy::compose_injection("基础人设", &["旧块".to_string()]);
    host.set_main_prompt(&legacy);
    host.sessions()
        .set_slot(SESSION_PROMPT_OWNER_MEMORY, "s1", "记忆块");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.shutdown().unwrap();
    assert_eq!(host.main_prompt(), "基础人设", "停用必须清掉旧注入块");
    assert!(
        host.sessions()
            .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
            .is_none(),
        "停用必须清掉本 Mod 的会话槽"
    );
    assert!(!rt.is_registered());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shutdown_without_block_does_not_write() {
    let dir = temp_dir("shutdown-clean");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.shutdown().unwrap();
    assert!(host.patches().is_empty(), "没有残留就不该写盘");
    assert_eq!(host.main_prompt(), "基础人设");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn state_json_snapshot_shape_is_read_only() {
    let dir = temp_dir("state");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(
        &dir,
        &host,
        serde_json::json!({"top_k": 5, "max_records": 42, "enabled_injection": false}),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    let snap = rt.state_json().expect("必须提供快照");
    assert_eq!(snap["top_k"], 5);
    assert_eq!(snap["max_records"], 42);
    assert_eq!(snap["enabled_injection"], false);
    assert_eq!(snap["turns_seen"], 1);
    assert_eq!(snap["remembered"], 1);
    assert!(
        snap["store_path"]
            .as_str()
            .unwrap()
            .ends_with("memory.jsonl"),
        "缺省路径必须落在配置文件同目录: {snap}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- 配置

#[test]
fn config_defaults_and_clamping() {
    assert_eq!(
        MemoryConfig::from_value(&serde_json::json!({})),
        MemoryConfig::default()
    );
    let clamped = MemoryConfig::from_value(&serde_json::json!({
        "top_k": 99,
        "max_records": 0,
        "enabled_injection": false,
        "store_path": "  /tmp/mem.jsonl  ",
    }));
    assert_eq!(clamped.top_k, MAX_TOP_K);
    assert_eq!(clamped.max_records, MIN_MAX_RECORDS, "0 → 钳到下限而非关掉");
    assert!(!clamped.enabled_injection);
    assert_eq!(
        clamped.store_path, "  /tmp/mem.jsonl  ",
        "路径原样保留，解析时 trim"
    );
    // 类型不对 → 用缺省，不失败。
    let bad = MemoryConfig::from_value(&serde_json::json!({
        "top_k": "3", "max_records": null, "enabled_injection": 1,
    }));
    assert_eq!(bad.top_k, DEFAULT_TOP_K);
    assert_eq!(bad.max_records, DEFAULT_MAX_RECORDS);
    assert!(bad.enabled_injection);
    // 负数钳到下限。
    let negative = MemoryConfig::from_value(&serde_json::json!({"top_k": -4}));
    assert_eq!(negative.top_k, MIN_TOP_K);
}

#[test]
fn config_store_path_resolution_matches_strategy() {
    let cfg = MemoryConfig::from_value(&serde_json::json!({"store_path": "sub/mem.jsonl"}));
    assert_eq!(
        cfg.resolve_store_path("/etc/live2d/live2d-ai.toml"),
        Some(PathBuf::from("/etc/live2d/sub/mem.jsonl"))
    );
    let default_cfg = MemoryConfig::default();
    assert_eq!(
        default_cfg.resolve_store_path("/etc/live2d/live2d-ai.toml"),
        Some(PathBuf::from("/etc/live2d/memory.jsonl"))
    );
    assert_eq!(default_cfg.resolve_store_path(""), None);
}

// ------------------------------------------------- Wave 3：计数 / 共存 / 无命中

#[test]
fn no_related_memory_reports_zero_hits_and_never_injects() {
    let dir = temp_dir("nohit");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "量子力学导论")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "帮我写一首关于星空的诗")
        .unwrap();
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["last_hits"], 0, "词面不重叠 → 不命中");
    assert_eq!(snap["hits"], 0, "累计命中仍为 0");
    assert_eq!(snap["injects"], 0, "不命中 → 不注入");
    assert!(host.patches().is_empty(), "零命中不得写配置");
    assert_eq!(host.main_prompt(), "基础人设");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn state_json_exposes_the_four_required_counters() {
    let dir = temp_dir("counters");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    // 轮 1：写入、无命中、不注入。
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "今天天气很好", Some("s1"))
        .unwrap();
    let first = rt.state_json().unwrap();
    assert_eq!(first["writes"], 1);
    assert_eq!(first["hits"], 0);
    assert_eq!(first["injects"], 0);
    assert_eq!(first["errors"], 0);
    assert_eq!(first["last_hits"], 0);
    // 轮 2：命中上一条并注入到会话槽。
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "今天天气不错，出门走走",
        Some("s1"),
    )
    .unwrap();
    let second = rt.state_json().unwrap();
    assert_eq!(second["writes"], 2);
    assert!(second["hits"].as_u64().unwrap() >= 1, "{second}");
    assert_eq!(second["injects"], 1);
    assert_eq!(second["errors"], 0);
    // 四个键必须是数字，且 Wave 2 别名与它们恒同值。
    for key in ["writes", "hits", "injects", "errors"] {
        assert!(second[key].is_u64(), "{key} 必须是数字: {second}");
    }
    assert_eq!(second["remembered"], second["writes"]);
    assert_eq!(second["injected"], second["injects"]);
    // P1-5：预算 / conversation 键必须在快照里可观察。
    assert!(second["injection_budget_chars"].is_u64(), "{second}");
    assert!(second["budget_used"].is_u64(), "{second}");
    assert!(second["budget_ratio"].is_number(), "{second}");
    assert_eq!(second["conversation_id"], "s1", "{second}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn errors_counter_counts_unresolved_path_and_missing_session_capability() {
    // 路径不可解析 → errors 1、writes 0。
    let host = FakeHost::new("基础人设");
    let mut broken = MemoryRuntime::new(
        host.services(""),
        MemoryConfig::from_value(&serde_json::json!({})),
    );
    broken
        .on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    let snap = broken.state_json().unwrap();
    assert_eq!(snap["errors"], 1);
    assert_eq!(snap["writes"], 0);

    // 会话能力缺失 → errors 1（记忆照记，不 fatal）。
    let dir = temp_dir("errors");
    let host = FakeHost::new("基础人设");
    host.sessions().set_enabled(false);
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "今天天气很好", Some("s1"))
        .unwrap();
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "今天天气不错，出门走走",
        Some("s1"),
    )
    .unwrap();
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["errors"], 1);
    assert_eq!(snap["injects"], 0);
    assert_eq!(snap["writes"], 2);
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------- 产品级加强：records 计数 / clear 命令

#[test]
fn records_reports_current_store_count_read_only() {
    let dir = temp_dir("records");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    // 空库（文件不存在）→ 0 条，不是「读不到」。
    assert_eq!(rt.state_json().unwrap()["records"], 0);
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "量子力学导论")
        .unwrap();
    let raw_before = std::fs::read_to_string(dir.join("memory.jsonl")).unwrap();
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["records"], 2, "现存条数 = 文件里的有效行数");
    assert_eq!(snap["writes"], 2, "writes 是累计写入，不是现存条数");
    // 只读：取快照不得改文件、不得创建临时文件。
    assert_eq!(
        std::fs::read_to_string(dir.join("memory.jsonl")).unwrap(),
        raw_before
    );
    assert!(!dir.join("memory.jsonl.tmp").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn records_is_null_when_store_path_unresolvable() {
    let host = FakeHost::new("基础人设");
    let mut rt = MemoryRuntime::new(
        host.services(""),
        MemoryConfig::from_value(&serde_json::json!({})),
    );
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["store_path"], serde_json::Value::Null);
    assert_eq!(snap["records"], serde_json::Value::Null, "读不到 ≠ 0 条");
    assert_eq!(
        snap["errors"], 0,
        "路径不可解析由 store_path 表达，不谎报 error"
    );
}

#[test]
fn clear_command_empties_store_atomically_and_reports_counts() {
    let dir = temp_dir("clear");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    assert_eq!(rt.state_json().unwrap()["records"], 2);

    let result = rt
        .command("clear", &serde_json::json!({}))
        .expect("clear 应成功");
    assert_eq!(result["cleared"], true);
    assert_eq!(result["records"], 0, "清空后现存条数 = 0");
    assert_eq!(result["removed"], 2, "报告被清掉的条数");

    // 文件真被重写：仍在、内容为空、无 .tmp 残留。
    let path = dir.join("memory.jsonl");
    assert!(path.exists(), "clear 用原子重写，不是 remove_file");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    assert!(!dir.join("memory.jsonl.tmp").exists());

    // state 跟随：现存 0 条；累计计数不重置。
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["records"], 0);
    assert_eq!(snap["writes"], 2, "writes 是生命周期计数，清空不重置");
    assert!(snap["hits"].as_u64().unwrap() >= 1);
    // 清空后再记一轮：records 回到 1、writes +1。
    rt.on_event(ModEventTopic::TurnPrompt, "新的记忆").unwrap();
    let after = rt.state_json().unwrap();
    assert_eq!(after["records"], 1);
    assert_eq!(after["writes"], 3);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clear_command_does_not_touch_session_prompt_or_persona() {
    let dir = temp_dir("clear-residue");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "今天天气很好", Some("s1"))
        .unwrap();
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "今天天气不错，出门走走",
        Some("s1"),
    )
    .unwrap();
    let injected = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
        .expect("会话槽应有块");
    let patches_before = host.patches().len();

    let result = rt
        .command("clear", &serde_json::json!({"session_id": "s1"}))
        .unwrap();
    // 清空只动 JSONL：提示词一字不改（残留按既有 strip_residue 生命周期处理）。
    assert_eq!(
        host.sessions()
            .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
            .as_deref(),
        Some(injected.as_str()),
        "clear 不得顺手清会话提示词"
    );
    assert_eq!(host.patches().len(), patches_before, "clear 不得写配置");
    assert_eq!(result["residue"], true, "如实报告提示词里仍有注入块");
    assert_eq!(host.main_prompt(), "基础人设");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unknown_command_is_unsupported_and_side_effect_free() {
    let dir = temp_dir("unknown-cmd");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    let err = rt
        .command("export", &serde_json::json!({}))
        .expect_err("未实现的命令必须报 UnsupportedCommand");
    assert_eq!(
        err,
        ModError::UnsupportedCommand {
            command: "export".to_string()
        }
    );
    assert!(!dir.join("memory.jsonl").exists());
    assert!(host.patches().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clear_without_store_path_fails_as_command_failed() {
    let host = FakeHost::new("基础人设");
    let mut rt = MemoryRuntime::new(
        host.services(""),
        MemoryConfig::from_value(&serde_json::json!({})),
    );
    let err = rt
        .command("clear", &serde_json::json!({}))
        .expect_err("无路径必须失败");
    assert!(
        matches!(err, ModError::Other(_)),
        "执行失败走 command_failed"
    );
    assert_eq!(rt.state_json().unwrap()["errors"], 1);
}

// ------------------------------------------- 与 persona 共存：last-writer-wins

#[test]
fn no_conversation_never_touches_global_persona_prompt() {
    let dir = temp_dir("no-conv");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "这个月预算要省着花")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "这个月预算还剩多少")
        .unwrap();
    // 无 conversation：**绝不**写全局 persona.system_prompt（旧「降级」已删除）。
    assert!(host.patches().is_empty(), "无会话不得 apply_settings");
    assert_eq!(host.main_prompt(), "基础人设");
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["no_conversation_turns"], 2);
    assert_eq!(snap["injects"], 0);
    assert_eq!(snap["writes"], 2, "记忆仍写入老桶");
    assert!(
        host.logs().iter().any(|l| l.contains("本轮不注入")),
        "必须如实说明无 conversation: {:?}",
        host.logs()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn budget_over_trim_ratio_really_drops_memories() {
    let dir = temp_dir("budget");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(
        &dir,
        &host,
        serde_json::json!({"top_k": 5, "injection_budget_chars": 200, "trim_ratio": 0.6}),
    );
    let long = "预算主题甲甲乙乙丙丙丁丁戊戊己己庚庚辛辛壬壬癸癸子子丑丑寅寅卯卯辰辰巳巳午午";
    for suffix in ["壹", "贰", "叁"] {
        rt.on_scoped_event(
            ModEventTopic::TurnPrompt,
            &format!("{long}{suffix}"),
            Some("s1"),
        )
        .unwrap();
    }
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        &format!("{long}查询"),
        Some("s1"),
    )
    .unwrap();
    let snap = rt.state_json().unwrap();
    assert!(
        snap["budget_dropped"].as_u64().unwrap() >= 1,
        "超预算必须真裁条数: {snap}"
    );
    assert!(
        snap["budget_used"].as_u64().unwrap() <= 200,
        "裁剪后不得超预算: {snap}"
    );
    let block = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
        .expect("应有注入块");
    assert!(block.lines().count() <= 5, "{block}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn crossing_summary_ratio_without_a_client_stays_on_the_rule_path() {
    // P1-5 之前这条只验「标记待摘要」；现在摘要真的跑了，所以**缺省（无客户端）**
    // 的行为必须被钉死：越阈值也不排队、不写旁车，只有原文 top-k 照常注入。
    // 真摘要的完整触发 / 落地 / 注入在 summary_tests.rs 里验。
    let dir = temp_dir("summary-off");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(
        &dir,
        &host,
        serde_json::json!({
            "top_k": 3,
            "injection_budget_chars": 200,
            "trim_ratio": 0.6,
            "summary_ratio": 0.75,
            "summary_cooldown_turns": 5,
        }),
    );
    let very_long: String = "摘要素材".to_string() + &"甲".repeat(150);
    for suffix in ["壹", "贰", "叁"] {
        rt.on_scoped_event(
            ModEventTopic::TurnPrompt,
            &format!("{very_long}{suffix}"),
            Some("s1"),
        )
        .unwrap();
    }
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["summary"]["enabled"], false, "{snap}");
    assert_eq!(snap["summary"]["client"], "disabled");
    assert_eq!(
        snap["summary_pending"], false,
        "没有客户端就不该排队：{snap}"
    );
    assert_eq!(snap["summary_marks"], 0, "{snap}");
    assert!(
        snap["summary"]["bucket_ratio"].as_f64().unwrap() > 0.75,
        "{snap}"
    );
    assert_eq!(snap["summary_keep_recent_turns"], 4);
    // 规则裁 top-k 仍生效（预算 200、注入块按分数裁）。
    assert!(snap["budget_dropped"].as_u64().unwrap() >= 1, "{snap}");
    // 旁车文件从未被创建（未启用不得留半截状态）。
    assert!(
        !dir.join("sessions/s1.memory.summary.json").exists(),
        "无客户端时不得创建摘要旁车"
    );
    let _ = std::fs::remove_dir_all(dir);
}
