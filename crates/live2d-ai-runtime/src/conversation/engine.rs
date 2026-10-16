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
use crate::llm::ChatMessage;
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
    /// **动作门**（T14，2026-10-13）：导演启用且 `staging_*` 两项都空 / 都有时开。
    /// 开着时按句清洗的那一次响应带 `cues`，失败回退按句规则表。
    pub(super) motion: bool,
    /// 按句规则回退（`motion` 开着且模型失败时用；单一真源 = director）。
    pub(super) sentence_rule: Option<crate::cleaning::SentenceRuleFallback>,
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
            performance: None,
            cleaner: None,
            motion: false,
            sentence_rule: None,
        }
    }

    /// 注入**动作门**（T14）：开着时按句清洗带 `cues`，并可按句规则回退。
    pub fn set_motion(&mut self, on: bool) {
        self.motion = on;
    }

    /// 注入**按句规则回退**（单一真源 = director 的 `rule_cues_for_sentence`）。
    pub fn set_sentence_rule(&mut self, rule: Option<crate::cleaning::SentenceRuleFallback>) {
        self.sentence_rule = rule;
    }

    /// 动作门是否开着（supervisor 用它决定要不要把 `SentenceReady` 转给 Mod）。
    pub fn motion_enabled(&self) -> bool {
        self.motion
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
    ///
    /// **弹幕解说（2026-10-14 执行单 §3）**：本轮正文以 `弹幕消息：` 开头时，
    /// 把一句固定说明接在**已生效**的 system prompt 后面（没有 system 消息时，
    /// 这一轮就只有这一句）。它只在本轮请求里生效——不写 `live2d-ai.toml`、
    /// 不写 persona / memory 的槽；普通对话一行不加。
    pub(super) fn build_messages(&self, user_text: &str) -> Vec<ChatMessage> {
        let mut out = Vec::with_capacity(2 * self.history.len() + 2);
        let base = self.effective_system_prompt();
        let system = match danmaku_preface_for(user_text) {
            None => base.to_string(),
            Some(preface) if base.is_empty() => preface.to_string(),
            Some(preface) => format!("{base}\n{preface}"),
        };
        if !system.is_empty() {
            out.push(ChatMessage::system(system));
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
}
