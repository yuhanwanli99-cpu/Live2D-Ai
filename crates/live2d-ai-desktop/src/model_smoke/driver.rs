//! 冒烟驱动 + 动作序列 + 帧时钟。
//!
//! 纯逻辑层：不碰 GPU/窗口/事件循环；冒烟流程在 [`crate::app`] 接线。

use live2d_ai_core::{ActionId, ParameterFrame, PerformancePlayer, SampleStatus, SemanticAction};

// ---------------------------------------------------------------- 固定步长

/// 模拟固定步长：60 Hz（l2d bakeoff 验证值；物理引擎按此步进最稳定）。
pub const SIM_DT: f32 = l2d::renderer::FIXED_DT_60HZ;

/// 单帧墙钟 dt 的上限（秒）：超过按该值截断（拖尾帧不放大模拟步数）。
///
/// 合理范围口径：正常 60 Hz present 下 dt ≈ 16.7 ms；前台切换/断点等异常长帧
/// 按 250 ms 截断，避免一次追帧把模拟时间快进数秒。
pub const MAX_FRAME_DT_SECS: f32 = 0.25;

/// 累加器水位上限：最多预存 4 个模拟步（约一帧的量），超出部分丢弃——
/// 渲染跟不上时宁可放慢模拟时间也不累积爆炸（螺旋死亡保护）。
pub const MAX_ACCUMULATOR_STEPS: u32 = 4;

/// 单帧最多执行的模拟步数（防御性上限；正常 ≤ [`MAX_ACCUMULATOR_STEPS`]）。
pub const MAX_TICKS_PER_FRAME: u32 = 8;

// ---------------------------------------------------------------- 动作序列

/// 六动作固定顺序播放器：`nod → shake_no → tilt → look_around → listen → surprise`
/// （即 [`ActionId::ALL`] 顺序），循环回绕。每动作 [`live2d_ai_core::Strength::Medium`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionSequence {
    index: usize,
}

impl Default for ActionSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionSequence {
    /// 新序列：从第一个动作开始。
    pub fn new() -> Self {
        Self { index: 0 }
    }

    /// 当前应播放的动作（不推进）。
    pub fn current(&self) -> SemanticAction {
        crate::model_smoke::scripted_action(ActionId::ALL[self.index])
    }

    /// 推进到下一动作（动作结束/被打断时调用）；末尾回绕到第一个。
    pub fn advance(&mut self) -> SemanticAction {
        self.index = (self.index + 1) % ActionId::ALL.len();
        self.current()
    }

    /// 完整顺序的动作名表（测试与文档对照用）。
    pub fn names() -> Vec<&'static str> {
        ActionId::ALL.iter().map(|a| a.name()).collect()
    }
}

// ---------------------------------------------------------------- 冒烟驱动

/// 把 core 表演播放器与固定动作序列绑定的冒烟驱动（纯逻辑，无时钟依赖）。
#[derive(Debug)]
pub struct SmokeDriver {
    player: PerformancePlayer,
    sequence: ActionSequence,
    completed: u32,
}

/// 一次模拟 tick 的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TickOutcome {
    /// 当前动作表演中。
    Playing,
    /// 本 tick 恰好完成了一个动作并已切换到下一动作（首帧中性）。
    Advanced {
        /// 刚完成的动作名。
        finished: &'static str,
        /// 新开始的动作名。
        started: &'static str,
        /// 已完成动作总数。
        completed: u32,
    },
}

impl SmokeDriver {
    /// chat 模式的直接表演入口（表演源是 root 下发的命令而非固定序列）；
    /// 冒烟模式继续走 [`Self::tick`] 的序列驱动。
    pub(crate) fn player_mut(&mut self) -> &mut PerformancePlayer {
        &mut self.player
    }

    /// 新驱动：立即开始第一个动作（时间轴归零）。
    pub fn new() -> Self {
        let mut driver = Self {
            player: PerformancePlayer::new(),
            sequence: ActionSequence::new(),
            completed: 0,
        };
        driver.player.play(driver.sequence.current());
        driver
    }

    /// 当前活动动作（若有；完成瞬间到下一次 play 之间可能为 None）。
    pub fn active(&self) -> Option<SemanticAction> {
        self.player.active()
    }

    /// 已完成的动作总数。
    pub const fn completed(&self) -> u32 {
        self.completed
    }

    /// 推进一个固定模拟步并把表演帧写入 `out`（零分配缓冲由调用方持有）。
    ///
    /// 动作到达终点时：记录完成 → 序列推进 → 立即开始下一动作（本步输出
    /// 中性帧，下一动作从下一步起采样）——"动作结束下一动作"的固定顺序语义。
    pub fn tick(&mut self, out: &mut ParameterFrame) -> TickOutcome {
        // 序列只在完成时推进，因此当前序列位置即正在播放的动作。
        let playing = self.sequence.current().action.name();
        match self.player.update(SIM_DT, out) {
            SampleStatus::Finished => {
                self.completed += 1;
                let started = self.sequence.advance();
                self.player.play(started);
                TickOutcome::Advanced {
                    finished: playing,
                    started: started.action.name(),
                    completed: self.completed,
                }
            }
            // Playing / Idle（Idle 只出现在极端越界 dt 后，输出已是中性帧）。
            _ => TickOutcome::Playing,
        }
    }
}

// ---------------------------------------------------------------- 帧时钟

/// 固定步长累加器：真实墙钟 dt（截断到合理范围）→ 60 Hz 模拟步。
///
/// - 输入 dt 先夹到 `[0, MAX_FRAME_DT_SECS]`（非有限按 0）；
/// - 累加水位封顶 [`MAX_ACCUMULATOR_STEPS`] 步，超出丢弃（防螺旋死亡）；
/// - [`Self::advance`] 返回本帧应执行的模拟步数并在内部等量扣减。
#[derive(Debug, Clone, PartialEq)]
pub struct FrameClock {
    accumulator: f32,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameClock {
    /// 新时钟（零累计）。
    pub fn new() -> Self {
        Self { accumulator: 0.0 }
    }

    /// 当前未消费的累计量（秒；诊断/测试观测用）。
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn accumulator(&self) -> f32 {
        self.accumulator
    }

    /// 累计一段墙钟时间，返回本帧应执行的模拟步数。
    pub fn advance(&mut self, wall_dt: f32) -> u32 {
        let dt = if wall_dt.is_finite() && wall_dt > 0.0 {
            wall_dt.min(MAX_FRAME_DT_SECS)
        } else {
            0.0
        };
        self.accumulator += dt;
        let cap = MAX_ACCUMULATOR_STEPS as f32 * SIM_DT;
        if self.accumulator > cap {
            self.accumulator = cap;
        }
        let mut ticks = (self.accumulator / SIM_DT).floor() as u32;
        ticks = ticks.min(MAX_TICKS_PER_FRAME);
        self.accumulator -= ticks as f32 * SIM_DT;
        ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use live2d_ai_core::ActionId;

    #[test]
    fn action_sequence_plays_fixed_order_then_wraps() {
        assert_eq!(
            ActionSequence::names(),
            vec![
                "nod",
                "shake_no",
                "tilt",
                "look_around",
                "listen",
                "surprise"
            ]
        );
        let mut seq = ActionSequence::new();
        let mut seen = Vec::new();
        for _ in 0..(ActionId::ALL.len() + 2) {
            seen.push(seq.current().action.name());
            seq.advance();
        }
        // 固定顺序循环：第 7、8 个动作回到序列开头；当前位置与名字一致。
        assert_eq!(
            &seen[..6],
            &[
                "nod",
                "shake_no",
                "tilt",
                "look_around",
                "listen",
                "surprise"
            ][..]
        );
        assert_eq!(seen[6], "nod");
        assert_eq!(seen[7], "shake_no");
        assert_eq!(
            seq.current().action.name(),
            "tilt",
            "推进 8 次后应位于序列第 3 位"
        );
    }

    #[test]
    fn smoke_driver_finishes_each_action_before_the_next() {
        let mut driver = SmokeDriver::new();
        // 构造即开始第一个动作。
        assert_eq!(
            driver.active().map(|a| a.action.name()),
            Some(ActionId::ALL[0].name())
        );
        let mut frame = live2d_ai_core::ParameterFrame::neutral();
        let mut transitions: Vec<(&'static str, &'static str)> = Vec::new();
        let names = ActionSequence::names();
        // 两轮完整序列的步数预算（最长动作 look_around 2.4s ≈ 144 步）。
        for _ in 0..(names.len() * 2 * 200) {
            match driver.tick(&mut frame) {
                TickOutcome::Playing => {}
                TickOutcome::Advanced {
                    finished,
                    started,
                    completed,
                } => {
                    // 完成瞬间输出精确中性帧。
                    assert!(frame.is_neutral());
                    transitions.push((finished, started));
                    assert_eq!(completed, transitions.len() as u32);
                }
            }
            if driver.completed() > names.len() as u32 {
                break;
            }
        }
        let started: Vec<&'static str> = transitions.iter().map(|(_, started)| *started).collect();
        // 第一个动作在构造时已开始，故 started 是名字表左旋一位的循环序列
        // （nod 完成后起 shake_no … surprise 完成后回绕到 nod）。
        let mut rotated = names.clone();
        rotated.rotate_left(1);
        assert_eq!(&started[..6], &rotated[..]);
        // 第 7 次完成：surprise 已回绕到 nod，故此时开始的动作是第二轮的
        // 第二个动作（shake_no）。
        assert_eq!(started[6], names[1]);
        // finished 与 started 首尾相接。
        for pair in transitions.windows(2) {
            assert_eq!(pair[0].1, pair[1].0);
        }
        assert_eq!(transitions[0].0, "nod");
    }

    #[test]
    fn frame_clock_accumulates_fixed_steps_with_caps() {
        let mut clock = FrameClock::new();
        assert_eq!(clock.advance(0.0), 0);
        // 一个标准步触发一个 tick。
        assert_eq!(clock.advance(SIM_DT), 1);
        // 小于一步的时间不触发，累计后触发。
        assert_eq!(clock.advance(SIM_DT / 2.0), 0);
        assert_eq!(clock.advance(SIM_DT / 2.0), 1);
        // 超长帧被截断并封顶：最多 MAX_ACCUMULATOR_STEPS 个 tick。
        let burst = clock.advance(MAX_FRAME_DT_SECS);
        assert_eq!(burst, MAX_ACCUMULATOR_STEPS);
        assert!(clock.accumulator() < SIM_DT);
        // 非法输入按 0 处理。
        assert_eq!(clock.advance(f32::NAN), 0);
        assert_eq!(clock.advance(-1.0), 0);
        // 契约：非有限输入按 0 处理（不触发追帧）。
        assert_eq!(clock.advance(f32::INFINITY), 0);
        // 连续正常帧：60Hz 下每帧恰好一步。
        let mut clock = FrameClock::new();
        for _ in 0..10 {
            assert_eq!(clock.advance(SIM_DT), 1);
        }
    }
}
