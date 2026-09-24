//! P1-3 / P1-4 回归：SentenceReady 句子锚点 → 三级仲裁 → cue 通道，以及异步
//! 第二路（真实 OpenAI 兼容 HTTP 客户端）的装配 / 节流 / 回退。
//!
//! # 为什么单独一个文件
//!
//! AGENTS「测试文件 ≤800 行」：本段从 `tests.rs` 拆出，只覆盖句子锚点与二路。
//! 共享 helper（`RecordingRegistrar` / `spy_services`）由 `crate::tests` 以
//! `pub(crate)` 暴露，不复制第二份。

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use live2d_ai_mod_system::*;

use crate::arbiter::Arbiter;
use crate::plan::{DirectorPlan, PRIORITY_ASYNC, PRIORITY_RULE, parse_plan};
use crate::staging::StagingClient;
use crate::tests::{RecordingRegistrar, spy_services};
use crate::{DirectorConfig, DirectorFactory, DirectorRuntime};

#[derive(Clone)]
struct MockStaging {
    reply: Arc<Mutex<Option<String>>>,
    calls: Arc<AtomicUsize>,
}

impl MockStaging {
    fn replying(reply: &str) -> Self {
        Self {
            reply: Arc::new(Mutex::new(Some(reply.to_string()))),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl StagingClient for MockStaging {
    fn complete(&self, _system: &str, _user: &str, _timeout_ms: u64) -> Option<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.reply.lock().expect("reply lock").clone()
    }
}

struct FailingStaging;

impl StagingClient for FailingStaging {
    fn complete(&self, _system: &str, _user: &str, _timeout_ms: u64) -> Option<String> {
        None
    }
}

fn services_with_cues() -> (ModServices, Arc<Mutex<Vec<serde_json::Value>>>) {
    let cues: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&cues);
    let services = ModServices::new(
        ModActionSender::new(|_| false),
        SaySender::new(|_| true),
        ModEventSender::new(|_, _| true),
        ModLogger::new(|_, _| {}),
    )
    .with_cues(ModCueSender::new(move |cue| {
        sink.lock().expect("cue lock").push(cue);
        true
    }));
    (services, cues)
}

fn runtime_with_staging(
    services: ModServices,
    config: serde_json::Value,
    staging: Box<dyn StagingClient>,
) -> DirectorRuntime {
    let mut rt =
        DirectorRuntime::new(services, DirectorConfig::from_value(&config)).with_staging(staging);
    let mut reg = RecordingRegistrar::default();
    rt.start(&mut reg).expect("start 应成功");
    rt
}

fn sentence(rt: &mut DirectorRuntime, epoch: u64, seq: u64, ts: u64) {
    let payload = serde_json::json!({
        "epoch": epoch,
        "ts_ms": ts,
        "sentence_seq": seq,
        "text": format!("第{seq}句"),
    })
    .to_string();
    rt.on_event(ModEventTopic::SentenceReady, &payload).unwrap();
}

#[test]
fn plan_parse_clamps_and_drops_unknown_presets() {
    let raw = r#"{"epoch":7,"covers_upto_seq":3,"cues":[
        {"sentence_seq":1,"preset_id":"nod","intensity":99,"ttl_ms":99999},
        {"sentence_seq":0,"preset_id":"nod"},
        {"sentence_seq":2,"preset_id":"arm_wave"},
        {"sentence_seq":3,"preset_id":"smile","intensity":0,"ttl_ms":0}
    ]}"#;
    let (plan, warnings) = parse_plan(raw, crate::PRESET_IDS).expect("应可解析");
    assert_eq!(plan.epoch, 7);
    assert_eq!(plan.covers_upto_seq, 3);
    assert_eq!(plan.cues.len(), 2, "未知 preset / 非法句号必须丢");
    assert_eq!(plan.cues[0].preset_id, "nod");
    assert_eq!(plan.cues[0].intensity, crate::MAX_INTENSITY, "99 应钳到 3");
    assert_eq!(plan.cues[0].ttl_ms, crate::MAX_TTL_MS);
    assert_eq!(
        plan.cues[0].priority, PRIORITY_ASYNC,
        "priority 由应用层写死"
    );
    assert_eq!(plan.cues[1].intensity, 1, "0 应钳到 1");
    assert_eq!(plan.cues[1].ttl_ms, 1, "0 应钳到 1");
    assert_eq!(warnings.len(), 2);
    assert!(parse_plan("{ not json", crate::PRESET_IDS).is_err());
    assert!(parse_plan(r#"{"covers_upto_seq":1,"cues":[]}"#, crate::PRESET_IDS).is_err());
}

#[test]
fn arbiter_priority_and_epoch_mismatch_zero_side_effect() {
    let mut arb = Arbiter::new(42);
    let rule = DirectorPlan::rule(42, Some("nod"), 1, 2_000);
    assert!(arb.apply(&rule));
    assert_eq!(arb.cue_for(1).map(|c| c.priority), Some(PRIORITY_RULE));

    // 异步（40）覆盖规则（10）。
    let async_plan = DirectorPlan {
        epoch: 42,
        covers_upto_seq: 2,
        cues: vec![crate::Cue {
            sentence_seq: 1,
            preset_id: "shake".to_string(),
            intensity: 2,
            ttl_ms: 1_500,
            priority: PRIORITY_ASYNC,
        }],
    };
    assert!(arb.apply(&async_plan));
    assert_eq!(arb.cue_for(1).map(|c| c.preset_id.as_str()), Some("shake"));
    assert_eq!(arb.covers_upto_seq(), 2);

    // epoch 不匹配：整份丢弃、零状态变更（离线回归 2）。
    let stale = DirectorPlan {
        epoch: 41,
        covers_upto_seq: 99,
        cues: vec![crate::Cue {
            sentence_seq: 2,
            preset_id: "smile".to_string(),
            intensity: 3,
            ttl_ms: 5_000,
            priority: PRIORITY_ASYNC,
        }],
    };
    let before_len = arb.len();
    let before_covers = arb.covers_upto_seq();
    assert!(!arb.apply(&stale), "epoch 不匹配必须整份丢弃");
    assert_eq!(arb.len(), before_len);
    assert_eq!(arb.covers_upto_seq(), before_covers);
    assert!(arb.cue_for(2).is_none(), "不得产生任何 cue");
}

/// **活服务实测抓到的缺陷（2026-09-19）**：core 只在 stop 时推进 epoch，
/// 普通轮次的 SentenceReady **epoch 恒为 0**（真实链路日志实测）。
/// 旧守卫「epoch == 0 就丢」会让规则层与二路在**所有普通轮次**都不工作。
#[test]
fn epoch_zero_is_accepted_and_turn_prompt_resets_the_arbiter() {
    let (services, cues) = services_with_cues();
    let mut rt = runtime_with_staging(
        services,
        serde_json::json!({}),
        Box::new(crate::DisabledStaging),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
    sentence(&mut rt, 0, 1, 0);
    assert_eq!(
        rt.state_json().unwrap()["plan"]["rule_cues"],
        1,
        "epoch=0 是合法的首轮，必须被接受"
    );

    // 第二轮：epoch 仍是 0（普通轮次不推进），TurnPrompt 必须换一张干净的表。
    rt.on_event(ModEventTopic::TurnEnded, "1").unwrap();
    rt.on_event(ModEventTopic::TurnPrompt, "抱抱").unwrap();
    sentence(&mut rt, 0, 1, 0);
    let state = rt.state_json().unwrap();
    assert_eq!(state["plan"]["rule_cues"], 2, "第二轮规则 cue 必须再生效");
    let got = cues.lock().expect("cue lock").clone();
    let last = got.last().expect("第二轮必须 emit");
    let arr = last["cues"].as_array().expect("cues 必须是数组");
    assert_eq!(
        arr.len(),
        1,
        "上一轮的 cue 不得残留到本轮（同优先级 upsert 会保留旧的）: {last}"
    );
    assert_eq!(arr[0]["sentence_seq"], 1);
    assert_eq!(arr[0]["priority"], PRIORITY_RULE);
}

#[test]
fn sentence_ready_emits_rule_cue_on_first_sentence() {
    let (services, cues) = services_with_cues();
    let mut rt = runtime_with_staging(
        services,
        serde_json::json!({}),
        Box::new(crate::DisabledStaging),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
    let decided = rt
        .ledger()
        .latest()
        .and_then(|e| e.preset_id.clone())
        .expect("greeting 应选出一条 preset");
    sentence(&mut rt, 5, 1, 0);
    let got = cues.lock().expect("cue lock").clone();
    assert_eq!(got.len(), 1, "首句应产出一份规则 cue: {got:?}");
    assert_eq!(got[0]["epoch"], 5);
    assert_eq!(got[0]["covers_upto_seq"], 1);
    let cue = &got[0]["cues"][0];
    assert_eq!(cue["preset_id"], decided);
    assert_eq!(cue["priority"], PRIORITY_RULE);
    assert_eq!(rt.state_json().unwrap()["plan"]["rule_cues"], 1);
}

#[test]
fn staging_failure_is_identical_to_no_staging() {
    let (services_a, cues_a) = services_with_cues();
    let mut with_fail = runtime_with_staging(
        services_a,
        serde_json::json!({"staging_enabled": true}),
        Box::new(FailingStaging),
    );
    let (services_b, cues_b) = services_with_cues();
    let mut without = runtime_with_staging(
        services_b,
        serde_json::json!({"staging_enabled": true}),
        Box::new(crate::DisabledStaging),
    );
    for rt in [&mut with_fail, &mut without] {
        rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
        sentence(rt, 9, 1, 0);
    }
    let a = cues_a.lock().expect("cue lock").clone();
    let b = cues_b.lock().expect("cue lock").clone();
    assert!(!a.is_empty());
    assert_eq!(a, b, "导演 100% 失败必须与没有导演逐字段相同（离线回归 1）");
    assert_eq!(
        with_fail.state_json().unwrap()["staging"]["async_failures"],
        1
    );
}

#[test]
fn staging_throttle_first_sentence_interval_and_cap() {
    let (services, cues) = services_with_cues();
    let staging = MockStaging::replying(
        r#"{"epoch":3,"covers_upto_seq":1,"cues":[{"sentence_seq":1,"preset_id":"nod"}]}"#,
    );
    let calls = Arc::clone(&staging.calls);
    let mut rt = runtime_with_staging(
        services,
        serde_json::json!({
            "staging_enabled": true,
            "staging_min_interval_ms": 1200,
            "staging_max_per_turn": 3,
        }),
        Box::new(staging),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
    sentence(&mut rt, 3, 1, 0);
    sentence(&mut rt, 3, 2, 100);
    sentence(&mut rt, 3, 3, 2_000);
    sentence(&mut rt, 3, 4, 4_000);
    sentence(&mut rt, 3, 5, 6_000);
    assert_eq!(calls.load(Ordering::SeqCst), 3, "首句 + 间隔 + 每轮上限 3");
    assert!(!cues.lock().expect("cue lock").is_empty());
    assert_eq!(rt.state_json().unwrap()["staging"]["async_plans"], 3);
}

#[test]
fn director_never_writes_config_or_actions() {
    let (services, spies) = spy_services();
    let cues: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&cues);
    let services = services.with_cues(ModCueSender::new(move |cue| {
        sink.lock().expect("cue lock").push(cue);
        true
    }));
    let mut rt = runtime_with_staging(
        services,
        serde_json::json!({}),
        Box::new(crate::DisabledStaging),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
    sentence(&mut rt, 1, 1, 0);
    assert_eq!(
        spies.action_calls.load(Ordering::SeqCst),
        0,
        "动作通道必须零调用"
    );
    assert_eq!(
        spies.apply_calls.load(Ordering::SeqCst),
        0,
        "配置写回必须零调用"
    );
    assert!(!cues.lock().expect("cue lock").is_empty());
}

// ------------------------------------------ P1-4（2026-09-19）：真实二路接线

/// **Disabled ≡ 未注入**：生产缺省（从不调 with_staging）与显式注入
/// `DisabledStaging` 必须逐字段相同——「默认关」只有一个语义。
#[test]
fn disabled_staging_is_identical_to_no_injection() {
    let (services_a, cues_a) = services_with_cues();
    let mut injected = runtime_with_staging(
        services_a,
        serde_json::json!({"staging_enabled": true}),
        Box::new(crate::DisabledStaging),
    );
    let (services_b, cues_b) = services_with_cues();
    let mut default_rt = DirectorRuntime::new(
        services_b,
        DirectorConfig::from_value(&serde_json::json!({"staging_enabled": true})),
    );
    let mut registrar = RecordingRegistrar::default();
    default_rt.start(&mut registrar).expect("start 应成功");

    for rt in [&mut injected, &mut default_rt] {
        rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
        sentence(rt, 4, 1, 0);
        sentence(rt, 4, 2, 100);
    }
    assert_eq!(
        cues_a.lock().expect("cue lock").clone(),
        cues_b.lock().expect("cue lock").clone(),
        "Disabled 与未注入必须产出同一份 cue"
    );
    let a = injected.state_json().unwrap();
    let b = default_rt.state_json().unwrap();
    assert_eq!(a["staging"], b["staging"], "staging 状态面必须逐字相同");
    assert_eq!(a["staging"]["client"], "disabled");
    assert_eq!(a["staging"]["enabled"], false);
    assert_eq!(
        a["staging"]["async_failures"], 0,
        "Disabled 不得被调用（不是「调了失败」）"
    );
}

/// mock 成功：异步 plan 真的进 cue 通道，且 **priority=40 覆盖规则(10)**。
#[test]
fn staging_async_plan_reaches_cue_sink_with_async_priority() {
    let (services, cues) = services_with_cues();
    let staging = MockStaging::replying(
        r#"{"epoch":3,"covers_upto_seq":2,"cues":[
            {"sentence_seq":1,"preset_id":"nod","intensity":2,"ttl_ms":1500},
            {"sentence_seq":2,"preset_id":"shake"}
        ]}"#,
    );
    let mut rt = runtime_with_staging(
        services,
        serde_json::json!({"staging_enabled": true, "staging_max_per_turn": 3}),
        Box::new(staging),
    );
    rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
    sentence(&mut rt, 3, 1, 0);
    let got = cues.lock().expect("cue lock").clone();
    let plan = got.last().expect("首句必须 emit 一份 plan");
    let cues_json = plan["cues"].as_array().expect("cues 必须是数组");
    let first = cues_json
        .iter()
        .find(|c| c["sentence_seq"] == 1)
        .expect("必须含 seq=1 的 cue");
    assert_eq!(first["preset_id"], "nod", "异步 preset 必须覆盖规则 preset");
    assert_eq!(first["priority"], PRIORITY_ASYNC, "priority 由应用层写死");
    assert_eq!(first["intensity"], 2);
    assert_eq!(first["ttl_ms"], 1_500);
    assert_eq!(
        rt.state_json().unwrap()["staging"]["async_plans"],
        1,
        "成功 plan 必须计数"
    );
}

/// mock 返回**过期 epoch**：整份丢弃、零副作用——与纯规则逐字相同。
#[test]
fn staging_epoch_mismatch_is_rule_only_and_leaves_arbiter_clean() {
    let (services, cues) = services_with_cues();
    let stale = MockStaging::replying(
        r#"{"epoch":999,"covers_upto_seq":9,"cues":[{"sentence_seq":1,"preset_id":"shake"}]}"#,
    );
    let mut with_stale = runtime_with_staging(
        services,
        serde_json::json!({"staging_enabled": true}),
        Box::new(stale),
    );
    let (services_ref, cues_ref) = services_with_cues();
    let mut rule_only = runtime_with_staging(
        services_ref,
        serde_json::json!({"staging_enabled": true}),
        Box::new(crate::DisabledStaging),
    );
    for rt in [&mut with_stale, &mut rule_only] {
        rt.on_event(ModEventTopic::TurnPrompt, "你好呀").unwrap();
        sentence(rt, 3, 1, 0);
        sentence(rt, 3, 2, 100);
    }
    assert_eq!(
        cues.lock().expect("cue lock").clone(),
        cues_ref.lock().expect("cue lock").clone(),
        "过期 epoch 的 plan 必须整份丢弃（离线回归 2）"
    );
    let state = with_stale.state_json().unwrap();
    assert_eq!(state["staging"]["async_plans"], 0, "不得计成功");
    assert_eq!(
        state["staging"]["async_failures"], 0,
        "解析成功、只是被 epoch 闸丢弃——不算失败"
    );
    assert_eq!(state["plan"]["covers_upto_seq"], 1, "arbiter 不得被污染");
}

/// P1-4：prompt 必须带本轮 epoch（不带 → 真实端点的 plan 永远过不了 epoch 硬闸），
/// 且用户正文 / 助手正文原样进 prompt、思考不进 prompt。
#[test]
fn staging_prompt_carries_epoch_and_verbatim_text() {
    let prompt =
        crate::staging::build_user_prompt(7, "用户说的话", "助手说的话。", &["nod", "smile"]);
    assert!(prompt.contains("本轮 epoch：7"), "{prompt}");
    assert!(prompt.contains("用户说的话"), "{prompt}");
    assert!(prompt.contains("助手说的话。"), "{prompt}");
    assert!(prompt.contains("nod, smile"), "{prompt}");
    assert!(
        !prompt.to_ascii_lowercase().contains("reasoning"),
        "prompt 不得携带思考：{prompt}"
    );
}

/// 工厂装配（host 注入点）：缺配置 → Disabled；开闸缺端点 → degraded；
/// 配齐 → openai（**不发请求**，只有 SentenceReady 才发）。
#[test]
fn factory_create_assembles_staging_client_from_config() {
    let mut registrar = RecordingRegistrar::default();

    let (services, _spies) = spy_services();
    let mut idle = DirectorFactory
        .create(services, serde_json::json!({}))
        .expect("create 应成功");
    idle.start(&mut registrar).expect("start 应成功");
    let state = idle.state_json().unwrap();
    assert_eq!(state["staging"]["client"], "disabled");
    assert_eq!(state["staging"]["enabled"], false);
    assert_eq!(state["staging"]["degraded"], false, "没开闸不算 degraded");

    let (services, _spies) = spy_services();
    let mut no_endpoint = DirectorFactory
        .create(services, serde_json::json!({"staging_enabled": true}))
        .expect("create 应成功");
    no_endpoint.start(&mut registrar).expect("start 应成功");
    let state = no_endpoint.state_json().unwrap();
    assert_eq!(
        state["staging"]["client"], "disabled",
        "缺端点必须回落 Disabled"
    );
    assert_eq!(state["staging"]["degraded"], true, "开了没接上必须可观察");
    assert!(
        state["staging"]["note"]
            .as_str()
            .unwrap_or("")
            .contains("staging_base_url"),
        "degraded 原因要能直接读出来: {}",
        state["staging"]["note"]
    );

    let (services, _spies) = spy_services();
    let mut configured = DirectorFactory
        .create(
            services,
            serde_json::json!({
                "staging_enabled": true,
                "staging_base_url": "http://127.0.0.1:59999/v1",
                "staging_model": "qwen2.5:7b",
                "staging_api_key_env": "L2D_STAGING_TEST_KEY_SHOULD_BE_UNSET",
            }),
        )
        .expect("create 应成功");
    configured.start(&mut registrar).expect("start 应成功");
    let state = configured.state_json().unwrap();
    assert_eq!(state["staging"]["client"], "openai", "配齐必须真接上");
    assert_eq!(state["staging"]["enabled"], true);
    assert_eq!(state["staging"]["degraded"], false);
    assert_eq!(state["staging"]["model"], "qwen2.5:7b");
    assert_eq!(
        state["staging"]["api_key_set"], false,
        "只回布尔；未设置的变量名不得变成真"
    );
    assert_eq!(
        state["staging"]["async_failures"], 0,
        "没触发过 = 没发过请求"
    );
}
