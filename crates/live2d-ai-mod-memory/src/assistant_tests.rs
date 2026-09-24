//! **助手侧记忆**回归（Wave 3，2026-09-21）：AssistantReplied → 按 conversation
//! 分桶追加、role=assistant、失败只 warn、无会话不注入。
//!
//! 为什么单列一个文件：tests.rs 已 759 行（测试文件口径 ≤800），把这一组塞进去
//! 会越线；这一组的主题也自成一块（事件 → 桶 → 注入的角色区分）。

use super::*;
use crate::test_support::*;

/// 一轮完整对话（用户 TurnPrompt + 助手 AssistantReplied）之后，**同一个会话桶**
/// 里必须同时有 user 与 assistant 两条，且 role 正确。
#[test]
fn assistant_reply_lands_in_the_same_session_bucket_with_role_assistant() {
    let dir = temp_dir("asst-bucket");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢薄荷", Some("s1"))
        .unwrap();
    let payload = serde_json::json!({
        "turn": 1,
        "role": "assistant",
        "text": "好呀，我记住薄荷了。",
        "interrupted": false,
    })
    .to_string();
    rt.on_scoped_event(ModEventTopic::AssistantReplied, &payload, Some("s1"))
        .unwrap();

    let raw = std::fs::read_to_string(dir.join("sessions").join("s1.memory.jsonl"))
        .expect("会话桶必须存在");
    let lines: Vec<serde_json::Value> = raw
        .lines()
        .map(|l| serde_json::from_str(l).expect("每行都是 JSON"))
        .collect();
    assert_eq!(lines.len(), 2, "一轮 = 用户 + 助手两条: {raw}");
    assert_eq!(lines[0]["role"], "user");
    assert_eq!(lines[0]["text"], "我喜欢薄荷");
    assert_eq!(lines[1]["role"], "assistant");
    assert_eq!(lines[1]["text"], "好呀，我记住薄荷了。");
    // 同一轮共用同一个 turn（摘要按轮分组保留的前提）。
    assert_eq!(lines[0]["turn"], 1);
    assert_eq!(lines[1]["turn"], 1);

    let state = rt.state_json().unwrap();
    assert_eq!(state["writes"], 2);
    assert_eq!(state["user_writes"], 1);
    assert_eq!(state["assistant_writes"], 1);
    let _ = std::fs::remove_dir_all(dir);
}

/// 助手侧与用户侧**在同一条记忆块里可区分**（角色前缀），且助手侧事实可被后续
/// 用户话检索到（token 重叠不区分角色）。
#[test]
fn assistant_fact_is_retrievable_and_lines_carry_role_prefixes() {
    let dir = temp_dir("asst-inject");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({"top_k": 5}));
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "我喜欢薄荷", Some("s1"))
        .unwrap();
    let payload = serde_json::json!({
        "turn": 1, "role": "assistant", "text": "好的，我记住了薄荷。",
    })
    .to_string();
    rt.on_scoped_event(ModEventTopic::AssistantReplied, &payload, Some("s1"))
        .unwrap();
    // 第二轮用户话把两条都检索出来。
    rt.on_scoped_event(ModEventTopic::TurnPrompt, "薄荷怎么样", Some("s1"))
        .unwrap();

    let block = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "s1")
        .expect("应注入会话槽");
    assert!(
        block.contains("- [用户] 我喜欢薄荷"),
        "用户行必须带 [用户] 前缀: {block}"
    );
    assert!(
        block.contains("- [助手] 好的，我记住了薄荷。"),
        "助手行必须带 [助手] 前缀: {block}"
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// 无 conversation：助手侧正文仍落**老路径全局桶**（与用户侧同口径），但
/// **不注入**、不写全局 persona.system_prompt。
#[test]
fn assistant_reply_without_conversation_records_but_never_injects() {
    let dir = temp_dir("asst-global");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_event(ModEventTopic::TurnPrompt, "今天天气很好")
        .unwrap();
    let payload = serde_json::json!({
        "turn": 1, "role": "assistant", "text": "是呀，天气不错。",
    })
    .to_string();
    rt.on_event(ModEventTopic::AssistantReplied, &payload)
        .unwrap();

    let raw = std::fs::read_to_string(dir.join("memory.jsonl")).expect("老桶必须存在");
    assert_eq!(raw.lines().count(), 2, "无会话也照记（只记不注入）: {raw}");
    assert!(raw.contains("\"role\":\"assistant\""));
    assert!(host.patches().is_empty(), "无会话不得写全局 persona");
    assert_eq!(host.main_prompt(), "基础人设");
    assert!(host.sessions().stored_sessions().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// 坏 payload / 空正文：**只 warn，不 panic、不落盘**（记忆是增量能力）。
#[test]
fn bad_assistant_payloads_are_warn_only() {
    let dir = temp_dir("asst-bad");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    rt.on_scoped_event(ModEventTopic::AssistantReplied, "{不是 JSON", Some("s1"))
        .unwrap();
    rt.on_scoped_event(ModEventTopic::AssistantReplied, "{}", Some("s1"))
        .unwrap();
    let empty = serde_json::json!({"turn": 1, "role": "assistant", "text": "   "}).to_string();
    rt.on_scoped_event(ModEventTopic::AssistantReplied, &empty, Some("s1"))
        .unwrap();
    assert!(
        !dir.join("sessions").join("s1.memory.jsonl").exists(),
        "坏 payload 不得落盘"
    );
    assert_eq!(rt.state_json().unwrap()["assistant_writes"], 0);
    assert!(
        host.logs().iter().any(|l| l.contains("无法解析")),
        "坏 payload 必须有 warn: {:?}",
        host.logs()
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// 非法会话 id：退回老路径全局桶 + warn（绝不拿用户输入拼路径）。
#[test]
fn illegal_session_id_falls_back_to_the_legacy_bucket() {
    let dir = temp_dir("asst-badsess");
    let host = FakeHost::new("基础人设");
    let mut rt = runtime(&dir, &host, serde_json::json!({}));
    let payload = serde_json::json!({
        "turn": 1, "role": "assistant", "text": "这句没有会话归属。",
    })
    .to_string();
    rt.on_scoped_event(ModEventTopic::AssistantReplied, &payload, Some("a/b"))
        .unwrap();
    assert!(!dir.join("sessions").exists(), "非法 id 不得生成会话桶目录");
    let raw = std::fs::read_to_string(dir.join("memory.jsonl")).expect("回落老桶");
    assert!(raw.contains("这句没有会话归属。"), "{raw}");
    assert!(
        host.logs().iter().any(|l| l.contains("非法会话 id")),
        "{:?}",
        host.logs()
    );
    let _ = std::fs::remove_dir_all(dir);
}

/// 助手记录与同轮用户记录**不会撞 id**（同秒、同轮、同一句话）。
#[test]
fn user_and_assistant_records_never_share_an_id() {
    let user = MemoryRecord::new("嗯", 100, 1);
    let assistant = MemoryRecord::assistant("嗯", 100, 1);
    assert_ne!(
        user.id, assistant.id,
        "角色必须参与 id，否则 delete 会误删两条"
    );
    // 老行（无 id）按用户口径派生，id 与旧版一致。
    assert_eq!(user.id, MemoryRecord::make_id(100, 1, "嗯"));
}
