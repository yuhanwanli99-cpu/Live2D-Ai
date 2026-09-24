//! L1 产品级 C 轨：**主动管理面**（list / import / update / delete / clear）与
//! **会话分桶**的运行时回归。
//!
//! 单独一个测试文件（`#[path]` 挂进 crate root）是因为 `tests.rs` 已接近
//! 「测试文件 ≤800 行」的上限；两者共用 `test_support.rs` 的替身。
//!
//! 契约真源：`crates/live2d-ai-mod-memory/src/commands.rs` 与 `sessions.rs`
//! 的模块头注 + `docs/architecture/memory-mod-v0.md` §13。

use super::*;
use crate::test_support::*;

/// 从 `list` 结果里取记录文本（按返回顺序）。
fn list_texts(value: &serde_json::Value) -> Vec<String> {
    value["records"]
        .as_array()
        .expect("list.records 必须是数组")
        .iter()
        .map(|r| r["text"].as_str().expect("record.text").to_string())
        .collect()
}

// ------------------------------------------------------------ list / limit

#[test]
fn list_returns_newest_first_and_reports_total() {
    let dir = temp_dir("cmd-list");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    for text in ["第一条", "第二条", "第三条"] {
        rt.command("import", &serde_json::json!({"text": text}))
            .expect("import 应成功");
    }

    let listed = rt.command("list", &serde_json::json!({})).unwrap();
    assert_eq!(listed["ok"], true);
    assert_eq!(listed["total"], 3, "total 是桶里的全部条数");
    assert_eq!(
        list_texts(&listed),
        vec!["第三条", "第二条", "第一条"],
        "最新在前（追加序的逆序）"
    );
    assert_eq!(listed["session_id"], serde_json::Value::Null);
    assert!(
        listed["bucket"].as_str().unwrap().ends_with("memory.jsonl"),
        "无会话 → 老路径桶: {listed}"
    );

    // id 稳定：同一条记录两次 list 得到同一串 id。
    let again = rt.command("list", &serde_json::json!({})).unwrap();
    let ids = |v: &serde_json::Value| -> Vec<String> {
        v["records"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(ids(&listed), ids(&again), "两次载入 id 必须相同");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn list_limit_is_clamped_and_invalid_falls_back_to_default() {
    let dir = temp_dir("cmd-limit");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    for i in 1..=4 {
        rt.command("import", &serde_json::json!({"text": format!("第{i}条")}))
            .unwrap();
    }

    let two = rt
        .command("list", &serde_json::json!({"limit": 2}))
        .unwrap();
    assert_eq!(list_texts(&two).len(), 2, "limit=2 只回 2 条");
    assert_eq!(two["total"], 4, "total 仍是全部");
    assert_eq!(two["limit"], 2);

    // 超大 limit → 钳到上限（这里只有 4 条，条数照实）。
    let huge = rt
        .command("list", &serde_json::json!({"limit": 9999}))
        .unwrap();
    assert_eq!(huge["limit"], 200, "上限 200");
    assert_eq!(list_texts(&huge).len(), 4);

    // 非法值（0 / 负数 / 字符串 / null）→ 回落缺省 50。
    for bad in [
        serde_json::json!(0),
        serde_json::json!(-3),
        serde_json::json!("3"),
        serde_json::json!(null),
    ] {
        let out = rt
            .command("list", &serde_json::json!({"limit": bad}))
            .unwrap();
        assert_eq!(out["limit"], 50, "非法 limit {bad} 必须回落缺省");
    }
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------- import/update/delete

#[test]
fn import_update_delete_round_trip() {
    let dir = temp_dir("cmd-roundtrip");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));

    let imported = rt
        .command("import", &serde_json::json!({"text": "  我喜欢薄荷  "}))
        .unwrap();
    let id = imported["id"]
        .as_str()
        .expect("import 必须回 id")
        .to_string();
    assert_eq!(imported["records"], 1);
    assert_eq!(imported["total"], 1);

    let listed = rt.command("list", &serde_json::json!({})).unwrap();
    assert_eq!(list_texts(&listed), vec!["我喜欢薄荷"], "import 去首尾空白");
    assert_eq!(listed["records"][0]["id"].as_str(), Some(id.as_str()));

    let updated = rt
        .command(
            "update",
            &serde_json::json!({"id": id, "text": "我喜欢乌龙茶"}),
        )
        .unwrap();
    assert_eq!(updated["ok"], true);
    assert_eq!(
        updated["id"].as_str(),
        Some(id.as_str()),
        "update 保持原 id"
    );
    assert_eq!(updated["records"], 1);

    let after_update = rt.command("list", &serde_json::json!({})).unwrap();
    assert_eq!(list_texts(&after_update), vec!["我喜欢乌龙茶"]);
    assert_eq!(
        after_update["records"][0]["id"].as_str(),
        Some(id.as_str()),
        "编辑不改变身份"
    );

    let deleted = rt
        .command("delete", &serde_json::json!({"id": id}))
        .unwrap();
    assert_eq!(deleted["removed"], 1);
    assert_eq!(deleted["records"], 0);
    let after_delete = rt.command("list", &serde_json::json!({})).unwrap();
    assert_eq!(after_delete["total"], 0, "删掉后桶里空了");

    // 文件是真被重写（仍然存在、内容为空、无 .tmp 残留）。
    let path = dir.join("memory.jsonl");
    assert!(path.exists());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    assert!(!dir.join("memory.jsonl.tmp").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn import_rejects_empty_text_with_readable_error() {
    let dir = temp_dir("cmd-empty");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    for bad in ["", "   ", "\n\t"] {
        let err = rt
            .command("import", &serde_json::json!({"text": bad}))
            .expect_err("空 text 必须报错");
        let ModError::Other(message) = err else {
            panic!("空 text 应该是可读的 command_failed（Other）");
        };
        assert!(message.contains("text 不能为空"), "{message}");
    }
    // 目标路径压根没被创建（失败无副作用）。
    assert!(!dir.join("memory.jsonl").exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unknown_id_and_missing_id_are_readable_errors() {
    let dir = temp_dir("cmd-badid");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.command("import", &serde_json::json!({"text": "唯一一条"}))
        .unwrap();

    let err = rt
        .command("update", &serde_json::json!({"id": "nope", "text": "x"}))
        .expect_err("不存在的 id 必须报错");
    let ModError::Other(message) = err else {
        panic!("应走 command_failed");
    };
    assert!(message.contains("找不到 id=nope"), "{message}");
    assert!(message.contains("list"), "必须提示先 list 拿 id: {message}");

    let err = rt
        .command("delete", &serde_json::json!({"id": "nope"}))
        .expect_err("删除不存在的 id 必须报错");
    assert!(matches!(err, ModError::Other(_)));
    assert!(err.to_string().contains("找不到"), "{err}");

    // 缺 id / 缺 text 都是可读错误。
    let err = rt
        .command("update", &serde_json::json!({"text": "x"}))
        .expect_err("缺 id 必须报错");
    assert!(err.to_string().contains("缺少 id"), "{err}");
    let err = rt
        .command("update", &serde_json::json!({"id": "x", "text": "  "}))
        .expect_err("空 text 必须报错");
    assert!(err.to_string().contains("text 不能为空"), "{err}");

    // 失败不动库：那条记录还在。
    let listed = rt.command("list", &serde_json::json!({})).unwrap();
    assert_eq!(list_texts(&listed), vec!["唯一一条"]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn unknown_command_is_still_unsupported() {
    let dir = temp_dir("cmd-unknown");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    let err = rt
        .command("export", &serde_json::json!({}))
        .expect_err("未实现命令必须 UnsupportedCommand");
    assert_eq!(
        err,
        ModError::UnsupportedCommand {
            command: "export".to_string()
        }
    );
    let _ = std::fs::remove_dir_all(dir);
}

// ------------------------------------------------------------ 会话分桶

#[test]
fn session_turn_lands_in_sessions_bucket_not_legacy_path() {
    let dir = temp_dir("bucket-turn");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "会话 A 的第一句",
        Some("session-a"),
    )
    .unwrap();

    let bucket = dir.join("sessions").join("session-a.memory.jsonl");
    assert!(bucket.exists(), "有会话必须落到 sessions/<id>.memory.jsonl");
    let raw = std::fs::read_to_string(&bucket).unwrap();
    assert!(raw.contains("会话 A 的第一句"));
    assert!(raw.contains("\"id\""), "落盘的行必须带 id: {raw}");
    assert!(
        !dir.join("memory.jsonl").exists(),
        "有会话时不得再写老路径全局桶"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn no_session_keeps_the_legacy_global_path() {
    let dir = temp_dir("bucket-legacy");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    // 裸 HTTP / 终端壳走 on_event（没有会话）。
    rt.on_event(ModEventTopic::TurnPrompt, "裸 HTTP 输入")
        .unwrap();
    assert!(
        dir.join("memory.jsonl").exists(),
        "无会话必须保持老路径（老记忆还能读到）"
    );
    assert!(!dir.join("sessions").exists(), "不该凭空创建 sessions 目录");

    let snap = rt.state_json().unwrap();
    assert_eq!(snap["session_scoped"], false);
    assert_eq!(snap["active_session"], serde_json::Value::Null);
    assert!(
        snap["bucket_path"]
            .as_str()
            .unwrap()
            .ends_with("memory.jsonl"),
        "{snap}"
    );
    assert!(
        snap["store_path"]
            .as_str()
            .unwrap()
            .ends_with("memory.jsonl"),
        "store_path 仍是全局老路径"
    );

    // 命令不带 session_id 同样落老路径。
    rt.command("import", &serde_json::json!({"text": "面板手加一条"}))
        .unwrap();
    let listed = rt.command("list", &serde_json::json!({})).unwrap();
    assert_eq!(listed["total"], 2, "裸输入 + 手动导入都在全局桶");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn sessions_do_not_leak_between_buckets() {
    let dir = temp_dir("bucket-ab");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));

    rt.command(
        "import",
        &serde_json::json!({"text": "A 的私货", "session_id": "session-a"}),
    )
    .unwrap();
    rt.command(
        "import",
        &serde_json::json!({"text": "B 的私货", "session_id": "session-b"}),
    )
    .unwrap();

    let a = std::fs::read_to_string(dir.join("sessions").join("session-a.memory.jsonl")).unwrap();
    let b = std::fs::read_to_string(dir.join("sessions").join("session-b.memory.jsonl")).unwrap();
    assert!(
        a.contains("A 的私货") && !a.contains("B 的私货"),
        "A 桶: {a}"
    );
    assert!(
        b.contains("B 的私货") && !b.contains("A 的私货"),
        "B 桶: {b}"
    );
    assert!(!dir.join("memory.jsonl").exists(), "全局桶不该被会话写入");

    let list_a = rt
        .command("list", &serde_json::json!({"session_id": "session-a"}))
        .unwrap();
    let list_b = rt
        .command("list", &serde_json::json!({"session_id": "session-b"}))
        .unwrap();
    assert_eq!(list_texts(&list_a), vec!["A 的私货"]);
    assert_eq!(list_texts(&list_b), vec!["B 的私货"]);
    assert_eq!(list_a["session_id"], "session-a");

    // A 桶的提交不影响 B 桶的条数。
    rt.command(
        "import",
        &serde_json::json!({"text": "A 又一条", "session_id": "session-a"}),
    )
    .unwrap();
    let list_b2 = rt
        .command("list", &serde_json::json!({"session_id": "session-b"}))
        .unwrap();
    assert_eq!(list_b2["total"], 1, "B 桶不受 A 的影响");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn clear_with_session_only_empties_that_bucket() {
    let dir = temp_dir("bucket-clear");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.command(
        "import",
        &serde_json::json!({"text": "A 一条", "session_id": "session-a"}),
    )
    .unwrap();
    rt.command(
        "import",
        &serde_json::json!({"text": "B 一条", "session_id": "session-b"}),
    )
    .unwrap();

    let cleared = rt
        .command("clear", &serde_json::json!({"session_id": "session-a"}))
        .unwrap();
    assert_eq!(cleared["removed"], 1);
    assert_eq!(cleared["records"], 0);
    assert_eq!(
        rt.command("list", &serde_json::json!({"session_id": "session-a"}))
            .unwrap()["total"],
        0
    );
    assert_eq!(
        rt.command("list", &serde_json::json!({"session_id": "session-b"}))
            .unwrap()["total"],
        1,
        "清 A 不得动 B"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn invalid_session_id_in_command_is_a_readable_error() {
    let dir = temp_dir("bucket-badid");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    for bad in ["../etc/passwd", "a/b", "a b", "会话"] {
        let err = rt
            .command("list", &serde_json::json!({"session_id": bad}))
            .expect_err("非法会话 id 必须报错");
        assert!(matches!(err, ModError::Other(_)));
        assert!(err.to_string().contains("会话 id 不合法"), "{err}");
    }
    assert!(!dir.join("sessions").exists(), "非法 id 不得建任何目录");
    let _ = std::fs::remove_dir_all(dir);
}

// -------------------------------------------------------- 按会话注入

#[test]
fn session_turn_injects_only_its_own_slot_not_the_global_prompt() {
    let dir = temp_dir("inject-session");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢薄荷", Some("session-a"))
        .unwrap();
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢什么", Some("session-a"))
        .unwrap();

    // 本 Mod 只写自己的槽：槽内容 == **只有** memory 块（不含 base）。
    let slot = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "session-a")
        .expect("会话 A 必须拿到注入");
    assert!(slot.starts_with(MEMORY_MARKER_BEGIN), "{slot}");
    assert!(slot.contains("- [用户] 我喜欢薄荷"));
    assert!(slot.ends_with(MEMORY_MARKER_END));
    assert!(
        !slot.contains("基础人设"),
        "不得把 base 拼进本 Mod 的槽: {slot}"
    );
    // 组合结果（= supervisor 的读点）在该会话只有这一槽时就是槽内容。
    assert_eq!(
        host.sessions().prompt_for("session-a").as_deref(),
        Some(slot.as_str())
    );
    // **关键**：全局提示词一字未改，别的会话也没有。
    assert_eq!(host.main_prompt(), "基础人设", "会话注入不得碰全局");
    assert!(host.patches().is_empty(), "不得走 apply_settings");
    assert!(host.sessions().prompt_for("session-b").is_none());
    assert_eq!(rt.state_json().unwrap()["injects"], 1);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn session_injection_coexists_with_the_persona_slot() {
    let dir = temp_dir("inject-base");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    // 模拟 persona 先给会话 A 写了自己那张卡（独立的来源槽）。
    host.sessions().set_slot(
        SESSION_PROMPT_OWNER_PERSONA,
        "session-a",
        "人格卡：你是猫娘小灰",
    );
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "这个月预算要省着花",
        Some("session-a"),
    )
    .unwrap();
    rt.on_scoped_event(
        ModEventTopic::TurnPrompt,
        "这个月预算还剩多少",
        Some("session-a"),
    )
    .unwrap();

    // 本 Mod 的槽只有记忆块——不再把 persona 的卡拼进来。
    let slot = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "session-a")
        .expect("memory 槽");
    assert!(slot.starts_with(MEMORY_MARKER_BEGIN), "{slot}");
    assert!(
        !slot.contains("人格卡"),
        "不得把 persona 的卡拼进 memory 槽: {slot}"
    );
    // 组合结果里 persona 那部分仍在，且按固定顺序排在 memory 之前。
    let prompt = host.sessions().prompt_for("session-a").unwrap();
    assert!(prompt.starts_with("人格卡：你是猫娘小灰"), "{prompt}");
    assert!(prompt.contains("- [用户] 这个月预算要省着花"), "{prompt}");
    assert_eq!(host.main_prompt(), "基础人设", "全局仍是原来的 base");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn session_without_host_capability_never_falls_back_to_global() {
    let dir = temp_dir("inject-nocap");
    let host = FakeHost::new("基础人设");
    host.sessions().set_enabled(false);
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢薄荷", Some("session-a"))
        .unwrap();
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢什么", Some("session-a"))
        .unwrap();

    assert_eq!(
        host.main_prompt(),
        "基础人设",
        "能力不可用时绝不退化成全局写回（那会串会话）"
    );
    assert!(host.patches().is_empty());
    assert!(host.sessions().stored_sessions().is_empty());
    let snap = rt.state_json().unwrap();
    assert!(snap["errors"].as_u64().unwrap() >= 1, "{snap}");
    assert!(
        host.logs()
            .iter()
            .any(|l| l.contains("未注入会话提示词能力")),
        "必须留可排障的 warn: {:?}",
        host.logs()
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn state_json_exposes_session_bucket_keys_without_dropping_old_ones() {
    let dir = temp_dir("state-session");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "会话里的一句", Some("s-1"))
        .unwrap();

    let snap = rt.state_json().unwrap();
    assert_eq!(snap["session_scoped"], true);
    assert_eq!(snap["active_session"], "s-1");
    assert_eq!(snap["records"], 1, "records 跟随当前生效桶");
    assert!(
        snap["bucket_path"]
            .as_str()
            .unwrap()
            .ends_with("sessions/s-1.memory.jsonl"),
        "{snap}"
    );
    assert!(
        snap["store_path"]
            .as_str()
            .unwrap()
            .ends_with("memory.jsonl"),
        "store_path 保持全局老路径含义: {snap}"
    );
    // 现有键一个未删（Wave 3 四计数 + 兼容别名）。
    for key in [
        "records",
        "top_k",
        "max_records",
        "enabled_injection",
        "turns_seen",
        "writes",
        "hits",
        "injects",
        "errors",
        "evicted",
        "last_hits",
        "remembered",
        "injected",
    ] {
        assert!(!snap[key].is_null(), "{key} 不得丢: {snap}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn shutdown_clears_only_its_own_slot_and_global_residue() {
    let dir = temp_dir("shutdown-session");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢薄荷", Some("session-a"))
        .unwrap();
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢什么", Some("session-a"))
        .unwrap();
    assert!(host.sessions().prompt_for("session-a").is_some());

    rt.shutdown().unwrap();
    assert!(
        host.sessions()
            .slot(SESSION_PROMPT_OWNER_MEMORY, "session-a")
            .is_none(),
        "停用必须清空本 Mod 的槽"
    );
    assert!(
        host.sessions().stored_sessions().is_empty(),
        "没有别的来源时该会话条目也一起清掉"
    );
    assert_eq!(host.main_prompt(), "基础人设");
    assert!(!rt.is_registered());
    let _ = std::fs::remove_dir_all(dir);
}

/// **本次修复的验收**：memory 停用（shutdown）之后，同一会话里 persona 的贡献仍在。
#[test]
fn shutdown_keeps_another_owners_contribution_in_the_same_session() {
    let dir = temp_dir("shutdown-keeps-persona");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 3}));
    host.sessions().set_slot(
        SESSION_PROMPT_OWNER_PERSONA,
        "session-a",
        "人格卡：你是猫娘小灰",
    );
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢薄荷", Some("session-a"))
        .unwrap();
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢什么", Some("session-a"))
        .unwrap();
    assert!(
        host.sessions()
            .slot(SESSION_PROMPT_OWNER_MEMORY, "session-a")
            .is_some()
    );

    rt.shutdown().unwrap();
    assert_eq!(
        host.sessions().prompt_for("session-a").as_deref(),
        Some("人格卡：你是猫娘小灰"),
        "memory 停用不得清掉 persona 在同一会话的槽"
    );
    assert_eq!(
        host.sessions().stored_sessions(),
        vec!["session-a".to_string()],
        "会话条目必须保留（persona 的槽还在）"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn invalid_session_in_scoped_event_falls_back_to_global_bucket() {
    let dir = temp_dir("event-badid");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "非法会话也照记", Some("../x"))
        .unwrap();
    assert!(
        dir.join("memory.jsonl").exists(),
        "非法会话 id 退回全局桶（不拿它拼路径）"
    );
    assert!(!dir.join("sessions").exists());
    assert_eq!(rt.state_json().unwrap()["session_scoped"], false);
    assert!(
        host.logs().iter().any(|l| l.contains("非法会话 id")),
        "{:?}",
        host.logs()
    );
    let _ = std::fs::remove_dir_all(dir);
}
