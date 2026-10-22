//! 外部注入 FIFO 队列（0.3.14）：挂回 external-input 前先在宿主侧立好排队账本。
//!
//! # 为什么不再拼接
//!
//! 0.3.1 的旧行为把后来的弹幕用换行拼进同一次 say：一次 say 的正文里出现多条
//! 原文，TTS 会把它们当一段念，前端字幕也混成一条。0.3.14 的契约是：**一条原文
//! 一次 say**，多出来的按 FIFO 排队，**禁止**用换行拼成一条。
//!
//! # 契约
//!
//! - push 一次收**一条原文**，FIFO；
//! - 待发字符预算 [PENDING_CHAR_BUDGET] = 2000，按**字符**（不是字节）计；
//!   放不下的那一条**只丢当条**，已经在队列里的原样保留（丢的是「后来者」，
//!   不是整队）；
//! - pump 只在调用方说 ready（= 本轮生成已结束 **且** 播放不忙，见
//!   crate::web_api::playback_gate）时**至多 say 一条**；say 返回 false 时头一条
//!   **留在队列**（不丢、不重排）；
//! - **超时不得倒掉队列**：队列状态与「播放忙」标志完全解耦——播放忙 120 秒
//!   超时只影响 pump 的 ready 参数（playback_gate 判闲），本模块没有任何清空 /
//!   过期逻辑。
//!
//! # 尚未接线（hang-back）
//!
//! 当前缺省二进制里 POST /api/v1/external/chat 仍是 404 mod_disabled 存根
//! （external-input Mod 实现已移出主仓）。将来挂回 external-input 时：
//! 宿主在 handler 里调用 [ExternalQueue::push]；在「本轮结束 / 播放由忙转闲」
//! 的时机调用 [ExternalQueue::pump]，ready = !turn_in_flight &&
//! !playback_gate.is_busy()。**不得**恢复换行合并。
//!
//! 在那之前本模块的公开入口在生产构建里没有调用方，故整模块 allow(dead_code)
//! （测试里全部入口都被用到，不会掩盖测试代码的问题）。
#![allow(dead_code)] // 挂回 external-input 前无调用方（见上方「尚未接线」）。

use std::collections::VecDeque;
use std::sync::Mutex;

/// 待发字符合计上限：**按字符计，不按字节**（中文一条 3 字节，按字节算会让
/// 预算缩水到 1/3）。
pub const PENDING_CHAR_BUDGET: usize = 2000;

/// push 的结果：入队 / 因窗口满**只丢当条**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// 已入队（追加到队尾）。
    Queued,
    /// 放不下（pending + 本条 > 预算）：**只丢本条**，队列不变。
    DroppedFull,
}

/// pump 的结果：调用方据此记日志 / 决定下一拍。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpOutcome {
    /// 调用方说还不 ready（生成未结束 / 播放忙）：一条都不发，队列原样。
    NotReady,
    /// 队列为空：无事可做。
    Empty,
    /// 已 say 一条并从队列移除。
    Said,
    /// say 返回 false：头一条**留在队列**里（下次 ready 再试）。
    SayRejected,
}

/// 队列内部状态：文本 + 待发字符数（两者必须一起改，故同锁）。
#[derive(Debug, Default)]
struct QueueState {
    items: VecDeque<String>,
    pending_chars: usize,
}

/// 外部注入 FIFO 队列（宿主侧；挂回 external-input 后由宿主 push / pump）。
#[derive(Debug, Default)]
pub struct ExternalQueue {
    state: Mutex<QueueState>,
}

impl ExternalQueue {
    /// 构造空队列。
    pub fn new() -> Self {
        Self::default()
    }

    /// 收**一条**原文（追加到队尾）。返回是否入队。
    ///
    /// 预算按字符数算：pending_chars + 本条字符数 > [PENDING_CHAR_BUDGET] 时
    /// 只丢**当条**，已在队列里的不动。
    pub fn push(&self, text: String) -> PushOutcome {
        let chars = text.chars().count();
        let mut st = self.lock();
        if st.pending_chars + chars > PENDING_CHAR_BUDGET {
            return PushOutcome::DroppedFull;
        }
        st.pending_chars += chars;
        st.items.push_back(text);
        PushOutcome::Queued
    }

    /// 当前队列条数。
    pub fn len(&self) -> usize {
        self.lock().items.len()
    }

    /// 队列是否为空。
    pub fn is_empty(&self) -> bool {
        self.lock().items.is_empty()
    }

    /// 当前待发字符合计（测试与日志用；不计分隔符，因为没有分隔符）。
    pub fn pending_chars(&self) -> usize {
        self.lock().pending_chars
    }

    /// 条件放行：ready 时**至多 say 一条**。
    ///
    /// - ready = false：直接 [PumpOutcome::NotReady]，不碰 say、不碰队列；
    /// - say 返回 false：头一条保留（[PumpOutcome::SayRejected]），下次 ready
    ///   再试——**不重排、不丢**；
    /// - 成功才出队：从队头按原样交出，**不**与后面的条目拼接。
    pub fn pump<F>(&self, ready: bool, say: F) -> PumpOutcome
    where
        F: FnOnce(&str) -> bool,
    {
        if !ready {
            return PumpOutcome::NotReady;
        }
        let mut st = self.lock();
        let Some(head) = st.items.front() else {
            return PumpOutcome::Empty;
        };
        if !say(head) {
            return PumpOutcome::SayRejected;
        }
        let text = st.items.pop_front().expect("队头刚刚确认存在");
        st.pending_chars -= text.chars().count();
        PumpOutcome::Said
    }

    /// 取锁；互斥量中毒也继续用（这里只有队列与一个计数，没有需要放弃的不变量）。
    fn lock(&self) -> std::sync::MutexGuard<'_, QueueState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::web_api::playback_gate::{BUSY_STALE_AFTER, PlaybackGate};

    /// 把队列抽干（断言「抽干后 pending_chars 归零」用）。
    fn drain(queue: &ExternalQueue) -> Vec<String> {
        let mut said = Vec::new();
        while queue.pump(true, |text| {
            said.push(text.to_string());
            true
        }) == PumpOutcome::Said
        {}
        said
    }

    /// 不 ready 时连收两条：两次 pump 依次得到原文，且**没有任何换行拼接**。
    #[test]
    fn two_pushes_while_not_ready_are_said_separately_without_newline() {
        let queue = ExternalQueue::new();
        assert_eq!(queue.push("第一条弹幕".into()), PushOutcome::Queued);
        assert_eq!(queue.push("第二条弹幕".into()), PushOutcome::Queued);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.pending_chars(), 10);

        // 不能发时：pump 两次都只回 NotReady，say 一次都不调。
        let mut said: Vec<String> = Vec::new();
        for _ in 0..2 {
            assert_eq!(
                queue.pump(false, |text| {
                    said.push(text.to_string());
                    true
                }),
                PumpOutcome::NotReady
            );
        }
        assert!(said.is_empty(), "不 ready 时不许 say：{said:?}");
        assert_eq!(queue.len(), 2, "不 ready 不得动队列");

        // ready：每次至多一条，逐条原文发出。
        assert_eq!(
            queue.pump(true, |text| {
                said.push(text.to_string());
                true
            }),
            PumpOutcome::Said
        );
        assert_eq!(queue.len(), 1, "一次 pump 至多出队一条");
        assert_eq!(
            queue.pump(true, |text| {
                said.push(text.to_string());
                true
            }),
            PumpOutcome::Said
        );
        assert_eq!(
            queue.pump(true, |text| {
                said.push(text.to_string());
                true
            }),
            PumpOutcome::Empty
        );

        assert_eq!(
            said,
            vec!["第一条弹幕".to_string(), "第二条弹幕".to_string()],
            "必须按 FIFO 逐条给原文"
        );
        assert!(
            said.iter().all(|text| !text.contains('\n')),
            "禁止用换行拼接：{said:?}"
        );
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.pending_chars(), 0);
    }

    /// 预算按**字符**计，不按 UTF-8 字节：1000 个汉字 = 3000 字节，但只算 1000。
    #[test]
    fn pending_budget_counts_chars_not_utf8_bytes() {
        let queue = ExternalQueue::new();
        let long_utf8 = "中".repeat(1000);
        assert_eq!(long_utf8.len(), 3000, "前提：字节数确实超过 2000");
        assert_eq!(
            queue.push(long_utf8),
            PushOutcome::Queued,
            "预算按字符算，1000 字符应放得下"
        );
        assert_eq!(queue.pending_chars(), 1000);
    }

    /// 窗口满：只丢**当条**，已在队列里的原样保留；刚好 2000 仍收。
    #[test]
    fn pending_budget_full_drops_only_the_incoming_item() {
        let queue = ExternalQueue::new();
        let first = "中".repeat(1999);
        assert_eq!(queue.push(first.clone()), PushOutcome::Queued);

        assert_eq!(
            queue.push("后二".into()),
            PushOutcome::DroppedFull,
            "1999 + 2 > 2000，只丢当条"
        );
        assert_eq!(queue.len(), 1, "已在队列里的必须保留");
        assert_eq!(queue.pending_chars(), 1999);

        assert_eq!(
            queue.push("尾".into()),
            PushOutcome::Queued,
            "恰好 2000 应收"
        );
        assert_eq!(queue.pending_chars(), 2000);

        assert_eq!(
            queue.push("爆".into()),
            PushOutcome::DroppedFull,
            "满窗后只丢当条"
        );
        assert_eq!(queue.len(), 2);

        // 队列内容未被改动：第一条原文完整。
        let said = drain(&queue);
        assert_eq!(said[0], first, "队头原文不得被改动 / 截断");
        assert_eq!(said[1], "尾");
        assert_eq!(queue.pending_chars(), 0, "抽干后字符计数归零");
    }

    /// say 返回 false：头一条留在队列（不丢、不重排），下次 ready 再试。
    #[test]
    fn say_false_keeps_head_item_for_retry() {
        let queue = ExternalQueue::new();
        assert_eq!(queue.push("头".into()), PushOutcome::Queued);
        assert_eq!(queue.push("次".into()), PushOutcome::Queued);

        assert_eq!(
            queue.pump(true, |_| false),
            PumpOutcome::SayRejected,
            "say=false 必须回 SayRejected"
        );
        assert_eq!(queue.len(), 2, "say=false 不丢队列");
        assert_eq!(queue.pending_chars(), 2);

        let mut said = Vec::new();
        assert_eq!(
            queue.pump(true, |text| {
                said.push(text.to_string());
                true
            }),
            PumpOutcome::Said
        );
        assert_eq!(said, vec!["头".to_string()], "保住的必须是原来的队头");
        assert_eq!(queue.len(), 1);
    }

    /// 播放忙满 120 秒：闸门视为闲，队列**一条不少**（超时不得倒掉队列）。
    #[test]
    fn busy_timeout_treats_idle_and_keeps_whole_queue() {
        let gate = PlaybackGate::new();
        let t0 = Instant::now();
        gate.report_at(true, t0);

        let queue = ExternalQueue::new();
        assert_eq!(queue.push("仍在队列的一".into()), PushOutcome::Queued);
        assert_eq!(queue.push("仍在队列的二".into()), PushOutcome::Queued);

        // 忙窗口内（60s）：ready=false → 一条不发，队列不动。
        let ready_in_window = !gate.is_busy_at(t0 + Duration::from_secs(60));
        assert!(!ready_in_window);
        assert_eq!(
            queue.pump(ready_in_window, |_| unreachable!("不 ready 时不得调 say")),
            PumpOutcome::NotReady
        );
        assert_eq!(queue.len(), 2, "忙窗口内不得动队列");

        // 超过 BUSY_STALE_AFTER：闸门视为闲（只影响 ready），队列仍是两条。
        let ready_after_timeout = !gate.is_busy_at(t0 + BUSY_STALE_AFTER + Duration::from_secs(1));
        assert!(ready_after_timeout, "满 120 秒无新上报必须视为闲");
        assert_eq!(queue.len(), 2, "超时不得丢队列");

        let said = drain(&queue);
        assert_eq!(
            said,
            vec!["仍在队列的一".to_string(), "仍在队列的二".to_string()],
            "超时后仍按原顺序发出"
        );
    }
}
