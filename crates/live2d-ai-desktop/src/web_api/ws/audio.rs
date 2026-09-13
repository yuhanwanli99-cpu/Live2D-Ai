//! WS 音频帧构造（W2+ / F6-T2 接入，v1 最小可播）。
//!
//! 把 s16le PCM 切成 ~20 ms base64 音频帧；v1 为单声道、单 volume 标量
//! （RMS/32768，保留 3 位），不下发 viseme。帧 schema 见 build_audio_frames。
//!
//! - **句子边界**（2026-09-11 修）：start=true 只出现在某一句第一块的首片，
//!   end=true 只出现在该句末块的末片；每句恰好各一个。这两个标志**必须按句**
//!   给（由引擎 first_chunk/final_chunk 给出），不得按块或按 epoch 推断；
//! - **服务端静音**：resolve_muted 决定产品默认，mute 置零 PCM 但保留真实
//!   volume（口型与静音正交）。

/// 每片毫秒数（v1 固定 20 ms，与 RMS_WINDOW_MS / TTS chunk 同尺度）。
#[allow(dead_code)]
pub const AUDIO_SLICE_MS: u32 = 20;
/// s16le 每样本字节数（测试经 `ws::audio_tests` 引用）。
#[allow(dead_code)]
pub(super) const S16LE_BYTES_PER_SAMPLE: usize = 2;
/// s16 最大值（用于归一化）：32768.0。
#[allow(dead_code)]
const S16_NORM: f32 = 32_768.0;

/// 由两个环境变量开关决定**输出静音**状态（纯函数，便于单测）。
///
/// 这是「产品默认是否出声」的唯一落点，所以从 `cli_entry` 的启动分支里抽出来，
/// 让它能被离线回归——默认值翻转是很容易被无意改回去的那类行为。
///
/// 规则（优先级从高到低）：
/// 1. `force_unmute`（`LIVE2D_AI_UNMUTE_AUDIO=1`）→ **出声**。保留的旧开关，
///    让旧脚本/旧文档不会因为默认值翻转而静默变成静音；
/// 2. `force_mute`（`LIVE2D_AI_MUTE_AUDIO=1`）→ **静音**（跑测试 / 无人值守用）；
/// 3. 都不设 → **出声**（2026-09-10 用户裁决的默认值）。
pub fn resolve_muted(force_unmute: bool, force_mute: bool) -> bool {
    !force_unmute && force_mute
}

/// 把一段 s16le PCM 按 ~20 ms 切片，产出 WS 音频帧 JSON 字符串列表。
///
/// - `epoch`：所属业务轮次；
/// - `pcm`：s16le 交错样本字节（mono 视为单声道；多声道时仅首声道参与 RMS，
///   其它声道字节原样 Base64 透传）；
/// - `sample_rate`：PCM 采样率（Hz）；
/// - `sentence_seq`：句子序号（本轮内从 1 严格递增，来自引擎
///   `EngineEvent::AudioChunk`）；
/// - `first_chunk`：本段 PCM 是否为**该句的第一块**——为 true 时首帧 `start=true`；
/// - `final_chunk`：本段 PCM 是否为**该句的最后一块**——为 true 时末帧 `end=true`。
///   两者都由引擎给出（`first_chunk` / `final_chunk`），**不由本函数推断**：
///   一句音频会被切成十几块，只有引擎知道哪块是首/末；
/// - `mute`：**静音输出**。为 true 时下发的 PCM 字节全部置零（长度不变），
///   但 `volume` **仍按原始样本计算** —— 于是「任何客户端都发不出声音」，
///   而口型驱动所需的信息完整保留；
/// - 返回值：空输入 → 空 Vec；非空 → ≥1 帧，`start`/`end` 按上表标注。
///
/// # 为什么在服务端静音（2026-09-10 用户裁决）
///
/// 用户要求「关掉声音但保留口型」。若只在某个客户端做静音，就绕不过：
/// 浏览器 Service Worker 缓存下来的**旧前端**、遗留的原生 JS 前端 `/`、
/// 以及任何第三方客户端。静音属于**产品语义**，必须在音频的**源头**强制，
/// 而不是指望每个消费者自觉。
///
/// 置零而不是「不发帧」：分片节奏与时间轴保持不变（客户端仍按同样节奏排片），
/// 口型的到期释放不会漂移；旧客户端「照常播放」——只是播的是静音。
pub fn build_audio_frames(
    epoch: u64,
    pcm: &[u8],
    sample_rate: u32,
    sentence_seq: u64,
    first_chunk: bool,
    final_chunk: bool,
    mute: bool,
) -> Vec<String> {
    if sample_rate == 0 {
        return Vec::new();
    }
    // **空 PCM 的句界帧（2026-09-11 修，实测缺陷）**：引擎允许某句的末块
    // 样本数为 0——只要该句样本数正好是 `audio_chunk_samples` 的整数倍就会出现
    // （默认 4 800 样本 = 200 ms @24 kHz，所以 200/400/600 ms 的句子全都命中）。
    // `chunks()` 在空切片上**不产出任何片**，所以过去这里直接返回空表，于是：
    // 该句最后一个**非空**片带 `final_chunk=false` → `end=false`；而承载
    // `end=true` 的空块又被丢掉 → **前端永远等不到句尾闸门**，那一句就播不出来
    // （听感是「尾巴没了」，队列还会一直卡着）。
    //
    // 所以这里改为发**一帧只有边界、没有音频体**的帧（`audio` 为空串）：
    // `start`/`end` 本来就是**标记**，标记不需要带载荷。前端把空音频体解析成
    // 0 字节 PCM，照常在 `end=true` 上封口（`ws_frame.dart` 的 `_str` 对 `""`
    // 返回非 null，因此这一帧不会被「没有音频体就丢弃」那条规则吃掉）。
    if pcm.is_empty() {
        if !final_chunk {
            return Vec::new();
        }
        let frame = serde_json::json!({
            "type": "audio",
            "data": {
                "epoch": epoch,
                "audio": "",
                "sample_rate": sample_rate,
                "slice_ms": AUDIO_SLICE_MS,
                // 空块的 RMS 恒为 0：句尾闭嘴是真话，不是猜测。
                "volume": 0.0,
                "sentence_seq": sentence_seq,
                "start": first_chunk,
                "end": true,
                "muted": mute,
            }
        });
        return vec![frame.to_string()];
    }
    // 每片字节数 = sample_rate * 2 bytes/sample * slice_ms / 1000。
    let slice_bytes = (sample_rate as usize * S16LE_BYTES_PER_SAMPLE * AUDIO_SLICE_MS as usize)
        .div_ceil(1000)
        .max(S16LE_BYTES_PER_SAMPLE);
    let total = pcm.chunks(slice_bytes).count();
    let mut out = Vec::with_capacity(total);
    for (i, slice) in pcm.chunks(slice_bytes).enumerate() {
        // 句界：首块的首片 = start，末块的末片 = end（每句各恰好一个）。
        let start = first_chunk && i == 0;
        let end = final_chunk && i + 1 == total;
        // volume 永远取自**原始**样本：静音不能把口型一起关掉。
        let volume = rms_s16le_mono(slice);
        let b64 = if mute {
            // 零填充到相同字节数（s16le 全零 = 绝对静音）。
            base64_encode(&vec![0u8; slice.len()])
        } else {
            base64_encode(slice)
        };
        let frame = serde_json::json!({
            "type": "audio",
            "data": {
                "epoch": epoch,
                "audio": b64,
                "sample_rate": sample_rate,
                "slice_ms": AUDIO_SLICE_MS,
                "volume": volume,
                "sentence_seq": sentence_seq,
                "start": start,
                "end": end,
                "muted": mute,
            }
        });
        out.push(frame.to_string());
    }
    out
}

/// s16le 单声道片的 RMS 音量归一化标量 ∈ [0, 1]，保留 3 位。
///
/// 对多声道输入（>2 字节/样本），仅首声道用于 RMS；其它声道忽略。
/// 样本数不足整取时，余数样本参与 RMS 但不做填充。
#[allow(dead_code)]
fn rms_s16le_mono(slice: &[u8]) -> f32 {
    let n = slice.len() / S16LE_BYTES_PER_SAMPLE;
    if n == 0 {
        return 0.0;
    }
    let mut sum_sq: f64 = 0.0;
    for i in 0..n {
        let off = i * S16LE_BYTES_PER_SAMPLE;
        let sample = i16::from_le_bytes([slice[off], slice[off + 1]]);
        let f = (sample as f32 / S16_NORM).clamp(-1.0, 1.0);
        let fv = f as f64;
        sum_sq += fv * fv;
    }
    let rms = (sum_sq / n as f64).sqrt();
    // rms 已经 [0,1] 归一化，保留 3 位小数。
    let normalized = rms as f32;
    (normalized * 1000.0).round() / 1000.0
}

/// 把字节片 Base64（标准表）编码 —— 自包含实现（避免新增 crate 依赖）。
///
/// **注**：若日后 desktop Cargo.toml 加入 `base64 = "0.22"`，可替换为
/// `base64::engine::general_purpose::STANDARD.encode(slice)`。
#[allow(dead_code)]
pub(super) fn base64_encode(slice: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(slice.len().div_ceil(3) * 4);
    for chunk in slice.as_chunks::<3>().0 {
        let b0 = chunk[0] as u32;
        let b1 = chunk[1] as u32;
        let b2 = chunk[2] as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push(TABLE[(n & 0x3F) as usize] as char);
    }
    let rem = slice.len() % 3;
    if rem == 1 {
        let b0 = slice[slice.len() - 1] as u32;
        let n = b0 << 16;
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if rem == 2 {
        let b0 = slice[slice.len() - 2] as u32;
        let b1 = slice[slice.len() - 1] as u32;
        let n = (b0 << 16) | (b1 << 8);
        out.push(TABLE[((n >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3F) as usize] as char);
        out.push(TABLE[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
    out
}
