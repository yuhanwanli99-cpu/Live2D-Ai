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
//!      └─ ToolReady ─────► ToolAction   │  严格按 sentence_seq 串行
//! ```
//!
//! LLM 边流边切句，第一句进队后 TTS worker **立刻**开始合成——不等 LLM 结束；
//! 队列满时推送阻塞（背压回传到 LLM 消费速率），内存因此有界。
//!
//! # 事件契约
//!
//! - 每个[`EngineEvent`]都携带构造 `run_turn` 时传入的 `epoch`，core 据此做
//!   二次闸门（丢弃过期轮次的迟到事件）；
//! - 每次成功运行的收尾**恰好一个终态序列**：
//!   正常 → `Done`；可恢复错误（如残缺工具调用）→ 先 `Error` 再 `Done`；
//!   致命错误 → `Error` 后停止生成并 `Done`；取消 → 只发 `Cancelled`。
//!   `Done` 保证出现在「LLM 已结束 **且** TTS worker 完全排空」之后；
//! - 每句音频恰好一个 `final_chunk = true` 的 [`EngineEvent::AudioChunk`]，
//!   `sentence_seq` 从 1 开始严格递增，与播放顺序一致。
//!
//! # 取消与并发
//!
//! - 每个 await 点（网络读取、事件发送、队列推送、TTS 读取）都与
//!   `CancellationToken` `select!`；取消后引擎不再产生新业务事件，尽力 drop
//!   底层响应，最后只补发一次 `Cancelled`；
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
//!         EngineEvent::TextDelta { text, .. } => print!("{text}"),
//!         EngineEvent::AudioChunk { samples, .. } => { /* 送声卡 */ }
//!         EngineEvent::ToolAction { .. } => { /* 转交 live2d-ai-core 仲裁 */ }
//!         EngineEvent::Done { .. } | EngineEvent::Cancelled { .. } => break,
//!         EngineEvent::Error { kind, .. } => eprintln!("turn error: {kind}"),
//!     }
//! }
//! # let _ = report;
//! # }
//! ```

use std::collections::VecDeque;
use std::time::Duration;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::OpenAiClient;
use crate::audio::{AudioSpec, PcmS16LeDecoder};
use crate::dialogue::{DialogueAssembler, DialogueEvent, IncompleteToolCalls};
use crate::error::Error;
use crate::llm::{ChatMessage, LlmEvent};

/// 对话引擎配置（最小字段集；历史记忆可先关闭）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationConfig {
    /// 每轮请求固定注入的 system 提示；空串表示不发 system 消息。
    pub system_prompt: String,
    /// 保留的历史轮数上限（一轮 = 一条 user + 一条 assistant）。
    /// `0` = 不保留历史（每轮独立，本批默认）。
    pub max_history_pairs: usize,
    /// 切句缓冲的字符上限（透传 [`DialogueAssembler`]）。
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

/// 对话引擎产出的统一事件。每个变体都携带 `epoch`（本轮轮次号），由 core 做二次闸门。
#[derive(Debug)]
pub enum EngineEvent {
    /// LLM 正文增量（原样转发，未做切句假设）。
    TextDelta {
        /// 本轮轮次号。
        epoch: u64,
        /// 文本增量片段。
        text: String,
    },
    /// 一个完整且校验通过的工具调用（名称必为 `live2d_perform_action`）。
    /// 是否执行、强度是否越界由 core 仲裁。
    ToolAction {
        /// 本轮轮次号。
        epoch: u64,
        /// 该调用在本次响应内的流式下标。
        index: usize,
        /// 服务端调用 id（wire 未携带时为 `None`）。
        id: Option<String>,
        /// 解析后的动作参数。
        args: crate::tool::Live2dPerformActionArgs,
    },
    /// 一段增量解码后的音频样本（interleaved f32 ∈ [-1, 1]）。
    ///
    /// 同一句内按顺序发出；每句**恰好一个** `final_chunk = true` 的事件
    /// （该句最后一个块，样本数可能为 0）。
    AudioChunk {
        /// 本轮轮次号。
        epoch: u64,
        /// 句子序号（本轮内从 1 开始递增，与播放顺序一致）。
        sentence_seq: u64,
        /// 归一化样本（长度 ≤ 配置的单块上限）。
        samples: Vec<f32>,
        /// PCM 流规格（采样率/声道数，来自 `TtsConfig::spec`）。
        spec: AudioSpec,
        /// 是否为本句最后一块。
        final_chunk: bool,
    },
    /// 轮内错误。语义见 [`ErrorKind`]：可见，但未必终止本轮。
    Error {
        /// 本轮轮次号。
        epoch: u64,
        /// 错误分类。
        kind: ErrorKind,
    },
    /// 终态：LLM 已结束且 TTS worker 完全排空。每轮至多一次，且是最后一个事件。
    Done {
        /// 本轮轮次号。
        epoch: u64,
    },
    /// 终态：收到取消。每轮至多一次，且是最后一个事件（此后不再有任何事件）。
    Cancelled {
        /// 本轮轮次号。
        epoch: u64,
    },
}

/// 错误分类：LLM / 工具残缺 / TTS / 解码 / 背压。
///
/// - [`ErrorKind::Llm`] 与 [`ErrorKind::IncompleteTools`] 不阻断**已生成**内容：
///   已入队的句子仍会被 TTS 排空，随后照常 `Done`；
/// - [`ErrorKind::Tts`] / [`ErrorKind::Decode`] / [`ErrorKind::Backpressure`]
///   是致命错误：立即停止生成，未合成的排队句子被丢弃，发完 `Error` 后 `Done`。
#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    /// LLM 阶段的传输/协议/状态码错误（致命：停止本轮）。
    #[error("LLM 阶段错误: {0}")]
    Llm(#[source] Error),
    /// 流结束时仍有残缺工具调用（类型化详情）。**可见但不阻断**已生成文本/TTS。
    #[error("工具调用未完成: {0}")]
    IncompleteTools(IncompleteToolCalls),
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
    /// 因致命错误提前结束（对应 `Error` 事件已先行发出，之后发了 `Done`）。
    Failed,
    /// 收到取消（只发了 `Cancelled`）。历史不提交。
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

/// 内部：送进 TTS worker 的任务。
#[derive(Debug)]
struct TtsJob {
    sentence_seq: u64,
    text: String,
}

/// 最小异步对话引擎：持有客户端、配置与历史记忆。
///
/// 构造成本低；`run_turn(&mut self, …)` 在单任务内天然互斥，跨任务共享由
/// 调用方加锁（单 turn 并发防护归 root core，见模块文档）。
#[derive(Debug, Clone)]
pub struct ConversationEngine {
    client: OpenAiClient,
    config: ConversationConfig,
    history: VecDeque<(String, String)>,
}

impl ConversationEngine {
    /// 创建引擎。
    pub fn new(client: OpenAiClient, config: ConversationConfig) -> Self {
        Self {
            client,
            config,
            history: VecDeque::new(),
        }
    }

    /// 配置只读视图。
    pub const fn config(&self) -> &ConversationConfig {
        &self.config
    }

    /// 当前保留的历史轮数。
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// 清空历史。
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 组装本轮消息：[system?] + 历史 + 当前输入。
    fn build_messages(&self, user_text: &str) -> Vec<ChatMessage> {
        let mut out = Vec::with_capacity(2 * self.history.len() + 2);
        if !self.config.system_prompt.is_empty() {
            out.push(ChatMessage::system(self.config.system_prompt.clone()));
        }
        for (user, assistant) in &self.history {
            out.push(ChatMessage::user(user.clone()));
            out.push(ChatMessage::assistant(assistant.clone()));
        }
        out.push(ChatMessage::user(user_text));
        out
    }

    /// 成功结束后提交历史（受 `max_history_pairs` 约束；`0` 表示永不保留）。
    fn commit_history(&mut self, user_text: &str, assistant_text: &str) {
        if self.config.max_history_pairs == 0 {
            return;
        }
        self.history
            .push_back((user_text.to_string(), assistant_text.to_string()));
        while self.history.len() > self.config.max_history_pairs {
            self.history.pop_front();
        }
    }

    /// 运行一轮对话：LLM 流式请求 → 实时 `TextDelta` → 切句入队 → TTS 串行
    /// 合成 → `AudioChunk`。全程可取消；终态语义见模块文档与 [`EngineEvent`]。
    ///
    /// `event_tx` 由调用方创建（建议容量 ≥ `tts_queue_capacity + 8`，避免消费端
    /// 成为额外瓶颈）；消费停止（receiver 被 drop）视同取消。
    pub async fn run_turn(
        &mut self,
        epoch: u64,
        user_text: &str,
        event_tx: mpsc::Sender<EngineEvent>,
        cancel: CancellationToken,
    ) -> TurnReport {
        // 引擎内部使用父令牌的 child：致命错误时可以只中止自己的 TTS worker，
        // 而不触碰调用方的令牌语义（父取消会自动传播到 child）。
        let worker_cancel = cancel.child_token();

        let queue_capacity = self.config.tts_queue_capacity.max(1);
        let chunk_limit = self.config.audio_chunk_samples.max(1);

        let (job_tx, job_rx) = mpsc::channel::<TtsJob>(queue_capacity);
        let worker = tokio::spawn(tts_worker(
            TtsWorkerCtx {
                client: self.client.clone(),
                epoch,
                event_tx: event_tx.clone(),
                cancel: worker_cancel.clone(),
                chunk_samples: chunk_limit,
            },
            job_rx,
        ));

        let messages = self.build_messages(user_text);
        let mut assembler = DialogueAssembler::new(self.config.sentence_max_chars);
        let mut assistant_text = String::new();
        let mut sentence_seq: u64 = 0;

        let mut cancelled = false;
        let mut tx_closed = false;
        let mut llm_finished = false;
        let mut fatal = false;
        let mut queue_dead = false;

        // ---- 阶段 0：发起 LLM 请求（可取消/可失败）。
        let stream = tokio::select! {
            _ = cancel.cancelled() => {
                cancelled = true;
                None
            }
            established = self.client.chat_stream(&messages) => match established {
                Ok(stream) => Some(stream),
                Err(e) => {
                    let _ = send_event(
                        &event_tx,
                        &cancel,
                        EngineEvent::Error { epoch, kind: ErrorKind::Llm(e) },
                    )
                    .await;
                    fatal = true;
                    None
                }
            },
        };

        // ---- 阶段 1：LLM 流式循环（边流边发 TextDelta / ToolAction / 入队句子）。
        if let Some(mut stream) = stream {
            'llm: loop {
                let item = tokio::select! {
                    _ = cancel.cancelled() => {
                        cancelled = true;
                        break 'llm;
                    }
                    next = stream.next() => match next {
                        Some(item) => item,
                        None => {
                            // try_unfold 在 Done 之后恰好返回 None：流正常耗尽。
                            llm_finished = true;
                            break 'llm;
                        }
                    },
                };

                let event = match item {
                    Ok(event) => event,
                    Err(e) => {
                        // 流中途出错：已生成的内容仍然尽力交付（残余句子封口入队）。
                        let _ = send_event(
                            &event_tx,
                            &cancel,
                            EngineEvent::Error {
                                epoch,
                                kind: ErrorKind::Llm(e),
                            },
                        )
                        .await;
                        fatal = true;
                        break 'llm;
                    }
                };

                if let LlmEvent::TextDelta(text) = &event {
                    assistant_text.push_str(text);
                    let delivered = send_event(
                        &event_tx,
                        &cancel,
                        EngineEvent::TextDelta {
                            epoch,
                            text: text.clone(),
                        },
                    )
                    .await;
                    if !delivered {
                        tx_closed = true;
                        break 'llm;
                    }
                }

                for dialogue in assembler.push(&event) {
                    match dialogue {
                        DialogueEvent::SentenceReady { text } => {
                            sentence_seq += 1;
                            match push_job(
                                &job_tx,
                                &cancel,
                                self.config.queue_push_timeout,
                                TtsJob { sentence_seq, text },
                            )
                            .await
                            {
                                PushOutcome::Accepted => {}
                                // worker 已因 TTS/解码错误退出（它自己已发过 Error 事件）。
                                PushOutcome::QueueDead => {
                                    queue_dead = true;
                                    break 'llm;
                                }
                                // 消费者消失/取消：安静收场。
                                PushOutcome::Stopped => {
                                    tx_closed = true;
                                    break 'llm;
                                }
                                PushOutcome::TimedOut(message) => {
                                    let _ = send_event(
                                        &event_tx,
                                        &cancel,
                                        EngineEvent::Error {
                                            epoch,
                                            kind: ErrorKind::Backpressure { message },
                                        },
                                    )
                                    .await;
                                    fatal = true;
                                    break 'llm;
                                }
                            }
                        }
                        DialogueEvent::ToolReady { call } => {
                            let delivered = send_event(
                                &event_tx,
                                &cancel,
                                EngineEvent::ToolAction {
                                    epoch,
                                    index: call.index,
                                    id: call.id,
                                    args: call.arguments,
                                },
                            )
                            .await;
                            if !delivered {
                                tx_closed = true;
                                break 'llm;
                            }
                        }
                        // 残缺工具调用：立即可见，但不阻断已生成文本/TTS。
                        // （真正的流结束以 `None` 分支为准。）
                        DialogueEvent::Done { incomplete_tools } => {
                            if let Some(incomplete) = incomplete_tools {
                                let delivered = send_event(
                                    &event_tx,
                                    &cancel,
                                    EngineEvent::Error {
                                        epoch,
                                        kind: ErrorKind::IncompleteTools(incomplete),
                                    },
                                )
                                .await;
                                if !delivered {
                                    tx_closed = true;
                                    break 'llm;
                                }
                            }
                        }
                    }
                }
            }

            if fatal && !queue_dead {
                // LLM 中途失败但 TTS 存活：把切句器里悬挂的残余封口入队，
                // 让「已生成的文本」仍能被念出来（best effort）。
                for text in assembler.flush().into_iter().filter_map(|d| match d {
                    DialogueEvent::SentenceReady { text } => Some(text),
                    _ => None,
                }) {
                    sentence_seq += 1;
                    match push_job(
                        &job_tx,
                        &cancel,
                        self.config.queue_push_timeout,
                        TtsJob { sentence_seq, text },
                    )
                    .await
                    {
                        PushOutcome::Accepted | PushOutcome::TimedOut(_) => {}
                        // worker 已死（其 Error 事件已自行发出）。
                        PushOutcome::QueueDead => {
                            queue_dead = true;
                            break;
                        }
                        // 消费者消失/取消：停止入队，终态判定会兜住。
                        PushOutcome::Stopped => {
                            tx_closed = true;
                            break;
                        }
                    }
                }
            }
        }

        // ---- 阶段 2：关队列 → 等 worker 排空（无条件 join，杜绝泄漏）。
        if fatal || queue_dead {
            // 致命错误：让 worker 立刻放弃卡住的网络读取/发送，尽快退出。
            worker_cancel.cancel();
        }
        drop(job_tx); // 关闭队列：worker 排空剩余句子后自然退出。
        // worker 的退出原因参与终态判定：若它在排空途中因 TTS/解码错误退出
        // （Error 事件已由 worker 自行发出），本轮必须以 Failed 收尾；
        // worker panic 同样按致命错误处理。
        let worker_exit = worker.await.unwrap_or(WorkerExit::Failed);

        // ---- 阶段 3：恰一次终态事件。
        // 终局前再读一次令牌：堵住「取消与最后一个业务事件同时到达」的竞态窗口——
        // 一旦已取消，就不再补发 Done，只发 Cancelled。
        let cancelled = cancelled || tx_closed || cancel.is_cancelled();
        let status = if cancelled {
            let _ = event_tx.send(EngineEvent::Cancelled { epoch }).await;
            TurnStatus::Cancelled
        } else if fatal || queue_dead || worker_exit == WorkerExit::Failed {
            let _ = event_tx.send(EngineEvent::Done { epoch }).await;
            TurnStatus::Failed
        } else {
            debug_assert!(llm_finished, "非取消/非致命路径必须来自正常流耗尽");
            let _ = event_tx.send(EngineEvent::Done { epoch }).await;
            self.commit_history(user_text, &assistant_text);
            TurnStatus::Completed
        };

        TurnReport {
            status,
            assistant_text,
        }
    }
}

// ---------------------------------------------------------------- 事件投递

/// 向外发一个事件：与取消竞争；接收端已关闭或已取消 → `false`（调用方据此停机）。
///
/// 注意：终态事件（`Done`/`Cancelled`）不走这里——它们必须尝试送达，
/// 失败也只是说明消费者已经离开，静默忽略即可。
async fn send_event(
    event_tx: &mpsc::Sender<EngineEvent>,
    cancel: &CancellationToken,
    event: EngineEvent,
) -> bool {
    tokio::select! {
        _ = cancel.cancelled() => false,
        sent = event_tx.send(event) => sent.is_ok(),
    }
}

// ---------------------------------------------------------------- 句子入队

/// 句子入队结果。
enum PushOutcome {
    /// 已进入有界队列。
    Accepted,
    /// 队列已关闭：TTS worker 已异常退出（其 Error 事件已自行发出）。
    QueueDead,
    /// 取消或事件消费者消失：安静收场。
    Stopped,
    /// 背压超限：队列长期满且未被消费。
    TimedOut(String),
}

/// 把一个句子推进有界队列：满则等待（背压），可选超时升级为
/// [`ErrorKind::Backpressure`]；取消随时打断。
async fn push_job(
    job_tx: &mpsc::Sender<TtsJob>,
    cancel: &CancellationToken,
    timeout: Option<Duration>,
    job: TtsJob,
) -> PushOutcome {
    let push = job_tx.send(job);
    tokio::select! {
        _ = cancel.cancelled() => PushOutcome::Stopped,
        sent = push => match sent {
            Ok(()) => PushOutcome::Accepted,
            Err(_) => PushOutcome::QueueDead,
        },
        // 仅在配置了超时时参与竞争：等待超过时限仍未入队 → 下游卡死。
        _ = sleep_for(timeout), if timeout.is_some() => PushOutcome::TimedOut(format!(
            "句子在 {timeout:?} 内未能进入 TTS 队列（容量 {}）：下游持续不消费",
            job_tx.max_capacity()
        )),
    }
}

/// `Some(d)` → 睡 d；`None` → 永不完成（供 `select!` 的 `if` 前置条件短路）。
async fn sleep_for(timeout: Option<Duration>) {
    match timeout {
        Some(d) => tokio::time::sleep(d).await,
        None => std::future::pending::<()>().await,
    }
}

// ---------------------------------------------------------------- TTS worker

/// TTS worker 的不可变上下文。
struct TtsWorkerCtx {
    client: OpenAiClient,
    epoch: u64,
    event_tx: mpsc::Sender<EngineEvent>,
    cancel: CancellationToken,
    chunk_samples: usize,
}

/// TTS worker 的退出原因（经 `JoinHandle` 回传给主任务，用于终态判定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerExit {
    /// 队列关闭且完全排空：正常下岗。
    Drained,
    /// 取消或事件通道关闭：安静退出。
    Stopped,
    /// 致命错误退出（对应的 `Error` 事件已由 worker 自行发出）。
    Failed,
}

/// 单个 TTS worker：严格按 `sentence_seq` 到达序串行合成整条队列。
///
/// 退出条件（任一）：队列关闭且排空（正常）/ 取消 / 首个致命错误 /
/// 事件通道关闭。除正常退出外都会先尽力发出对应的 `Error` 事件。
async fn tts_worker(ctx: TtsWorkerCtx, mut rx: mpsc::Receiver<TtsJob>) -> WorkerExit {
    let spec = ctx.client.tts().spec;
    loop {
        let job = tokio::select! {
            _ = ctx.cancel.cancelled() => return WorkerExit::Stopped,
            job = rx.recv() => match job {
                Some(job) => job,
                None => return WorkerExit::Drained, // 队列关闭且已排空。
            },
        };
        match synthesize_sentence(&ctx, spec, &job).await {
            SynthOutcome::Done => {}
            SynthOutcome::Cancelled => return WorkerExit::Stopped,
            SynthOutcome::Failed(kind) => {
                let _ = send_event(
                    &ctx.event_tx,
                    &ctx.cancel,
                    EngineEvent::Error {
                        epoch: ctx.epoch,
                        kind,
                    },
                )
                .await;
                return WorkerExit::Failed;
            }
        }
    }
}

/// 单句合成结果。
enum SynthOutcome {
    /// 整句字节流消费完毕并发出了 `final_chunk`。
    Done,
    /// 取消或事件通道关闭：安静退出，不再发声。
    Cancelled,
    /// 致命错误（TTS/解码）。
    Failed(ErrorKind),
}

/// 合成一句话：请求 `/audio/speech` → 增量解码 → 按上限切块发送。
async fn synthesize_sentence(ctx: &TtsWorkerCtx, spec: AudioSpec, job: &TtsJob) -> SynthOutcome {
    // 格式门禁：本引擎只承诺 pcm（默认配置恒满足；显式改配置则明确报错）。
    if let Err(e) =
        crate::audio::ensure_supported_format(ctx.client.tts().response_format.as_deref())
    {
        return SynthOutcome::Failed(ErrorKind::Decode(e));
    }

    let mut speech = tokio::select! {
        _ = ctx.cancel.cancelled() => return SynthOutcome::Cancelled,
        opened = ctx.client.synthesize_speech(&job.text) => match opened {
            Ok(speech) => speech,
            Err(e) => return SynthOutcome::Failed(ErrorKind::Tts(e)),
        },
    };

    // 增量解码：任意字节切割安全（半个样本留到下批）；块大小受限避免无限内存。
    let mut decoder = PcmS16LeDecoder::new(spec);
    let mut pending: Vec<f32> = Vec::new();
    loop {
        let chunk = tokio::select! {
            _ = ctx.cancel.cancelled() => return SynthOutcome::Cancelled,
            next = speech.next() => match next {
                Some(chunk) => chunk,
                None => break,
            },
        };
        // 显式标注 Vec<u8>：decode 的入参是 &[u8]，不标注会把推断方向带偏
        // （编译器会试图把 bytes 统一成未定长 [u8]）。
        let bytes: Vec<u8> = match chunk {
            Ok(bytes) => bytes,
            Err(e) => return SynthOutcome::Failed(ErrorKind::Tts(e)),
        };
        pending.extend(decoder.decode(&bytes));
        while pending.len() >= ctx.chunk_samples {
            let block = take_leading_chunk(&mut pending, ctx.chunk_samples);
            let delivered = send_event(
                &ctx.event_tx,
                &ctx.cancel,
                EngineEvent::AudioChunk {
                    epoch: ctx.epoch,
                    sentence_seq: job.sentence_seq,
                    samples: block,
                    spec,
                    final_chunk: false,
                },
            )
            .await;
            if !delivered {
                return SynthOutcome::Cancelled;
            }
        }
    }

    // 流结束：总字节数必须按样本对齐（奇数字节尾巴是明确的 Decode 错误）。
    if let Err(e) = decoder.finish() {
        return SynthOutcome::Failed(ErrorKind::Decode(e));
    }
    // 每句恰好一个 final_chunk（即使该句没有任何样本）。
    let tail = std::mem::take(&mut pending);
    let delivered = send_event(
        &ctx.event_tx,
        &ctx.cancel,
        EngineEvent::AudioChunk {
            epoch: ctx.epoch,
            sentence_seq: job.sentence_seq,
            samples: tail,
            spec,
            final_chunk: true,
        },
    )
    .await;
    if !delivered {
        return SynthOutcome::Cancelled;
    }
    SynthOutcome::Done
}

/// 取走缓冲区前 `limit` 个样本（不足则全取）。独立成函数便于单测切块逻辑。
fn take_leading_chunk(buffer: &mut Vec<f32>, limit: usize) -> Vec<f32> {
    buffer.drain(..limit.min(buffer.len())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmConfig, TtsConfig};
    use crate::tool::Live2dPerformActionArgs;

    // ------------------------------------------------------------ 配置/历史

    #[test]
    fn default_config_matches_documented_values() {
        let config = ConversationConfig::default();
        assert_eq!(config.system_prompt, "");
        assert_eq!(config.max_history_pairs, 0);
        assert_eq!(config.sentence_max_chars, 48);
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
        engine.commit_history("早", "早呀");
        let messages = engine.build_messages("吃饭了吗");
        assert_eq!(
            messages,
            vec![
                ChatMessage::system("人设"),
                ChatMessage::user("早"),
                ChatMessage::assistant("早呀"),
                ChatMessage::user("吃饭了吗"),
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
        assert_eq!(messages, vec![ChatMessage::user("hi")]);
    }

    #[test]
    fn zero_max_history_pairs_never_stores_and_trim_keeps_latest() {
        let client = OpenAiClient::new(
            LlmConfig::new("http://a/v1", "m"),
            TtsConfig::new("http://b/v1", "v"),
        )
        .expect("client");
        let mut engine = ConversationEngine::new(client, ConversationConfig::default());
        engine.commit_history("a", "b");
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
            engine.commit_history(&format!("u{round}"), &format!("a{round}"));
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
        assert_eq!(take_leading_chunk(&mut buffer, 3), [0.0, 1.0, 2.0]);
        assert_eq!(take_leading_chunk(&mut buffer, 3), [3.0, 4.0, 5.0]);
        // 余量不足 limit：全取且清空。
        assert_eq!(take_leading_chunk(&mut buffer, 3), [6.0]);
        assert!(buffer.is_empty());
        assert!(take_leading_chunk(&mut buffer, 3).is_empty());
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

    // 类型触达（保证 wire 参数类型出现在引擎公共面上）。
    #[test]
    fn tool_args_type_is_reexported_through_events() {
        fn accepts(_: EngineEvent) {}
        accepts(EngineEvent::ToolAction {
            epoch: 1,
            index: 0,
            id: None,
            args: Live2dPerformActionArgs {
                action: crate::tool::PerformAction::Nod,
                strength: 1,
                reason: None,
            },
        });
    }
}
