//! 实际声卡输出第一批（RFC §4 批次 4 部分落地）：**生产者 → 无锁 SPSC 环 →
//! cpal 回调 → DAC**，口型电平来自回调内对「实际写给声卡的 f32 值」的 RMS。
//!
//! # 链路与线程模型
//!
//! ```text
//! TTS/测试线程（允许分配/转换）              cpal 音频回调线程（实时约束）
//! ┌────────────────────────────┐            ┌───────────────────────────────┐
//! │ enqueue_pcm_f32(&[f32])    │            │ render_*(&mut [T])            │
//! │  1. 非有限值净化为 0        │   ringbuf   │  1. Consumer::pop_slice       │
//! │  2. convert_spec 换到设备域 │  ─────────▶ │  2. epoch 打断前缀置静音      │
//! │  3. 整帧对齐写入 Producer   │  (无锁SPSC) │  3. 欠载补 0                  │
//! └────────────────────────────┘            │  4. RmsMeter.push(实际输出值) │
//! stop_and_clear：推进 garbage 线           │  5. mouth_bits ← f32 bits     │
//! ────────────────────────────             └───────────────────────────────┘
//! ```
//!
//! 回调侧硬约束（本模块以类型与代码结构保证）：**不锁 Mutex、不分配、不日志、
//! 不做重采样/声道映射**。所有跨线程通信只经 `ringbuf` 与 [`std::sync::atomic`]。
//!
//! # epoch 打断（`stop_and_clear`）语义
//!
//! 物理上不清环形缓冲（生产者无法回收消费者一侧），而是记录单调递增的
//! 「垃圾线」`garbage_upto`（= 打断时刻已生产样本总数）。样本按 FIFO 顺序
//! 天然按生产序号排列，因此「生产序号 ≤ 垃圾线」的样本恰为环内**前缀**；
//! 回调把该前缀整体替换为静音。打断之后新入队的样本序号必然大于垃圾线，
//! 照常播放——即「epoch 打断后旧音频不能继续，新音频立即生效」。
//!
//! `stop_and_clear` 与并发 `enqueue_pcm_f32` 的线性化点：入队以其末尾样本
//! 计入 `produced` 的时刻为准——计入在垃圾线快照之前则被打断，否则保留。
//!
//! # 溢出策略
//!
//! 入环空间不足时接纳**整帧对齐的前缀**（绝不写半个声道帧），其余拒绝并
//! 在 [`EnqueueOutcome::rejected`] 给出明确数量；永不阻塞、永不覆盖已有数据。
//!
//! # 设备格式
//!
//! 第一批支持设备样本格式 f32 / i16 / u16（见 [`f32_to_i16`] / [`f32_to_u16`]），
//! 其余格式显式报错（不假装支持）。RMS 一律在 f32 域、对最终输出值计算。
//!
//! # 子模块划分
//!
//! - [`ring`]：SPSC 环、[`SharedState`]、[`PlaybackHandle`] / [`MouthSnapshot`] /
//!   [`OutputStats`]、回调 [`RenderCore`] 与 `render_block` 自由函数。
//! - [`prepared`]：[`PreparedPcm`] / [`TryEnqueue`] / [`EnqueueOutcome`]，
//!   以及冒烟报告值对象 [`AudioSmokeReport`]。
//! - [`output`]：cpal 设备 facade [`AudioOutputFacade`] 与冒烟执行体
//!   [`run_audio_smoke`]。
//! - [`tests`]：`#[cfg(test)]` 模块整块迁出。

use std::fmt;

use live2d_ai_runtime::AudioSpec;

pub mod output;
pub mod prepared;
pub mod ring;
#[cfg(test)]
mod tests;

// 公开 API 透传：保持 `crate::audio::X` 路径稳定（supervisor / backend / app
// 依赖的若干符号在 bin 自身不直接使用，但 crate 内其它模块/测试要用）。
#[allow(unused_imports)]
pub use crate::audio::output::{AudioOutputFacade, DeviceSummary, run_audio_smoke};
#[allow(unused_imports)]
pub use crate::audio::prepared::{AudioSmokeReport, EnqueueOutcome, PreparedPcm, TryEnqueue};
#[allow(unused_imports)]
pub(crate) use crate::audio::ring::detached_pair_for_tests;
#[allow(unused_imports)]
pub use crate::audio::ring::{MouthSnapshot, OutputStats, PlaybackHandle, RenderCore};

/// RMS 滑动窗口（毫秒）：约一个音节片长的能量平均。
pub(crate) const RMS_WINDOW_MS: f32 = 20.0;
/// RMS attack 时间常数（秒）：张嘴响应速度。
pub(crate) const RMS_ATTACK_SECS: f32 = 0.03;
/// RMS release 时间常数（秒）：闭嘴回落速度。
pub(crate) const RMS_RELEASE_SECS: f32 = 0.10;

/// 回调侧搬运暂存容量（样本数）：构造时一次分配，回调内只复用。
/// 处理超过该长度的回调缓冲时分块循环，任何情况下都不再分配。
pub(crate) const SCRATCH_SAMPLES: usize = 1024;

/// 音频输出错误。区分「环境无音频设备」（[`Self::is_environment`]）与代码缺陷，
/// CLI 据此映射退出码 3 / 1。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioOutputError {
    /// 环境不满足：无默认输出设备或设备不可达（非代码缺陷）。
    NoDevice(String),
    /// 设备配置读取失败（常见于环境问题，如 ALSA 缺插件）。
    Config(String),
    /// 流构建/启动失败。
    Stream(String),
    /// 设备样本格式不在 f32/i16/u16 支持集内。
    UnsupportedSampleFormat(String),
    /// 参数非法（如环形缓冲容量为 0）。
    InvalidInput(String),
}

impl AudioOutputError {
    /// 是否属于「运行环境不满足」类错误（映射 CLI 退出码 3）。
    pub fn is_environment(&self) -> bool {
        matches!(self, Self::NoDevice(_) | Self::Config(_))
    }
}

impl fmt::Display for AudioOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDevice(m) => write!(f, "无可用音频输出设备: {m}"),
            Self::Config(m) => write!(f, "音频设备配置不可用: {m}"),
            Self::Stream(m) => write!(f, "音频流构建/启动失败: {m}"),
            Self::UnsupportedSampleFormat(m) => {
                write!(f, "设备样本格式不支持: {m}（支持 f32/i16/u16）")
            }
            Self::InvalidInput(m) => write!(f, "音频输出参数非法: {m}"),
        }
    }
}

impl std::error::Error for AudioOutputError {}

/// f32 ∈ [-1, 1] → i16（标准对称 s16 刻度：×32768 后四舍五入，正上界夹持
/// 到 32767；因此 -1.0 → -32768、+1.0 → 32767；非有限 → 0）。
pub fn f32_to_i16(x: f32) -> i16 {
    if !x.is_finite() {
        return 0;
    }
    let rounded = (x.clamp(-1.0, 1.0) * 32_768.0).round();
    if rounded >= 32_767.0 {
        32_767
    } else {
        // 下界 -32768 精确可表示，无需再夹持。
        rounded as i16
    }
}

/// f32 ∈ [-1, 1] → u16（bipolar 偏移刻度，与 s16 同刻度异或符号位：
/// -1.0 → 0、0 → 32768、+1.0 → 65535）。
pub fn f32_to_u16(x: f32) -> u16 {
    (f32_to_i16(x) as u16) ^ 0x8000
}

/// f32 直通（f32 设备格式的转换函数）。
pub fn passthrough_f32(x: f32) -> f32 {
    x
}

/// 生产者侧抽象（节点 B 竞态测试注入面）：supervisor 的闭环状态机可在
/// 无声卡环境用与真实链路一致的 PCM 路径验收。两个实现：cpal facade 与
/// detached 句柄（测试）。
pub trait PcmProducer: Send {
    fn prepare_pcm_f32(&self, samples: &[f32], source_spec: AudioSpec) -> PreparedPcm;
    fn try_enqueue_prepared(&mut self, prepared: &mut PreparedPcm) -> TryEnqueue;
    fn stop_and_clear(&self) -> u64;
    fn is_drained(&self) -> bool;
    fn healthy(&self) -> bool;
}

impl PcmProducer for AudioOutputFacade {
    fn prepare_pcm_f32(&self, samples: &[f32], source_spec: AudioSpec) -> PreparedPcm {
        AudioOutputFacade::prepare_pcm_f32(self, samples, source_spec)
    }
    fn try_enqueue_prepared(&mut self, prepared: &mut PreparedPcm) -> TryEnqueue {
        AudioOutputFacade::try_enqueue_prepared(self, prepared)
    }
    fn stop_and_clear(&self) -> u64 {
        AudioOutputFacade::stop_and_clear(self)
    }
    fn is_drained(&self) -> bool {
        AudioOutputFacade::is_drained(self)
    }
    fn healthy(&self) -> bool {
        AudioOutputFacade::healthy(self)
    }
}

impl PcmProducer for PlaybackHandle {
    fn prepare_pcm_f32(&self, samples: &[f32], source_spec: AudioSpec) -> PreparedPcm {
        PreparedPcm::from_source(samples, source_spec, self.device_spec_pub())
    }
    fn try_enqueue_prepared(&mut self, prepared: &mut PreparedPcm) -> TryEnqueue {
        PlaybackHandle::try_enqueue_prepared(self, prepared)
    }
    fn stop_and_clear(&self) -> u64 {
        PlaybackHandle::stop_and_clear(self)
    }
    fn is_drained(&self) -> bool {
        PlaybackHandle::is_drained(self)
    }
    fn healthy(&self) -> bool {
        PlaybackHandle::healthy(self)
    }
}
