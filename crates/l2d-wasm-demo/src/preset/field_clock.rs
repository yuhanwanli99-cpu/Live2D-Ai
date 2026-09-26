//! 音频播放时钟（stage-clock）→ 渲染面 `now_ms` + `segment-ended` 恰好一次
//! （阶段4c；协议 §6.1 / §6.2 / O13）。
//!
//! 从 `field_runtime.rs` 拆出（AGENTS.md 行数纪律：单文件 ≤1000 行）：时钟跟踪
//! 与字段状态机是两件事，且 `segment-ended` 的「每段恰好一次」需要独立的
//! `closed` 记录。纯逻辑、原生可测（`preset/tests/fields.rs`）。

use std::collections::BTreeSet;

/// 时钟域：没有 stage-clock = 墙钟；有 = 音频时钟。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClockDomain {
    /// 段 A（首个音频之前）或 clock 停止。
    #[default]
    Wall,
    /// 段 B（首音频之后）。
    Audio,
}

/// 一份 stage-clock 采样（wire：`{seg,pos_ms,playing}`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StageClock {
    pub seg: u32,
    pub pos_ms: f64,
    pub playing: bool,
    /// 跨段单调基（段 N 起点的音频时间轴值）。
    pub base_ms: f64,
}

/// stage-clock 跟踪器：音频时间轴 + `segment-ended` 恰好一次。
#[derive(Debug, Clone, Default)]
pub struct ClockState {
    active: Option<StageClock>,
    open_seg: Option<u32>,
    closed: BTreeSet<u32>,
    base_ms: f64,
    last_pos_ms: f64,
}

impl ClockState {
    /// 收一条 stage-clock；返回**本次新结束**的段号（供上层发 `segment-ended`）。
    ///
    /// 结束条件：新段号到来（段切换）或 `playing=false`。同一段只结束一次
    /// （`closed` 集合；新一轮由 [`ClockState::reset_closed`] 清零）。
    pub fn on_clock(&mut self, seg: u32, pos_ms: f64, playing: bool) -> Vec<u32> {
        let pos = if pos_ms.is_finite() {
            pos_ms.max(0.0)
        } else {
            0.0
        };
        let mut ended = Vec::new();
        if !playing {
            if let Some(open) = self.open_seg.take() {
                self.closed.insert(open);
                ended.push(open);
            }
            self.active = None;
            return ended;
        }
        match self.open_seg {
            Some(open) if open != seg => {
                self.closed.insert(open);
                ended.push(open);
                self.base_ms += self.last_pos_ms.max(0.0);
                self.open_seg = Some(seg);
            }
            Some(_) => {}
            None => {
                if !self.closed.contains(&seg) {
                    self.open_seg = Some(seg);
                }
            }
        }
        self.last_pos_ms = pos;
        self.active = Some(StageClock {
            seg,
            pos_ms: pos,
            playing: true,
            base_ms: self.base_ms,
        });
        ended
    }

    /// 有效 `now_ms`：有 clock → 音频时间轴（段基 + 段内位置）；无 → 墙钟（段 A）。
    ///
    /// **不外推**：两条 clock 之间返回同一个采样值（前端 30ms 粒度，协议 §6.5）。
    /// 这样「墙钟被音频时钟替换」是可断言的（墙钟前进不改变有效 now，除非新 clock 到达）。
    pub fn effective_now_ms(&self, wall_ms: f64) -> f64 {
        match self.active {
            Some(c) => c.base_ms + c.pos_ms,
            None => wall_ms,
        }
    }

    /// 当前时钟域。
    pub fn domain(&self) -> ClockDomain {
        if self.active.is_some() {
            ClockDomain::Audio
        } else {
            ClockDomain::Wall
        }
    }

    /// 当前正在播放的段号。
    pub fn active_seg(&self) -> Option<u32> {
        self.active.map(|c| c.seg)
    }

    /// 当前采样。
    pub fn active(&self) -> Option<StageClock> {
        self.active
    }

    /// 新一轮：允许复用段号（旧的 `segment-ended` 记录清零）。
    pub fn reset_closed(&mut self) {
        self.closed.clear();
    }

    /// 全清（停止 / 新消息）。
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
