use super::*;

fn at(ms: u64) -> Instant {
    Instant::now() + Duration::from_millis(ms)
}

#[test]
fn hold_over_300ms_without_movement_starts_recording() {
    let mut hold = HoldState::new();
    let t0 = at(0);
    hold.down(t0);
    assert!(!hold.should_start(t0 + Duration::from_millis(299)));
    assert!(hold.should_start(t0 + Duration::from_millis(300)));
    hold.moved(3, 5); // 阈值内的手抖
    assert!(hold.should_start(t0 + Duration::from_millis(301)));
    hold.mark_recording(t0 + Duration::from_millis(301));
    assert!(hold.is_recording());
    assert_eq!(
        hold.up(t0 + Duration::from_millis(900)),
        HoldOutcome::Record
    );
}

#[test]
fn movement_over_8px_is_drag_not_recording() {
    let mut hold = HoldState::new();
    let t0 = at(0);
    hold.down(t0);
    hold.moved(9, 0);
    assert!(hold.is_drag());
    assert!(!hold.should_start(t0 + Duration::from_secs(1)));
    assert_eq!(hold.up(t0 + Duration::from_secs(1)), HoldOutcome::Drag);
}

#[test]
fn short_press_does_not_record() {
    let mut hold = HoldState::new();
    let t0 = at(0);
    hold.down(t0);
    assert_eq!(
        hold.up(t0 + Duration::from_millis(120)),
        HoldOutcome::TooShort
    );
}

#[test]
fn recording_auto_releases_at_120_seconds() {
    let mut hold = HoldState::new();
    let t0 = at(0);
    hold.down(t0);
    hold.mark_recording(t0 + Duration::from_millis(300));
    assert!(!hold.auto_stop_due(t0 + Duration::from_secs(119)));
    assert!(hold.auto_stop_due(t0 + Duration::from_secs(300) + Duration::from_secs(120)));
}

/// **没配 ASR 钥匙时不打上游**：决策层绝不返回 Transcribe。
#[test]
fn no_asr_key_never_calls_upstream() {
    assert_eq!(
        plan_asr(None, false, HoldOutcome::Record),
        AsrPlan::Notice(NO_KEY_NOTICE, Duration::from_secs(3))
    );
    assert_eq!(
        plan_asr(Some("   "), false, HoldOutcome::Record),
        AsrPlan::Notice(NO_KEY_NOTICE, Duration::from_secs(3))
    );
    assert_eq!(plan_asr(None, false, HoldOutcome::Drag), AsrPlan::Skip);
    assert_eq!(plan_asr(None, false, HoldOutcome::TooShort), AsrPlan::Skip);
    assert_eq!(
        plan_asr(Some("k"), true, HoldOutcome::Record),
        AsrPlan::Skip
    );
    assert_eq!(
        plan_asr(Some("k"), false, HoldOutcome::Record),
        AsrPlan::Transcribe("k".to_string())
    );
}

/// 16 位单声道 16 kHz WAV：头合法，时长可被 `voice/deepseek` 的解析器读出。
#[test]
fn wav_is_16bit_mono_16khz_and_duration_is_readable() {
    let wav = encode_wav(&vec![0_i16; SAMPLE_RATE as usize]);
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1, "单声道");
    assert_eq!(
        u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
        SAMPLE_RATE
    );
    assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16, "16 位");
    assert_eq!(live2d_ai_runtime::asr::wav_duration_ms(&wav), Some(1_000));
    assert_eq!(encode_wav(&[]).len(), 44, "空录音也有一份合法头");
}

#[test]
fn wav_over_8mib_would_be_discarded_locally() {
    // 120 秒封顶后真编一份 WAV 再量：3,840,044 字节，本机就得小于 8 MiB。
    let capped = encode_wav(&vec![0_i16; MAX_SAMPLES]);
    assert!(
        capped.len() < MAX_WAV_BYTES,
        "{} 必须小于 {MAX_WAV_BYTES}",
        capped.len()
    );
    assert_eq!(capped.len(), 44 + MAX_SAMPLES * 2);
}
