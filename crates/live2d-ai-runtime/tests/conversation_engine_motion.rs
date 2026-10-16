//! **动作门**（T14，2026-10-13）的引擎集成回归。
//!
//! 口径（`docs/plans/PLAN-0.2-closeout-2026-10-13.md` §2）：导演 Mod 开启且
//! `staging_*` 两项都空 / 都有时，按句清洗的**同一次**响应多带一份 `cues`；
//! cues 的依据是**这一句一路原文**（不是 `speech`，不是用户输入）。模型失败
//! （坏 JSON / 超时）才回退按句规则表，输入同样是这一句原文。
//!
//! 二路用注入的假客户端（零网络）；规则表用注入的闭包（真表 = director 的
//! `rule_cues_for_sentence`，其「只改句号」由 director 自己的单测钉住——
//! crate 依赖方向决定 runtime 不能反向依赖 director）。
//!
//! 2026-10-13 执行单 T3（表演倾向）：克制 / 平常的空数组仍是成功、不回退；
//! **只有主动档**在模型交回空数组时改用按句规则表（实参仍是这一句原文）。

mod common;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use live2d_ai_runtime::cleaning::{
    CLEAN_TIMEOUT_MS, MotionBias, SentenceCleaner, SentenceRuleFallback,
};
use live2d_ai_runtime::performance::PerformanceCue;
use live2d_ai_runtime::performance::client::{
    PerformanceClient, PerformanceFuture, PerformanceReply,
};
use live2d_ai_runtime::{ConversationConfig, EngineEvent, TurnStatus};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::{
    drain_events, engine, respond_pieces, spawn_multi_server, spawn_tts_mock, sse_content, sse_done,
};

/// 固定回一份二路 JSON 的替身（`reply=None` = 请求失败）。
#[derive(Debug)]
struct StubCleaner {
    reply: Option<String>,
}

impl PerformanceClient for StubCleaner {
    fn enabled(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "injected"
    }
    fn has_api_key(&self) -> bool {
        false
    }
    fn request<'a>(
        &'a self,
        _system: &'a str,
        _user: &'a str,
        _timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
        let reply = self.reply.clone().map(|raw| PerformanceReply {
            raw,
            structured: false,
        });
        Box::pin(async move { reply })
    }
}

fn ids(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| (*s).to_string()).collect()
}

/// 造一个动作门开着的引擎：注入假二路 + 记录实参的规则回退。
///
/// `fallback_on_empty` = 表演倾向（2026-10-13 执行单 T3）的空数组回退开关
/// （真源在 director 的 `motion_bias`；crate 依赖方向决定这里只能投影布尔）。
/// `rule_cue` = 注入规则表交回的形状：true = 一条 `tilt_left`（seq = 这一句），
/// false = 空表（模拟真规则表的「中性 = 空」语义；实参照记不误）。
fn motion_engine(
    llm_base: &str,
    tts_base: &str,
    reply: Option<&str>,
    allowed: Vec<String>,
    fallback_on_empty: bool,
    rule_cue: bool,
    rule_log: Arc<Mutex<Vec<(String, u64)>>>,
) -> live2d_ai_runtime::ConversationEngine {
    let mut eng = engine(llm_base, tts_base, ConversationConfig::default());
    eng.set_cleaner(Some(Arc::new(
        SentenceCleaner::new(
            Box::new(StubCleaner {
                reply: reply.map(str::to_string),
            }),
            CLEAN_TIMEOUT_MS,
        )
        .with_motion(
            allowed,
            MotionBias::new("执行单 T3 测试子句".to_string(), fallback_on_empty),
        ),
    )));
    eng.set_motion(true);
    eng.set_sentence_rule(Some(SentenceRuleFallback::new(
        move |text: &str, seq: u64| {
            rule_log
                .lock()
                .expect("poison")
                .push((text.to_string(), seq));
            // 规则表内容不在本文件断言（真表在 director）；这里回一份可辨识的 cue。
            if rule_cue {
                vec![PerformanceCue {
                    sentence_seq: seq,
                    preset_id: "tilt_left".to_string(),
                    intensity: 1,
                    ttl_ms: 2_000,
                    ..Default::default()
                }]
            } else {
                Vec::new()
            }
        },
    )));
    eng
}

fn action_cues(events: &[EngineEvent]) -> Vec<(u64, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::ActionCue { cues, .. } => Some(
                cues.iter()
                    .map(|c| (c.sentence_seq, c.preset_id.clone()))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

fn ready_by_seq(events: &[EngineEvent]) -> Vec<(u64, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SentenceReady {
                sentence_seq, text, ..
            } => Some((*sentence_seq, text.clone())),
            _ => None,
        })
        .collect()
}

/// T17-2：动作门开着，模型给 `nod` → cue 是 `nod`，句号 = 这一句。
///
/// 用户句带「开心」（规则表会因此给 smile），但 cue 必须按**这一句原文**（中性句）
/// ——`nod` 只可能来自模型对原文的判断，证明输入不是用户句。
#[tokio::test]
async fn motion_cues_follow_the_assistant_sentence() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天天气不错。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let rule_log = Arc::new(Mutex::new(Vec::new()));
    let mut eng = motion_engine(
        &llm_base,
        &tts_base,
        Some(r#"{"display":"今天天气不错。","speech":"今天天气不错。","cues":["nod"]}"#),
        ids(&["nod", "smile", "thinking", "tilt_left", "none"]),
        false,
        true,
        Arc::clone(&rule_log),
    );

    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(401, "我好开心", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    let events = drain_events(rx).await;

    assert_eq!(action_cues(&events), [(1, "nod".to_string())]);
    assert!(
        rule_log.lock().expect("poison").is_empty(),
        "模型成功 → 不调用规则表"
    );
    assert_eq!(
        *tts_inputs.lock().expect("poison"),
        ["今天天气不错。"],
        "送 TTS 的仍是 speech"
    );
}

/// T17-5：合法 JSON `cues: []` = 这一句不动（成功），**不**回退规则表。
#[tokio::test]
async fn empty_cues_mean_no_motion() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天天气不错。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let rule_log = Arc::new(Mutex::new(Vec::new()));
    let mut eng = motion_engine(
        &llm_base,
        &tts_base,
        Some(r#"{"display":"今天天气不错。","speech":"今天天气不错。","cues":[]}"#),
        ids(&["nod", "tilt_left", "none"]),
        false,
        true,
        Arc::clone(&rule_log),
    );

    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(402, "随便聊聊", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    let events = drain_events(rx).await;

    assert!(
        action_cues(&events).is_empty(),
        "空 cues = 这一句不动，不得发 ActionCue"
    );
    assert!(
        rule_log.lock().expect("poison").is_empty(),
        "空 cues 是成功，不得回退规则表"
    );
}

/// T17-4：坏 JSON → `speech` 退回这一句原文，规则表只看**这一句原文**。
#[tokio::test]
async fn bad_json_falls_back_to_original_sentence_rules() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("这样可以吗？"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let rule_log = Arc::new(Mutex::new(Vec::new()));
    let mut eng = motion_engine(
        &llm_base,
        &tts_base,
        Some("这不是 JSON"),
        ids(&["nod", "tilt_left", "thinking", "none"]),
        false,
        true,
        Arc::clone(&rule_log),
    );

    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(403, "用户问的是别的", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    let events = drain_events(rx).await;

    // 规则表回的是注入的 tilt_left（seq = 这一句）；实参 = 这一句原文。
    assert_eq!(action_cues(&events), [(1, "tilt_left".to_string())]);
    assert_eq!(
        ready_by_seq(&events),
        [(1, "这样可以吗？".to_string())],
        "坏 JSON → speech 退回这一句原文"
    );
    assert_eq!(
        *tts_inputs.lock().expect("poison"),
        ["这样可以吗？"],
        "坏 JSON → 送 TTS 的也是原文"
    );
    let log = rule_log.lock().expect("poison").clone();
    assert_eq!(
        log,
        [("这样可以吗？".to_string(), 1)],
        "规则表实参必须是这一句原文 + 这一句的句号"
    );
}

/// T17-3：用户句到不了规则回退——记录到的实参是这一句原文，绝不是用户输入。
#[tokio::test]
async fn user_text_is_not_a_cue_input() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("嗯。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let rule_log = Arc::new(Mutex::new(Vec::new()));
    let user_text = "我特别特别开心，快笑一个";
    let mut eng = motion_engine(
        &llm_base,
        &tts_base,
        None, // 二路请求失败 → 回退规则
        ids(&["smile", "tilt_left", "none"]),
        false,
        true,
        Arc::clone(&rule_log),
    );

    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(404, user_text, tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    let _ = drain_events(rx).await;

    let log = rule_log.lock().expect("poison").clone();
    assert_eq!(log.len(), 1, "一句回退一次");
    assert_eq!(log[0].0, "嗯。", "规则表实参是这一句原文");
    assert_ne!(log[0].0, user_text, "用户句绝不能进规则回退");
    assert_eq!(log[0].1, 1, "句号 = 这一句");
}

/// 动作门**关**着：不发 ActionCue（回归既有行为）。
#[tokio::test]
async fn motion_off_emits_no_action_cue() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天天气不错。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    // 普通（非 motion）二路：`set_motion` 不调用。
    let mut eng = engine(&llm_base, &tts_base, ConversationConfig::default());
    eng.set_cleaner(Some(Arc::new(SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: Some(r#"{"display":"今天天气不错。","speech":"今天天气不错。"}"#.to_string()),
        }),
        CLEAN_TIMEOUT_MS,
    ))));

    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(405, "你好", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    let events = drain_events(rx).await;
    assert!(
        action_cues(&events).is_empty(),
        "动作门关着不得发 ActionCue"
    );
    // 用例只证明「关着不发」，给个超时余量防挂。
    let _ = Duration::from_millis(1);
}

// ---- 表演倾向（2026-10-13 执行单 T3）-----------------------------------------

/// 克制 / 平常：模型交回空数组仍是**成功**，不得回退规则表（两档各跑一遍）。
#[tokio::test]
async fn motion_bias_restrained_and_normal_empty_cues_do_not_fall_back() {
    for (label,) in [("克制",), ("平常",)] {
        let llm_base = spawn_multi_server(respond_pieces(
            200,
            "OK",
            "text/event-stream",
            vec![sse_content("今天天气不错。"), sse_done()],
        ))
        .await;
        let (tts_base, _tts_inputs) = spawn_tts_mock().await;
        let rule_log = Arc::new(Mutex::new(Vec::new()));
        let mut eng = motion_engine(
            &llm_base,
            &tts_base,
            Some(r#"{"display":"今天天气不错。","speech":"今天天气不错。","cues":[]}"#),
            ids(&["nod", "tilt_left", "none"]),
            false, // 两档的空数组回退开关都是 false（真源判据在 director 单测）
            true,
            Arc::clone(&rule_log),
        );

        let (tx, rx) = mpsc::channel(64);
        let report = eng
            .run_turn(410, "随便聊聊", tx, CancellationToken::new())
            .await;
        assert_eq!(report.status, TurnStatus::Completed, "{label}");
        let events = drain_events(rx).await;

        assert!(
            action_cues(&events).is_empty(),
            "{label}：空数组 = 这一句不动，不得发 ActionCue"
        );
        assert!(
            rule_log.lock().expect("poison").is_empty(),
            "{label}：空数组是成功，不得回退规则表"
        );
    }
}

/// 主动档：模型交回空数组、且规则表对这一句（含 `？` 的原文）有动作 →
/// 改用规则表（非空 cue，实参 = 这一句原文）。
#[tokio::test]
async fn motion_bias_eager_empty_cues_fall_back_to_the_rule_table() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("这样可以吗？"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let rule_log = Arc::new(Mutex::new(Vec::new()));
    let mut eng = motion_engine(
        &llm_base,
        &tts_base,
        Some(r#"{"display":"这样可以吗？","speech":"这样可以吗？","cues":[]}"#),
        ids(&["nod", "tilt_left", "thinking", "none"]),
        true,
        true,
        Arc::clone(&rule_log),
    );

    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(411, "别的问题", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    let events = drain_events(rx).await;

    let log = rule_log.lock().expect("poison").clone();
    assert_eq!(
        log,
        [("这样可以吗？".to_string(), 1)],
        "规则表只看这一句原文"
    );
    assert_eq!(
        action_cues(&events),
        [(1, "tilt_left".to_string())],
        "主动档：空数组 + 规则表有动作 → 非空 cue（seq = 这一句）"
    );
}

/// 主动档：规则表对**这一句**（无问号、无情绪词）没有动作 → 仍为空、不发 ActionCue。
///
/// 注入的「规则表」模拟真表的**中性 = 空**语义（真表的中性空行为由 director
/// 的单测钉住）：规则表被咨询一次、交回空表 = 这一句不动。
#[tokio::test]
async fn motion_bias_eager_neutral_sentence_stays_empty() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天天气不错。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let rule_log = Arc::new(Mutex::new(Vec::new()));
    let mut eng = motion_engine(
        &llm_base,
        &tts_base,
        Some(r#"{"display":"今天天气不错。","speech":"今天天气不错。","cues":[]}"#),
        ids(&["nod", "tilt_left", "none"]),
        true,
        false, // 中性句：真规则表交回空表（注入同语义替身）
        Arc::clone(&rule_log),
    );

    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(412, "随便聊聊", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    let events = drain_events(rx).await;

    let log = rule_log.lock().expect("poison").clone();
    assert_eq!(
        log,
        [("今天天气不错。".to_string(), 1)],
        "主动档：空数组时确实咨询了规则表（实参 = 这一句原文）"
    );
    assert!(
        action_cues(&events).is_empty(),
        "规则表没动作 → 仍为空，不得发 ActionCue"
    );
}
