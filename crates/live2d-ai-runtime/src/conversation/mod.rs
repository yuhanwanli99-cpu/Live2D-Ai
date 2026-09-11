//! 最小异步对话引擎 [`ConversationEngine`]：把「LLM 流式回复 → 切句 → TTS →
//! PCM 音频块」串成**一条流水线**，通过单个有界事件通道对外暴露
//! [`EngineEvent`] 流。
//!
//! # 流水线与重叠
//!
//! ```text
//! run_turn 任务                         TTS worker 任务（每次 run_turn 恰一个）
//! ─────────────────────────────        ──────────────────────────────────────
//! OpenAiClient::chat_stream ─┐         synthesize_speech(第 1 句)
//!   └─ TextDelta ───────────►│ event_tx │  └─ PcmS16LeDecoder 增量解码
//!      DialogueAssembler      ◄────────┤     └─ AudioChunk{final_chunk=false…}
//!      └─ SentenceReady ──► 有界队列 ──► │  末尾 AudioChunk{final_chunk=true}
//! ```
//!
//! LLM 边流边切句，第一句进队后 TTS worker **立刻**开始合成——不等 LLM 结束；
//! 队列满时推送阻塞（背压回传到 LLM 消费速率），内存因此有界。
//!
//! 2026-09-11 用户裁决：**LLM 不暴露任何工具，只做对话**——过去图中那条
//! `ToolReady ──► ToolAction` 支线（工具调用 → 动作事件）已整体移除，
//! 引擎只产出正文文本、语音与终态。
//!
//! # 事件契约
//!
//! - 每个[`EngineEvent`]都携带构造 `run_turn` 时传入的 `epoch`，core 据此做
//!   二次闸门（丢弃过期轮次的迟到事件）；
//! - 每次成功运行的收尾**恰好一个终态序列**：
//!   三态统一经 `Terminal { status }` 收场（步骤 7 / D8）：正常 → `Completed`；
//!   致命/取消各有专属 status。`Terminal` 保证出现在「LLM 已结束 **且**
//!   TTS worker 完全排空」之后；
//! - 每句音频恰好一个 `first_chunk = true` 与一个 `final_chunk = true` 的
//!   [`EngineEvent::AudioChunk`]（首块/末块，共同界定这一句的音频边界），
//!   `sentence_seq` 从 1 开始严格递增，与播放顺序一致。
//!
//! # 取消与并发
//!
//! - 每个 await 点（网络读取、事件发送、队列推送、TTS 读取）都与
//!   `CancellationToken` `select!`；取消后引擎不再产生新业务事件，尽力 drop
//!   底层响应，最后只补发一次 `Terminal { Cancelled }`；
//! - 引擎自身每次 `run_turn` 只 spawn 一个 TTS worker 并在返回前 `join`——
//!   不存在泄漏的 worker；同一引擎的并发多轮防护由调用方/root core 保证
//!   （`&mut self` 在单任务内已天然互斥）。
//!
//! # 用法示意
//!
//! ```no_run
//! use live2d_ai_runtime::{
//!     ConversationConfig, ConversationEngine, EngineEvent, LlmConfig, OpenAiClient, TtsConfig,
//! };
//! use tokio::sync::mpsc;
//! use tokio_util::sync::CancellationToken;
//!
//! # async fn demo() {
//! let client = OpenAiClient::new(
//!     LlmConfig::new("http://127.0.0.1:11434/v1", "qwen2.5:7b"),
//!     TtsConfig::new("http://127.0.0.1:8000/v1", "alloy"),
//! )
//! .unwrap();
//! let mut engine = ConversationEngine::new(client, ConversationConfig::new("你是桌宠"));
//!
//! let (event_tx, mut event_rx) = mpsc::channel(64);
//! let cancel = CancellationToken::new();
//! let report = engine.run_turn(1, "打个招呼", event_tx, cancel).await;
//!
//! while let Some(event) = event_rx.recv().await {
//!     match event {
//!         // LLM 原始增量（**不上屏**；上屏以 SentenceVoiced 为准）。
//!         EngineEvent::TextDelta { text, .. } => print!("{text}"),
//!         EngineEvent::AudioChunk { samples, .. } => { /* 送声卡 */ }
//!         // 该句语音已完整合成 → 此时才把整句文字推给 UI。
//!         EngineEvent::SentenceVoiced { text, .. } => println!("[屏] {text}"),
//!         EngineEvent::Terminal { .. } => break,
//!         EngineEvent::Error { kind, .. } => eprintln!("turn error: {kind}"),
//!     }
//! }
//! # let _ = report;
//! # }
//! ```
//!
//! # 子模块布局
//!
//! - [`events`]：事件投递（与取消竞争 + 终态兜底超时）。
//! - [`queue`]：句子入队结果与背压超时。
//! - [`worker`]：TTS worker、单句合成、音频切块辅助。
//! - [`engine`]：`ConversationEngine` 主体（`run_turn`、历史装配与提交）。

mod engine;
/// `ErrorKind` 的可观测投影（错误码 / 阶段 / 提示）。**纯函数**，见文件头注。
mod error_code;
mod events;
mod queue;
mod worker;

pub use engine::ConversationEngine;

use std::time::Duration;

use crate::audio::AudioSpec;
use crate::error::Error;

/// 终态事件的有界投递上限（附带发现 #3 兜底）：卡死的消费端最多拖住
/// 这么久；超时即放弃——supervisor 的权威终态是 [`TurnReport`] 返回值，
/// 不依赖事件送达。
pub(crate) const TERMINAL_SEND_TIMEOUT: Duration = Duration::from_secs(3);

// ---------------------------------------------------------------- 配置

/// 对话引擎配置（最小字段集；历史记忆可先关闭）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationConfig {
    /// 每轮请求固定注入的 system 提示；空串表示不发 system 消息。
    pub system_prompt: String,
    /// 保留的历史轮数上限（一轮 = 一条 user + 一条 assistant）。
    /// `0` = 不保留历史（每轮独立，本批默认）。
    pub max_history_pairs: usize,
    /// 切句缓冲的字符上限（透传 [`crate::dialogue::SentenceAssembler`]）。
    pub sentence_max_chars: usize,
    /// LLM → TTS 的有界句子队列容量。推满后阻塞等待（背压），或按
    /// [`Self::queue_push_timeout`] 报 [`ErrorKind::Backpressure`]。
    pub tts_queue_capacity: usize,
    /// 单个 [`EngineEvent::AudioChunk`] 的最大样本数；超出按此切块，
    /// 句尾剩余样本随 `final_chunk = true` 的事件发出。
    pub audio_chunk_samples: usize,
    /// 把句子推进有界队列的最长等待时长；`None` = 无限等待（纯背压语义）。
    pub queue_push_timeout: Option<Duration>,
}

impl ConversationConfig {
    /// 默认单块音频样本数：约 200 ms @ 24 kHz 单声道。
    pub const DEFAULT_AUDIO_CHUNK_SAMPLES: usize = 4_800;
    /// 默认 TTS 句子队列容量：够覆盖一次网络抖动，又不至于堆积过多待播文本。
    pub const DEFAULT_TTS_QUEUE_CAPACITY: usize = 4;

    /// 以给定 system 提示创建配置（其余字段取 [`Default`]）。
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            ..Self::default()
        }
    }
}

impl Default for ConversationConfig {
    fn default() -> Self {
        Self {
            system_prompt: String::new(),
            max_history_pairs: 0,
            sentence_max_chars: crate::dialogue::SentenceAssembler::DEFAULT_MAX_CHARS,
            tts_queue_capacity: Self::DEFAULT_TTS_QUEUE_CAPACITY,
            audio_chunk_samples: Self::DEFAULT_AUDIO_CHUNK_SAMPLES,
            queue_push_timeout: None,
        }
    }
}

// ---------------------------------------------------------------- 事件

/// 对话引擎产出的统一事件。每个变体都携带 `epoch`（本轮轮次号），由 core 做二次闸门。
///
/// 此外每个变体都额外带 `ts_ms: u64`——**相对本轮 `run_turn` 起点的毫秒数**，
/// 由引擎在内部用 `std::time::Instant` 基准单调产生；纯计时、不依赖系统时钟，
/// 仅作可观测性/可调试性用途，**不**影响事件顺序与业务判定。
/// 节点 D 开发者模式（链路每步耗时可视化）专用字段。
#[derive(Debug)]
pub enum EngineEvent {
    /// LLM 正文增量（原样转发，未做切句假设）。
    TextDelta {
        /// 本轮轮次号。
        epoch: u64,
        /// 相对本轮起点的毫秒数（单调递增；用于链路耗时可视化）。
        ts_ms: u64,
        /// 文本增量片段。
        text: String,
    },
    /// 一段增量解码后的音频样本（interleaved f32 ∈ [-1, 1]）。
    ///
    /// 同一句内按顺序发出；每句**恰好一个** `final_chunk = true` 的事件
    /// （该句最后一个块，样本数可能为 0），以及**恰好一个**
    /// `first_chunk = true` 的事件（该句第一个块）。
    AudioChunk {
        /// 本轮轮次号。
        epoch: u64,
        /// 相对本轮起点的毫秒数。
        ts_ms: u64,
        /// 句子序号（本轮内从 1 开始递增，与播放顺序一致）。
        sentence_seq: u64,
        /// 归一化样本（长度 ≤ 配置的单块上限）。
        samples: Vec<f32>,
        /// PCM 流规格（采样率/声道数，来自 `TtsConfig::spec`）。
        spec: AudioSpec,
        /// 是否为本句**第一块**（2026-09-11 新增）。
        ///
        /// 与 `final_chunk` 配对，共同给出**句子级的音频边界**：一句音频会被切成
        /// 十几块，而播放端必须按**句**而不是按**块**对齐（口型包络、播放时间线，
        /// 以及「一句一单元」的封装）。
        ///
        /// **为什么必须由引擎给出**：桌面端曾经用 epoch 记账推 `start`
        /// （`audio_epoch_seen`，整轮只可能出现一次），实测 WS 上出现
        /// 「0 个 start、13 个 end」——前端按 `start`→`end` 攒句会攒错、
        /// 播出来断断续续。句子的首/末块只有引擎知道，所以它是引擎的契约，
        /// 不该让消费端去推断。
        first_chunk: bool,
        /// 是否为本句最后一块。
        final_chunk: bool,
    },
    /// 一句的语音**已完整合成**（紧随该句的 `final_chunk = true` 之后发出）。
    ///
    /// **为什么需要这个事件（2026-09-10 用户裁决）**：文字此前在 LLM 流式生成时
    /// 就实时上屏，而语音要等合成本句才响 —— 文字跑在声音前面好几秒。
    /// 现在是「**一句一单元：先完整合成，再显示**」：UI 侧的文字上屏以本事件为准，
    /// 而不是以 LLM 增量 [`Self::TextDelta`] 为准。
    ///
    /// - `text`：该句**完整**文本（与 `TtsJob.text` 同源，逐字无损）；
    /// - 顺序保证：同一句先发完 `AudioChunk`（含 final），再发本事件 ——
    ///   消费端「先拿到声音，再上屏文字」；
    /// - **TTS 未配置时同样发出**（空句路径也走这里），因此纯文字模式不受影响；
    /// - 合成失败时**不发**本事件（该句文字随 `Error` 一并归属失败，不假装成功）。
    SentenceVoiced {
        /// 本轮轮次号。
        epoch: u64,
        /// 相对本轮起点的毫秒数。
        ts_ms: u64,
        /// 句子序号（与对应 `AudioChunk.sentence_seq` 一致）。
        sentence_seq: u64,
        /// 该句完整文本。
        text: String,
    },
    /// 轮内错误。语义见 [`ErrorKind`]：可见，但未必终止本轮。
    Error {
        /// 本轮轮次号。
        epoch: u64,
        /// 相对本轮起点的毫秒数。
        ts_ms: u64,
        /// 错误分类。
        kind: ErrorKind,
    },
    /// 终态：本轮按 [`status`] 收场。每轮**恰好一次**，且是最后一个事件。
    ///
    /// 节点 A 裁决（步骤 7 / D8）：Completed / Failed / Cancelled 三态统一经本
    /// 变体表达，**不再用「致命 Error 后仍发 Done」暗示失败**。注意它只覆盖
    /// **生成侧**终态（LLM 结束 + TTS worker 排空）；声卡是否播完是 root
    /// 双闩锁的另一个闩，二者齐才允许结束 turn / 提交历史。
    Terminal {
        /// 本轮轮次号。
        epoch: u64,
        /// 相对本轮起点的毫秒数。
        ts_ms: u64,
        /// 结束状态。
        status: TurnStatus,
    },
}

/// 错误分类：LLM / TTS / 解码 / 背压。
///
/// - [`ErrorKind::Llm`] 不阻断**已生成**内容：已完整切句并入队的句子仍会被
///   TTS worker 排空播放；
/// - [`ErrorKind::Tts`] / [`ErrorKind::Decode`] / [`ErrorKind::Backpressure`]
///   是生成侧致命错误：立即停止生成，未合成的排队句子被丢弃，发完 `Error`
///   后以 `Terminal { Failed }` 收场。
///
/// 2026-09-11 用户裁决后，`IncompleteTools`（残缺工具调用）变体已随工具一起
/// 删除——不再存在「工具调用」这条可恢复错误路径。
#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    /// LLM 阶段的传输/协议/状态码错误。**不取消 TTS worker**：
    /// 已入队句子照常合成播放，未封口残余不再送 TTS（D8 裁决）。
    #[error("LLM 阶段错误: {0}")]
    Llm(#[source] Error),
    /// TTS 请求/传输错误（致命：丢弃剩余排队句子并结束本轮）。
    #[error("TTS 阶段错误: {0}")]
    Tts(#[source] Error),
    /// PCM 增量解码错误（如流总长为奇数字节；致命）。
    #[error("PCM 解码错误: {0}")]
    Decode(#[source] Error),
    /// 向有界 TTS 队列推送超时（下游长期不消费且未取消；致命）。
    #[error("背压超限: {message}")]
    Backpressure {
        /// 具体原因描述。
        message: String,
    },
}

/// 一轮结束的状态汇总。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStatus {
    /// LLM 正常结束且 TTS 排空（途中可能有已通过事件上报的可恢复错误）。
    Completed,
    /// 因致命错误或 LLM 阶段失败结束（细节经先行 `Error` 事件上报；
    /// 终态即 `Terminal { Failed }`——不再用 Done 隐示成败）。
    Failed,
    /// 收到取消（终态 `Terminal { Cancelled }`）。历史不提交。
    Cancelled,
}

/// `run_turn` 的返回值：轻量汇总；全部细节都经事件通道传递。
#[derive(Debug, Clone)]
pub struct TurnReport {
    /// 结束状态。
    pub status: TurnStatus,
    /// 本轮 LLM 生成的完整正文（含未成句残余；取消时为截断处的前缀）。
    pub assistant_text: String,
}

// ---------------------------------------------------------------- 单测

#[cfg(test)]
mod tests {
    use crate::config::{LlmConfig, TtsConfig};

    use super::*;
    use crate::OpenAiClient;

    // ------------------------------------------------------------ 配置/历史

    #[test]
    fn default_config_matches_documented_values() {
        let config = ConversationConfig::default();
        assert_eq!(config.system_prompt, "");
        assert_eq!(config.max_history_pairs, 0);
        // 2026-09-10：48 → 200（「一句一单元」契约，见
        // `SentenceAssembler::DEFAULT_MAX_CHARS` 的说明）。
        // 本测试锁的是「默认值 == 文档值」，所以跟着文档一起改，
        // 而不是放宽断言。
        assert_eq!(
            config.sentence_max_chars,
            crate::dialogue::SentenceAssembler::DEFAULT_MAX_CHARS
        );
        assert_eq!(config.sentence_max_chars, 200);
        assert_eq!(
            config.tts_queue_capacity,
            ConversationConfig::DEFAULT_TTS_QUEUE_CAPACITY
        );
        assert_eq!(
            config.audio_chunk_samples,
            ConversationConfig::DEFAULT_AUDIO_CHUNK_SAMPLES
        );
        assert_eq!(config.queue_push_timeout, None);
        // ConversationConfig::new 只覆盖 system_prompt。
        assert_eq!(ConversationConfig::new("你好").system_prompt, "你好");
        assert_eq!(
            ConversationConfig::new("你好").max_history_pairs,
            config.max_history_pairs
        );
    }

    #[test]
    fn build_messages_includes_system_history_and_current_input() {
        let client = OpenAiClient::new(
            LlmConfig::new("http://a/v1", "m"),
            TtsConfig::new("http://b/v1", "v"),
        )
        .expect("client");
        let mut engine = ConversationEngine::new(
            client,
            ConversationConfig {
                system_prompt: "人设".into(),
                max_history_pairs: 8,
                ..ConversationConfig::default()
            },
        );
        engine.commit_completed_turn("早", "早呀");
        let messages = engine.build_messages("吃饭了吗");
        assert_eq!(
            messages,
            vec![
                crate::llm::ChatMessage::system("人设"),
                crate::llm::ChatMessage::user("早"),
                crate::llm::ChatMessage::assistant("早呀"),
                crate::llm::ChatMessage::user("吃饭了吗"),
            ]
        );
        assert_eq!(engine.history_len(), 1);
    }

    #[test]
    fn empty_system_prompt_is_omitted_from_wire_messages() {
        let client = OpenAiClient::new(
            LlmConfig::new("http://a/v1", "m"),
            TtsConfig::new("http://b/v1", "v"),
        )
        .expect("client");
        let engine = ConversationEngine::new(client, ConversationConfig::new(""));
        let messages = engine.build_messages("hi");
        assert_eq!(messages, vec![crate::llm::ChatMessage::user("hi")]);
    }

    #[test]
    fn zero_max_history_pairs_never_stores_and_trim_keeps_latest() {
        let client = OpenAiClient::new(
            LlmConfig::new("http://a/v1", "m"),
            TtsConfig::new("http://b/v1", "v"),
        )
        .expect("client");
        let mut engine = ConversationEngine::new(client, ConversationConfig::default());
        engine.commit_completed_turn("a", "b");
        assert_eq!(engine.history_len(), 0);

        let client = OpenAiClient::new(
            LlmConfig::new("http://a/v1", "m"),
            TtsConfig::new("http://b/v1", "v"),
        )
        .expect("client");
        let mut engine = ConversationEngine::new(
            client,
            ConversationConfig {
                max_history_pairs: 2,
                ..ConversationConfig::default()
            },
        );
        for round in 0..4 {
            engine.commit_completed_turn(&format!("u{round}"), &format!("a{round}"));
        }
        assert_eq!(engine.history_len(), 2);
        let latest: Vec<_> = engine.history.iter().map(|(u, _)| u.clone()).collect();
        assert_eq!(latest, ["u2", "u3"]);
        engine.clear_history();
        assert_eq!(engine.history_len(), 0);
    }

    // ------------------------------------------------------------ 音频切块

    #[test]
    fn leading_chunk_take_respects_limit_and_remainder() {
        let mut buffer: Vec<f32> = (0..7i16).map(f32::from).collect();
        assert_eq!(
            super::worker::take_leading_chunk(&mut buffer, 3),
            [0.0, 1.0, 2.0]
        );
        assert_eq!(
            super::worker::take_leading_chunk(&mut buffer, 3),
            [3.0, 4.0, 5.0]
        );
        // 余量不足 limit：全取且清空。
        assert_eq!(super::worker::take_leading_chunk(&mut buffer, 3), [6.0]);
        assert!(buffer.is_empty());
        assert!(super::worker::take_leading_chunk(&mut buffer, 3).is_empty());
    }

    #[test]
    fn engine_is_cloneable_and_exposes_read_only_views() {
        let client = OpenAiClient::new(
            LlmConfig::new("http://a/v1", "m"),
            TtsConfig::new("http://b/v1", "v"),
        )
        .expect("client");
        let engine = ConversationEngine::new(client.clone(), ConversationConfig::new("s"));
        assert_eq!(engine.config().system_prompt, "s");
        let cloned = engine.clone();
        assert_eq!(cloned.config(), engine.config());
    }
}
