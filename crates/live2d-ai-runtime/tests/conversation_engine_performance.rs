//! 表演层接线集成测试（2026-09-22）：
//! **speak = TTS / 上屏真源**、noop / 只说 / 只动 三态、失败回退。
//!
//! rc4（2026-10-17 返工）：[performance].enabled=true 时文本路径由表演层接管
//!（segments / speak），但**导演常驻**——每一段原文仍走核心规则表产 ActionCue
//!（纯本地纯函数、**不新增 LLM 调用**）；表演层自己的 resolution.cues 继续抑制。
//! 缺省关（performance == None）的流式按句路径行为不变（回归见
//! conversation_engine_motion.rs）。

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use live2d_ai_runtime::performance::{
    PerformanceClient, PerformanceCue, PerformanceFuture, PerformanceReply, PerformanceRuntime,
    RuleFallback, StructuredMode,
};
use live2d_ai_runtime::{ConversationConfig, EngineEvent, TurnStatus};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::{
    drain_events, engine, respond_pieces, spawn_multi_server, spawn_tts_mock, sse_content, sse_done,
};

/// 表演层测试替身：固定回一份原始正文（或失败）；calls 计二路 LLM 调用次数。
#[derive(Debug)]
struct StubPerformance {
    reply: Option<String>,
    calls: Arc<AtomicUsize>,
}

impl PerformanceClient for StubPerformance {
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
        let reply = self.reply.clone().map(|raw| PerformanceReply {
            raw,
            structured: true,
        });
        Box::pin(async move { reply })
    }
}

fn allow() -> Vec<String> {
    vec!["nod".to_string(), "smile".to_string()]
}

fn rule_cue() -> RuleFallback {
    Arc::new(|text: &str| {
        if text.trim().is_empty() {
            return Vec::new();
        }
        vec![PerformanceCue {
            sentence_seq: 1,
            preset_id: "smile".to_string(),
            intensity: 1,
            ttl_ms: 2_000,
            ..Default::default()
        }]
    })
}

fn performance_engine(
    llm_base: &str,
    tts_base: &str,
    reply: Option<&str>,
) -> live2d_ai_runtime::ConversationEngine {
    performance_engine_counted(llm_base, tts_base, reply).0
}

/// 同 performance_engine，另交回二路 LLM 调用计数（证明规则表不新增 LLM 调用）。
fn performance_engine_counted(
    llm_base: &str,
    tts_base: &str,
    reply: Option<&str>,
) -> (live2d_ai_runtime::ConversationEngine, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut eng = engine(llm_base, tts_base, ConversationConfig::default());
    let runtime = PerformanceRuntime::new(
        Box::new(StubPerformance {
            reply: reply.map(str::to_string),
            calls: Arc::clone(&calls),
        }),
        allow(),
        1_500,
        StructuredMode::Auto,
        Some(rule_cue()),
    );
    eng.set_performance(Some(Arc::new(runtime)));
    (eng, calls)
}

fn cues_of(events: &[EngineEvent]) -> Vec<Vec<(u64, String)>> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::ActionCue { cues, .. } => Some(
                cues.iter()
                    .map(|c| (c.sentence_seq, c.preset_id.clone()))
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

/// ActionCue 帧级视图：covers_upto_seq + 帧内 cues（证明「非空才发 / 无双发」）。
fn cue_frames(events: &[EngineEvent]) -> Vec<(u64, Vec<(u64, String)>)> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::ActionCue {
                covers_upto_seq,
                cues,
                ..
            } => Some((
                *covers_upto_seq,
                cues.iter()
                    .map(|c| (c.sentence_seq, c.preset_id.clone()))
                    .collect(),
            )),
            _ => None,
        })
        .collect()
}

/// **只说**：speak 切句后逐句送 TTS；导演常驻 → 规则命中段仍发 ActionCue，
/// 表演层自己的 cues 被抑制；二路 LLM 只调用 1 次（规则表是纯本地函数）。
#[tokio::test]
async fn speak_is_the_tts_source_and_rule_cues_still_fire() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("（挥手）你好呀。*歪头*再见。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = performance_engine_counted(
        &llm_base,
        &tts_base,
        Some(
            r#"{"speak":"你好呀。再见。","cues":[{"sentence_seq":1,"preset_id":"smile","intensity":2,"ttl_ms":1500}]}"#,
        ),
    );
    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(101, "你好", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    // 回灌历史的仍是主模型**原文**（表演层的整理不进模型上下文）。
    assert_eq!(report.assistant_text, "（挥手）你好呀。*歪头*再见。");

    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["你好呀。", "再见。"],
        "送 TTS 的必须是 speak 切出的句子"
    );

    let events = drain_events(rx).await;
    let voiced: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SentenceVoiced { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(voiced, ["你好呀。", "再见。"], "上屏必须等于 speak 的句子");
    // 「你好呀。」= 问候 → 规则表 nod；「再见。」= 告别（缺省 none）→ 无 cue。
    // 若错误地把表演层 cues（smile）发出来，这里会看到 smile。
    assert_eq!(
        cue_frames(&events),
        vec![(1, vec![(1, "nod".to_string())])],
        "规则命中段恰一条 ActionCue，表演层 cues 被抑制：{events:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "规则表不得新增 LLM 调用");
}

/// **noop**：不说也不动——零 TTS、零上屏，仍然 Completed。
#[tokio::test]
async fn noop_plan_sends_no_tts_and_no_text() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("（沉默地看了你一眼）"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let mut eng = performance_engine(&llm_base, &tts_base, Some(r#"{"speak":null,"cues":[]}"#));
    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(102, "在吗", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert!(tts_inputs.lock().unwrap().is_empty(), "noop 不得发 TTS");

    let events = drain_events(rx).await;
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, EngineEvent::SentenceVoiced { .. })),
        "noop 不得上屏任何文字：{events:?}"
    );
    assert!(
        cues_of(&events).is_empty(),
        "没有文本段就没有规则 cue，也不发空 ActionCue：{events:?}"
    );
}

/// **只动**：speak/segments 皆空 + 表演层 cues → 引擎什么都不发（无无声锚句，
/// 表演层 cue 不投递；规则表没有文本段可看 → 0 cue）。
#[tokio::test]
async fn cue_only_plan_sends_nothing() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("（点头）"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let mut eng = performance_engine(
        &llm_base,
        &tts_base,
        Some(
            r#"{"speak":"","cues":[{"sentence_seq":1,"preset_id":"nod","intensity":1,"ttl_ms":900}]}"#,
        ),
    );
    let (tx, rx) = mpsc::channel(64);
    let report = eng.run_turn(103, "嗯", tx, CancellationToken::new()).await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert!(tts_inputs.lock().unwrap().is_empty(), "没有可说的文本");

    let events = drain_events(rx).await;
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, EngineEvent::SentenceVoiced { .. })),
        "不计造无声锚句：{events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, EngineEvent::AudioChunk { .. })),
        "没有句子就没有音频边界帧：{events:?}"
    );
    assert!(
        cues_of(&events).is_empty(),
        "表演层 cues 不投递，且无文本段 → 0 ActionCue：{events:?}"
    );
}

/// **失败回退**：坏 JSON → speak=clean_for_tts(原文)；导演常驻 → 规则表按段出 cue。
#[tokio::test]
async fn invalid_plan_falls_back_to_cleaned_raw_and_rule_cues() {
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("（挥手）你好呀。*歪头*再见。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = performance_engine_counted(&llm_base, &tts_base, Some("这不是 JSON"));
    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(104, "你好", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["你好呀。", "再见。"],
        "回退也必须出声，且文本是确定性清洗产物"
    );
    let events = drain_events(rx).await;
    // 回退路径的表演层 cue 是注入闭包的 smile（不允许投递）；真 cue 来自核心
    // 规则表：「你好呀。」→ nod。
    assert_eq!(
        cue_frames(&events),
        vec![(1, vec![(1, "nod".to_string())])],
        "回退后规则命中段仍恰一条 ActionCue：{events:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "不得新增 LLM 调用");
    assert!(eng.performance_stats().is_some());
}

/// **主模型路径审计**（2026-09-22）：表演层开着也不会把它的提示词 / schema /
/// cues 注入主模型请求体——主模型只拿 system（人设）+ 历史 + 本轮输入。
#[tokio::test]
async fn performance_never_leaks_into_the_main_model_request() {
    let seen: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let handler_seen = Arc::clone(&seen);
    let llm_base = spawn_multi_server(common::Handler::new(move |mut sock| {
        let seen = Arc::clone(&handler_seen);
        Box::pin(async move {
            let raw = common::read_request(&mut sock).await;
            seen.lock()
                .expect("lock")
                .push(String::from_utf8_lossy(&raw.body).to_string());
            let pieces = [sse_content("你好呀。"), sse_done()];
            let total: usize = pieces.iter().map(Vec::len).sum();
            common::write_head(&mut sock, 200, "OK", "text/event-stream", total).await;
            common::write_pieces(&mut sock, &pieces, 5).await;
            common::close_sock(&mut sock).await;
        })
    }))
    .await;
    let (tts_base, _tts_inputs) = spawn_tts_mock().await;
    let mut eng = engine(
        &llm_base,
        &tts_base,
        ConversationConfig::new("你是桌宠，只写剧情。"),
    );
    let runtime = PerformanceRuntime::new(
        Box::new(StubPerformance {
            reply: Some(r#"{"speak":"你好呀。","cues":[]}"#.to_string()),
            calls: Arc::new(AtomicUsize::new(0)),
        }),
        allow(),
        1_500,
        StructuredMode::Auto,
        None,
    );
    eng.set_performance(Some(Arc::new(runtime)));
    let (tx, _rx) = mpsc::channel(64);
    let report = eng
        .run_turn(105, "你好", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);

    let body = seen.lock().expect("lock")[0].clone();
    for forbidden in [
        "speak",
        "cues",
        "preset_id",
        "json_schema",
        "表演层",
        "sentence_seq",
    ] {
        assert!(
            !body.contains(forbidden),
            "主模型请求体不得含表演层字段 {forbidden}：{body}"
        );
    }
    assert!(
        body.contains("你是桌宠，只写剧情。"),
        "人设必须原样保留：{body}"
    );
    assert_eq!(
        body.matches("\"role\"").count(),
        2,
        "只有 system + user（无历史）：{body}"
    );
}

// ---------------------------------------------------------------- v1 segments（D22/D23/D24）

/// 按 seq 收集上屏文本（SentenceVoiced）。
fn voiced_by_seq(events: &[EngineEvent]) -> Vec<(u64, String)> {
    events
        .iter()
        .filter_map(|e| match e {
            EngineEvent::SentenceVoiced {
                sentence_seq, text, ..
            } => Some((*sentence_seq, text.clone())),
            _ => None,
        })
        .collect()
}

/// 按 seq 收集送 TTS 前的句子事件文本（SentenceReady == 送 TTS）。
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

/// **D22**：表演层给的 segments **一对一**成为 TTS 单元 / 音频元素
///（seg:N ≡ sentence_seq==N），**不再过分句器二次切分**；中性段不发 cue。
#[tokio::test]
async fn segments_are_one_to_one_with_sentences_and_never_resplit() {
    // 原文有三个句读，但表演层只切两段：若引擎二次切句，就会变成 3 段。
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content("第一句。第二句。第三句。"), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = performance_engine_counted(
        &llm_base,
        &tts_base,
        Some(
            r#"{"segments":["第一句。第二句。","第三句。"],"cues":[{"field":"head","y":0.2,"intensity":1,"at":"seg:2","hold":false}]}"#,
        ),
    );
    let (tx, rx) = mpsc::channel(64);
    let report = eng.run_turn(201, "说", tx, CancellationToken::new()).await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["第一句。第二句。", "第三句。"],
        "segments 必须一对一送 TTS，不得二次切分"
    );
    let events = drain_events(rx).await;
    assert_eq!(
        ready_by_seq(&events),
        vec![
            (1, "第一句。第二句。".to_string()),
            (2, "第三句。".to_string())
        ],
        "seg:N ≡ sentence_seq==N：{events:?}"
    );
    assert_eq!(
        voiced_by_seq(&events),
        vec![
            (1, "第一句。第二句。".to_string()),
            (2, "第三句。".to_string())
        ]
    );
    // 两段都是中性原文 → 规则表空 → 0 ActionCue（中性段不发，表演层 cue 被抑制）。
    assert!(
        cues_of(&events).is_empty(),
        "中性段不发 ActionCue：{events:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1, "不得新增 LLM 调用");
}

/// **导演常驻 + 表演层接管**：每段原文独立过规则表——命中段恰一条 ActionCue
///（先于该段 SentenceReady / 音频），中性段不发，表演层自己的 cues 不投递。
#[tokio::test]
async fn performance_segments_get_rule_cues_once_per_hit_segment() {
    let raw = "你好呀。今天天气不错。这样吗？";
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content(raw), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let (mut eng, calls) = performance_engine_counted(
        &llm_base,
        &tts_base,
        Some(
            r#"{"segments":["你好呀。","今天天气不错。","这样吗？"],"cues":[{"sentence_seq":1,"preset_id":"smile","intensity":1,"ttl_ms":900}]}"#,
        ),
    );
    let (tx, rx) = mpsc::channel(64);
    let report = eng.run_turn(204, "说", tx, CancellationToken::new()).await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["你好呀。", "今天天气不错。", "这样吗？"],
        "segments 一对一送 TTS"
    );

    let events = drain_events(rx).await;
    // 规则表：问候 → nod；中性陈述 → 空；问句 → 手势 tilt_left + 表情 thinking。
    assert_eq!(
        cue_frames(&events),
        vec![
            (1, vec![(1, "nod".to_string())]),
            (
                3,
                vec![(3, "tilt_left".to_string()), (3, "thinking".to_string())]
            ),
        ],
        "命中段恰一条 ActionCue，中性段不发，表演层 smile 被抑制：{events:?}"
    );
    // 无「双发」：摊平后恰好这三条，且每帧 covers_upto_seq == 该段 seq（上面已断言）。
    let flat = cues_of(&events);
    assert_eq!(flat.len(), 2, "两个命中段各一帧：{events:?}");
    assert!(
        flat.iter().flatten().all(|(_, id)| id != "smile"),
        "表演层自己的 cues 不得投递"
    );
    // cue 必须在该段 SentenceReady 之前（前端才能按 seq 锚到音频）。
    let first_cue = events
        .iter()
        .position(|e| matches!(e, EngineEvent::ActionCue { .. }))
        .expect("必须有 cue 帧");
    let first_ready = events
        .iter()
        .position(|e| matches!(e, EngineEvent::SentenceReady { .. }))
        .expect("必须有 SentenceReady");
    assert!(first_cue < first_ready, "cue 必须先于该段音频/上屏");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "规则表不得新增 LLM 调用");
}

/// **D23**：每段上屏文本 == 送 TTS 文本 == clean_for_tts(段)——不得把原文
/// 的 Markdown / 舞台指示直接上屏。
#[tokio::test]
async fn displayed_text_equals_tts_text_per_segment() {
    let raw = "（挥手）你好呀。*歪头*再见。";
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content(raw), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let mut eng = performance_engine(
        &llm_base,
        &tts_base,
        Some(r#"{"segments":["（挥手）你好呀。","*歪头*再见。"],"cues":[]}"#),
    );
    let (tx, rx) = mpsc::channel(64);
    let report = eng
        .run_turn(202, "你好", tx, CancellationToken::new())
        .await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["你好呀。", "再见。"],
        "送 TTS 的必须是 clean_for_tts(段)"
    );
    let events = drain_events(rx).await;
    let expected = vec![(1, "你好呀。".to_string()), (2, "再见。".to_string())];
    assert_eq!(ready_by_seq(&events), expected, "SentenceReady == 送 TTS");
    assert_eq!(voiced_by_seq(&events), expected, "上屏 == 送 TTS");
    for (_, text) in voiced_by_seq(&events) {
        assert!(
            !text.contains('（') && !text.contains('*'),
            "不得上屏原始标记：{text}"
        );
    }
}

/// **D24**：纯空白段**仍产静音元素与 start/end 边界帧**，seg 编号**不跳**。
#[tokio::test]
async fn blank_segment_keeps_its_seg_index() {
    let raw = "第一句。  第三句。";
    let llm_base = spawn_multi_server(respond_pieces(
        200,
        "OK",
        "text/event-stream",
        vec![sse_content(raw), sse_done()],
    ))
    .await;
    let (tts_base, tts_inputs) = spawn_tts_mock().await;
    let mut eng = performance_engine(
        &llm_base,
        &tts_base,
        Some(r#"{"segments":["第一句。","  ","第三句。"],"cues":[]}"#),
    );
    let (tx, rx) = mpsc::channel(64);
    let report = eng.run_turn(203, "说", tx, CancellationToken::new()).await;
    assert_eq!(report.status, TurnStatus::Completed);
    assert_eq!(
        *tts_inputs.lock().unwrap(),
        ["第一句。", "第三句。"],
        "空白段不发 TTS HTTP（上游会对空 input 回 400）"
    );
    let events = drain_events(rx).await;
    let expected = vec![
        (1, "第一句。".to_string()),
        (2, String::new()),
        (3, "第三句。".to_string()),
    ];
    assert_eq!(ready_by_seq(&events), expected, "空白段仍占号：{events:?}");
    assert_eq!(voiced_by_seq(&events), expected, "空白段仍上屏（空文本）");
    let blank = events.iter().find(|e| {
        matches!(
            e,
            EngineEvent::AudioChunk {
                sentence_seq: 2,
                first_chunk: true,
                final_chunk: true,
                ..
            }
        )
    });
    assert!(
        blank.is_some(),
        "空白段必须产出 start/end 边界帧（seg 编号不跳）：{events:?}"
    );
}
