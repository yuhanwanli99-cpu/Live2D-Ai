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
    assert_eq!(reg.topics, vec![ModEventTopic::TurnPrompt]);
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
        vec!["store_path", "top_k", "max_records", "enabled_injection"]
    );
    assert!(
        !keys.contains(&"enabled"),
        "启停唯一真源是 manifest，schema 不许有第二个 enabled"
    );
    assert_eq!(spec.mod_id, "memory");
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
fn related_second_turn_injects_previous_memory_into_next_round() {
    let dir = temp_dir("inject");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();

    let patches = host.patches();
    assert_eq!(patches.len(), 1, "只应有一次注入");
    let prompt = patches[0]["persona"]["system_prompt"]
        .as_str()
        .expect("patch 形状必须是 persona.system_prompt");
    assert!(prompt.starts_with("基础人设\n\n"), "{prompt}");
    assert!(prompt.contains(MEMORY_MARKER_BEGIN));
    assert!(prompt.contains("- 今天天气很好"));
    assert!(prompt.ends_with(MEMORY_MARKER_END));
    // 主链快照已被 change（下一轮请求体才会带上它）。
    assert_eq!(host.main_prompt(), prompt);
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
fn identical_repeat_turn_is_idempotent_no_second_write() {
    let dir = temp_dir("idem");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    for _ in 0..3 {
        rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
            .unwrap();
    }
    let patches = host.patches();
    assert_eq!(patches.len(), 1, "重拼结果与当前一致 → 第二次起不再写盘");
    let prompt = host.main_prompt();
    assert_eq!(
        prompt.matches("- 今天天气很好").count(),
        1,
        "同一句话不重复注入: {prompt}"
    );
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
fn rejected_apply_settings_is_warned_not_fatal() {
    let dir = temp_dir("reject");
    let host = FakeHost::new("基础人设");
    host.set_accept(false);
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    // 第二次相关输入会尝试注入，但 host 拒绝。
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    assert!(host.patches().is_empty());
    assert_eq!(host.main_prompt(), "基础人设", "被拒后主链快照不变");
    assert!(
        host.logs()
            .iter()
            .any(|l| l.contains("apply_settings 拒绝")),
        "拒绝必须可见: {:?}",
        host.logs()
    );
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["injected"], 0, "被拒不计数");
    assert_eq!(snap["remembered"], 2, "记忆仍然照记");
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 停用 / 状态

#[test]
fn shutdown_strips_injected_block() {
    let dir = temp_dir("shutdown");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    assert!(host.main_prompt().contains(MEMORY_MARKER_BEGIN));
    rt.shutdown().unwrap();
    assert_eq!(host.main_prompt(), "基础人设", "停用必须清掉注入块");
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
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    let first = rt.state_json().unwrap();
    assert_eq!(first["writes"], 1);
    assert_eq!(first["hits"], 0);
    assert_eq!(first["injects"], 0);
    assert_eq!(first["errors"], 0);
    assert_eq!(first["last_hits"], 0);
    // 轮 2：命中上一条并注入。
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
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
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn errors_counter_counts_unresolved_path_and_rejected_inject() {
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

    // apply_settings 被拒 → errors 1（记忆照记，不 fatal）。
    let dir = temp_dir("errors");
    let host = FakeHost::new("基础人设");
    host.set_accept(false);
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
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
fn clear_command_does_not_touch_persona_prompt() {
    let dir = temp_dir("clear-residue");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气不错，出门走走")
        .unwrap();
    let injected = host.main_prompt();
    assert!(injected.contains(MEMORY_MARKER_BEGIN));
    let patches_before = host.patches().len();

    let result = rt.command("clear", &serde_json::json!({})).unwrap();
    // 清空只动 JSONL：提示词一字不改（残留按既有 strip_residue 生命周期处理）。
    assert_eq!(
        host.main_prompt(),
        injected,
        "clear 不得顺手清 persona 提示词"
    );
    assert_eq!(host.patches().len(), patches_before, "clear 不得写配置");
    assert_eq!(result["residue"], true, "如实报告提示词里仍有注入块");
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
fn last_writer_wins_persona_base_survives_memory_injection() {
    // persona 先写 base（模拟其合成结果），memory 下一轮在其上重拼记忆块。
    let dir = temp_dir("lww-persona-first");
    let host = FakeHost::new("基础人设");
    host.set_main_prompt("人格卡合成：你是猫娘小灰");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "这个月预算要省着花")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "这个月预算还剩多少")
        .unwrap();
    let prompt = host.main_prompt();
    assert!(prompt.starts_with("人格卡合成：你是猫娘小灰"), "{prompt}");
    assert!(
        prompt.contains(MEMORY_MARKER_BEGIN),
        "memory 后写 → 块必须在: {prompt}"
    );
    assert!(prompt.contains("- 这个月预算要省着花"), "{prompt}");
    assert_eq!(
        prompt.matches(MEMORY_MARKER_BEGIN).count(),
        1,
        "只有一个记忆块"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn last_writer_wins_persona_overwrite_then_memory_reinjects_on_new_base() {
    let dir = temp_dir("lww-persona-last");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_event(ModEventTopic::TurnPrompt, "这个月预算要省着花")
        .unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "这个月预算还剩多少")
        .unwrap();
    assert!(host.main_prompt().contains(MEMORY_MARKER_BEGIN));
    // persona 后写：整份覆盖（不含 memory marker）——last-writer-wins 的直接后果。
    host.set_main_prompt("人格卡合成：你是猫娘小灰");
    assert!(
        !host.main_prompt().contains(MEMORY_MARKER_BEGIN),
        "后写者覆盖整个 system_prompt → 记忆块被冲掉"
    );
    // memory 下一轮再写：它读到新的 base，重新拼上块（base 一字不改）。
    rt.on_event(ModEventTopic::TurnPrompt, "这个月预算还剩多少")
        .unwrap();
    let prompt = host.main_prompt();
    assert!(prompt.starts_with("人格卡合成：你是猫娘小灰"), "{prompt}");
    assert!(prompt.contains(MEMORY_MARKER_BEGIN), "{prompt}");
    assert!(prompt.contains("- 这个月预算要省着花"), "{prompt}");
    let _ = std::fs::remove_dir_all(dir);
}
