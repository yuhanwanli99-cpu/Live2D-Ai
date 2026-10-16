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
///
/// **两份文本**（2026-10-08）：
/// - `text` = 送去**合成**的文本（二路交回的可念文本；空 = 静音句，不发 HTTP）；
/// - `display` = 这一句**上屏**的文本（二路交回的原文切片 / 失败时的原文）。
///
/// `SentenceVoiced.text` 取的是 `display`：文字与声音同拍，且上屏保留
/// emoji / 颜文字 / 括号里的动作描写。
#[derive(Debug)]
pub(super) struct TtsJob {
    pub(super) sentence_seq: u64,
    pub(super) text: String,
    pub(super) display: String,
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
    // **不走 HTTP 合成的几种情形，全走同一条「静音句」收口**（silent_sentence）：
    // 发一个空 `final_chunk` 再发 `SentenceVoiced`，文本不丢、闩锁照常推进。
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
        return silent_sentence(ctx, spec, job).await;
    }

    // 格式门禁：本引擎只承诺 pcm（默认配置恒满足；显式改配置则明确报错）。
    // `UnsupportedFormat` 属「只影响这一句」那一类（见 speakable）：静音句，轮次继续。
    if let Err(e) =
        crate::audio::ensure_supported_format(ctx.client.tts().response_format.as_deref())
    {
        return recover_sentence(ctx, spec, job, ErrorKind::Decode(e), false).await;
    }

    // **T1（2026-10-13 执行单）：只改送 TTS 的这一份文本**。
    // 上屏那份（SentenceVoiced 的 job.display）一个字不动；这里把 ≥17 位的
    // 连续数字段换成「一长串数字」——本地 TTS 会回 400「超出数据范围，最长
    // 支持 16 位」，一个 31 位数字足以把整轮判失败。
    let speech_text = crate::cleaning::speech_for_tts(&job.text);

    // **碎片句（2026-10-15 执行单 §3）**：判的是**送去合成的这一份**
    // （`speech_for_tts` 之后），不是屏幕上那份。纯列表序号（`1.` `2.`）与洗完
    // 只剩标记（`*` `#` 反引号）的句子不发 HTTP——屏幕上的字一个不动。
    if super::speakable::is_fragment_marker(&speech_text) {
        return silent_sentence(ctx, spec, job).await;
    }

    let mut speech = tokio::select! {
        _ = ctx.cancel.cancelled() => return SynthOutcome::Cancelled,
        opened = ctx.client.synthesize_speech(&speech_text) => match opened {
            Ok(speech) => speech,
            // 本地 TTS 的长度拒绝：**这一句**改走静音句，轮次继续（T1）。
            Err(e) if is_tts_length_rejection(&e) => {
                tracing::warn!(
                    target: "tts",
                    sentence_seq = job.sentence_seq,
                    code = "tts_upstream_400_digit_range",
                    "本地 TTS 拒收这一句（超出数据范围）：本句改走静音句，后续句子继续"
                );
                return silent_sentence(ctx, spec, job).await;
            }
            // 其余 TTS 错误按变体分流（见 speakable）：可续走的走静音句，轮次继续。
            Err(e) => return recover_sentence(ctx, spec, job, ErrorKind::Tts(e), false).await,
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
            Err(e) => return recover_sentence(ctx, spec, job, ErrorKind::Tts(e), sent_any).await,
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
        return recover_sentence(ctx, spec, job, ErrorKind::Decode(e), sent_any).await;
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

/// 本地 TTS 长度拒绝的响应体标记（T1，2026-10-13 执行单）。
///
/// 上游原文：`400` + 响应体里一句「超出数据范围，最长支持 16 位」。
/// 用**响应体标记**而不是主机名判定：这条错误的语义是「这一串数字念不出来」，
/// 与端点在谁的机器上无关；标记本身足够具体，别的 400 不会误命中。
const LOCAL_TTS_LENGTH_MARKER: &str = "最长支持 16 位";

/// 这条 TTS 错误是不是「长度超范围」的拒绝（T1，2026-10-13 执行单）。
///
/// 命中 ⇒ 这一句走**静音句**路径（不发第二个请求）+ **既有**的
/// `tts_upstream_400_digit_range` 警告，轮次继续；它**不并进** [`recover_sentence`]
/// 的 error 日志（两条口径不同，2026-10-15 执行单 §2）。
fn is_tts_length_rejection(err: &crate::Error) -> bool {
    err.upstream_status() == Some(400)
        && matches!(err, crate::Error::Status { body, .. } if body.contains(LOCAL_TTS_LENGTH_MARKER))
}

/// 单句失败的**分流口**（2026-10-15 执行单 §2）：可续走的失败在这里收口；其余
/// （端点 / 协议本身就坏的）仍返回 [`SynthOutcome::Failed`] 走原来的致命路径。
///
/// 还没发出过 `AudioChunk` → [`silent_sentence`]（静音、边界帧还在、正文照上屏）；
/// 已经发出过 → 只补一个**空尾块**（`first_chunk = false`）再 `SentenceVoiced`，
/// **不发第二次 HTTP**。两种都**不发** `EngineEvent::Error`（轮次不以 `Failed` 收尾），
/// 只按现成 [`ErrorKind::code`] 打一行 `tracing::error!`。
async fn recover_sentence(
    ctx: &TtsWorkerCtx,
    spec: AudioSpec,
    job: &TtsJob,
    kind: ErrorKind,
    sent_any: bool,
) -> SynthOutcome {
    if !super::speakable::kind_is_recoverable(&kind) {
        return SynthOutcome::Failed(kind);
    }
    tracing::error!(
        target: "tts",
        sentence_seq = job.sentence_seq,
        code = %kind.code(),
        "本句合成失败：本句改走静音句，worker 不退出、后续句子继续"
    );
    if !sent_any {
        return silent_sentence(ctx, spec, job).await;
    }
    let delivered = send_event(
        &ctx.event_tx,
        &ctx.cancel,
        EngineEvent::AudioChunk {
            epoch: ctx.epoch,
            ts_ms: ctx.turn_started.elapsed().as_millis() as u64,
            sentence_seq: job.sentence_seq,
            samples: Vec::new(),
            spec,
            // 已经发出过首块：这里只补末块。
            first_chunk: false,
            final_chunk: true,
        },
    )
    .await;
    if !delivered || !emit_sentence_voiced(ctx, job).await {
        return SynthOutcome::Cancelled;
    }
    SynthOutcome::Done
}

/// **静音句**收口：不发 TTS HTTP，只发一个既是首块也是末块的空 `AudioChunk`
/// 再发 `SentenceVoiced`（文本不丢，后续句子照常）。
///
/// 三个调用点：
/// ① TTS 未接入 / 纯空白句 / 碎片句（既有行为 + 2026-10-15 执行单 §3）；
/// ② 本地 TTS 明确拒收这一句（数字超范围）；
/// ③ 这一句的合成 / 解码失败属**可续走**那一类（2026-10-15 执行单 §2）——
///    由 [`recover_sentence`] 在「这一句还没发出过音频块」时转来。
async fn silent_sentence(ctx: &TtsWorkerCtx, spec: AudioSpec, job: &TtsJob) -> SynthOutcome {
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
    // 仍按「一句一单元」上屏（文本侧不受影响）。
    let voiced = delivered && emit_sentence_voiced(ctx, job).await;
    if voiced {
        SynthOutcome::Done
    } else {
        SynthOutcome::Cancelled
    }
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
            // **上屏文本**（不是送去合成的那一份）：见 [TtsJob]。
            text: job.display.clone(),
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
    // 失败续走时要发一个**空**块：规格用配置里的那份（0 样本，播放端不消费）。
    let cfg_spec = ctx.client.tts().spec;
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
            Err(e) => return recover_sentence(ctx, cfg_spec, job, ErrorKind::Tts(e), false).await,
        };
        if buf.len() + bytes.len() > MAX_WAV_BYTES {
            let kind = ErrorKind::Decode(crate::Error::InvalidAudioConfig {
                message: format!("WAV 响应超过 {MAX_WAV_BYTES} 字节上限"),
            });
            return recover_sentence(ctx, cfg_spec, job, kind, false).await;
        }
        buf.extend_from_slice(&bytes);
    }

    let parsed = match crate::audio::parse_wav_s16(&buf) {
        Ok(parsed) => parsed,
        Err(e) => return recover_sentence(ctx, cfg_spec, job, ErrorKind::Decode(e), false).await,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use reqwest::StatusCode;

    /// T1（2026-10-13 执行单）：本地 TTS 的「长度超范围」400 是**静音句**那一类；
    /// 别的 400 / 别的状态码走 [`super::super::speakable`] 的分流（可续走的继续）。
    #[test]
    fn long_digit_tts_length_400_is_the_silent_sentence_case() {
        let local = Error::status(
            StatusCode::BAD_REQUEST,
            "{\"detail\":\"超出数据范围，最长支持 16 位\"}",
        );
        assert!(
            is_tts_length_rejection(&local),
            "本地 TTS 的长度拒绝必须走静音句"
        );

        // ① 同一状态码但别的原因（空 input）：本函数仍返回 false（不走这条 400
        //    分支）；空 input 的 400 现归「可续走」那一档（断言见 speakable）。
        assert!(!is_tts_length_rejection(&Error::status(
            StatusCode::BAD_REQUEST,
            "{\"detail\":\"input 为空\"}"
        )));
        // ② 标记在、状态码不是 400：不是这一条契约。
        assert!(!is_tts_length_rejection(&Error::status(
            StatusCode::UNAUTHORIZED,
            "超出数据范围，最长支持 16 位"
        )));
        // ③ 根本不是 Status 变体。
        assert!(!is_tts_length_rejection(&Error::Sse {
            message: "x".into()
        }));
        assert!(!is_tts_length_rejection(&Error::InvalidBaseUrl {
            url: "ftp://x".into()
        }));
    }
}
