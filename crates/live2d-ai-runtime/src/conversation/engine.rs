//! [`ConversationEngine`] 主体：发起 LLM 请求、增量切句、入队给 TTS worker，
//! 并在每轮结尾统一发出恰一次的 [`EngineEvent::Terminal`]。
//!
//! 与子模块的协作：
//! - [`super::events::send_event`] / [`super::events::send_terminal`]：事件投递；
//! - [`super::queue::push_job`]：句子入队 + 背压超时；
//! - [`super::worker::tts_worker`]：TTS 串行合成。

use std::collections::VecDeque;
use std::time::Instant;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::EngineEvent;
use super::ErrorKind;
use super::TurnReport;
use super::TurnStatus;
use super::events::{send_event, send_terminal};
use super::queue::{PushOutcome, push_job};
use super::worker::{TtsJob, TtsWorkerCtx, WorkerExit, tts_worker};
use crate::OpenAiClient;
use crate::dialogue::{DialogueAssembler, DialogueEvent};
use crate::llm::{ChatMessage, LlmEvent};

/// 最小异步对话引擎：持有客户端、配置与历史记忆。
///
/// 构造成本低；`run_turn(&mut self, …)` 在单任务内天然互斥，跨任务共享由
/// 调用方加锁（单 turn 并发防护归 root core，见模块文档）。
#[derive(Debug, Clone)]
pub struct ConversationEngine {
    client: OpenAiClient,
    config: super::ConversationConfig,
    pub(super) history: VecDeque<(String, String)>,
}

impl ConversationEngine {
    /// 创建引擎。
    pub fn new(client: OpenAiClient, config: super::ConversationConfig) -> Self {
        Self {
            client,
            config,
            history: VecDeque::new(),
        }
    }

    /// 配置只读视图。
    pub const fn config(&self) -> &super::ConversationConfig {
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
    pub(super) fn build_messages(&self, user_text: &str) -> Vec<ChatMessage> {
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
    ///
    /// **P0-4 裁决：[`Self::run_turn`] 不再自动提交历史**——合成完成时并不知道
    /// PCM 是否无损入环、声卡是否播完、是否已被 `/stop`、cpal 是否故障。
    /// 由 supervisor 在「真实播放排空 + 未取消 + 无设备故障」后显式调用
    /// 本方法；判定条件清单见节点 A 裁决 D8 表。
    ///
    /// 幂等性由调用方保证：同轮重复提交会写入重复历史对（supervisor 的
    /// turn_terminal 单次触发即天然防重）。
    pub fn commit_completed_turn(&mut self, user_text: &str, assistant_text: &str) {
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

        // 节点 D 可观测性：本轮时间戳基准。所有 EngineEvent.ts_ms 都相对
        // 此点计算；纯计时，**不**影响事件顺序与业务判定。
        let turn_started = Instant::now();
        let now_ms = || turn_started.elapsed().as_millis() as u64;

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
                turn_started,
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
        // LLM 阶段失败（连接/流中途）：按 D8 不取消 worker，已入队句子照常排空。
        let mut llm_failed = false;
        // 生成侧致命（背压超限）：停止生成并取消 worker。
        let mut enqueue_failed = false;
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
                        EngineEvent::Error {
                            epoch,
                            ts_ms: now_ms(),
                            kind: ErrorKind::Llm(e),
                        },
                    )
                    .await;
                    llm_failed = true;
                    None
                }
            },
        };

        // ---- 阶段 1：LLM 流式循环（边流边发 TextDelta / 入队句子）。
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
                        // 流中途出错（D8）：已完整切句并入队的内容允许播完；
                        // 未封口残余不再 TTS。不取消 worker。
                        let _ = send_event(
                            &event_tx,
                            &cancel,
                            EngineEvent::Error {
                                epoch,
                                ts_ms: now_ms(),
                                kind: ErrorKind::Llm(e),
                            },
                        )
                        .await;
                        llm_failed = true;
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
                            ts_ms: now_ms(),
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
                                            ts_ms: now_ms(),
                                            kind: ErrorKind::Backpressure { message },
                                        },
                                    )
                                    .await;
                                    enqueue_failed = true;
                                    break 'llm;
                                }
                            }
                        }
                        // 流结束标记（`Done`/`flush`）。真正的「流已耗尽」以
                        // 上面的 `stream.next()` 返回 `None` 为准，这里无事可做。
                        DialogueEvent::Done => {}
                    }
                }
            }
        }

        // ---- 阶段 2：关队列 → 等 worker 排空（无条件 join，杜绝泄漏）。
        // 注意（D8 / 步骤 7）：`llm_failed` **不**在此列——已完整切句并入队的
        // 句子必须由 worker 正常排空播放；未封口残余也不再封口送 TTS。
        // 只有生成侧致命（背压超限）或 worker 已死才需要立即叫停 worker。
        if enqueue_failed || queue_dead {
            worker_cancel.cancel();
        }
        drop(job_tx); // 关闭队列：worker 排空剩余句子后自然退出。
        // worker 的退出原因参与终态判定：若它在排空途中因 TTS/解码错误退出
        // （Error 事件已由 worker 自行发出），本轮必须以 Failed 收尾；
        // worker panic 同样按致命错误处理。
        let worker_exit = worker.await.unwrap_or(WorkerExit::Failed);

        // ---- 阶段 3：恰一次终态（统一经 [`EngineEvent::Terminal`] 表达）。
        // 终局前再读一次令牌：堵住「取消与最后一个业务事件同时到达」的竞态窗口——
        // 一旦已取消，终态就是 Cancelled。
        let cancelled = cancelled || tx_closed || cancel.is_cancelled();
        let status = if cancelled {
            TurnStatus::Cancelled
        } else if llm_failed || enqueue_failed || queue_dead || worker_exit == WorkerExit::Failed {
            TurnStatus::Failed
        } else {
            debug_assert!(llm_finished, "非取消/非致命路径必须来自正常流耗尽");
            TurnStatus::Completed
        };
        // 终态兜底投递：消费端卡死时最多等 [`super::TERMINAL_SEND_TIMEOUT`]，
        // 之后放弃——supervisor 的权威终态是 [`TurnReport`]，不依赖本事件送达。
        send_terminal(
            &event_tx,
            EngineEvent::Terminal {
                epoch,
                ts_ms: now_ms(),
                status,
            },
        )
        .await;

        TurnReport {
            status,
            assistant_text,
        }
    }
}
