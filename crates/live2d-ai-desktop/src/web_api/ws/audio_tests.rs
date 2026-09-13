//! WS 音频帧 / 服务端静音 / 句界标记的回归测试（自 ws.rs 拆出）。
//!
//! 覆盖 build_audio_frames 的切片、base64 round-trip、RMS volume、空末块句界帧、
//! 静音契约（PCM 全零 + volume 真实），以及 Broadcaster 的 muted 状态与广播投递。

use super::audio::{
    AUDIO_SLICE_MS, S16LE_BYTES_PER_SAMPLE, base64_encode, build_audio_frames, resolve_muted,
};
use super::broadcaster::Broadcaster;

/// 把 Base64（标准表）字符串解码回字节 —— 自包含实现（测试用）。
fn decode_b64(s: &str) -> Vec<u8> {
    const REV: [u8; 256] = {
        let mut rev = [255u8; 256];
        let table = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < 64 {
            rev[table[i] as usize] = i as u8;
            i += 1;
        }
        rev
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = [0u8; 4];
    let mut n = 0usize;
    for &b in s.as_bytes() {
        if b == b'=' {
            break;
        }
        let v = REV[b as usize];
        if v == 255 {
            continue; // 跳过非法/空白字符
        }
        buf[n] = v;
        n += 1;
        if n == 4 {
            out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
            out.push(((buf[1] as u32 & 0x0F) << 4 | (buf[2] as u32) >> 2) as u8);
            out.push(((buf[2] as u32 & 0x03) << 6 | (buf[3] as u32)) as u8);
            n = 0;
        }
    }
    if n == 2 {
        out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
    } else if n == 3 {
        out.push(((buf[0] as u32) << 2 | (buf[1] as u32) >> 4) as u8);
        out.push(((buf[1] as u32 & 0x0F) << 4 | (buf[2] as u32) >> 2) as u8);
    }
    out
}

/// 每片字节数计算 helper。
fn slice_bytes_for_rate(sample_rate: u32) -> usize {
    (sample_rate as usize * S16LE_BYTES_PER_SAMPLE * AUDIO_SLICE_MS as usize)
        .div_ceil(1000)
        .max(S16LE_BYTES_PER_SAMPLE)
}

/// i16 向量转为 s16le 字节。
fn i16_to_bytes(samples: &[i16]) -> Vec<u8> {
    samples.iter().flat_map(|&s| s.to_le_bytes()).collect()
}

/// 空 PCM + `final_chunk` → **一帧只有边界、没有音频体**的句界帧
/// （2026-09-11 修，见 [`super::build_audio_frames`] 的空 PCM 分支）。
///
/// 来历：引擎允许某句末块样本数为 0（句子样本数正好是 `audio_chunk_samples`
/// 整数倍时必然出现）。旧实现直接返回空表，于是那一句**永远没有 `end=true`**
/// ——前端等不到句尾闸门，整句播不出来。这是本次修复的钉子。
#[test]
fn empty_pcm_with_final_chunk_emits_boundary_only_frame() {
    let frames = build_audio_frames(9, &[], 24000, 7, false, true, false);
    assert_eq!(
        frames.len(),
        1,
        "末块为空时仍须发一帧，否则该句没有 end 闸门"
    );
    let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
    assert_eq!(v["type"], "audio");
    assert_eq!(v["data"]["epoch"], 9);
    assert_eq!(v["data"]["sentence_seq"], 7);
    assert_eq!(v["data"]["end"], true, "句尾闸门必须送达");
    assert_eq!(v["data"]["start"], false, "末块不是首块");
    assert_eq!(v["data"]["audio"], "", "边界帧没有音频体");
    assert_eq!(v["data"]["volume"], 0.0, "空块 RMS 恒为 0");
    assert_eq!(
        v["data"]["sample_rate"], 24000,
        "仍带采样率（前端建 WAV 用）"
    );
}

/// 空 PCM 且**也不是末块** → 没有任何边界要报，空表（不造无用帧）。
#[test]
fn empty_pcm_without_final_chunk_yields_nothing() {
    let frames = build_audio_frames(9, &[], 24000, 7, false, false, false);
    assert!(frames.is_empty(), "非末块的空片没有边界语义，不该发帧");
}

/// 空 PCM 且是**单块成句**（首块也是末块）→ 边界帧同时带 start 与 end。
#[test]
fn empty_pcm_single_chunk_sentence_marks_both_boundaries() {
    let frames = build_audio_frames(9, &[], 24000, 7, true, true, false);
    assert_eq!(frames.len(), 1);
    let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
    assert_eq!(v["data"]["start"], true);
    assert_eq!(v["data"]["end"], true);
}

/// 零采样率一律空表（防除零），**优先于**上面的边界帧规则。
#[test]
fn zero_sample_rate_yields_empty() {
    let frames = build_audio_frames(1, &[0u8; 16], 0, 1, true, true, false);
    assert!(frames.is_empty(), "零采样率应产出 0 帧");
    let frames = build_audio_frames(1, &[], 0, 1, true, true, false);
    assert!(frames.is_empty(), "零采样率 + 空 PCM 也不该产出边界帧");
}

/// **静音契约（2026-09-10 用户裁决）**：`mute=true` 时
/// ① 下发的 PCM 必须**全零**（谁都发不出声，与客户端缓存/第三方客户端无关）；
/// ② `volume` 必须**仍是真实值**（口型不能跟着被关掉）；
/// ③ 分片数量与长度不变（时间轴与排片节奏不漂移）。
#[test]
fn muted_frames_carry_silence_but_real_volume() {
    let sample_rate: u32 = 24000;
    // 非零样本：若静音被实现成「整帧丢弃」或「volume 也清零」，本测试必红。
    let loud = i16_to_bytes(&vec![16_000i16; sample_rate as usize]);

    let normal = build_audio_frames(0, &loud, sample_rate, 1, true, true, false);
    let muted = build_audio_frames(0, &loud, sample_rate, 1, true, true, true);

    assert_eq!(
        muted.len(),
        normal.len(),
        "静音不得改变分片数量（时间轴必须一致）"
    );

    // 静音帧的 audio 必须恰好等于「同长度全零」的 base64。
    let silent_b64 = base64_encode(&vec![0u8; slice_bytes_for_rate(sample_rate)]);

    for (i, (n, m)) in normal.iter().zip(muted.iter()).enumerate() {
        let nv: serde_json::Value = serde_json::from_str(n).expect("normal json");
        let mv: serde_json::Value = serde_json::from_str(m).expect("muted json");

        // ② volume 一致且非零（真实电平被保留）。
        let nvol = nv["data"]["volume"].as_f64().expect("volume");
        let mvol = mv["data"]["volume"].as_f64().expect("volume");
        assert!(nvol > 0.0, "前置：原始样本应有非零音量");
        assert_eq!(mvol, nvol, "静音不得改变 volume（第 {i} 帧）");

        // ① 静音帧 PCM 全零。
        let naudio = nv["data"]["audio"].as_str().expect("audio");
        let maudio = mv["data"]["audio"].as_str().expect("audio");
        assert_eq!(maudio, silent_b64, "静音帧 PCM 必须全零（第 {i} 帧）");
        assert_ne!(naudio, silent_b64, "前置：正常帧应含非零样本（第 {i} 帧）");

        // 观测字段：前端可据此提示「已静音」。
        assert_eq!(mv["data"]["muted"], serde_json::Value::Bool(true));
        assert_eq!(nv["data"]["muted"], serde_json::Value::Bool(false));
    }
}

/// **默认出声**，且 `set_muted` 立即生效；clone 共享同一开关。
#[test]
fn broadcaster_is_unmuted_by_default() {
    let bc = Broadcaster::new();
    assert!(
        !bc.is_muted(),
        "默认必须出声（2026-09-10 用户裁决）；静音需显式 LIVE2D_AI_MUTE_AUDIO=1"
    );
    bc.set_muted(true);
    assert!(bc.is_muted(), "set_muted(true) 应即时生效");
    let alias = bc.clone();
    alias.set_muted(false);
    assert!(!bc.is_muted(), "clone 应共享同一 muted 状态");
}

/// 环境变量 → 输出静音的优先级：默认出声；显式 UNMUTE 压过 MUTE。
#[test]
fn resolve_muted_defaults_to_audible() {
    assert!(
        !resolve_muted(false, false),
        "都不设时必须出声（产品默认值）"
    );
    assert!(resolve_muted(false, true), "LIVE2D_AI_MUTE_AUDIO=1 应静音");
    assert!(
        !resolve_muted(true, false),
        "LIVE2D_AI_UNMUTE_AUDIO=1 应出声"
    );
    assert!(
        !resolve_muted(true, true),
        "两个开关冲突时以出声为准（旧开关优先级最高）"
    );
}

/// 24 kHz 1 秒 s16le mono PCM → 50 帧（20 ms / 帧）。
#[test]
fn one_second_24khz_yields_50_frames() {
    let sample_rate: u32 = 24000;
    let one_sec = i16_to_bytes(&vec![0i16; sample_rate as usize]);

    let frames = build_audio_frames(0, &one_sec, sample_rate, 1, true, true, false);
    assert_eq!(frames.len(), 50, "24kHz 1s 应切 50 帧（每帧 20ms）");
}

/// 首帧 start=true，末帧 end=true，中间帧 start/end 均 false。
#[test]
fn start_and_end_flags_on_boundary_frames() {
    let sample_rate: u32 = 24000;
    let one_sec = i16_to_bytes(&vec![0i16; sample_rate as usize]);
    let frames = build_audio_frames(0, &one_sec, sample_rate, 1, true, true, false);
    assert!(!frames.is_empty());

    let first: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
    let last: serde_json::Value = serde_json::from_str(&frames[frames.len() - 1]).expect("json");
    assert_eq!(first["data"]["start"], true, "首帧 start 应为 true");
    assert_eq!(last["data"]["end"], true, "末帧 end 应为 true");

    if frames.len() > 2 {
        let mid: serde_json::Value = serde_json::from_str(&frames[1]).expect("json");
        assert_eq!(mid["data"]["start"], false, "中间帧 start 应为 false");
        assert_eq!(mid["data"]["end"], false, "中间帧 end 应为 false");
    }
}

/// volume ∈ [0, 1]，保留 3 位。
#[test]
fn volume_in_unit_range() {
    let sample_rate: u32 = 24000;
    // 填充 0.5 振幅正弦（i16 = 32767 * 0.5 ≈ 16383）。
    let samples: Vec<i16> = (0..sample_rate as usize)
        .map(|i| {
            let phase = i as f32 / sample_rate as f32 * 2.0 * std::f32::consts::PI * 440.0;
            (phase.sin() * 16383.0) as i16
        })
        .collect();
    let pcm = i16_to_bytes(&samples);
    let frames = build_audio_frames(0, &pcm, sample_rate, 1, true, true, false);
    for raw in &frames {
        let v: serde_json::Value = serde_json::from_str(raw).expect("json");
        let vol = v["data"]["volume"].as_f64().expect("volume f64");
        assert!((0.0..=1.0).contains(&vol), "volume 超出 [0,1]: {vol}");
    }
}

/// base64 可解码回原字节（round-trip）。
#[test]
fn base64_round_trips_original_bytes() {
    let sample_rate: u32 = 24000;
    let raw_samples: Vec<i16> = (0..1920).map(|i| (i as i16) * 10).collect();
    let pcm = i16_to_bytes(&raw_samples);
    let frames = build_audio_frames(0, &pcm, sample_rate, 1, true, true, false);
    assert!(!frames.is_empty());

    let first: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
    let b64 = first["data"]["audio"].as_str().expect("audio b64 str");
    let decoded = decode_b64(b64);
    let expected_len = pcm.len().min(slice_bytes_for_rate(sample_rate));
    assert_eq!(decoded.len(), expected_len);
    assert_eq!(&decoded[..], &pcm[..decoded.len()]);
}

/// 帧 schema 校验：type/audio/sample_rate/slice_ms/volume/start/end。
#[test]
fn frame_schema_shape() {
    let sample_rate: u32 = 24000;
    // 480 i16 样本 = 960 字节 = 1 片（24kHz 20ms）。
    let pcm = i16_to_bytes(&vec![0i16; 480]);
    let frames = build_audio_frames(42, &pcm, sample_rate, 1, true, true, false);
    assert_eq!(frames.len(), 1, "960 字节 = 1 片（24kHz 20ms）");
    let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
    assert_eq!(v["type"], "audio");
    assert_eq!(v["data"]["epoch"], 42);
    assert_eq!(v["data"]["sample_rate"], 24000);
    assert_eq!(v["data"]["slice_ms"], 20);
    assert_eq!(v["data"]["start"], true);
    assert_eq!(v["data"]["end"], true);
    assert!(v["data"]["volume"].as_f64().is_some());
    assert!(v["data"]["audio"].as_str().is_some());
}

/// silent（全零）PCM → volume == 0.0。
#[test]
fn silent_pcm_zero_volume() {
    let sample_rate: u32 = 24000;
    let pcm = i16_to_bytes(&vec![0i16; 960]);
    let frames = build_audio_frames(0, &pcm, sample_rate, 1, true, true, false);
    let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
    assert_eq!(v["data"]["volume"], 0.0, "静音应 volume=0");
}

/// broadcaster.broadcast_audio_frames 在有订阅者时能收到音频帧。
#[test]
fn broadcast_audio_frames_delivers_to_subscriber() {
    let bc = Broadcaster::new();
    let (tx, rx) = std::sync::mpsc::sync_channel::<String>(32);
    bc.try_subscribe(tx).expect("subscribe");

    let sample_rate: u32 = 24000;
    // 480 i16 样本 = 960 字节 = 1 片（24kHz 20ms）。
    let pcm = i16_to_bytes(&vec![100i16; 480]);
    bc.broadcast_audio_frames(1, &pcm, sample_rate, 1, true, true);

    let received = rx
        .recv_timeout(std::time::Duration::from_millis(200))
        .expect("收到音频帧");
    let v: serde_json::Value = serde_json::from_str(&received).expect("json");
    assert_eq!(v["type"], "audio");
    assert_eq!(v["data"]["epoch"], 1);
    assert_eq!(v["data"]["start"], true);
    assert_eq!(v["data"]["end"], true);
}

/// 无订阅者时广播音频帧也是安全的 no-op。
#[test]
fn broadcast_audio_frames_no_subscribers_is_safe() {
    let bc = Broadcaster::new();
    let sample_rate: u32 = 24000;
    let pcm = i16_to_bytes(&vec![0i16; 480]);
    bc.broadcast_audio_frames(0, &pcm, sample_rate, 1, true, true); // 不应 panic
    assert_eq!(bc.subscriber_count(), 0);
}

/// 句内**非末块**：所有帧 start/end 均为 false（句界不在这一块上）。
///
/// 2026-09-11 重写（原测试名 `segment_start_false_marks_no_frame_as_start`）：
/// 旧语义里「本段不是整轮首片」就意味着没有 start；现在 `start`/`end` 是
/// **句子边界**，所以「句内中间块」两个标志都应为 false。
#[test]
fn mid_sentence_chunk_marks_neither_boundary() {
    let sample_rate: u32 = 24000;
    let two_sec = i16_to_bytes(&vec![0i16; sample_rate as usize * 2]);
    // 一句的第 2 块（first=false），且不是末块（final=false）。
    let frames = build_audio_frames(3, &two_sec, sample_rate, 1, false, false, false);
    assert_eq!(frames.len(), 100, "2 s / 20 ms = 100 帧");
    for (i, raw) in frames.iter().enumerate() {
        let v: serde_json::Value = serde_json::from_str(raw).expect("json");
        assert_eq!(v["data"]["start"], false, "第 {i} 帧 start 应为 false");
        assert_eq!(v["data"]["end"], false, "第 {i} 帧 end 应为 false");
    }
}

/// **句界回归钉子（2026-09-11）**：一句的音频被切成多块时，
/// **整句只有一片 start=true、一片 end=true**，且它们可以落在**不同块**里。
///
/// 这是「前端按句封 WAV」的前提。旧实现按 epoch 记账推 start，实测产出
/// 「0 个 start、13 个 end」——前端攒句会攒错、播出来断断续续。
#[test]
fn sentence_boundaries_are_exactly_one_start_and_one_end() {
    let bc = Broadcaster::new();
    let (tx, rx) = std::sync::mpsc::sync_channel::<String>(64);
    bc.try_subscribe(tx).expect("subscribe");

    // 4_800 样本 @24 kHz = 200 ms = 10 片。
    let chunk = i16_to_bytes(&vec![100i16; 4_800]);
    // 收 10 帧，返回 (start, end) 列表。
    let flags = |rx: &std::sync::mpsc::Receiver<String>| -> Vec<(bool, bool)> {
        (0..10)
            .map(|_| {
                let raw = rx
                    .recv_timeout(std::time::Duration::from_millis(500))
                    .expect("收到音频帧");
                let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
                (
                    v["data"]["start"].as_bool().expect("start 是 bool"),
                    v["data"]["end"].as_bool().expect("end 是 bool"),
                )
            })
            .collect()
    };

    // 第 1 句·第 1 块（首块，非末块）：首片 start，无 end。
    bc.broadcast_audio_frames(7, &chunk, 24_000, 1, true, false);
    let a = flags(&rx);
    assert!(a[0].0, "句首块的首片应为 start=true");
    assert!(
        a[1..].iter().all(|(s, _)| !s),
        "同一块内其余帧 start 应为 false"
    );
    assert!(a.iter().all(|(_, e)| !e), "非末块不得出现 end=true");

    // 第 1 句·第 2 块（末块）：末片 end，且**不得**再出现 start。
    bc.broadcast_audio_frames(7, &chunk, 24_000, 1, false, true);
    let b = flags(&rx);
    assert!(b.iter().all(|(s, _)| !s), "句内后续块不得再发 start=true");
    assert!(b[9].1, "句末块的末片应为 end=true");
    assert!(
        b[..9].iter().all(|(_, e)| !e),
        "末块内只有最后一片带 end=true"
    );

    // 第 2 句（新 seq）：句首块重新标记 start=true —— 句界按句，不按 epoch。
    bc.broadcast_audio_frames(7, &chunk, 24_000, 2, true, true);
    let c = flags(&rx);
    assert!(c[0].0, "新一句的首片应为 start=true");
    assert!(c[9].1, "单块成句时首末片同时成立");
}

/// 帧里带 `sentence_seq`（前端据此归组同一句的分片）。
#[test]
fn frames_carry_sentence_seq() {
    let sample_rate: u32 = 24000;
    let pcm = i16_to_bytes(&vec![0i16; 480]);
    let frames = build_audio_frames(5, &pcm, sample_rate, 9, true, true, false);
    let v: serde_json::Value = serde_json::from_str(&frames[0]).expect("json");
    assert_eq!(v["data"]["epoch"], 5);
    assert_eq!(v["data"]["sentence_seq"], 9);
}
