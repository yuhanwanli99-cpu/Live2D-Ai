//! TTS worker：单次 `run_turn` 一个，按 `sentence_seq` 到达序串行合成整条队列。
//!
//! 退出条件（任一）：队列关闭且排空（正常）/ 取消 / 首个致命错误 / 事件通道关闭。
//! 除正常退出外都会先尽力发出对应的 `Error` 事件。
//!
//! 内部状态不对外暴露；通过 [`WorkerExit`] 经 `JoinHandle` 回传给主任务做终态判定。

use std::time::Instant;

use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::EngineEvent;
use super::ErrorKind;
use super::events::send_event;
use crate::OpenAiClient;
use crate::audio::{AudioSpec, PcmS16LeDecoder};

/// 内部：送进 TTS worker 的任务。
#[derive(Debug)]
pub(super) struct TtsJob {
    pub(super) sentence_seq: u64,
    pub(super) text: String,
}

/// TTS worker 的不可变上下文。
pub(super) struct TtsWorkerCtx {
    pub(super) client: OpenAiClient,
    pub(super) epoch: u64,
    pub(super) event_tx: mpsc::Sender<EngineEvent>,
    pub(super) cancel: CancellationToken,
    pub(super) chunk_samples: usize,
    /// 节点 D 可观测性：本轮 `run_turn` 起点基准。worker 发送的所有
    /// `EngineEvent.ts_ms` 都相对此点计算——保证跨任务（engine + worker）
    /// 时间轴一致。
    pub(super) turn_started: Instant,
}

/// TTS worker 的退出原因（经 `JoinHandle` 回传给主任务，用于终态判定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WorkerExit {
    /// 队列关闭且完全排空：正常下岗。
    Drained,
    /// 取消或事件通道关闭：安静退出。
    Stopped,
    /// 致命错误退出（对应的 `Error` 事件已由 worker 自行发出）。
    Failed,
}

/// 单句合成结果。
enum SynthOutcome {
    /// 整句字节流消费完毕并发出了 `final_chunk`。
    Done,
    /// 取消或事件通道关闭：安静退出，不再发声。
    Cancelled,
    /// 致命错误（TTS/解码）。
    Failed(ErrorKind),
}

/// 单个 TTS worker：严格按 `sentence_seq` 到达序串行合成整条队列。
pub(super) async fn tts_worker(ctx: TtsWorkerCtx, mut rx: mpsc::Receiver<TtsJob>) -> WorkerExit {
    let spec = ctx.client.tts().spec;
    loop {
        let job = tokio::select! {
            _ = ctx.cancel.cancelled() => return WorkerExit::Stopped,
            job = rx.recv() => match job {
                Some(job) => job,
                None => return WorkerExit::Drained, // 队列关闭且已排空。
            },
        };
        match synthesize_sentence(&ctx, spec, &job).await {
            SynthOutcome::Done => {}
            SynthOutcome::Cancelled => return WorkerExit::Stopped,
            SynthOutcome::Failed(kind) => {
                let _ = send_event(
                    &ctx.event_tx,
                    &ctx.cancel,
                    EngineEvent::Error {
                        epoch: ctx.epoch,
                        ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
                        kind,
                    },
                )
                .await;
                return WorkerExit::Failed;
            }
        }
    }
}

/// 合成一句话：请求 `/audio/speech` → 增量解码 → 按上限切块发送。
async fn synthesize_sentence(ctx: &TtsWorkerCtx, spec: AudioSpec, job: &TtsJob) -> SynthOutcome {
    // **不发 HTTP 的两种情形，走同一条「静音句」收口**：发一个空 `final_chunk`
    // 再发 `SentenceVoiced`，文本不丢、闩锁照常推进。
    //
    // ① **TTS 未接入（2026-09-10）**：`base_url` 为空 = 本轮未选定 TTS。
    // ② **纯空白句（2026-09-11 修，实测缺陷）**：分句器把**换行**也算句读
    //    （`is_terminator` 含 `'\n'`），所以模型输出 `"你好！\n"` 会切出一个只含
    //    空白的「句子」。它 `is_empty() == false`（因此躲过了分句器的非空守卫）
    //    却会被 TTS 上游 trim 成空串，上游直接回
    //    `400 {"detail":"input 为空"}`——而 TTS 错误是 **fatal**，于是一句纯空白
    //    会把**整轮**判失败（用户看到的是「说了半句就没了」）。
    //    这里不发这个注定失败的请求，也不让分句器丢字符（那是它的硬不变量）。
    if !ctx.client.tts().is_configured() || job.text.trim().is_empty() {
        let delivered = send_event(
            &ctx.event_tx,
            &ctx.cancel,
            EngineEvent::AudioChunk {
                epoch: ctx.epoch,
                ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
                sentence_seq: job.sentence_seq,
                samples: Vec::new(),
                spec,
                // 空句：这一块既是首块也是末块。
                first_chunk: true,
                final_chunk: true,
            },
        )
        .await;
        // 仍按「一句一单元」上屏（纯文字模式 / 空白句都不受影响的文本侧）。
        let voiced = delivered && emit_sentence_voiced(ctx, job).await;
        return if voiced {
            SynthOutcome::Done
        } else {
            SynthOutcome::Cancelled
        };
    }

    // 格式门禁：本引擎只承诺 pcm（默认配置恒满足；显式改配置则明确报错）。
    if let Err(e) =
        crate::audio::ensure_supported_format(ctx.client.tts().response_format.as_deref())
    {
        return SynthOutcome::Failed(ErrorKind::Decode(e));
    }

    let mut speech = tokio::select! {
        _ = ctx.cancel.cancelled() => return SynthOutcome::Cancelled,
        opened = ctx.client.synthesize_speech(&job.text) => match opened {
            Ok(speech) => speech,
            Err(e) => return SynthOutcome::Failed(ErrorKind::Tts(e)),
        },
    };

    // **WAV 容器（2026-09-10，CosyVoice 3 接入）**：需要先拿到完整 RIFF 头才能
    // 确定真实规格，故整段收集后解析——单句 TTS 音频很短，缓冲无碍。
    if ctx
        .client
        .tts()
        .response_format
        .as_deref()
        .is_some_and(|f| f.trim().eq_ignore_ascii_case(crate::audio::WAV_FORMAT))
    {
        return synthesize_wav_sentence(ctx, job, speech).await;
    }

    // 增量解码：任意字节切割安全（半个样本留到下批）；块大小受限避免无限内存。
    let mut decoder = PcmS16LeDecoder::new(spec);
    let mut pending: Vec<f32> = Vec::new();
    // 「本句是否已经发过块」——`first_chunk` 的唯一来源（见 `EngineEvent::AudioChunk`）。
    let mut sent_any = false;
    loop {
        let chunk = tokio::select! {
            _ = ctx.cancel.cancelled() => return SynthOutcome::Cancelled,
            next = speech.next() => match next {
                Some(chunk) => chunk,
                None => break,
            },
        };
        // 显式标注 Vec<u8>：decode 的入参是 &[u8]，不标注会把推断方向带偏
        // （编译器会试图把 bytes 统一成未定长 [u8]）。
        let bytes: Vec<u8> = match chunk {
            Ok(bytes) => bytes,
            Err(e) => return SynthOutcome::Failed(ErrorKind::Tts(e)),
        };
        pending.extend(decoder.decode(&bytes));
        while pending.len() >= ctx.chunk_samples {
            let block = take_leading_chunk(&mut pending, ctx.chunk_samples);
            let delivered = send_event(
                &ctx.event_tx,
                &ctx.cancel,
                EngineEvent::AudioChunk {
                    epoch: ctx.epoch,
                    ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
                    sentence_seq: job.sentence_seq,
                    samples: block,
                    spec,
                    first_chunk: !sent_any,
                    final_chunk: false,
                },
            )
            .await;
            sent_any = true;
            if !delivered {
                return SynthOutcome::Cancelled;
            }
        }
    }

    // 流结束：总字节数必须按样本对齐（奇数字节尾巴是明确的 Decode 错误）。
    if let Err(e) = decoder.finish() {
        return SynthOutcome::Failed(ErrorKind::Decode(e));
    }
    // 每句恰好一个 final_chunk（即使该句没有任何样本）。
    let tail = std::mem::take(&mut pending);
    let delivered = send_event(
        &ctx.event_tx,
        &ctx.cancel,
        EngineEvent::AudioChunk {
            epoch: ctx.epoch,
            ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
            sentence_seq: job.sentence_seq,
            samples: tail,
            spec,
            // 前面一块都没发过 ⇒ 这个末块同时也是首块（短句 / 空句）。
            first_chunk: !sent_any,
            final_chunk: true,
        },
    )
    .await;
    // 顺序保证：final AudioChunk 已发出后，才发「本句已合成」——消费端先拿到声音。
    if !delivered || !emit_sentence_voiced(ctx, job).await {
        return SynthOutcome::Cancelled;
    }
    SynthOutcome::Done
}

/// 发「本句语音已完整合成」事件（紧随 final `AudioChunk`）。
///
/// 返回 `false` 表示消费端已关闭（调用方应视同取消）。
///
/// 为什么单列一个 helper：`synthesize_sentence` 有**三处**收口
/// （TTS 未配置 / 裸 PCM 流结束 / WAV 解析后），三处都必须发出本事件，
/// 否则「先完整合成、再上屏文字」的契约会在某条路径上静默失效。
async fn emit_sentence_voiced(ctx: &TtsWorkerCtx, job: &TtsJob) -> bool {
    send_event(
        &ctx.event_tx,
        &ctx.cancel,
        EngineEvent::SentenceVoiced {
            epoch: ctx.epoch,
            ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
            sentence_seq: job.sentence_seq,
            text: job.text.clone(),
        },
    )
    .await
}

/// WAV 句合成：整段收集 → [`crate::audio::parse_wav_s16`] → 按 `chunk_samples` 切块发事件。
///
/// 与裸 PCM 路径的两点差异：
/// 1. **必须缓冲整段**（RIFF 头声明规格，`data` 块可能不在首个 chunk 内）；
/// 2. 事件里的 `spec` 用**容器内声明**的规格（可能与 `[tts] sample_rate/channels`
///    不一致——以容器为准，播放端据此重采样）。
///
/// 上限 [`MAX_WAV_BYTES`] 防异常服务端打满内存；超限报 `Decode` 错误而非静默截断。
async fn synthesize_wav_sentence(
    ctx: &TtsWorkerCtx,
    job: &TtsJob,
    mut speech: std::pin::Pin<Box<dyn futures_util::Stream<Item = crate::Result<Vec<u8>>> + Send>>,
) -> SynthOutcome {
    let mut buf: Vec<u8> = Vec::new();
    loop {
        let chunk = tokio::select! {
            _ = ctx.cancel.cancelled() => return SynthOutcome::Cancelled,
            next = speech.next() => match next {
                Some(chunk) => chunk,
                None => break,
            },
        };
        let bytes = match chunk {
            Ok(bytes) => bytes,
            Err(e) => return SynthOutcome::Failed(ErrorKind::Tts(e)),
        };
        if buf.len() + bytes.len() > MAX_WAV_BYTES {
            return SynthOutcome::Failed(ErrorKind::Decode(crate::Error::InvalidAudioConfig {
                message: format!("WAV 响应超过 {MAX_WAV_BYTES} 字节上限"),
            }));
        }
        buf.extend_from_slice(&bytes);
    }

    let parsed = match crate::audio::parse_wav_s16(&buf) {
        Ok(parsed) => parsed,
        Err(e) => return SynthOutcome::Failed(ErrorKind::Decode(e)),
    };
    let spec = parsed.spec;
    let mut pending = parsed.samples;
    // 同裸 PCM 路径：`first_chunk` 由「本句是否已发过块」决定。
    let mut sent_any = false;

    // 非末块：留出末块（含空尾块）作 final_chunk，保证每句恰好一个 final_chunk。
    while pending.len() > ctx.chunk_samples {
        let block = take_leading_chunk(&mut pending, ctx.chunk_samples);
        let delivered = send_event(
            &ctx.event_tx,
            &ctx.cancel,
            EngineEvent::AudioChunk {
                epoch: ctx.epoch,
                ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
                sentence_seq: job.sentence_seq,
                samples: block,
                spec,
                first_chunk: !sent_any,
                final_chunk: false,
            },
        )
        .await;
        sent_any = true;
        if !delivered {
            return SynthOutcome::Cancelled;
        }
    }

    let tail = std::mem::take(&mut pending);
    let delivered = send_event(
        &ctx.event_tx,
        &ctx.cancel,
        EngineEvent::AudioChunk {
            epoch: ctx.epoch,
            ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
            sentence_seq: job.sentence_seq,
            samples: tail,
            spec,
            // 前面一块都没发过 ⇒ 这个末块同时也是首块。
            first_chunk: !sent_any,
            final_chunk: true,
        },
    )
    .await;
    // 顺序保证：final AudioChunk 已发出后，才发「本句已合成」——消费端先拿到声音。
    if !delivered || !emit_sentence_voiced(ctx, job).await {
        return SynthOutcome::Cancelled;
    }
    SynthOutcome::Done
}

/// 单句 WAV 响应上限（64 MiB）：防异常服务端把内存打满。
const MAX_WAV_BYTES: usize = 64 * 1024 * 1024;

/// 取走缓冲区前 `limit` 个样本（不足则全取）。独立成函数便于单测切块逻辑。
pub(super) fn take_leading_chunk(buffer: &mut Vec<f32>, limit: usize) -> Vec<f32> {
    buffer.drain(..limit.min(buffer.len())).collect()
}
