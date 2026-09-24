//! P1-5 真摘要的回归（**A/B 不串、失败 = 无摘要、回滚、无会话不污染**）。
//!
//! 覆盖的口径（逐条对任务书的验收）：
//! - 触发后摘要**非空**且**进注入**（本会话的注入槽里能看到它）；
//! - 失败 = 无摘要（旁车不写 versions，只记 last_error；top-k 规则裁剪照常）；
//! - 回滚后注入**不再含**该摘要、被覆盖的原文**重新可检索**；
//! - A / B conversation 不串（各自的旁车、各自的注入槽）；
//! - 无会话零全局污染（不写 persona.system_prompt，只有计数变化）。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use live2d_ai_mod_system::ModRuntime;

use crate::strategy::MemoryRecord;
use crate::summary::Summarizer;
use crate::summary_store::SummaryStore;
use crate::test_support::{FakeHost, runtime, temp_dir};
use crate::{MemoryConfig, MemoryRuntime, SESSION_PROMPT_OWNER_MEMORY};

/// 可控摘要替身：成功（返回固定正文）/ 失败（None）；记录调用次数与最后一次请求。
#[derive(Clone)]
struct FakeSummarizer {
    answer: Option<String>,
    calls: Arc<AtomicUsize>,
    fail: Arc<AtomicBool>,
    last_user: Arc<std::sync::Mutex<Option<String>>>,
}

impl FakeSummarizer {
    fn ok(text: &str) -> Self {
        Self {
            answer: Some(text.to_string()),
            calls: Arc::new(AtomicUsize::new(0)),
            fail: Arc::new(AtomicBool::new(false)),
            last_user: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn failing() -> Self {
        Self {
            answer: None,
            calls: Arc::new(AtomicUsize::new(0)),
            fail: Arc::new(AtomicBool::new(true)),
            last_user: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn last_user(&self) -> Option<String> {
        self.last_user.lock().expect("lock").clone()
    }
}

impl Summarizer for FakeSummarizer {
    fn kind(&self) -> &'static str {
        "injected"
    }

    fn has_api_key(&self) -> bool {
        true
    }

    fn endpoint(&self) -> &str {
        "http://fake.invalid/v1/chat/completions"
    }

    fn summarize(&self, _system: &str, user: &str, _timeout_ms: u64) -> Option<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail.load(Ordering::SeqCst) {
            return None;
        }
        *self.last_user.lock().expect("lock") = Some(user.to_string());
        self.answer.clone()
    }
}

/// 摘要配置：总闸开 + 最小合法预算（200 字符；让阈值轻易越过）。
///
/// `injection_budget_chars` 的**下限是 200**（配置解析会钳），所以测试语料必须
/// 够长：6 条 seed + 本轮 1 条约 190 字符，比值 >0.75 才真的越阈值。
fn summary_config(extra: serde_json::Value) -> serde_json::Value {
    let mut base = serde_json::json!({
        "injection_budget_chars": 200,
        "summary_ratio": 0.75,
        "summary_cooldown_turns": 0,
        "summary_keep_recent_turns": 2,
        "top_k": 3,
        "max_records": 100,
    });
    if let (Some(base_map), Some(extra_map)) = (base.as_object_mut(), extra.as_object()) {
        for (k, v) in extra_map {
            base_map.insert(k.clone(), v.clone());
        }
    }
    base
}

/// 直接往桶里塞 N 条（跳过 remember，便于精确控制位点）。
fn seed_bucket(dir: &std::path::Path, session: Option<&str>, texts: &[&str]) {
    let base = dir.join("memory.jsonl");
    let path = match session {
        Some(id) => crate::resolve_session_store_path(&base, id).expect("合法会话 id"),
        None => base,
    };
    let store = crate::store::JsonlStore::new(path);
    for (i, text) in texts.iter().enumerate() {
        let record = MemoryRecord::new(*text, 1_700_000_000 + i as i64, (i + 1) as u64);
        store.append(&record).expect("seed 写入");
    }
}

/// 等到后台摘要结束（轮询回执）；返回「是否落了摘要版本」。
///
/// 单测里**必须**经过这个等待：summarize 在另一个线程上跑，
/// 直接断言调用次数 / version 会与线程调度赛跑（实测偶发红）。
fn wait_for_summary(rt: &mut MemoryRuntime, tag: &str) -> bool {
    for _ in 0..300 {
        rt.drain_summary_results();
        let state = rt.summary_state();
        if !state["pending"].as_bool().unwrap_or(false) {
            return state["version"].as_u64().unwrap_or(0) > 0;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("{tag}: 后台摘要没有在 3s 内结束");
}

#[test]
fn true_summary_is_generated_and_injected_into_the_same_conversation() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-ok");
    let fake = FakeSummarizer::ok("用户喜欢薄荷，且自称星梦。");
    let mut rt = runtime(&dir, &host, summary_config(serde_json::json!({})))
        .with_summarizer(Arc::new(fake.clone()), None);
    seed_bucket(
        &dir,
        Some("conv-A"),
        &[
            "我叫星梦，住在海边的小城，喜欢在傍晚沿着防波堤散步，看远处的灯塔",
            "我喜欢薄荷，尤其是夏天冰过的薄荷茶，冬天也会泡一杯热的薄荷茶",
            "我养了一只黑猫叫煤球，它今年三岁，最喜欢趴在窗台上晒太阳打呼噜",
            "我们约定周末去爬山，早上六点出发，走东边的缓坡上去看日出",
            "我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼很有趣",
            "我明天要早起赶第一班车，所以今晚打算早点睡，不熬夜了",
        ],
    );

    // 第一轮：越阈值 -> 起后台；本轮注入槽仍只有原文命中（摘要还没回来）。
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    assert!(
        rt.summary_state()["pending"].as_bool().unwrap_or(false),
        "越阈值当轮必须进 pending（后台线程此时刚起）"
    );
    assert!(wait_for_summary(&mut rt, "ok"), "后台摘要必须落地一版");
    assert!(fake.calls() >= 1, "越阈值必须真的调一次摘要客户端");
    let state = rt.summary_state();
    assert_eq!(state["enabled"], true);
    assert_eq!(state["client"], "injected");
    assert!(
        state["version"].as_u64().unwrap_or(0) >= 1,
        "必须落一版摘要：{state}"
    );
    assert_eq!(state["pending"], false, "收完结果 pending 必须落回 false");
    assert_eq!(state["last_error"], serde_json::Value::Null);
    assert!(
        state["text"].as_str().unwrap_or_default().contains("薄荷"),
        "摘要正文必须非空且可读：{state}"
    );
    // 旁车文件真的在盘上（可回滚的前提）。
    let bucket = crate::store::JsonlStore::new(dir.join("sessions/conv-A.memory.jsonl"));
    let sidecar = SummaryStore::for_memory_store(&bucket);
    let file = sidecar.load().expect("旁车可读");
    assert_eq!(file.versions.len(), 1);
    assert_eq!(file.current().unwrap().version, 1);
    assert!(file.current().unwrap().covers_upto >= 2);

    // 下一轮：摘要必须进本会话的注入槽（五段里的「摘要」段）。
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "再说说薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    let slot = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "conv-A")
        .expect("本会话必须被注入");
    assert!(
        slot.contains("用户喜欢薄荷，且自称星梦。"),
        "摘要必须进注入块：{slot}"
    );
    // 前缀**只出现一次**：`compose_lines` 是加前缀的唯一一处，调用方再拼一遍
    // 就会得到 `- [摘要] [摘要] …`（2026-09-20 活服务冒烟当场抓到过）。
    assert_eq!(
        slot.matches(crate::summary::SUMMARY_LINE_PREFIX).count(),
        1,
        "摘要前缀必须恰好一次：{slot}"
    );
    // 摘要覆盖过的原文**不再重复注入**（压缩才有意义）。
    assert!(
        !slot.contains("我们约定周末去爬山"),
        "已被摘要覆盖的原文不该再整条进注入：{slot}"
    );
    // 全局 persona 一字未动（有会话绝不写全局）。
    assert_eq!(host.main_prompt(), "匿名基线");

    // 喂给 LLM 的原文里只含**未覆盖**的条目（覆盖过的被排除）。
    let user = fake.last_user().expect("应记录最后一次请求");
    assert!(
        user.contains("我喜欢薄荷"),
        "待压原文应包含更早的条目：{user}"
    );
    assert!(
        !user.contains("我明天要早起"),
        "最近 K 轮原文必须排除在摘要输入之外：{user}"
    );
}

#[test]
fn failure_is_identical_to_no_summary_and_keeps_the_rule_trim() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-fail");
    let fake = FakeSummarizer::failing();
    let mut rt = runtime(&dir, &host, summary_config(serde_json::json!({})))
        .with_summarizer(Arc::new(fake.clone()), None);
    seed_bucket(
        &dir,
        Some("conv-A"),
        &[
            "我叫星梦，住在海边的小城，喜欢在傍晚沿着防波堤散步，看远处的灯塔",
            "我喜欢薄荷，尤其是夏天冰过的薄荷茶，冬天也会泡一杯热的薄荷茶",
            "我养了一只黑猫叫煤球，它今年三岁，最喜欢趴在窗台上晒太阳打呼噜",
            "我们约定周末去爬山，早上六点出发，走东边的缓坡上去看日出",
            "我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼很有趣",
            "我明天要早起赶第一班车，所以今晚打算早点睡，不熬夜了",
        ],
    );
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    assert!(!wait_for_summary(&mut rt, "fail"), "失败不得落版本");
    assert!(fake.calls() >= 1, "失败也必须真的发过请求");
    let state = rt.summary_state();
    assert_eq!(state["version"], 0, "失败不得产生摘要版本：{state}");
    assert_eq!(state["text"], serde_json::Value::Null);
    assert!(
        state["last_error"]
            .as_str()
            .is_some_and(|e| e.contains("timeout_ms")),
        "失败原因必须可见：{state}"
    );
    // 旁车只有失败记录，没有任何版本。
    let bucket = crate::store::JsonlStore::new(dir.join("sessions/conv-A.memory.jsonl"));
    let file = SummaryStore::for_memory_store(&bucket)
        .load()
        .expect("可读");
    assert!(file.versions.is_empty());
    assert!(file.last_error.is_some());
    // 规则裁 top-k 仍生效：下一轮注入里没有摘要行，只有原文命中。
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    let slot = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "conv-A")
        .expect("原文命中仍要注入");
    assert!(!slot.contains(crate::summary::SUMMARY_LINE_PREFIX));
    assert!(slot.contains("薄荷"));
}

#[test]
fn rollback_removes_the_summary_from_injection_and_restores_originals() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-rollback");
    let fake = FakeSummarizer::ok("第一版摘要：用户喜欢薄荷。");
    let mut rt = runtime(&dir, &host, summary_config(serde_json::json!({})))
        .with_summarizer(Arc::new(fake.clone()), None);
    seed_bucket(
        &dir,
        Some("conv-A"),
        &[
            "我叫星梦，住在海边的小城，喜欢在傍晚沿着防波堤散步，看远处的灯塔",
            "我喜欢薄荷，尤其是夏天冰过的薄荷茶，冬天也会泡一杯热的薄荷茶",
            "我养了一只黑猫叫煤球，它今年三岁，最喜欢趴在窗台上晒太阳打呼噜",
            "我们约定周末去爬山，早上六点出发，走东边的缓坡上去看日出",
            "我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼很有趣",
            "我明天要早起赶第一班车，所以今晚打算早点睡，不熬夜了",
        ],
    );
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    assert!(wait_for_summary(&mut rt, "rollback"), "第一版必须落地");
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "再说说薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    let before = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "conv-A")
        .expect("注入过");
    assert!(before.contains("第一版摘要"));

    // 回滚：丢当前版本 -> 注入不再含摘要。
    let out = rt
        .command(
            "summary_rollback",
            &serde_json::json!({ "session_id": "conv-A" }),
        )
        .expect("回滚成功");
    assert_eq!(out["rolled_back"]["version"], 1);
    assert_eq!(out["version"], 0, "回滚到无摘要：{out}");
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "再聊聊薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    let after = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "conv-A")
        .expect("原文命中仍会注入");
    assert!(
        !after.contains("第一版摘要"),
        "回滚后注入不得再含该摘要：{after}"
    );
    // 原文一条没删：桶里是 6 条 seed + 3 轮 remember = 9 条。
    assert_eq!(rt.record_count(), Some(9), "回滚绝不动原文");
    // 再回滚 → 可读错误（不谎报成功）。
    let err = rt
        .command(
            "summary_rollback",
            &serde_json::json!({ "session_id": "conv-A" }),
        )
        .expect_err("没有可回滚的版本");
    assert!(format!("{err:?}").contains("没有可回滚"));
}

#[test]
fn conversations_do_not_leak_summaries_into_each_other() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-ab");
    let fake = FakeSummarizer::ok("A 会话摘要：用户喜欢薄荷。");
    let config = summary_config(serde_json::json!({}));
    let mut rt = runtime(&dir, &host, config).with_summarizer(Arc::new(fake), None);
    seed_bucket(
        &dir,
        Some("conv-A"),
        &[
            "我叫星梦，住在海边的小城，喜欢在傍晚沿着防波堤散步，看远处的灯塔",
            "我喜欢薄荷，尤其是夏天冰过的薄荷茶，冬天也会泡一杯热的薄荷茶",
            "我养了一只黑猫叫煤球，它今年三岁，最喜欢趴在窗台上晒太阳打呼噜",
            "我们约定周末去爬山，早上六点出发，走东边的缓坡上去看日出",
            "我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼很有趣",
            "我明天要早起赶第一班车，所以今晚打算早点睡，不熬夜了",
        ],
    );
    seed_bucket(
        &dir,
        Some("conv-B"),
        &["B 会话第一句", "B 会话喜欢喝咖啡", "B 会话住在城里"],
    );
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    assert!(wait_for_summary(&mut rt, "ab"), "A 的摘要必须落地");
    // 切到 B：B 自己的旁车不存在 -> 无摘要。
    let b_state = rt
        .command("summary", &serde_json::json!({ "session_id": "conv-B" }))
        .expect("summary 命令");
    assert_eq!(b_state["summary"]["version"], 0, "B 不得看到 A 的摘要");
    assert_eq!(b_state["summary"]["text"], serde_json::Value::Null);
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "B 会话喜欢喝咖啡",
        Some("conv-B"),
    )
    .expect("turn ok");
    let b_slot = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "conv-B")
        .expect("B 的原文命中");
    assert!(
        !b_slot.contains("A 会话摘要"),
        "A 的摘要不得串进 B：{b_slot}"
    );
    // A 的槽里仍然只有 A 的东西。
    let a_slot = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "conv-A")
        .expect("A 的槽");
    assert!(!a_slot.contains("B 会话"));
    assert_eq!(host.main_prompt(), "匿名基线", "全程不写全局 persona");
}

#[test]
fn no_conversation_never_writes_a_summary_or_pollutes_global_persona() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-none");
    let fake = FakeSummarizer::ok("不该出现的摘要");
    let mut rt = runtime(&dir, &host, summary_config(serde_json::json!({})))
        .with_summarizer(Arc::new(fake.clone()), None);
    seed_bucket(
        &dir,
        None,
        &[
            "我叫星梦，住在海边的小城，喜欢在傍晚沿着防波堤散步，看远处的灯塔",
            "我喜欢薄荷，尤其是夏天冰过的薄荷茶，冬天也会泡一杯热的薄荷茶",
            "我养了一只黑猫叫煤球，它今年三岁，最喜欢趴在窗台上晒太阳打呼噜",
            "我们约定周末去爬山，早上六点出发，走东边的缓坡上去看日出",
            "我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼很有趣",
            "我明天要早起赶第一班车，所以今晚打算早点睡，不熬夜了",
        ],
    );
    // 裸 TurnPrompt（无会话）：不注入，也不写全局。
    rt.on_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
    )
    .expect("turn ok");
    assert_eq!(host.main_prompt(), "匿名基线");
    assert_eq!(host.patches().len(), 0, "无会话不得写任何设置");
    assert_eq!(rt.summary_state()["version"], 0, "无会话不摘要");
    assert_eq!(fake.calls(), 0, "无会话连摘要请求都不该发");
    let state = rt.state_json().expect("state");
    assert_eq!(state["no_conversation_turns"], 1);
    assert_eq!(state["summary"]["pending"], false);
}

#[test]
fn uncovered_start_accounts_for_physical_eviction() {
    // 覆盖到前 5 条，但头部被淘汰了 2 条 -> 有效起点 3。
    assert_eq!(crate::summary::uncovered_start(5, 2, 10), 3);
    // 淘汰比覆盖还多（极端）-> 起点 0，不越界。
    assert_eq!(crate::summary::uncovered_start(2, 5, 10), 0);
    // 桶比覆盖短（原文被清过）-> 钳到 len。
    assert_eq!(crate::summary::uncovered_start(9, 0, 4), 4);
}

#[test]
fn cooldown_blocks_a_second_trigger_and_the_ratio_is_always_visible() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-cooldown");
    let fake = FakeSummarizer::ok("冷却测试摘要");
    let mut rt = runtime(
        &dir,
        &host,
        summary_config(serde_json::json!({ "summary_cooldown_turns": 5 })),
    )
    .with_summarizer(Arc::new(fake), None);
    seed_bucket(
        &dir,
        Some("conv-A"),
        &[
            "我叫星梦，住在海边的小城，喜欢在傍晚沿着防波堤散步，看远处的灯塔",
            "我喜欢薄荷，尤其是夏天冰过的薄荷茶，冬天也会泡一杯热的薄荷茶",
            "我养了一只黑猫叫煤球，它今年三岁，最喜欢趴在窗台上晒太阳打呼噜",
            "我们约定周末去爬山，早上六点出发，走东边的缓坡上去看日出",
            "我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼很有趣",
            "我明天要早起赶第一班车，所以今晚打算早点睡，不熬夜了",
        ],
    );
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    let snap = rt.summary_state();
    assert_eq!(snap["marks"], 1, "越阈值必须触发一次：{snap}");
    assert!(
        snap["bucket_ratio"].as_f64().unwrap() > 0.75,
        "比值在摘要跑之前就必须可观察：{snap}"
    );
    assert!(
        snap["cooldown_left"].as_u64().unwrap() > 0,
        "触发后冷却必须可见：{snap}"
    );
    // 后台还在跑 / 冷却期内再越阈值：不重复触发。
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "再说说薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    assert_eq!(rt.summary_state()["marks"], 1, "冷却期内不得重复触发");
    assert!(wait_for_summary(&mut rt, "cooldown"), "第一次触发仍要落地");
}

#[test]
fn build_request_keeps_recent_turns_and_needs_at_least_two_records() {
    let records: Vec<MemoryRecord> = (0..6)
        .map(|i| MemoryRecord::new(format!("第{i}条"), 0, i as u64 + 1))
        .collect();
    let request = crate::summary::build_request(&records, 0, 2).expect("够 2 条应可压");
    assert_eq!(request.covers_upto, 4, "覆盖到 n-keep = 4");
    assert!(!request.user.contains("第4条"));
    assert!(!request.user.contains("第5条"));
    assert!(request.user.contains("第0条"));
    // keep 把候选压到 1 条 -> 不值得起调用。
    assert!(crate::summary::build_request(&records, 0, 5).is_none());
    // 全都覆盖过了 -> None。
    assert!(crate::summary::build_request(&records, 4, 2).is_none());
    // 空桶 -> None（不 panic）。
    assert!(crate::summary::build_request(&[], 0, 4).is_none());
}

#[test]
fn normalize_summary_strips_wrappers_and_empty_is_a_failure() {
    assert_eq!(
        crate::summary::normalize_summary("  摘要：\n用户喜欢薄荷。\n").as_deref(),
        Some("用户喜欢薄荷。")
    );
    assert_eq!(
        crate::summary::normalize_summary("- 用户喜欢薄荷。").as_deref(),
        Some("用户喜欢薄荷。")
    );
    assert_eq!(crate::summary::normalize_summary("   "), None);
    let long = "字".repeat(crate::summary::MAX_SUMMARY_CHARS + 10);
    let normalized = crate::summary::normalize_summary(&long).expect("非空");
    assert!(normalized.ends_with('…'));
    assert_eq!(
        normalized.chars().count(),
        crate::summary::MAX_SUMMARY_CHARS + 1
    );
}

#[test]
fn summary_config_default_is_disabled_until_endpoint_and_model_are_set() {
    let default = MemoryConfig::default();
    assert!(!default.summary_enabled, "缺省关闭摘要");
    assert!(
        !crate::summary::assemble(&default.summary_config())
            .client
            .enabled()
    );
    let partial = MemoryConfig::from_value(&serde_json::json!({
        "summary_enabled": true,
        "summary_model": "m",
    }));
    assert!(partial.summary_enabled);
    let setup = crate::summary::assemble(&partial.summary_config());
    assert!(!setup.client.enabled(), "缺 base_url 必须仍是 Disabled");
    assert!(setup.degraded_note.is_some(), "要可观察 degraded");
    let full = MemoryConfig::from_value(&serde_json::json!({
        "summary_enabled": true,
        "summary_model": "m",
        "summary_base_url": "http://127.0.0.1:11434/v1",
        "summary_api_key_env": "FAKE_ENV_NOT_SET_ANYWHERE",
    }));
    let setup = crate::summary::assemble(&full.summary_config());
    assert!(setup.client.enabled());
    assert_eq!(setup.client.kind(), "openai");
    assert!(setup.degraded_note.is_some(), "没配 key 值也要如实说");
}

#[test]
fn clearing_the_bucket_also_removes_its_summary_sidecar() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-clear");
    let fake = FakeSummarizer::ok("会被清掉的摘要");
    let mut rt = runtime(&dir, &host, summary_config(serde_json::json!({})))
        .with_summarizer(Arc::new(fake), None);
    seed_bucket(
        &dir,
        Some("conv-A"),
        &[
            "我叫星梦，住在海边的小城，喜欢在傍晚沿着防波堤散步，看远处的灯塔",
            "我喜欢薄荷，尤其是夏天冰过的薄荷茶，冬天也会泡一杯热的薄荷茶",
            "我养了一只黑猫叫煤球，它今年三岁，最喜欢趴在窗台上晒太阳打呼噜",
            "我们约定周末去爬山，早上六点出发，走东边的缓坡上去看日出",
            "我最近在读一本关于海洋生物的书，里面讲到深海发光的鱼很有趣",
            "我明天要早起赶第一班车，所以今晚打算早点睡，不熬夜了",
        ],
    );
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "我喜欢薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    assert!(wait_for_summary(&mut rt, "clear"), "摘要应先落地");
    let bucket = crate::store::JsonlStore::new(dir.join("sessions/conv-A.memory.jsonl"));
    let sidecar = SummaryStore::for_memory_store(&bucket);
    assert!(sidecar.path().exists());
    let out = rt
        .command("clear", &serde_json::json!({ "session_id": "conv-A" }))
        .expect("清空");
    assert_eq!(out["summary_removed"], true);
    assert!(!sidecar.path().exists(), "清空必须连带删掉摘要旁车");
    assert_eq!(rt.summary_state()["version"], 0);
}

// ------------------------------------------------ Wave 3：摘要含助手侧（2026-09-21）

/// 纯函数：待压窗口里**用户与助手交错**，每条带角色前缀；最近 K **轮**（不是
/// K 条）连同助手的半条一起排除。
#[test]
fn build_request_interleaves_both_roles_and_keeps_recent_turns() {
    let records = vec![
        MemoryRecord::new("用户第一句", 1, 1),
        MemoryRecord::assistant("助手第一句", 1, 1),
        MemoryRecord::new("用户第二句", 2, 2),
        MemoryRecord::assistant("助手第二句", 2, 2),
        MemoryRecord::new("用户第三句", 3, 3),
        MemoryRecord::assistant("助手第三句", 3, 3),
    ];
    // 保留最近 1 轮 → 只排除第 3 轮两条，前两轮（4 条）是候选。
    let req = crate::summary::build_request(&records, 0, 1).expect("有可压内容");
    assert_eq!(req.covers_upto, 4, "覆盖位点含助手记录");
    assert_eq!(req.covers_turn, 2);
    let user = &req.user;
    assert!(user.contains("- [用户] 用户第一句"), "{user}");
    assert!(user.contains("- [助手] 助手第一句"), "{user}");
    assert!(
        user.find("用户第一句").unwrap() < user.find("助手第一句").unwrap(),
        "顺序 = 落盘顺序（用户先、助手后）：{user}"
    );
    assert!(!user.contains("用户第三句"), "最近 1 轮必须排除：{user}");
    assert!(
        !user.contains("助手第三句"),
        "同轮的助手记录一起排除：{user}"
    );
}

/// 「保留最近 K 轮」按 turn 分组计数——助手记录入桶后，「最后 K 条」不再是 K 轮。
#[test]
fn recent_turn_window_counts_turns_not_records() {
    let records = vec![
        MemoryRecord::new("u1", 1, 1),
        MemoryRecord::assistant("a1", 1, 1),
        MemoryRecord::new("u2", 2, 2),
        MemoryRecord::assistant("a2", 2, 2),
    ];
    use crate::strategy::recent_turn_window_start;
    assert_eq!(
        recent_turn_window_start(&records, 1),
        2,
        "最近 1 轮 = 后两条"
    );
    assert_eq!(recent_turn_window_start(&records, 2), 0, "最近 2 轮 = 全部");
    assert_eq!(recent_turn_window_start(&records, 0), 4, "0 轮 = 全可压");
    // 老桶（turn 恒 0）挤在一起：只有 1 个不同 turn，保留 2 轮 → 全部留下。
    let legacy = vec![MemoryRecord::new("x", 1, 0), MemoryRecord::new("y", 2, 0)];
    assert_eq!(recent_turn_window_start(&legacy, 2), 0);
}

/// 往桶里塞「用户 + 助手」成对的若干轮（turn 从 1 递增）。
fn seed_bucket_pairs(dir: &std::path::Path, session: &str, pairs: &[(&str, &str)]) {
    let base = dir.join("memory.jsonl");
    let path = crate::resolve_session_store_path(&base, session).expect("合法会话 id");
    let store = crate::store::JsonlStore::new(path);
    for (i, (user, assistant)) in pairs.iter().enumerate() {
        let turn = (i + 1) as u64;
        let ts = 1_700_000_000 + turn as i64;
        store
            .append(&MemoryRecord::new(*user, ts, turn))
            .expect("seed 用户行");
        store
            .append(&MemoryRecord::assistant(*assistant, ts, turn))
            .expect("seed 助手行");
    }
}

/// 端到端：摘要的覆盖位点**含助手位点**；被覆盖的用户与助手原文都不再进注入。
#[test]
fn summary_covers_include_assistant_records() {
    let host = FakeHost::new("匿名基线");
    let dir = temp_dir("summary-asst");
    let fake = FakeSummarizer::ok("用户喜欢薄荷，夏天喝薄荷茶；养了黑猫煤球。");
    let mut rt = runtime(&dir, &host, summary_config(serde_json::json!({})))
        .with_summarizer(Arc::new(fake.clone()), None);
    seed_bucket_pairs(
        &dir,
        "conv-A",
        &[
            (
                "我住在海边的小城，喜欢傍晚散步看灯塔和潮汐",
                "好的，我记住了海边的小城。",
            ),
            (
                "我喜欢薄荷，夏天冰过的薄荷茶最好喝",
                "薄荷茶听起来很清爽，我记住啦。",
            ),
            (
                "我养了一只黑猫叫煤球，它今年三岁",
                "煤球这个名字很可爱，我记住了。",
            ),
            (
                "我明天要早起赶第一班车，今晚早点睡",
                "那我就不打扰你休息啦，晚安。",
            ),
        ],
    );
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    assert!(wait_for_summary(&mut rt, "asst"), "后台摘要必须落地");

    let bucket = crate::store::JsonlStore::new(dir.join("sessions/conv-A.memory.jsonl"));
    let file = SummaryStore::for_memory_store(&bucket)
        .load()
        .expect("旁车可读");
    let covers = file.current().expect("有版本").covers_upto;
    assert!(
        covers > 3,
        "覆盖位点必须数到助手记录（8 条记录、保留最近 1 轮 → 6）：{covers}"
    );
    // 送进 LLM 的待压原文里**有助手行**（摘要不再只压用户话）。
    let sent = fake.last_user().expect("应记录最后一次请求");
    assert!(sent.contains("- [助手]"), "摘要输入必须含助手行：{sent}");

    // 下一轮注入：摘要进块，被它覆盖的用户 / 助手原文都不再整条进。
    rt.on_scoped_event(
        live2d_ai_mod_system::ModEventTopic::TurnPrompt,
        "薄荷",
        Some("conv-A"),
    )
    .expect("turn ok");
    let slot = host
        .sessions()
        .slot(SESSION_PROMPT_OWNER_MEMORY, "conv-A")
        .expect("本会话必须被注入");
    assert!(
        slot.contains("用户喜欢薄荷，夏天喝薄荷茶；养了黑猫煤球。"),
        "摘要必须进注入块：{slot}"
    );
    assert!(
        !slot.contains("薄荷茶听起来很清爽"),
        "已被摘要覆盖的**助手**原文不该再整条进注入：{slot}"
    );
    assert!(
        !slot.contains("我喜欢薄荷，夏天冰过的薄荷茶最好喝"),
        "已被摘要覆盖的用户原文同样不进：{slot}"
    );
}
