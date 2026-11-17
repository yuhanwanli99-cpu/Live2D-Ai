//! 宠物窗「按住角色说话」：300 ms / 8 像素分界 → 麦克风 → 现有 DeepSeek ASR → `say`。
//!
//! # 判定（纯状态机，单测钉住）
//!
//! - 按在**不透明像素**上（`WM_LBUTTONDOWN` 本来只会落在命中像素上）开始计时；
//! - 相对按下点移动 > 8 像素 → 仍是原来的拖拽，**不开始录音**；
//! - 按住 ≥ 300 ms 且几乎没动 → 才开麦；松手结束；短按不录音；
//! - 16 位单声道 16 kHz WAV，最长 120 秒（到点自动松手），> 8 MiB 丢弃；
//! - `[asr]` 钥匙为空：**不录音、不打上游**，气泡写明「没配识别钥匙」；
//! - 正在播自己的声音时不听（与 Flutter `suspendForPlayback` 同理）。
//!
//! # 不打自己的 HTTP
//!
//! 识别直接调 `live2d_ai_runtime::asr::deepseek::transcribe`（`voice/deepseek`
//! 已经交付的那支函数），**不** POST 回 `/api/v1/voice/asr`。识别到非空文本
//! 走 `bridge::say` → `SupervisorHandle::say`，于是热槽四块、导演、逐句合成
//! 都按并入后的现成路径走，不在这里重做一遍。
//!
//! 麦克风用桌面进程里**已有**的 `cpal`（输入侧），不新增依赖、不新开播放器；
//! 出声仍是那一路 cpal 输出。宠物窗没开时本模块不会被调用，不会抢麦。

// 非 Windows：录音/ASR 段不编译，纯判定与 WAV 编码仍供单测使用。
#![cfg_attr(not(windows), allow(dead_code))]

use std::time::{Duration, Instant};

/// 按住多久才算说话。
pub const HOLD_MS: u64 = 300;
/// 相对按下点的最大位移（超过就是拖拽）。
pub const MOVE_LIMIT_PX: i32 = 8;
/// 16 位单声道，16 kHz。
pub const SAMPLE_RATE: u32 = 16_000;
/// 一次最多录 120 秒（DeepSeek ASR 的上限）。
pub const MAX_RECORD_SECS: u64 = 120;
/// 上游单次请求上限；超过就本机丢弃。
pub const MAX_WAV_BYTES: usize = 8 * 1024 * 1024;
/// 120 秒 × 16 kHz 的样本数（先按这个截，8 MiB 是第二道防线）。
pub const MAX_SAMPLES: usize = SAMPLE_RATE as usize * MAX_RECORD_SECS as usize;
pub const NO_KEY_NOTICE: &str = "没配识别钥匙";
pub const EMPTY_NOTICE: &str = "没听清";
pub const MIC_NOTICE: &str = "麦克风不可用";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldOutcome {
    Idle,
    /// 移动超过 8 像素：拖拽，不录音。
    Drag,
    /// 短于 300 ms 的点按：不录音。
    TooShort,
    /// 有效按住说话。
    Record,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Pressed,
    Drag,
    Recording,
    /// 有效按住但没开麦（没配钥匙 / 正在出声）；松手仍算一次「说话」。
    NoRecord,
}

/// 「按住 / 拖拽」分界的状态机。与平台无关，Linux/Windows 单测都真跑。
#[derive(Debug, Clone, Copy)]
pub struct HoldState {
    phase: Phase,
    pressed_at: Option<Instant>,
    recording_at: Option<Instant>,
    moved_px: i32,
}

impl HoldState {
    pub const fn new() -> Self {
        Self {
            phase: Phase::Idle,
            pressed_at: None,
            recording_at: None,
            moved_px: 0,
        }
    }

    pub fn down(&mut self, now: Instant) {
        self.phase = Phase::Pressed;
        self.pressed_at = Some(now);
        self.recording_at = None;
        self.moved_px = 0;
    }

    /// `dx/dy` 是光标相对**按下点**的位移（不是上一帧位移）。
    pub fn moved(&mut self, dx: i32, dy: i32) {
        if !matches!(
            self.phase,
            Phase::Pressed | Phase::Recording | Phase::NoRecord
        ) {
            return;
        }
        self.moved_px = self.moved_px.max(dx.abs()).max(dy.abs());
        if self.moved_px > MOVE_LIMIT_PX {
            self.phase = Phase::Drag;
        }
    }

    pub fn should_start(&self, now: Instant) -> bool {
        self.phase == Phase::Pressed
            && self
                .pressed_at
                .is_some_and(|t| now.saturating_duration_since(t) >= Duration::from_millis(HOLD_MS))
    }

    pub fn mark_recording(&mut self, now: Instant) {
        self.phase = Phase::Recording;
        self.recording_at = Some(now);
    }

    pub fn mark_no_record(&mut self) {
        self.phase = Phase::NoRecord;
    }

    pub fn cancel(&mut self) {
        self.phase = Phase::Drag;
    }

    pub fn is_drag(&self) -> bool {
        self.phase == Phase::Drag
    }

    pub fn is_recording(&self) -> bool {
        self.phase == Phase::Recording
    }

    pub fn auto_stop_due(&self, now: Instant) -> bool {
        self.phase == Phase::Recording
            && self.recording_at.is_some_and(|t| {
                now.saturating_duration_since(t) >= Duration::from_secs(MAX_RECORD_SECS)
            })
    }

    pub fn up(&mut self, now: Instant) -> HoldOutcome {
        let held = self
            .pressed_at
            .map(|t| now.saturating_duration_since(t) >= Duration::from_millis(HOLD_MS))
            .unwrap_or(false);
        let outcome = match self.phase {
            Phase::Idle => HoldOutcome::Idle,
            Phase::Drag => HoldOutcome::Drag,
            Phase::Pressed => {
                if held {
                    HoldOutcome::Record
                } else {
                    HoldOutcome::TooShort
                }
            }
            Phase::Recording | Phase::NoRecord => HoldOutcome::Record,
        };
        self.phase = Phase::Idle;
        self.pressed_at = None;
        self.recording_at = None;
        self.moved_px = 0;
        outcome
    }
}

impl Default for HoldState {
    fn default() -> Self {
        Self::new()
    }
}

/// 16 位单声道 16 kHz PCM WAV（44 字节标准头 + 小端样本）。
#[must_use]
pub fn encode_wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36_u32 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // 单声道
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2_u16.to_le_bytes()); // block align
    out.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// 松手后的纯决策：**没配钥匙时绝不产生「打上游」这个动作**。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsrPlan {
    Skip,
    Notice(&'static str, Duration),
    /// 带钥匙，允许调用 `deepseek::transcribe`。
    Transcribe(String),
}

#[must_use]
pub fn plan_asr(key: Option<&str>, playing: bool, outcome: HoldOutcome) -> AsrPlan {
    if outcome != HoldOutcome::Record || playing {
        return AsrPlan::Skip;
    }
    let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) else {
        return AsrPlan::Notice(NO_KEY_NOTICE, Duration::from_secs(3));
    };
    AsrPlan::Transcribe(key.to_string())
}

/// 一条给气泡的提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Notice {
    pub text: &'static str,
    pub ttl: Duration,
}

#[cfg(windows)]
#[path = "talk_win.rs"]
mod platform;

#[cfg(windows)]
pub use platform::{on_down, on_move, on_up, poll};

#[cfg(test)]
#[path = "talk_tests.rs"]
mod tests;
