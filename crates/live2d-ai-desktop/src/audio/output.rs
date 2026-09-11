//! cpal 设备 facade 与冒烟执行体。
//!
//! - [`AudioOutputFacade`]：持有 cpal 流与生产者句柄；Drop 时停止并释放音频流。
//! - [`DeviceSummary`]：观测/报告用的设备摘要。
//! - [`run_audio_smoke`]：`--audio-smoke` 执行体（生成 440 Hz 低音量正弦或静音，
//!   经完整链路播放并观测）。

use std::fmt;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use live2d_ai_runtime::AudioSpec;

use crate::audio::prepared::AudioSmokeReport;
use crate::audio::prepared::PreparedPcm;
use crate::audio::prepared::TryEnqueue;
use crate::audio::ring::PlaybackHandle;
use crate::audio::ring::detached_pair;
use crate::audio::{
    AudioOutputError, MouthSnapshot, OutputStats, f32_to_i16, f32_to_u16, passthrough_f32,
};

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
    /// 设备域业务规格（类型化来源，不经 DeviceSummary 反推）。
    device_spec: AudioSpec,
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

        // 流错误回调（非实时数据路径）：置位健康闩并记录（只置位不复位，
        // 复位/重开设备属上层策略 D8）。
        let fault_sink = handle.shared_state();
        let err_fn = move |err: cpal::Error| {
            fault_sink.mark_fault();
            tracing::warn!(%err, "audio output stream error（声卡健康闩已置位）");
        };

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
            device_spec,
            summary: DeviceSummary {
                device_name: device.to_string(),
                sample_format: format_label,
                sample_rate: device_spec.sample_rate(),
                channels: device_spec.channels(),
                ring_capacity_samples: capacity,
            },
        })
    }

    /// 设备域业务规格（prepared 转换目标域；类型化来源）。
    #[allow(dead_code)]
    pub fn device_spec(&self) -> AudioSpec {
        self.device_spec
    }

    /// 净化 + 一次性转换到设备域（P0-2 生产路径起点）。
    pub fn prepare_pcm_f32(&self, samples: &[f32], source_spec: AudioSpec) -> PreparedPcm {
        PreparedPcm::from_source(samples, source_spec, self.device_spec)
    }

    /// P0-2 生产入环入口。
    pub fn try_enqueue_prepared(&mut self, prepared: &mut PreparedPcm) -> TryEnqueue {
        self.handle.try_enqueue_prepared(prepared)
    }

    /// 声卡健康闩（D8 策略输入）。
    pub fn healthy(&self) -> bool {
        self.handle.healthy()
    }

    /// 只读窗口侧快照（D5）。
    pub fn mouth_snapshot(&self) -> MouthSnapshot {
        self.handle.mouth_snapshot()
    }

    /// 入队一批源域 f32 PCM（委托给 [`PlaybackHandle::enqueue_pcm_f32`]）。
    pub fn enqueue_pcm_f32(&mut self, samples: &[f32]) -> crate::audio::EnqueueOutcome {
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
