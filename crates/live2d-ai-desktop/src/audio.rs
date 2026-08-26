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

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use live2d_ai_runtime::{AudioSpec, RmsMeter, convert_spec};
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};

/// RMS 滑动窗口（毫秒）：约一个音节片长的能量平均。
const RMS_WINDOW_MS: f32 = 20.0;
/// RMS attack 时间常数（秒）：张嘴响应速度。
const RMS_ATTACK_SECS: f32 = 0.03;
/// RMS release 时间常数（秒）：闭嘴回落速度。
const RMS_RELEASE_SECS: f32 = 0.10;

/// 回调侧搬运暂存容量（样本数）：构造时一次分配，回调内只复用。
/// 处理超过该长度的回调缓冲时分块循环，任何情况下都不再分配。
const SCRATCH_SAMPLES: usize = 1024;

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

/// 生产/消费两侧共享的无锁状态。全部原子量；回调侧只做 load/store/fetch_add。
#[derive(Debug, Default)]
struct SharedState {
    /// 已生产（成功入环）的设备域样本总数——同时是环内样本的全局生产序号上限。
    produced: AtomicU64,
    /// 已被回调消费的样本总数。
    consumed: AtomicU64,
    /// 垃圾线：生产序号 ≤ 该值的样本应被回调静音（epoch 打断遗留）。
    garbage_upto: AtomicU64,
    /// 打断代数：每次 `stop_and_clear` +1（观测用）。
    epoch: AtomicU64,
    /// 当前 mouth level 的 f32 bits 快照（Relaxed 即可：单写单读的展示值）。
    mouth_bits: AtomicU32,
    /// 观测统计：欠载事件次数（曾活跃后队列断流）。
    underruns: AtomicU64,
    /// 观测统计：被 epoch 打断而静音的样本总数。
    interrupted_samples: AtomicU64,
}

/// 一次 [`PlaybackHandle::enqueue_pcm_f32`] 的明确结果（溢出可观测，不阻塞）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnqueueOutcome {
    /// 输入的源域样本数。
    pub source_samples: usize,
    /// 转换后的设备域样本数。
    pub device_samples: usize,
    /// 实际入环的设备域样本数（整帧对齐）。
    pub accepted: usize,
    /// 因环形缓冲满而被拒绝的设备域样本数 = `device_samples - accepted`。
    pub rejected: usize,
}

impl EnqueueOutcome {
    /// 是否有样本因溢出被拒绝。
    pub fn overflowed(&self) -> bool {
        self.rejected > 0
    }
}

/// 生产者侧句柄（TTS/控制线程持有）：入队、打断、查询。
pub struct PlaybackHandle {
    prod: HeapProd<f32>,
    shared: Arc<SharedState>,
    source_spec: AudioSpec,
    device_spec: AudioSpec,
}

impl fmt::Debug for PlaybackHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlaybackHandle")
            .field("source_spec", &self.source_spec)
            .field("device_spec", &self.device_spec)
            .field("shared", &self.shared)
            .finish_non_exhaustive()
    }
}

impl PlaybackHandle {
    /// 把源规格 PCM 写入环形缓冲（内部完成设备域转换）。
    ///
    /// - 非有限输入样本一律先净化为 0（防垃圾进 DAC/RMS）；
    /// - 源/设备规格一致时跳过转换（纯透传）；
    /// - 只接纳整帧前缀，剩余计入 [`EnqueueOutcome::rejected`]；不阻塞。
    ///
    /// 需要 `&mut`：ringbuf Producer 的无锁快速路径缓存写指针
    /// （单生产者语义；跨线程共享由调用方外层串行化）。
    pub fn enqueue_pcm_f32(&mut self, samples: &[f32]) -> EnqueueOutcome {
        let clean: Vec<f32> = samples
            .iter()
            .map(|&s| if s.is_finite() { s } else { 0.0 })
            .collect();
        let converted = if self.source_spec == self.device_spec {
            clean
        } else {
            convert_spec(&clean, self.source_spec, self.device_spec)
        };
        let ch = usize::from(self.device_spec.channels()).max(1);

        // 整帧对齐地填满可用空间：vacant 只会因消费增加（唯一生产者是我们），
        // 因此预留量不会被竞争者偷走。
        let mut accepted = 0usize;
        while accepted < converted.len() {
            let vacant_frames = self.prod.vacant_len() / ch;
            if vacant_frames == 0 {
                break;
            }
            let take = (converted.len() - accepted).min(vacant_frames * ch);
            let pushed = self.prod.push_slice(&converted[accepted..accepted + take]);
            accepted += pushed;
            if pushed < take {
                break; // 防御：意外短写即停，绝不多写半个帧
            }
        }

        // 先写数据后登记序号（Release）：stop_and_clear 观察到 produced 时，
        // 对应环内数据必然可见。
        self.shared
            .produced
            .fetch_add(accepted as u64, Ordering::Release);

        EnqueueOutcome {
            source_samples: samples.len(),
            device_samples: converted.len(),
            accepted,
            rejected: converted.len() - accepted,
        }
    }

    /// epoch 打断：推进垃圾线并递增代数。此后环内全部旧样本由回调静音，
    /// 新入队样本照常播放。返回打断后的代数值（观测用）。
    ///
    /// 与并发入队的线性化点见模块文档；重试保证垃圾线 ≥ 快照时刻的全部存量。
    pub fn stop_and_clear(&self) -> u64 {
        loop {
            let snapshot = self.shared.produced.load(Ordering::Acquire);
            self.shared
                .garbage_upto
                .fetch_max(snapshot, Ordering::AcqRel);
            let epoch = self.shared.epoch.fetch_add(1, Ordering::AcqRel) + 1;
            if self.shared.produced.load(Ordering::Acquire) == snapshot {
                return epoch;
            }
        }
    }

    /// 环内待播样本是否已全部被消费（含被打断静音的部分）。
    pub fn is_drained(&self) -> bool {
        let p = self.shared.produced.load(Ordering::Acquire);
        let c = self.shared.consumed.load(Ordering::Acquire);
        p <= c
    }

    /// 当前 mouth level ∈ [0, 1]（无锁 f32-bits 快照；回调线程单写）。
    pub fn mouth_level(&self) -> f32 {
        f32::from_bits(self.shared.mouth_bits.load(Ordering::Relaxed))
    }

    /// 观测统计快照。
    pub fn stats(&self) -> OutputStats {
        let produced = self.shared.produced.load(Ordering::Acquire);
        let consumed = self.shared.consumed.load(Ordering::Acquire);
        OutputStats {
            produced_samples: produced,
            consumed_samples: consumed,
            pending_samples: produced.saturating_sub(consumed),
            interrupted_samples: self.shared.interrupted_samples.load(Ordering::Acquire),
            underruns: self.shared.underruns.load(Ordering::Acquire),
            epoch: self.shared.epoch.load(Ordering::Acquire),
            mouth_level: self.mouth_level(),
        }
    }
}

/// [`PlaybackHandle::stats`] 的观测快照。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputStats {
    /// 累计入环样本数（设备域）。
    pub produced_samples: u64,
    /// 累计消费样本数（设备域）。
    pub consumed_samples: u64,
    /// 当前待播样本数。
    pub pending_samples: u64,
    /// 被 epoch 打断而静音的累计样本数。
    pub interrupted_samples: u64,
    /// 累计欠载事件数。
    pub underruns: u64,
    /// 当前打断代数。
    pub epoch: u64,
    /// 当前 mouth level ∈ [0, 1]。
    pub mouth_level: f32,
}

/// 消费者侧状态（cpal 回调线程独占）：出环、打断静音、欠载补零、RMS 快照。
///
/// 构造时一次性分配 `scratch`；[`Self::render_into`] 及其调用路径均不分配、
/// 不加锁、不打日志。热路径经底部自由函数 `render_block` 做字段级拆借。
pub struct RenderCore {
    cons: HeapCons<f32>,
    meter: RmsMeter,
    shared: Arc<SharedState>,
    scratch: Box<[f32]>,
    /// 本消费者的已消费样本计数（本地镜像，用于垃圾线区间判定）。
    consumed: u64,
    /// 上个回调是否交付了完整真实音频块（欠载事件判定）。
    was_active: bool,
}

impl fmt::Debug for RenderCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderCore")
            .field("consumed", &self.consumed)
            .field("was_active", &self.was_active)
            .field("scratch_len", &self.scratch.len())
            .finish_non_exhaustive()
    }
}

impl RenderCore {
    /// 以 f32 视角渲染单个回调缓冲（实际声卡将收到的最终值）。
    ///
    /// 直接回调入口；生产路径经 [`Self::render_into`]（借用拆分后调用
    /// `render_block`）。保留为单块 API 供测试与未来的直接回调装配。
    #[allow(dead_code)]
    pub(crate) fn render_f32(&mut self, out: &mut [f32]) {
        render_block(
            &mut self.cons,
            &mut self.meter,
            &self.shared,
            &mut self.consumed,
            &mut self.was_active,
            out,
        );
    }

    /// 渲染任意设备样本格式的回调缓冲：分块借道预分配 scratch，逐样本转换。
    /// `convert` 必须是纯函数（不得分配/加锁）。
    pub(crate) fn render_into<T: Copy>(&mut self, out: &mut [T], convert: fn(f32) -> T) {
        for chunk in out.chunks_mut(self.scratch.len()) {
            // 字段级拆借：scratch 与其余状态互不相交，块内循环复用同一分配。
            let buf = &mut self.scratch[..chunk.len()];
            render_block(
                &mut self.cons,
                &mut self.meter,
                &self.shared,
                &mut self.consumed,
                &mut self.was_active,
                buf,
            );
            for (dst, &src) in chunk.iter_mut().zip(buf.iter()) {
                *dst = convert(src);
            }
        }
    }
}

/// 单个回调块的完整处理（自由函数以便字段级拆借）：出环 → 打断静音前缀 →
/// 欠载补 0 → RMS（对最终输出值）→ 无锁快照。不分配、不加锁、不打日志。
fn render_block(
    cons: &mut HeapCons<f32>,
    meter: &mut RmsMeter,
    shared: &SharedState,
    consumed: &mut u64,
    was_active: &mut bool,
    out: &mut [f32],
) {
    let n = out.len();
    let got = cons.pop_slice(out);
    if got > 0 {
        *consumed += got as u64;
        shared.consumed.fetch_add(got as u64, Ordering::AcqRel);
    }

    // epoch 打断：FIFO 前缀中生产序号 ≤ 垃圾线的样本替换为静音。
    // 环是 FIFO，生产序号沿读取方向单调递增 ⇒ 陈旧部分必为本块前缀。
    let garbage_line = shared.garbage_upto.load(Ordering::Acquire);
    let block_start = *consumed - got as u64;
    let stale = garbage_line.saturating_sub(block_start).min(got as u64) as usize;
    if stale > 0 {
        out.copy_within(stale..got, 0);
        shared
            .interrupted_samples
            .fetch_add(stale as u64, Ordering::Relaxed);
    }
    let audible = got - stale;

    if audible == n {
        *was_active = true;
    } else {
        // 欠载补 0（或打断余量补 0）；仅「曾活跃后断流」记一次欠载事件。
        if got < n && *was_active {
            shared.underruns.fetch_add(1, Ordering::Relaxed);
        }
        *was_active = false;
        out[audible..].fill(0.0);
    }

    // RMS 只对最终写给声卡的值计算（含补零与打断静音）→ 口型不抢跑。
    let level = meter.push(out);
    shared.mouth_bits.store(level.to_bits(), Ordering::Release);
}

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

/// 构造一对脱离声卡的（生产者、消费者），供无设备环境的确定性单测使用。
pub(crate) fn detached_pair(
    ring_capacity_samples: usize,
    source_spec: AudioSpec,
    device_spec: AudioSpec,
) -> Result<(PlaybackHandle, RenderCore), AudioOutputError> {
    detached_pair_with(
        ring_capacity_samples,
        source_spec,
        device_spec,
        RMS_WINDOW_MS,
        RMS_ATTACK_SECS,
        RMS_RELEASE_SECS,
    )
}

/// [`detached_pair`] 的全参数版本（测试可注入即时平滑等参数）。
pub(crate) fn detached_pair_with(
    ring_capacity_samples: usize,
    source_spec: AudioSpec,
    device_spec: AudioSpec,
    window_ms: f32,
    attack_secs: f32,
    release_secs: f32,
) -> Result<(PlaybackHandle, RenderCore), AudioOutputError> {
    if ring_capacity_samples == 0 {
        return Err(AudioOutputError::InvalidInput(
            "环形缓冲容量必须非零".to_string(),
        ));
    }
    let rb = HeapRb::<f32>::new(ring_capacity_samples);
    let (prod, cons) = rb.split();
    let shared = Arc::new(SharedState::default());
    let meter = RmsMeter::new(device_spec, window_ms, attack_secs, release_secs)
        .map_err(|e| AudioOutputError::InvalidInput(format!("RMS 表参数非法: {e}")))?;
    let handle = PlaybackHandle {
        prod,
        shared: Arc::clone(&shared),
        source_spec,
        device_spec,
    };
    let render = RenderCore {
        cons,
        meter,
        shared,
        scratch: vec![0.0; SCRATCH_SAMPLES].into_boxed_slice(),
        consumed: 0,
        was_active: false,
    };
    Ok((handle, render))
}

/// 设备摘要（观测/报告用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSummary {
    /// 输出设备名。
    pub device_name: String,
    /// 设备样本格式标签（"f32"/"i16"/"u16"）。
    pub sample_format: &'static str,
    /// 实际协商采样率（Hz）。
    pub sample_rate: u32,
    /// 实际声道数。
    pub channels: u16,
    /// 环形缓冲容量（样本数）。
    pub ring_capacity_samples: usize,
}

/// 实际声卡输出 facade：持有 cpal 流与生产者句柄。
///
/// Drop 时停止并释放音频流（cpal `Stream` 的 drop 语义）。
pub struct AudioOutputFacade {
    /// 持有流即持有播放生命周期：drop 时停止回调线程（RAII，无读取方）。
    #[allow(dead_code)]
    stream: cpal::Stream,
    handle: PlaybackHandle,
    summary: DeviceSummary,
}

impl fmt::Debug for AudioOutputFacade {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioOutputFacade")
            .field("handle", &self.handle)
            .field("summary", &self.summary)
            .finish_non_exhaustive()
    }
}

impl AudioOutputFacade {
    /// 用默认输出设备的默认 config 打开播放流。
    ///
    /// - `source_spec`：入队 PCM 的源规格（如 TTS 默认 24 kHz mono）；
    /// - `buffering`：环形缓冲目标时长（按设备域换算容量，≥10 ms）。
    ///
    /// 无默认设备/配置不可用 → [`AudioOutputError::NoDevice`] /
    /// [`AudioOutputError::Config`]（环境类错误）。
    pub fn open_default(
        source_spec: AudioSpec,
        buffering: Duration,
    ) -> Result<Self, AudioOutputError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioOutputError::NoDevice("系统未提供默认输出设备".to_string()))?;
        let supported = device
            .default_output_config()
            .map_err(|e| AudioOutputError::Config(format!("读取默认输出配置失败: {e}")))?;

        let sample_format = supported.sample_format();
        let format_label: &'static str = match sample_format {
            cpal::SampleFormat::F32 => "f32",
            cpal::SampleFormat::I16 => "i16",
            cpal::SampleFormat::U16 => "u16",
            other => {
                return Err(AudioOutputError::UnsupportedSampleFormat(other.to_string()));
            }
        };
        let config: cpal::StreamConfig = supported.into();
        // cpal 0.18：SampleRate/ChannelCount 是 u32/u16 别名（非 newtype）。
        let device_spec = AudioSpec::new(config.sample_rate, config.channels)
            .map_err(|e| AudioOutputError::Config(format!("设备规格非法: {e}")))?;
        let buffering_ms = buffering.as_secs_f64() * 1_000.0;
        if !(buffering_ms.is_finite() && buffering_ms >= 10.0) {
            return Err(AudioOutputError::InvalidInput(format!(
                "buffering 必须 ≥ 10 ms，得到 {buffering_ms:.3} ms"
            )));
        }
        let frames = (device_spec.sample_rate() as f64 * buffering.as_secs_f64()).ceil() as u64;
        let capacity =
            (frames * u64::from(device_spec.channels())).min(u64::from(u32::MAX)) as usize;
        let (handle, mut render) = detached_pair(capacity, source_spec, device_spec)?;

        // 流错误回调（非实时数据路径）：仅记录，供诊断。
        let err_fn = |err: cpal::Error| tracing::warn!(%err, "audio output stream error");

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream::<f32, _, _>(
                config,
                move |data: &mut [f32], _| render.render_into(data, passthrough_f32),
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => device.build_output_stream::<i16, _, _>(
                config,
                move |data: &mut [i16], _| render.render_into(data, f32_to_i16),
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => device.build_output_stream::<u16, _, _>(
                config,
                move |data: &mut [u16], _| render.render_into(data, f32_to_u16),
                err_fn,
                None,
            ),
            other => unreachable!("format_label 分支已拦截其它格式: {other}"),
        }
        .map_err(|e| AudioOutputError::Stream(format!("{e}")))?;
        stream
            .play()
            .map_err(|e| AudioOutputError::Stream(format!("{e}")))?;

        Ok(Self {
            stream,
            handle,
            summary: DeviceSummary {
                device_name: device.to_string(),
                sample_format: format_label,
                sample_rate: device_spec.sample_rate(),
                channels: device_spec.channels(),
                ring_capacity_samples: capacity,
            },
        })
    }

    /// 入队一批源域 f32 PCM（委托给 [`PlaybackHandle::enqueue_pcm_f32`]）。
    pub fn enqueue_pcm_f32(&mut self, samples: &[f32]) -> EnqueueOutcome {
        self.handle.enqueue_pcm_f32(samples)
    }

    /// epoch 打断（委托给 [`PlaybackHandle::stop_and_clear`]）。
    pub fn stop_and_clear(&self) -> u64 {
        self.handle.stop_and_clear()
    }

    /// 是否已排空（委托给 [`PlaybackHandle::is_drained`]）。
    pub fn is_drained(&self) -> bool {
        self.handle.is_drained()
    }

    /// 当前 mouth level（委托给 [`PlaybackHandle::mouth_level`]）。
    pub fn mouth_level(&self) -> f32 {
        self.handle.mouth_level()
    }

    /// 观测统计快照（委托给 [`PlaybackHandle::stats`]）。
    pub fn stats(&self) -> OutputStats {
        self.handle.stats()
    }

    /// 设备摘要。
    pub fn summary(&self) -> &DeviceSummary {
        &self.summary
    }
}

/// `--audio-smoke` 冒烟报告（逐行摘要供 CLI 打印）。
#[derive(Debug, Clone)]
pub struct AudioSmokeReport {
    /// 设备摘要（None 表示环境无设备，本次为 skip 语义）。
    pub device: Option<DeviceSummary>,
    /// 源规格（TTS 默认域）。
    pub source_spec: AudioSpec,
    /// 生成的源域样本总数。
    pub generated_source_samples: usize,
    /// 实际接受的设备域样本总数。
    pub accepted_device_samples: u64,
    /// 因溢出被拒的设备域样本总数。
    pub rejected_device_samples: u64,
    /// 排空等待结果：true = 在超时前排空。
    pub drained: bool,
    /// 排空等待耗时。
    pub drain_waited: Duration,
    /// 播放期间观测到的 mouth level 峰值。
    pub peak_mouth: f32,
    /// 结束时的 mouth level。
    pub final_mouth: f32,
    /// 欠载事件数。
    pub underruns: u64,
    /// 被打断静音的样本数（冒烟流程不打断，恒 0）。
    pub interrupted_samples: u64,
}

impl AudioSmokeReport {
    /// 逐行摘要（CLI 打印用）。
    pub fn summarize_lines(&self) -> Vec<String> {
        match &self.device {
            None => vec!["audio-smoke     : 无音频设备（skip，如实记录）".to_string()],
            Some(dev) => {
                let outcome = if self.drained { "drained" } else { "TIMEOUT" };
                vec![
                    format!(
                        "device          : {} ({}, {} Hz × {} ch)",
                        dev.device_name, dev.sample_format, dev.sample_rate, dev.channels
                    ),
                    format!(
                        "specs           : source {} Hz × {} ch → device {} Hz × {} ch",
                        self.source_spec.sample_rate(),
                        self.source_spec.channels(),
                        dev.sample_rate,
                        dev.channels
                    ),
                    format!(
                        "enqueued        : generated={} src-samples, accepted={} dev-samples, rejected={}",
                        self.generated_source_samples,
                        self.accepted_device_samples,
                        self.rejected_device_samples
                    ),
                    format!(
                        "mouth           : peak={:.4} final={:.4}",
                        self.peak_mouth, self.final_mouth
                    ),
                    format!(
                        "health          : {} in {:.1?}, underruns={}, interrupted={}",
                        outcome, self.drain_waited, self.underruns, self.interrupted_samples
                    ),
                    format!(
                        "ring            : capacity={} samples",
                        dev.ring_capacity_samples
                    ),
                ]
            }
        }
    }
}

/// `--audio-smoke` 执行体：生成短促 440 Hz 低音量正弦（或数字静音），
/// 经完整链路（重采样/映射 → SPSC 环 → 回调 → RMS）播放并观测。
///
/// 返回 [`AudioSmokeReport`]；无设备时 `device=None` 且不视为代码错误。
pub fn run_audio_smoke(
    duration: Duration,
    silence: bool,
) -> Result<AudioSmokeReport, AudioOutputError> {
    let source_spec = AudioSpec::default(); // 与 TTS 默认一致：24 kHz / mono

    // 先探测设备可得性：无设备直接如实返回 skip 报告（不当作错误）。
    let host = cpal::default_host();
    let Some(_probe) = host.default_output_device() else {
        return Ok(AudioSmokeReport {
            device: None,
            source_spec,
            generated_source_samples: 0,
            accepted_device_samples: 0,
            rejected_device_samples: 0,
            drained: false,
            drain_waited: Duration::ZERO,
            peak_mouth: 0.0,
            final_mouth: 0.0,
            underruns: 0,
            interrupted_samples: 0,
        });
    };
    drop(_probe);

    // 环形缓冲按 min(时长, 250 ms) + 余量的目标缓冲打开；生成端按 ~20 ms
    // 步进节流喂入，模拟真实 TTS 流式到达。
    let buffering = duration.min(Duration::from_millis(250)) + Duration::from_millis(50);
    let mut facade = AudioOutputFacade::open_default(source_spec, buffering)?;
    let summary = facade.summary().clone();

    // 生成 + 节流入队（相位连续的正弦，幅度 0.05 低音量）。
    let src_rate = source_spec.sample_rate() as f32;
    let freq = 440.0_f32;
    let amp = 0.05_f32;
    let total_frames = (src_rate * duration.as_secs_f32()).round().max(1.0) as u64;
    let chunk_frames = (src_rate * 0.020).round() as usize; // ≈20 ms
    let mut phase = 0.0_f32;
    let phase_step = std::f32::consts::TAU * freq / src_rate;
    let mut next_frame = 0_u64;
    let mut accepted_total = 0_u64;
    let mut rejected_total = 0_u64;
    let deadline = Instant::now() + duration + Duration::from_secs(10);

    while next_frame < total_frames && Instant::now() < deadline {
        let frames = ((total_frames - next_frame) as usize).min(chunk_frames);
        let chunk: Vec<f32> = if silence {
            vec![0.0; frames]
        } else {
            (0..frames)
                .map(|_| {
                    let v = phase.sin() * amp;
                    phase += phase_step;
                    v
                })
                .collect()
        };
        next_frame += frames as u64;

        let outcome = facade.enqueue_pcm_f32(&chunk);
        accepted_total += outcome.accepted as u64;
        rejected_total += outcome.rejected as u64;
        if outcome.overflowed() {
            // 溢出：让回调先消化，再继续生成（不阻塞、不丢弃计数）。
            std::thread::sleep(Duration::from_millis(5));
        }
        // 环接近满时让回调消化一下；空转等待也受 deadline 保护。
        while facade.stats().pending_samples > chunk_frames as u64 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    // 生成提前截止（deadline 触发）且仍有存量时，epoch 打断确保不拖尾音。
    if !facade.is_drained() && Instant::now() >= deadline {
        facade.stop_and_clear();
    }

    // 等排空并采样峰值 mouth level。
    let drain_started = Instant::now();
    let drain_deadline = drain_started + duration.mul_f64(2.0) + Duration::from_secs(2);
    let mut peak_mouth = facade.mouth_level();
    while !facade.is_drained() && Instant::now() < drain_deadline {
        peak_mouth = peak_mouth.max(facade.mouth_level());
        std::thread::sleep(Duration::from_millis(5));
    }
    let drained = facade.is_drained();
    // 收尾：再多观察一拍，确认电平回落。
    let settle = drain_deadline.saturating_duration_since(Instant::now());
    std::thread::sleep(settle.min(Duration::from_millis(120)));
    peak_mouth = peak_mouth.max(facade.mouth_level());
    let final_stats = facade.stats();

    Ok(AudioSmokeReport {
        device: Some(summary),
        source_spec,
        generated_source_samples: next_frame as usize,
        accepted_device_samples: accepted_total,
        rejected_device_samples: rejected_total,
        drained,
        drain_waited: drain_started.elapsed(),
        peak_mouth,
        final_mouth: facade.mouth_level(),
        underruns: final_stats.underruns,
        interrupted_samples: final_stats.interrupted_samples,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mono(rate: u32) -> AudioSpec {
        AudioSpec::new(rate, 1).expect("valid")
    }

    fn stereo(rate: u32) -> AudioSpec {
        AudioSpec::new(rate, 2).expect("valid")
    }

    fn pair(cap: usize, src: AudioSpec, dev: AudioSpec) -> (PlaybackHandle, RenderCore) {
        detached_pair(cap, src, dev).expect("valid pair")
    }

    #[test]
    fn zero_ring_capacity_is_invalid_input() {
        let err = detached_pair(0, mono(24_000), mono(48_000)).unwrap_err();
        assert!(matches!(err, AudioOutputError::InvalidInput(_)));
        assert!(!err.is_environment());
        assert_eq!(err.to_string(), "音频输出参数非法: 环形缓冲容量必须非零");
    }

    #[test]
    fn format_converters_map_extremes_deterministically() {
        // i16：对称刻度 ×32768、round、正上界夹持 32767、非有限 → 0。
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(1.0), 32_767);
        assert_eq!(f32_to_i16(-1.0), -32_768);
        assert_eq!(f32_to_i16(42.0), 32_767, "越界 clamp 到上界");
        assert_eq!(f32_to_i16(-42.0), -32_768);
        assert_eq!(f32_to_i16(f32::NAN), 0);
        assert_eq!(
            f32_to_i16(f32::INFINITY),
            0,
            "非有限值（含 ±∞）一律按垃圾输入归零，上游已净化"
        );
        // u16：bipolar 偏移刻度（i16 异或 0x8000）：-1→0、0→32768、+1→65535。
        assert_eq!(f32_to_u16(-1.0), 0);
        assert_eq!(f32_to_u16(0.0), 32_768);
        assert_eq!(f32_to_u16(1.0), u16::MAX);
        assert_eq!(f32_to_u16(9.0), u16::MAX);
        assert_eq!(f32_to_u16(-9.0), 0);
        assert_eq!(f32_to_u16(f32::NAN), 32_768, "非有限按 0 电平处理");
    }

    #[test]
    fn passthrough_enqueue_render_matches_and_reports_drained() {
        let (mut h, mut r) = pair(16, mono(8_000), mono(8_000));
        let outcome = h.enqueue_pcm_f32(&[0.1, -0.2, 0.3]);
        assert_eq!(
            outcome,
            EnqueueOutcome {
                source_samples: 3,
                device_samples: 3,
                accepted: 3,
                rejected: 0,
            }
        );
        assert!(!h.is_drained());

        let mut buf = [0.0_f32; 4];
        r.render_f32(&mut buf);
        assert_eq!(&buf[..3], &[0.1, -0.2, 0.3], "FIFO 顺序逐位一致");
        assert_eq!(buf[3], 0.0, "欠载槽补 0");
        assert!(h.is_drained());
        // 空闲期（从未活跃过）不计欠载事件。
        assert_eq!(h.stats().underruns, 0);
    }

    #[test]
    fn resample_and_channel_mapping_flow_through_ring() {
        // 源 mono 8 kHz → 设备 stereo 16 kHz：与 runtime 参考实现逐位一致。
        let (mut h, mut r) = pair(64, mono(8_000), stereo(16_000));
        let input = vec![0.0_f32, 1.0, 0.0, -1.0];
        let expected = convert_spec(&input, mono(8_000), stereo(16_000));
        let outcome = h.enqueue_pcm_f32(&input);
        assert_eq!(outcome.accepted, expected.len());

        let mut buf = vec![f32::NAN; expected.len() + 4];
        r.render_f32(&mut buf);
        assert_eq!(&buf[..expected.len()], &expected[..], "与参考转换逐位一致");
        for pair_samples in buf[..expected.len()].chunks(2) {
            assert_eq!(pair_samples[0], pair_samples[1], "L==R 帧对齐");
        }
        assert!(buf[expected.len()..].iter().all(|v| *v == 0.0));
        assert!(h.is_drained());
    }

    #[test]
    fn mouth_level_snapshot_rises_with_tone_and_snaps_to_zero_in_silence() {
        let (mut h, mut r) =
            detached_pair_with(4096, mono(48_000), mono(48_000), 10.0, 0.0, 0.0).expect("valid");
        assert_eq!(h.mouth_level(), 0.0);

        // 恒幅 ±0.5 → RMS = 0.5；attack=release=0 立即贴合。
        let tone = [0.5_f32, -0.5].repeat(480); // 20 ms @48k
        h.enqueue_pcm_f32(&tone);
        let mut buf = vec![0.0_f32; tone.len()];
        r.render_f32(&mut buf);
        let level = h.mouth_level();
        assert!((level - 0.5).abs() < 1e-5, "快照应 ≈0.5，得到 {level}");

        // 静音块后精确回 0（silence snap）——口型闭合可观测。
        h.enqueue_pcm_f32(&vec![0.0_f32; 960]);
        let mut silence = vec![0.0_f32; 960];
        r.render_f32(&mut silence);
        assert_eq!(h.mouth_level(), 0.0, "静音后必须精确归零");
    }

    #[test]
    fn stop_and_clear_silences_old_audio_then_admits_new_epoch_audio() {
        let (mut h, mut r) = pair(128, mono(8_000), mono(8_000));

        // 旧 epoch：40 个满幅样本入环。
        h.enqueue_pcm_f32(&[0.5_f32; 40]);
        let epoch = h.stop_and_clear();
        assert_eq!(epoch, 1);

        // 打断后的回调不得再输出任何旧音频。
        let mut buf = [0.0_f32; 64];
        r.render_f32(&mut buf);
        assert!(buf.iter().all(|&v| v == 0.0), "打断后旧音频必须为静音");
        assert_eq!(h.mouth_level(), 0.0);
        let stats = h.stats();
        assert_eq!(stats.interrupted_samples, 40);
        assert_eq!(stats.epoch, 1);

        // 新 epoch 音频立即生效（生产序号已越过垃圾线）。
        h.enqueue_pcm_f32(&[0.25_f32; 8]);
        let mut fresh = [f32::NAN; 16];
        r.render_f32(&mut fresh);
        assert_eq!(&fresh[..8], &vec![0.25_f32; 8][..]);
        assert!(fresh[8..].iter().all(|&v| v == 0.0));

        // 再次打断代数继续递增。
        assert_eq!(h.stop_and_clear(), 2);
    }

    #[test]
    fn stop_before_any_playback_leaves_nothing_for_callback() {
        let (mut h, mut r) = pair(32, mono(8_000), mono(8_000));
        // 未入队直接打断是干净的无操作。
        assert_eq!(h.stop_and_clear(), 1);
        assert!(h.is_drained());
        let mut buf = [f32::NAN; 8];
        r.render_f32(&mut buf);
        assert!(buf.iter().all(|&v| v == 0.0));

        // 打断后再入队 → 属于新 epoch，正常播放。
        h.enqueue_pcm_f32(&[0.5_f32; 4]);
        let mut out = [0.0_f32; 4];
        r.render_f32(&mut out);
        assert_eq!(out, [0.5_f32; 4]);
    }

    #[test]
    fn overflow_rejects_exact_count_without_blocking_or_corruption() {
        let (mut h, mut r) = pair(8, mono(8_000), mono(8_000));
        let input: Vec<f32> = (0..20_i16).map(f32::from).collect();
        let outcome = h.enqueue_pcm_f32(&input);
        assert_eq!(outcome.source_samples, 20);
        assert_eq!(outcome.device_samples, 20);
        assert_eq!(outcome.accepted, 8, "只接纳整帧对齐前缀（mono 即样本）");
        assert_eq!(outcome.rejected, 12);
        assert!(outcome.overflowed());

        // 满环再写：全部拒绝且不阻塞。
        let again = h.enqueue_pcm_f32(&[9.9_f32]);
        assert_eq!((again.accepted, again.rejected), (0, 1));

        // 已接纳部分 FIFO 完好（渲染消费后环回到空）。
        let mut out = [f32::NAN; 8];
        r.render_f32(&mut out);
        assert_eq!(&out, &input[..8], "已接纳部分 FIFO 完好");
        assert!(h.is_drained());
    }

    #[test]
    fn ring_capacity_never_breaks_frame_alignment() {
        // 容量 5 与 stereo 设备：任何时刻环内样本都必须保持整帧。
        let (mut h, _r) = pair(5, mono(8_000), stereo(8_000));
        let first = h.enqueue_pcm_f32(&[0.25_f32, 0.5]); // → 4 个设备域样本
        assert_eq!(first.device_samples, 4);
        assert_eq!(first.accepted, 4, "容量 5 只容 2 整帧");

        // 只剩 1 个散位：宁可全拒也不写半个帧。
        let second = h.enqueue_pcm_f32(&[0.25_f32]);
        assert_eq!(second.device_samples, 2);
        assert_eq!((second.accepted, second.rejected), (0, 2));

        let stats = h.stats();
        assert_eq!(stats.pending_samples, 4);
    }

    #[test]
    fn render_into_adapters_convert_device_formats() {
        let values = [1.0_f32, 0.0, -1.0];

        let (mut h, mut r) = pair(16, mono(8_000), mono(8_000));
        h.enqueue_pcm_f32(&values);
        let mut i16s = [0_i16; 3];
        r.render_into(&mut i16s, f32_to_i16);
        assert_eq!(i16s, [32_767, 0, -32_768]);

        let (mut h, mut r) = pair(16, mono(8_000), mono(8_000));
        h.enqueue_pcm_f32(&values);
        let mut u16s = [0_u16; 3];
        r.render_into(&mut u16s, f32_to_u16);
        // bipolar 偏移刻度：+1.0 → 0xFFFF、0 → 0x8000、-1.0 → 0x0000。
        assert_eq!(u16s, [u16::MAX, 32_768, 0]);

        // f32 直通适配器不改变数值。
        let (mut h, mut r) = pair(16, mono(8_000), mono(8_000));
        h.enqueue_pcm_f32(&values);
        let mut f32s = [0.0_f32; 3];
        r.render_into(&mut f32s, passthrough_f32);
        assert_eq!(f32s, values);
    }

    #[test]
    fn underruns_are_counted_only_after_active_starvation() {
        let (mut h, mut r) = pair(64, mono(8_000), mono(8_000));
        // 空转阶段不算欠载。
        let mut idle = [0.0_f32; 8];
        r.render_f32(&mut idle);
        assert_eq!(h.stats().underruns, 0);

        // 播放一段后断流 → 记一次欠载事件；持续空转不再累计。
        h.enqueue_pcm_f32(&[0.5_f32; 4]);
        let mut play = [0.0_f32; 4];
        r.render_f32(&mut play);
        assert_eq!(play, [0.5_f32; 4]);
        let mut starved = [f32::NAN; 4];
        r.render_f32(&mut starved);
        assert!(starved.iter().all(|&v| v == 0.0));
        assert_eq!(h.stats().underruns, 1);
        let mut still_idle = [0.0_f32; 4];
        r.render_f32(&mut still_idle);
        assert_eq!(h.stats().underruns, 1, "连续空闲不重复计数");
    }

    #[test]
    fn non_finite_input_is_sanitized_before_entering_ring() {
        let (mut h, mut r) = pair(16, mono(8_000), mono(8_000));
        h.enqueue_pcm_f32(&[f32::NAN, 0.25, f32::INFINITY, -0.25]);
        let mut out = [1.0_f32; 4];
        r.render_f32(&mut out);
        assert_eq!(out, [0.0, 0.25, 0.0, -0.25], "非有限值一律净化为 0");
    }

    #[test]
    fn smoke_report_lines_cover_device_skip_honestly() {
        let report = AudioSmokeReport {
            device: None,
            source_spec: AudioSpec::default(),
            generated_source_samples: 0,
            accepted_device_samples: 0,
            rejected_device_samples: 0,
            drained: false,
            drain_waited: Duration::ZERO,
            peak_mouth: 0.0,
            final_mouth: 0.0,
            underruns: 0,
            interrupted_samples: 0,
        };
        let lines = report.summarize_lines();
        assert!(lines.iter().any(|l| l.contains("无音频设备")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("skip")), "{lines:?}");

        let with_device = AudioSmokeReport {
            device: Some(DeviceSummary {
                device_name: "TestDAC".to_string(),
                sample_format: "f32",
                sample_rate: 48_000,
                channels: 2,
                ring_capacity_samples: 9_600,
            }),
            generated_source_samples: 19_200,
            accepted_device_samples: 76_800,
            rejected_device_samples: 0,
            drained: true,
            drain_waited: Duration::from_millis(830),
            peak_mouth: 0.049_9,
            final_mouth: 0.0,
            ..report
        };
        let lines = with_device.summarize_lines().join("\n");
        assert!(lines.contains("TestDAC"));
        assert!(lines.contains("24000 Hz × 1 ch"), "源规格可见");
        assert!(lines.contains("48000 Hz × 2 ch"));
        assert!(lines.contains("rejected=0"));
        assert!(lines.contains("drained in"));
    }

    #[test]
    fn error_display_is_observable_vocabulary() {
        assert!(AudioOutputError::NoDevice("x".into()).is_environment());
        assert!(AudioOutputError::Config("alsa".into()).is_environment());
        assert!(!AudioOutputError::Stream("boom".into()).is_environment());
        assert!(!AudioOutputError::UnsupportedSampleFormat("i64".into()).is_environment());
        assert_eq!(
            AudioOutputError::UnsupportedSampleFormat("i64".into()).to_string(),
            "设备样本格式不支持: i64（支持 f32/i16/u16）"
        );
    }
}
