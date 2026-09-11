//! SPSC 环 + 共享无锁状态 + 生产/消费两端句柄。
//!
//! - [`SharedState`]：跨线程原子统计（含 epoch 打断代数、垃圾线、健康闩）。
//! - [`PlaybackHandle`]：生产/控制侧句柄（enqueue / stop / 查询）。
//! - [`MouthSnapshot`]：只读窗口侧快照（D5），可克隆。
//! - [`RenderCore`] + [`render_block`]：cpal 回调侧消费者（独占访问 ring cons）。
//! - [`detached_pair`] / [`detached_pair_with`]：脱离声卡的（生产者、消费者）对，
//!   供无设备环境确定性单测使用。

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use live2d_ai_runtime::{AudioSpec, RmsMeter};
use ringbuf::{
    HeapCons, HeapProd, HeapRb,
    traits::{Consumer, Observer, Producer, Split},
};

use crate::audio::prepared::{EnqueueOutcome, PreparedPcm, TryEnqueue};
use crate::audio::{
    AudioOutputError, RMS_ATTACK_SECS, RMS_RELEASE_SECS, RMS_WINDOW_MS, SCRATCH_SAMPLES,
};

/// 生产/消费两侧共享的无锁状态。全部原子量；回调侧只做 load/store/fetch_add。
#[derive(Debug, Default)]
pub struct SharedState {
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
    /// 健康闩：cpal 错误回调置位（设备失效/断连）；只置位不复位（D5/D8）。
    fault: AtomicBool,
}

impl SharedState {
    /// 标记声卡健康闩为已触发（cpal err_fn 置位用；D5/D8）。
    pub(crate) fn mark_fault(&self) {
        self.fault.store(true, Ordering::Release);
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
    /// 设备域业务规格（PcmProducer impl 透传用）。
    pub(crate) fn device_spec_pub(&self) -> AudioSpec {
        self.device_spec
    }

    /// 共享原子状态（cpal err_fn 闭包置位健康闩用）。
    pub(crate) fn shared_state(&self) -> Arc<SharedState> {
        Arc::clone(&self.shared)
    }

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
            live2d_ai_runtime::convert_spec(&clean, self.source_spec, self.device_spec)
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

    /// P0-2 生产入口：从 prepared.written 续写；整帧对齐；WouldBlock 零丢失。
    pub fn try_enqueue_prepared(&mut self, prepared: &mut PreparedPcm) -> TryEnqueue {
        let ch = usize::from(self.device_spec.channels()).max(1);
        let rest = &prepared.converted[prepared.written..];
        let mut accepted = 0usize;
        while accepted < rest.len() {
            let vacant_frames = self.prod.vacant_len() / ch;
            if vacant_frames == 0 {
                break;
            }
            let take = (rest.len() - accepted).min(vacant_frames * ch);
            let pushed = self.prod.push_slice(&rest[accepted..accepted + take]);
            accepted += pushed;
            if pushed < take {
                break;
            }
        }
        // 先写数据后登记序号（Release）：与 stop 线性化契约一致。
        self.shared
            .produced
            .fetch_add(accepted as u64, Ordering::Release);
        prepared.written += accepted;
        debug_assert_eq!(prepared.written % ch, 0, "prepared 游标必须保持帧对齐");
        if prepared.written >= prepared.converted.len() {
            TryEnqueue::Enqueued { accepted }
        } else {
            TryEnqueue::WouldBlock { accepted }
        }
    }

    /// 当前音频打断代数（观测用）。
    #[allow(dead_code)]
    pub fn audio_epoch(&self) -> u64 {
        self.shared.epoch.load(Ordering::Acquire)
    }

    /// 声卡健康闩：false = cpal 错误回调曾触发。
    pub fn healthy(&self) -> bool {
        !self.shared.fault.load(Ordering::Acquire)
    }

    /// 只读、可克隆的窗口侧快照（D5）。
    pub fn mouth_snapshot(&self) -> MouthSnapshot {
        MouthSnapshot {
            shared: Arc::clone(&self.shared),
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

/// 只读、可克隆的声卡观测快照（D5）：ShellApp 只持有本快照，
/// 不触碰 SPSC 生产者。访问器全部无锁原子读。
#[derive(Clone)]
pub struct MouthSnapshot {
    shared: Arc<SharedState>,
}

impl std::fmt::Debug for MouthSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MouthSnapshot")
            .field("level", &self.level())
            .finish_non_exhaustive()
    }
}

impl MouthSnapshot {
    /// 当前口型电平 ∈ [0,1]。
    #[allow(dead_code)]
    pub fn level(&self) -> f32 {
        f32::from_bits(self.shared.mouth_bits.load(Ordering::Relaxed))
    }
    /// 当前音频打断代数。
    #[allow(dead_code)]
    pub fn audio_epoch(&self) -> u64 {
        self.shared.epoch.load(Ordering::Acquire)
    }
    /// 声卡健康闩。
    #[allow(dead_code)]
    pub fn healthy(&self) -> bool {
        !self.shared.fault.load(Ordering::Acquire)
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

/// [`detached_pair`] 的测试入口（crate 内可见）：返回生产者句柄与「模拟声卡
/// 回调」的消费核心；节点 B 竞态测试按任意节奏驱动消费侧制造事实。
#[allow(dead_code)]
pub(crate) fn detached_pair_for_tests(
    cap: usize,
    source: AudioSpec,
    device: AudioSpec,
) -> (PlaybackHandle, RenderCore) {
    detached_pair(cap, source, device).expect("detached pair 构造失败")
}
