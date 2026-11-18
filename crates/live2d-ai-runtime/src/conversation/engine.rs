//! [`ConversationEngine`] 主体：发起 LLM 请求、增量切句、入队给 TTS worker，
//! 并在每轮结尾统一发出恰一次的 [`EngineEvent::Terminal`]。
//!
//! 与子模块的协作：
//! - [`super::events::send_event`] / [`super::events::send_terminal`]：事件投递；
//! - [`super::queue::push_job`]：句子入队 + 背压超时；
//! - [`super::worker::tts_worker`]：TTS 串行合成。

use std::collections::VecDeque;
use std::sync::Arc;

use crate::OpenAiClient;
use crate::cleaning::SentenceCleaner;
use crate::llm::{ChatMessage, Role};
use crate::performance::PerformanceRuntime;

/// 直播间弹幕的前缀（2026-10-14 执行单 §3）：弹幕经外部端点注入时正文以此开头。
pub(super) const DANMAKU_PREFIX: &str = "弹幕消息：";

/// 弹幕解说句（执行单 §0 原文，**逐字固定**、不加性格、不进任何配置）。
pub(super) const DANMAKU_PREFACE: &str =
    "以「弹幕消息：」开头的内容是直播间弹幕，不是操作者本人说的话。";

/// 本轮是否要带弹幕解说（纯函数，便于单测）。
///
/// 只看**开头**：正文中段出现同样的字样不算（那还是操作者自己说的话）。
pub(super) fn danmaku_preface_for(user_text: &str) -> Option<&'static str> {
    user_text
        .starts_with(DANMAKU_PREFIX)
        .then_some(DANMAKU_PREFACE)
}

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
    /// 因超出 `max_history_pairs` 被挤出窗口的轮次（宿主折摘要用；取走即清）。
    evicted: Vec<(String, String)>,
    /// **表演层运行时**（2026-09-22；rc4 起只保留 segments / speak）。
    ///
    /// - `None`（缺省）= 关闸：主链走「边流边切句 + 按句二路清洗 + 按句规则表
    ///   出 cue」的常驻路径；
    /// - `Some`：本轮先收齐助手原文，调表演层拿一份 JSON；v1 的 `segments` 是
    ///   本轮 TTS / 上屏的**切分方案**（一对一，D22）。端点缺失时运行时仍存在但
    ///   `enabled()==false`——每轮走确定性回退，degraded 在状态面可见。
    ///
    /// **rc4 单 cue 写者**：表演层的 `Resolution.cues` **不再**从这里发成
    /// `EngineEvent::ActionCue`——写 cue 的只留主链按句第二路（含规则表回退）。
    pub(super) performance: Option<Arc<PerformanceRuntime>>,
    /// **二路按句清洗**（2026-10-08；rc4 起常驻）。
    ///
    /// - `None` / `enabled()==false` = 不跑二路：上屏与送 TTS 都是
    ///   `clean_for_tts(句)`，动作走**这一句原文**的规则表；
    /// - `Some` 且 `enabled()`：每句原文先交给二路拿 `{display, speech, cues?}`，
    ///   `display` 上屏、`speech` 送 TTS（**两个不同的字符串，同一个
    ///   sentence_seq**）。二路失败 / 超时 / 坏 JSON / `display` 不是原文切片
    ///   → 这一句两份都用原文、动作回退这一句原文的规则表。
    ///
    /// 生产路径由 host 用 `[llm]` 同一份端点构造后注入；这里只消费它，不再自己造客户端。
    pub(super) cleaner: Option<Arc<SentenceCleaner>>,
}

mod run;
mod sentence;

impl ConversationEngine {
    /// 创建引擎。
    pub fn new(client: OpenAiClient, config: super::ConversationConfig) -> Self {
        Self {
            client,
            config,
            history: VecDeque::new(),
            system_prompt_override: None,
            evicted: Vec::new(),
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
    /// rc4 起 host 不再用它判 `SentenceReady` 转发（director Mod 已退休、不再
    /// 订阅该主题）；保留给状态面 / 既有装配测试。
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
    ///
    /// **弹幕解说（2026-10-14 执行单 §3）**：本轮正文以 `弹幕消息：` 开头时，
    /// 把一句固定说明接在**已生效**的 system prompt 后面（没有 system 消息时，
    /// 这一轮就只有这一句）。它只在本轮请求里生效——不写 `live2d-ai.toml`、
    /// 不写 persona / memory 的槽；普通对话一行不加。
    ///
    /// 生产路径统一走 [`Self::build_messages_with_history`]（协议 `history`）；
    /// 本方法保留为「不带注入」的语义入口，当前主要被本模块单测直接调用——
    /// 显式放行 dead_code，而不是让 `cargo check` 多一条噪声。
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn build_messages(&self, user_text: &str) -> Vec<ChatMessage> {
        self.build_messages_with_history(user_text, &[])
    }

    /// 组装本轮消息，并把客户端给的**本地会话历史**接进历史段。
    ///
    /// # 为什么是「传参」而不是 `replace_history`（一次性注入的选型）
    ///
    /// `replace_history` 改的是引擎**自己的记忆窗口**，它有三个宿主可见的
    /// 副作用：`commit_completed_turn` 往同一个桶里追加、`take_evicted_turns`
    /// 把溢出的轮次交给摘要折叠、`history_len` 被状态面读。本地历史属于
    /// **客户端这次请求**，不是引擎的轮次记录；写进窗口会污染这三处，还得
    /// 每轮再清一次（漏清一轮就会把上一个会话的历史带进下一个会话）。
    /// 传参则天然只活一轮：`run_turn_with_history` 不会把它交给任何提交路径。
    ///
    /// # 顺序与回退
    ///
    /// 固定为 `[system?] + 历史 + 当前输入`：
    /// - `injected` 非空 → **以它为准**，不再拼 `self.history`。同一段对话在
    ///   客户端和宿主进程各存一份，拼两遍是白翻倍的上下文；出厂的
    ///   `max_history_pairs = 0` 时 `self.history` 本来就是空，老协议请求体
    ///   逐字不变；
    /// - `injected` 为空（老客户端 / 第一轮 / 全部条目被过滤）→ 走既有
    ///   `self.history` 窗口，行为与 `build_messages` 完全一致；
    /// - `injected` 里的 system 条目被忽略：系统句只由引擎在这里拼，
    ///   注入方不能借历史改写人设。
    pub(super) fn build_messages_with_history(
        &self,
        user_text: &str,
        injected: &[ChatMessage],
    ) -> Vec<ChatMessage> {
        // 先定历史段：注入的本地历史优先，否则用引擎自己的窗口。
        let history: Vec<ChatMessage> = if injected.is_empty() {
            self.history
                .iter()
                .flat_map(|(user, assistant)| {
                    [
                        ChatMessage::user(user.clone()),
                        ChatMessage::assistant(assistant.clone()),
                    ]
                })
                .collect()
        } else {
            injected
                .iter()
                .filter(|message| message.role != Role::System)
                .cloned()
                .collect()
        };
        let mut out = Vec::with_capacity(history.len() + 2);
        let base = self.effective_system_prompt();
        let system = match danmaku_preface_for(user_text) {
            None => base.to_string(),
            Some(preface) if base.is_empty() => preface.to_string(),
            Some(preface) => format!("{base}\n{preface}"),
        };
        if !system.is_empty() {
            out.push(ChatMessage::system(system));
        }
        out.extend(history);
        out.push(ChatMessage::user(user_text));
        out
    }

    /// 运行一轮对话（不注入客户端本地历史）。
    pub async fn run_turn(
        &mut self,
        epoch: u64,
        user_text: &str,
        event_tx: tokio::sync::mpsc::Sender<super::EngineEvent>,
        cancel: super::CancellationToken,
    ) -> super::TurnReport {
        self.run_turn_with_history(epoch, user_text, &[], event_tx, cancel)
            .await
    }

    /// 运行一轮对话，并把客户端给的本地会话历史**只注入本轮请求**。
    ///
    /// 与 [Self::run_turn] 的唯一区别是 `history`：它只影响本轮的
    /// [Self::build_messages_with_history]，不写 [Self::history]、不参与
    /// `commit_completed_turn` / `max_history_pairs`（选型理由见前者注释）。
    pub async fn run_turn_with_history(
        &mut self,
        epoch: u64,
        user_text: &str,
        history: &[ChatMessage],
        event_tx: tokio::sync::mpsc::Sender<super::EngineEvent>,
        cancel: super::CancellationToken,
    ) -> super::TurnReport {
        self.run_turn_inner(epoch, user_text, history, event_tx, cancel)
            .await
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
            // 不保留历史 = 这一轮**全部**掉出窗口（摘要折叠要它）。
            self.evicted
                .push((user_text.to_string(), assistant_text.to_string()));
            return;
        }
        self.history
            .push_back((user_text.to_string(), assistant_text.to_string()));
        while self.history.len() > self.config.max_history_pairs {
            if let Some(pair) = self.history.pop_front() {
                // **不要在这里丢内容**：宿主要把被挤出的轮次折成滚动摘要
                // （见 `live2d_ai_runtime::hotmem::fold_summary`）。
                self.evicted.push(pair);
            }
        }
    }

    /// 取走「因超出 `max_history_pairs` 而被挤出窗口」的轮次（FIFO，取走后清空）。
    ///
    /// 宿主在每轮收口后调一次，把结果折成 ≤300 字的滚动摘要。返回空表 = 这一轮
    /// 没有轮次掉出窗口，摘要不用动。
    pub fn take_evicted_turns(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.evicted)
    }
}
