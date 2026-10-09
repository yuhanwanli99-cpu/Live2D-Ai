//! [`ConversationEngine`] 主体：发起 LLM 请求、增量切句、入队给 TTS worker，
//! 并在每轮结尾统一发出恰一次的 [`EngineEvent::Terminal`]。
//!
//! 与子模块的协作：
//! - [`super::events::send_event`] / [`super::events::send_terminal`]：事件投递；
//! - [`super::queue::push_job`]：句子入队 + 背压超时；
//! - [`super::worker::tts_worker`]：TTS 串行合成。

use std::collections::VecDeque;
use std::sync::Arc;
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
use crate::cleaning::SentenceCleaner;
use crate::dialogue::{DialogueAssembler, DialogueEvent, SentenceAssembler};
use crate::llm::{ChatMessage, LlmEvent};
use crate::performance::PerformanceRuntime;

/// 最小异步对话引擎：持有客户端、配置与历史记忆。
///
/// 构造成本低；`run_turn(&mut self, …)` 在单任务内天然互斥，跨任务共享由
/// 调用方加锁（单 turn 并发防护归 root core，见模块文档）。
#[derive(Debug, Clone)]
pub struct ConversationEngine {
    client: OpenAiClient,
    config: super::ConversationConfig,
    pub(super) history: VecDeque<(String, String)>,
    /// **会话级 system_prompt 覆盖**（L1 基座，2026-09-15）。
    ///
    /// `Some` 时**完全取代** `config.system_prompt`，`None` 时用配置里那一份。
    /// 为什么做成覆盖槽而不是直接改 `config.system_prompt`：
    /// - 配置里那份是**全局基线**（面板 / toml 的真相），不能被会话值污染；
    /// - 每轮开始时 supervisor 会显式 set 一次，所以「切换会话」是天然幂等的。
    ///
    /// 热重载会整体重建引擎 → 覆盖槽随之回到 `None`（下一轮再 set）。
    pub(super) system_prompt_override: Option<String>,
    /// **表演层运行时**（2026-09-22）。
    ///
    /// - `None`（缺省）= 关闸：主链走既有的「边流边切句边送 TTS」路径，行为与
    ///   本字段加入前**逐字一致**；
    /// - `Some`：本轮先收齐助手原文，调表演层拿一份 JSON；v1 的 `segments` 是
    ///   本轮 TTS / 上屏的**切分方案**（一对一，D22），`cues` 经
    ///   `EngineEvent::ActionCue` 出去。端点缺失时运行时仍存在但
    ///   `enabled()==false`——每轮走确定性回退（`clean_for_tts(原文)` + 规则 cue），
    ///   degraded 在状态面可见。
    pub(super) performance: Option<Arc<PerformanceRuntime>>,
    /// **二路按句清洗**（2026-10-08）。
    ///
    /// - `None`（缺省）= 不跑二路：上屏与送 TTS 都是 `clean_for_tts(句)`
    ///   （既有行为，逐字一致——所有既有测试与「手工装配」都走这条）；
    /// - `Some` 且 `enabled()`：每句原文先交给二路拿 `{display, speech}`，
    ///   `display` 上屏、`speech` 送 TTS（**两个不同的字符串，同一个
    ///   sentence_seq**）。二路失败 / 超时 / 坏 JSON / `display` 不是原文切片
    ///   → 这一句两份都用原文。
    ///
    /// 生产路径由 host 用 [SentenceCleaner::from_llm] 注入（`[llm]` 同一份
    /// base_url / model / key）；这里只消费它，不再自己造客户端。
    pub(super) cleaner: Option<Arc<SentenceCleaner>>,
}

impl ConversationEngine {
    /// 创建引擎。
    pub fn new(client: OpenAiClient, config: super::ConversationConfig) -> Self {
        Self {
            client,
            config,
            history: VecDeque::new(),
            system_prompt_override: None,
            performance: None,
            cleaner: None,
        }
    }

    /// 注入表演层运行时（host 装配点；`None` = 关闸）。
    pub fn set_performance(&mut self, performance: Option<Arc<PerformanceRuntime>>) {
        self.performance = performance;
    }

    /// 注入二路按句清洗（host 装配点；`None` = 不跑二路）。
    ///
    /// 与 [Self::set_performance] 相互独立：清洗只作用在**流式主链**的每句文本上，
    /// 表演层那份整段 JSON 仍按自己的路径走。
    pub fn set_cleaner(&mut self, cleaner: Option<Arc<SentenceCleaner>>) {
        self.cleaner = cleaner;
    }

    /// 表演层是否**可能**接管本轮（`Some` 即接管：含 enabled=false 的降级运行时）。
    ///
    /// supervisor 用它决定「要不要把 `SentenceReady` 转给 director Mod」——
    /// 表演层开着时规则导演不再并行投递按句 cue（避免两套 cue 打擂台）。
    pub fn performance_enabled(&self) -> bool {
        self.performance.is_some()
    }

    /// 表演层计数句柄（状态面 / 测试）。
    pub fn performance_stats(&self) -> Option<Arc<crate::performance::PerformanceStats>> {
        self.performance.as_ref().map(|p| p.stats())
    }

    /// 配置只读视图。
    pub const fn config(&self) -> &super::ConversationConfig {
        &self.config
    }

    /// 设置 / 清除**会话级** system_prompt 覆盖（L1 基座）。
    ///
    /// `None` = 回到配置里的全局 `system_prompt`。每轮对话开始前由 supervisor
    /// 按当前会话调用一次，因此「切会话」不需要重建引擎。
    ///
    /// 空串按 `None` 处理：空 system 与「没有 system」在 wire 层等价
    /// （见 `build_messages` 的非空判断），把它当覆盖会把全局人设悄悄清掉。
    pub fn set_system_prompt_override(&mut self, prompt: Option<String>) {
        self.system_prompt_override = prompt.filter(|p| !p.trim().is_empty());
    }

    /// 本轮**实际生效**的 system_prompt（覆盖优先；两者皆空 → 空串）。
    pub fn effective_system_prompt(&self) -> &str {
        self.system_prompt_override
            .as_deref()
            .unwrap_or(self.config.system_prompt.as_str())
    }

    /// 当前保留的历史轮数。
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// 清空历史。
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// 取走当前装入的历史，留下空桶。宿主按会话换桶时用。
    pub fn take_history(&mut self) -> VecDeque<(String, String)> {
        std::mem::take(&mut self.history)
    }

    /// 装入一份历史，并按当前 `max_history_pairs` 裁剪。
    ///
    /// `0` 表示永不保留：装入的内容直接丢掉，请求体里不会出现旧轮次。
    pub fn replace_history(&mut self, mut history: VecDeque<(String, String)>) {
        let cap = self.config.max_history_pairs;
        if cap == 0 {
            history.clear();
        } else {
            while history.len() > cap {
                history.pop_front();
            }
        }
        self.history = history;
    }

    /// 组装本轮消息：[system?] + 历史 + 当前输入。
    pub(super) fn build_messages(&self, user_text: &str) -> Vec<ChatMessage> {
        let mut out = Vec::with_capacity(2 * self.history.len() + 2);
        let system = self.effective_system_prompt();
        if !system.is_empty() {
            out.push(ChatMessage::system(system.to_string()));
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
        // 表演层运行时快照：`Some` = 本轮由表演层决定 speak/cues（收齐原文后再切句）。
        let performance = self.performance.clone();
        // 二路清洗快照（`None` / disabled = 不跑二路，上屏与送 TTS 同源）。
        let cleaner = self.cleaner.clone();
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

                // 思考：只转发给 UI，**不进** assembler（一旦进了就会被合成语音）。
                if let LlmEvent::ReasoningDelta(text) = &event {
                    let delivered = send_event(
                        &event_tx,
                        &cancel,
                        EngineEvent::ReasoningDelta {
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

                // **表演层开着时不切句、不入队**：先收齐本轮原文，阶段 1.5 再
                // 拿表演层的 speak 切句（主链等它一份 JSON，用户已接受这份延迟）。
                let dialogues = if performance.is_some() {
                    Vec::new()
                } else {
                    assembler.push(&event)
                };
                for dialogue in dialogues {
                    match dialogue {
                        DialogueEvent::SentenceReady { text } => {
                            // **一份原文、两份文本**（2026-10-08）：
                            // - 上屏 = `display`（二路交回的原文切片；不跑二路时 =
                            //   `clean_for_tts(句)`）；
                            // - 送 TTS = `speech`（二路交回的可念文本；失败 / 不跑二路时
                            //   与 display 同源）。
                            // 两者共用**同一个 `sentence_seq`**——气泡与音频仍按一个号
                            // 对齐。回灌 LLM 的历史（commit_completed_turn）始终用**原文**。
                            sentence_seq += 1;
                            let Some((display, speech)) =
                                sentence_texts(cleaner.as_ref(), &text, &cancel).await
                            else {
                                // 二路清洗期间被取消：安静收场（Cancelled）。
                                cancelled = true;
                                break 'llm;
                            };
                            // `speech` 为空 = 整句都是动作描写 / emoji：没有可说的内容。
                            // **空串不得发给 TTS**——那会被上游回 400 input 为空，而 TTS
                            // 错误是 fatal，会把整轮判失败。它照常走既有的**静音句**路径
                            //（worker 见 trim 为空即不发 HTTP，见 worker.rs；
                            // 回归 whitespace_only_sentence_...），因此空串永远到不了上游。
                            //
                            // P1-2（2026-09-16）：**送 TTS 之前**的锚点——异步导演按
                            // sentence_seq 对齐；它只能读这份文本，不能改。
                            let delivered = send_event(
                                &event_tx,
                                &cancel,
                                EngineEvent::SentenceReady {
                                    epoch,
                                    ts_ms: now_ms(),
                                    sentence_seq,
                                    text: display.clone(),
                                },
                            )
                            .await;
                            if !delivered {
                                tx_closed = true;
                                break 'llm;
                            }
                            match push_job(
                                &job_tx,
                                &cancel,
                                self.config.queue_push_timeout,
                                TtsJob {
                                    sentence_seq,
                                    text: speech,
                                    display,
                                },
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

        // ---- 阶段 1.5：表演层（v1，2026-09-26）----
        // 主链在这里**收齐本轮助手原文**，交给表演层拿一份合法化 JSON：
        //   segments → 逐段送 TTS + 上屏（D22：一对一，不再二次切句；D23：上屏==送 TTS）；
        //   cues     → EngineEvent::ActionCue（supervisor 投影为 WS action_cue）。
        // v0 的 speak 路径（segments 缺席 / 回退）仍走 SentenceAssembler。
        // 只有「表演层开着 + 正常流完 + 未取消 + 无致命」才走这里；失败轮的正文
        // 兜底仍由阶段 4 的 TextFallback 负责（那一轮不发 TTS）。
        if let Some(perf) = performance.as_ref()
            && !cancelled
            && !tx_closed
            && !llm_failed
            && !enqueue_failed
            && !queue_dead
        {
            let resolution = tokio::select! {
                _ = cancel.cancelled() => {
                    cancelled = true;
                    None
                }
                r = perf.resolve(user_text, &assistant_text) => Some(r),
            };
            if let Some(resolution) = resolution {
                // **D22（段 ↔ 句 1:1）**：表演层给的 segments **一对一**成为 TTS 单元
                // 与音频元素（`seg:N` ≡ `sentence_seq == N`）——**不再过分句器二次切分**。
                // 回退 / v0 旧路径（`segments == None`）仍走既有 SentenceAssembler，
                // 对 `clean_for_tts(原文)` 切多段（v0 行为不变）。
                let mut sentences: Vec<String> = match &resolution.segments {
                    Some(segments) => segments.clone(),
                    None => {
                        let mut out = Vec::new();
                        if let Some(speak) = resolution.speak.as_deref() {
                            let mut sentences_assembler =
                                SentenceAssembler::new(self.config.sentence_max_chars);
                            out.extend(sentences_assembler.push(speak));
                            out.extend(sentences_assembler.flush());
                        }
                        out
                    }
                };
                // **只动**（segments=[] + 有 cue）：给一条**无声锚句**。前端的 cue 锚点
                // 是「该句音频开始」（first_chunk），没有句子就没有锚点，动作永不发生。
                // 空文本句走 worker 既有的**静音句**路径（不发 TTS HTTP），只产出
                // 一个 start/end 边界帧——正是 cue 需要的锚。
                if sentences.is_empty() && !resolution.cues.is_empty() {
                    sentences.push(String::new());
                }
                let covers_upto_seq = sentences.len() as u64;
                // cue **先于**音频发出：前端按 sentence_seq 存好整份计划，
                // 等该句 first_chunk 时应用（与既有 action_cue 契约一致）。
                let delivered = send_event(
                    &event_tx,
                    &cancel,
                    EngineEvent::ActionCue {
                        epoch,
                        ts_ms: now_ms(),
                        covers_upto_seq,
                        cues: resolution.cues.clone(),
                    },
                )
                .await;
                if !delivered {
                    tx_closed = true;
                }
                'perform: for sentence in sentences {
                    // **D23（上屏 == 送 TTS）**：每段文本都是 `clean_for_tts(段)`，
                    // 与流式路径同一条确定性清洗（幂等；只拆标记、不改句界）。
                    // **D24（空白段不跳号）**：清洗后为空的段照常占一个 `sentence_seq`，
                    // 并走 worker 的静音句路径（仍产出 start/end 边界帧）。
                    let cleaned = crate::dialogue::clean_for_tts(&sentence);
                    sentence_seq += 1;
                    let delivered = send_event(
                        &event_tx,
                        &cancel,
                        EngineEvent::SentenceReady {
                            epoch,
                            ts_ms: now_ms(),
                            sentence_seq,
                            text: cleaned.clone(),
                        },
                    )
                    .await;
                    if !delivered {
                        tx_closed = true;
                        break 'perform;
                    }
                    match push_job(
                        &job_tx,
                        &cancel,
                        self.config.queue_push_timeout,
                        TtsJob {
                            sentence_seq,
                            text: cleaned.clone(),
                            // 表演层路径（整段 JSON）不做二路清洗：上屏 == 送 TTS。
                            display: cleaned,
                        },
                    )
                    .await
                    {
                        PushOutcome::Accepted => {}
                        PushOutcome::QueueDead => {
                            queue_dead = true;
                            break 'perform;
                        }
                        PushOutcome::Stopped => {
                            tx_closed = true;
                            break 'perform;
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
                            break 'perform;
                        }
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
        // ---- 阶段 4：正文兜底（rc.3 N0，2026-09-13）----
        // 失败轮里已经生成的正文不能因为「某句没凑齐」而整体消失：健康路径上屏
        // 仍以 `SentenceVoiced` 为准（文字与声音同拍），但失败轮缺一个出口，
        // 用户会以为「模型没回」。这里在终态**之前**补一次 `TextFallback`，
        // 携带**整轮正文**（不是残余）——消费端按覆盖式设置，天然幂等。
        // 只在 `Failed` 发：`Cancelled`（用户按了停止）不该再补一段文字。
        // 兜底正文同样走确定性清洗（策略：上屏与送 TTS 同清洗）——否则失败轮
        // 会把 (动作) / *舞台指示* / ** 原样显示出来，与健康轮不一致。
        let fallback_text = crate::dialogue::clean_for_tts(&assistant_text);
        if status == TurnStatus::Failed && !fallback_text.is_empty() {
            let _ = send_event(
                &event_tx,
                &cancel,
                EngineEvent::TextFallback {
                    epoch,
                    ts_ms: now_ms(),
                    text: fallback_text,
                },
            )
            .await;
        }
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

/// 一句原文 → **两份文本**（`(display, speech)`）；`None` = 期间被取消。
///
/// 三条口径（2026-10-08，见 `crate::cleaning` 与 `conversation/mod.rs` 的契约）：
/// 1. 不跑二路 / 二路 disabled → 上屏与送 TTS 都是 `clean_for_tts(原文)`
///    （既有行为逐字不变）；
/// 2. 二路可用 → 交给它；失败 / 超时 / 坏 JSON / `display` 不是原文切片由
///    [SentenceCleaner::clean] 收敛成「两份都用原文」；
/// 3. 清洗是**逐句、同步**的：这一句拿到结果之前不读下一条 token——不是延迟
///    优化，而是「这一句的提交必须发生在下一句原文出现之前」的可观测顺序。
async fn sentence_texts(
    cleaner: Option<&Arc<SentenceCleaner>>,
    raw: &str,
    cancel: &CancellationToken,
) -> Option<(String, String)> {
    // 句首/句尾空白由分句器保证已 trim；`clean_for_tts` 仍做幂等兜底。
    let legacy = || {
        let cleaned = crate::dialogue::clean_for_tts(raw);
        (cleaned.clone(), cleaned)
    };
    let Some(cleaner) = cleaner.filter(|c| c.enabled()) else {
        return Some(legacy());
    };
    let cleaned = tokio::select! {
        _ = cancel.cancelled() => return None,
        cleaned = cleaner.clean(raw) => cleaned,
    };
    Some((cleaned.display, cleaned.speech))
}
