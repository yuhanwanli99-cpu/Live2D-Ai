//! **按句第二路 / 动作表**的引擎集成回归（2026-10-13 T14；2026-10-17 rc4 常驻化）。
//!
//! rc4 起动作**恒开**、不依赖 director Mod 配置：
//! - 每句原文交给二路清洗**恰一次**；模型成功给 cues 就用模型的；
//! - 失败 / 超时 / 坏 JSON / display 不是原文切片 → 回退**这一句原文**的规则表；
//! - 空数组 = 这一句不动（**只有主动档**在规则表有动作时改用规则表）；
//! - 二路不可用 → `clean_for_tts(句)` + 这一句原文的规则表；
//! - 每句非空 cues 发一条 ActionCue，空表不发。
//!
//! 规则表单一真源已从 director 迁入 runtime（`performance::rules`），
//! 这里断言的是**真表**结果；用户句绝不能进规则回退。

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use live2d_ai_runtime::cleaning::{CLEAN_TIMEOUT_MS, MotionBias, SentenceCleaner};
use live2d_ai_runtime::performance::client::{
    PerformanceClient, PerformanceFuture, PerformanceReply,
};
use live2d_ai_runtime::{ConversationConfig, ConversationEngine, EngineEvent, TurnStatus};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::{
    drain_events, engine, respond_pieces, spawn_multi_server, spawn_tts_mock, sse_content, sse_done,
};

/// 二路替身：计数每次请求；`hang=true` = 永不返回（取消测试用）。
#[derive(Debug)]
struct StubCleaner {
    reply: Option<String>,
    hang: bool,
    calls: Arc<AtomicUsize>,
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
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.hang {
            return Box::pin(std::future::pending::<Option<PerformanceReply>>());
        }
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

/// 常驻引擎：cleaner 的三态（无 / 普通 / 带动作与倾向）。
enum Cleaner {
    None,
    Plain(Option<&'static str>),
    Motion {
        reply: Option<&'static str>,
        bias: MotionBias,
        hang: bool,
    },
}

fn build(
    llm_base: &str,
    tts_base: &str,
    cleaner: Cleaner,
) -> (ConversationEngine, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut eng = engine(llm_base, tts_base, ConversationConfig::default());
    let stub = |reply: Option<&str>, hang: bool| {
        Box::new(StubCleaner {
            reply: reply.map(str::to_string),
            hang,
            calls: Arc::clone(&calls),
        })
    };
    match cleaner {
        Cleaner::None => {}
        Cleaner::Plain(reply) => {
            eng.set_cleaner(Some(Arc::new(SentenceCleaner::new(
                stub(reply, false),
                CLEAN_TIMEOUT_MS,
            ))));
        }
        Cleaner::Motion { reply, bias, hang } => {
            let cleaner = SentenceCleaner::new(stub(reply, hang), CLEAN_TIMEOUT_MS).with_motion(
                ids(&["nod", "smile", "unhappy", "thinking", "tilt_left", "none"]),
                bias,
            );
            eng.set_cleaner(Some(Arc::new(cleaner)));
        }
    }
    (eng, calls)
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

async fn run(
    eng: &mut ConversationEngine,
    epoch: u64,
    user_text: &str,
) -> (TurnStatus, Vec<EngineEvent>) {
    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(epoch, user_text, tx, CancellationToken::new())
        .await;
    (report.status, drain_events(rx).await)
}

/// 没有任何 cleaner / 导演配置：动作仍按这一句原文恒出（always-on）。
#[tokio::test]
async fn always_on_rule_cues_without_any_cleaner() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("（挥手）今天真开心。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(&llm_base, &tts_base, Cleaner::None);

    let (status, events) = run(&mut eng, 501, "随便聊聊").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(
        action_cues(&events),
        [(1, "smile".to_string())],
        "无 cleaner 也按本句原文（开心）出规则 cue"
    );
    assert_eq!(ready_by_seq(&events), [(1, "今天真开心。".to_string())]);
    assert_eq!(*tts_inputs.lock().unwrap(), ["今天真开心。"]);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "没有 cleaner 就不发二路请求"
    );
}

/// 每句二路**恰一次**；模型 cues 生效。
#[tokio::test]
async fn cleaner_is_called_once_per_sentence_and_model_cues_win() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("你好呀。你好呀。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: Some(r#"{"display":"你好呀。","speech":"你好呀。","cues":["nod"]}"#),
            bias: MotionBias::new("test".to_string(), false),
            hang: false,
        },
    );

    let (status, events) = run(&mut eng, 502, "用户句在这里").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "两句 = 两次二路调用");
    assert_eq!(
        action_cues(&events),
        [(1, "nod".to_string()), (2, "nod".to_string())]
    );
}

/// 空数组 = 这一句不动（平常档**不**回退规则表）。
#[tokio::test]
async fn empty_cues_mean_no_motion_on_normal_bias() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天天气不错。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: Some(r#"{"display":"今天天气不错。","speech":"今天天气不错。","cues":[]}"#),
            bias: MotionBias::new("test".to_string(), false),
            hang: false,
        },
    );

    let (status, events) = run(&mut eng, 503, "随便聊聊").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(action_cues(&events).is_empty(), "空数组 = 这一句不动");
}

/// 主动档：空数组且规则表对这一句有动作 → 改用规则表（实参 = 这一句原文）。
#[tokio::test]
async fn eager_empty_cues_fall_back_to_this_sentence_rules() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("这样可以吗？"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: Some(r#"{"display":"这样可以吗？","speech":"这样可以吗？","cues":[]}"#),
            bias: MotionBias::new("test".to_string(), true),
            hang: false,
        },
    );

    let (status, events) = run(&mut eng, 504, "用户问的是别的").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        action_cues(&events),
        [(1, "tilt_left".to_string()), (1, "thinking".to_string())],
        "主动档 + 问句 → 规则表补手势与表情"
    );
}

/// 主动档但对**这一句**没有动作（中性陈述）→ 仍为空、不发 ActionCue。
#[tokio::test]
async fn eager_neutral_sentence_stays_empty() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天天气不错。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let (mut eng, _calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: Some(r#"{"display":"今天天气不错。","speech":"今天天气不错。","cues":[]}"#),
            bias: MotionBias::new("test".to_string(), true),
            hang: false,
        },
    );

    let (status, events) = run(&mut eng, 505, "随便聊聊").await;
    assert_eq!(status, TurnStatus::Completed);
    assert!(action_cues(&events).is_empty());
}

/// 二路请求失败 → 回退**这一句原文**的规则表；用户句（开心）不得进回退。
#[tokio::test]
async fn failed_request_falls_back_to_this_sentence_rules_not_user_text() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天有点难过。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: None,
            bias: MotionBias::new("test".to_string(), false),
            hang: false,
        },
    );

    let (status, events) = run(&mut eng, 506, "我好开心，快笑一个").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        action_cues(&events),
        [(1, "unhappy".to_string())],
        "回退看的是这一句原文（难过），不是用户句（开心）"
    );
    assert_eq!(
        ready_by_seq(&events),
        [(1, "今天有点难过。".to_string())],
        "失败时上屏/送 TTS 都是原文"
    );
    assert_eq!(*tts_inputs.lock().unwrap(), ["今天有点难过。"]);
}

/// 坏 JSON → 回退这一句原文的规则表。
#[tokio::test]
async fn bad_json_falls_back_to_this_sentence_rules() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("这样可以吗？"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: Some("这不是 JSON"),
            bias: MotionBias::new("test".to_string(), false),
            hang: false,
        },
    );

    let (status, events) = run(&mut eng, 507, "别的问题").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        action_cues(&events),
        [(1, "tilt_left".to_string()), (1, "thinking".to_string())]
    );
}

/// display 不是原文切片 → 整句作废 → 回退这一句原文的规则表。
#[tokio::test]
async fn display_not_a_slice_falls_back_to_this_sentence_rules() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天有点难过。"), sse_done()],
    ))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: Some(r#"{"display":"原文没有的片段","speech":"x","cues":["smile"]}"#),
            bias: MotionBias::new("test".to_string(), false),
            hang: false,
        },
    );

    let (status, events) = run(&mut eng, 508, "随便聊聊").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        action_cues(&events),
        [(1, "unhappy".to_string())],
        "校验失败 → 规则表（不是模型给的 smile）"
    );
}

/// 二路没请求 cues（motion=false）：模型两字段成功，但 cue 未解析 → 规则表。
#[tokio::test]
async fn cleaner_without_cues_falls_back_to_rules() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("今天真开心。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Plain(Some(
            r#"{"display":"今天真开心。","speech":"今天真开心。"}"#,
        )),
    );

    let (status, events) = run(&mut eng, 509, "随便聊聊").await;
    assert_eq!(status, TurnStatus::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        action_cues(&events),
        [(1, "smile".to_string())],
        "未解析 cues → 这一句原文的规则表"
    );
    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["今天真开心。"],
        "上屏 == 送 TTS"
    );
}

/// 二路清洗期间取消 → Cancelled，安静收场（不发 cue、不送 TTS）。
#[tokio::test]
async fn cancel_during_cleaner_is_cancelled_without_cues() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("你好呀。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = build(
        &llm_base,
        &tts_base,
        Cleaner::Motion {
            reply: None,
            bias: MotionBias::new("test".to_string(), false),
            hang: true,
        },
    );

    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let (tx, rx) = mpsc::channel(64);
    let task = tokio::spawn(async move { eng.run_turn(510, "你好", tx, token).await });

    // 等二路请求真的发出，再取消——确保取消落在 cleaner.clean() 的 select 上。
    for _ in 0..200 {
        if calls.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(calls.load(Ordering::SeqCst) > 0, "二路请求没发出");
    cancel.cancel();
    let report = task.await.expect("任务不应 panic");
    assert_eq!(report.status, TurnStatus::Cancelled);

    let events = drain_events(rx).await;
    assert!(action_cues(&events).is_empty(), "取消不得发 ActionCue");
    assert!(tts_inputs.lock().unwrap().is_empty(), "取消不得送 TTS");
}
