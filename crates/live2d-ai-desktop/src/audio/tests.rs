//! 从原 `audio.rs` 整体迁出的 `#[cfg(test)]` 模块。
//!
//! 公共 API 表面（`crate::audio::X`）完全保持原样，调用方无感知。

use std::time::Duration;

use live2d_ai_runtime::{AudioSpec, convert_spec};

use crate::audio::ring::{detached_pair as detached_pair_pub, detached_pair_with};
use crate::audio::{
    AudioOutputError, AudioSmokeReport, DeviceSummary, EnqueueOutcome, PlaybackHandle, RenderCore,
    f32_to_i16, f32_to_u16, passthrough_f32,
};

fn mono(rate: u32) -> AudioSpec {
    AudioSpec::new(rate, 1).expect("valid")
}

fn stereo(rate: u32) -> AudioSpec {
    AudioSpec::new(rate, 2).expect("valid")
}

fn pair(cap: usize, src: AudioSpec, dev: AudioSpec) -> (PlaybackHandle, RenderCore) {
    detached_pair_pub(cap, src, dev).expect("valid pair")
}

#[test]
fn zero_ring_capacity_is_invalid_input() {
    let err = detached_pair_pub(0, mono(24_000), mono(48_000)).unwrap_err();
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
