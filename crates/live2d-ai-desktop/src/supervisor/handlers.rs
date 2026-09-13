//! supervisor 的「事件→根 effect→副作用」翻译层（节点 B 第二轮裁决后拆分）。
//!
//! 本文件是 [`crate::supervisor`] 私有子模块：保留 `pub(crate)` 可见性使
//! `tests.rs`（同级）可直接调用 `handle_engine_event` / `forward_effects`
//! 完成真实闭环测试；外部 crate 仍只看到 `spawn_supervisor` /
//! [`SupervisorHandle`]。**禁止展开宏**（1 号裁决）—— 共享宏
//! `do_stop!` / `on_action_finished!` / `on_any_pcm_progress!` 留于
//! `turn.rs` 内的 [`run_one_turn`] 局部作用域。
//!
//! # 动作投影已整体移除（2026-09-11 用户裁决）
//!
//! 用户裁决：**LLM 不暴露任何工具，只做对话**（「llm 不暴露任何工具只做对话」）。
//! 据此本文件删掉了两条动作触发路径及其投影：
//! - `EngineEvent::ToolAction` → `tool_adapter::wire_to_semantic` → core 仲裁；
//! - `RootEffect::Action(..)` → `AppEvent::Render(RenderCommand::…)` 的副作用翻译。
//!
//! 也就是说：`live2d-ai-core` 的 action 子系统（`core::action` reducer、
//! capability gate、优先级仲裁、`rule_fallback`）本身**仍然存在、内部自洽且
//! 有单测**，但**不再被对话链路驱动**——它是**休眠的内部能力**，不属于本产品
//! 的成品链路（文本 → LLM → TTS → 口型 → 渲染）。按任务约定，
//! `crates/live2d-ai-core/` 本次**不动**：既不移除该子系统，也不为它保留
//! desktop 侧接线（留着空接线反而会被误读为「还在用」）。

use tokio::sync::mpsc;

use live2d_ai_core::{
    DropReason as CoreDropReason, Effect as RootEffect, Event as RootEvent, GenerationOutcome,
};
use live2d_ai_runtime::{EngineEvent, ErrorKind, TurnStatus};

use crate::app_event::{AppErrorEvent, AppEvent, ConversationUiEvent};
use crate::audio::{PreparedPcm, TryEnqueue};

use super::{Emit, RootState, root_apply};

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_engine_event(
    ev: &EngineEvent,
    root: &mut RootState,
    audio: &mut Option<Box<dyn crate::audio::PcmProducer>>,
    turn_id: u64,
    pending_pcm: &mut Option<PreparedPcm>,
    playback_started: &mut bool,
    voice_started_emitted: &mut bool,
    saw_fatal_kind: &mut bool,
    saw_llm_error: &mut bool,
    emit: &Emit,
) {
    match ev {
        EngineEvent::TextDelta { text, .. } => {
            print!("{text}");
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
            // **不再在此上屏**（2026-09-10 用户裁决：一句一单元，先完整合成再显示）。
            //
            // 这里仍是「LLM 正文增量」的原始流，只做控制台回显；
            // 真正推给 UI/WS 的文字由下面的 `SentenceVoiced` 承载 —— 那一刻该句
            // 语音**已经合成完毕**，于是文字与声音同拍，而不是领先好几秒。
            //
            // 若哪天要恢复「边生成边上屏」，把 emit 挪回这里即可（其余不动）。
        }
        EngineEvent::ReasoningDelta {
            epoch, ts_ms, text, ..
        } => {
            // 思考**直接上屏**：它没有音频可对齐（见 `ConversationUiEvent` 注释）。
            emit(AppEvent::Conversation(
                ConversationUiEvent::ReasoningDelta {
                    epoch: *epoch,
                    ts_ms: *ts_ms,
                    text: text.clone(),
                },
            ));
        }
        EngineEvent::SentenceVoiced {
            epoch, ts_ms, text, ..
        } => {
            // P1WS-1 语义保留：真实 text payload 透传给 UI/WS。
            // - 不变 core reducer（只走 emit）；
            // - 不变 TurnCompleted/双闩锁语义（文本不触发任何 root 事件）；
            // - `epoch` 来自引擎事件本身（与 `ev_epoch(&ev)` 一致），便于
            //   前端按 epoch 分组气泡；
            // - `ts_ms` 保留供节点 D 链路耗时可视化（ws 投影可选携带）。
            emit(AppEvent::Conversation(ConversationUiEvent::TextDelta {
                epoch: *epoch,
                ts_ms: *ts_ms,
                text: text.clone(),
            }));
        }
        EngineEvent::Error { kind, epoch, .. } => {
            // 2026-09-11：错误必须**同时**可被三种消费者看见——
            //   1. 后端日志（`tracing::error!` → stdout + 文件双 sink，带
            //      timestamp/level/target 与 `code=` 结构化字段；过去这里只有
            //      `println!`，文件 sink 里一个字都没有，用户「看后端日志」时
            //      什么也找不到）；
            //   2. 前端（`AppEvent::Error` → WS `error` 帧，带同一个 `code`）；
            //   3. 终端（保留原样的 `println!`，`--chat` 交互路径在读）。
            // `code` 在三条路径里是**同一个字符串**，用户才能拿界面上看到的码
            // 去日志里搜。
            let app_err = AppErrorEvent::from_kind(kind, *epoch);
            tracing::error!(
                code = %app_err.code,
                stage = %app_err.stage,
                epoch = app_err.epoch,
                fatal = app_err.fatal,
                hint = app_err.hint.as_deref().unwrap_or(""),
                "{app_err_message}",
                app_err_message = app_err.message,
            );
            println!("\n{}", app_err.log_line());
            emit(AppEvent::Error(app_err));
            // 失败归因观察（步骤 7/D8）：二者分别驱动排空策略的分支判定。
            match kind {
                ErrorKind::Llm(_) => *saw_llm_error = true,
                ErrorKind::Tts(_) | ErrorKind::Decode(_) | ErrorKind::Backpressure { .. } => {
                    *saw_fatal_kind = true;
                }
            }
        }
        EngineEvent::AudioChunk {
            epoch,
            samples,
            spec,
            sentence_seq,
            first_chunk,
            final_chunk,
            ..
        } => {
            // **空样本块**（2026-09-11 修）：引擎允许某句的末块样本数为 0
            // （句子样本数正好是 `audio_chunk_samples` 整数倍时就会出现）。
            // 过去这里直接 `return`，把那一块连同它的 `final_chunk=true` 一起丢了
            // ——于是那一句在 WS 上永远没有 `end=true`，前端等不到句尾闸门。
            //
            // 现在**只跳过声卡入环**（空样本没有可播的东西，入环会破坏
            // partial-write/排空语义），WS 广播照常进行：`build_audio_frames`
            // 会为空 PCM 发一帧只有边界、没有音频体的句界帧。
            if !samples.is_empty()
                && let Some(producer) = audio
            {
                debug_assert!(
                    pending_pcm.is_none(),
                    "背压实例化点失效：select 守卫应在读取前拦住"
                );
                // P0-2 生产路径：净化 + 转换恰一次；WouldBlock 部分写入也保留。
                let mut prepared = producer.prepare_pcm_f32(samples, *spec);
                let r = producer.try_enqueue_prepared(&mut prepared);
                // B-P0-2/B2-P0-2：accepted>0 即「播放已开始」——同步落 root
                // PlaybackStarted + VoiceStarted。幂等由 playback_started 把关。
                if r.any_accepted() && !*playback_started {
                    *playback_started = true;
                    let cur = root.epoch;
                    let _started = root_apply(
                        root,
                        RootEvent::PlaybackStarted {
                            epoch: cur,
                            turn_id: turn_id.into(),
                        },
                    );
                    emit(AppEvent::RootAudit(
                        crate::app_event::RootFact::PlaybackStarted { epoch: cur.get() },
                    ));
                    if !*voice_started_emitted {
                        *voice_started_emitted = true;
                        emit(AppEvent::Conversation(ConversationUiEvent::VoiceStarted {
                            epoch: cur.get(),
                        }));
                    }
                }
                if matches!(r, TryEnqueue::WouldBlock { .. }) {
                    *pending_pcm = Some(prepared); // 停止读下一事件：背压回传 TTS
                }
            }
            // F6-T2：PCM 绝不经 AppEvent——在 cpal 入环之外同步推 WS。
            // 线程局部回调由 `spawn_supervisor_impl` 在子线程安装；默认为 None
            //          （现网不变），仅 `LIVE2D_AI_WS_AUDIO=1` 时为 Some。
            //
            // 2026-09-11：把**句子边界**（`sentence_seq` / `first_chunk` /
            // `final_chunk`）一并透传——WS 帧的 `start`/`end` 必须按**句**给出，
            // 而不是按 epoch 记账去推（旧做法实测产出「0 个 start、13 个 end」）。
            super::AUDIO_CB.with(|cell| {
                if let Some(cb) = cell.borrow().as_ref() {
                    cb(
                        *epoch,
                        samples,
                        *spec,
                        *sentence_seq,
                        *first_chunk,
                        *final_chunk,
                    );
                }
            });
            // dry-run（无声卡）：跳过入环、永不报 Started。
        }
        // 终态事件：supervisor 以 gen_fut 返回的 TurnReport 为权威，忽略之。
        EngineEvent::Terminal { .. } => {}
    }
}

pub(crate) fn ev_epoch(ev: &EngineEvent) -> u64 {
    match ev {
        EngineEvent::TextDelta { epoch, .. }
        | EngineEvent::ReasoningDelta { epoch, .. }
        | EngineEvent::AudioChunk { epoch, .. }
        | EngineEvent::SentenceVoiced { epoch, .. }
        | EngineEvent::Error { epoch, .. }
        | EngineEvent::Terminal { epoch, .. } => *epoch,
    }
}

/// 节点 D 链路耗时埋点：从 `EngineEvent` 取出 `ts_ms`（相对本轮起点的
/// 毫秒数）。所有变体均带 `ts_ms`；返回 `Some` 即可。
pub(crate) fn ev_ts_ms(ev: &EngineEvent) -> Option<u64> {
    match ev {
        EngineEvent::TextDelta { ts_ms, .. }
        | EngineEvent::ReasoningDelta { ts_ms, .. }
        | EngineEvent::AudioChunk { ts_ms, .. }
        | EngineEvent::SentenceVoiced { ts_ms, .. }
        | EngineEvent::Error { ts_ms, .. }
        | EngineEvent::Terminal { ts_ms, .. } => Some(*ts_ms),
    }
}

pub(crate) fn emit_voice_ended_if_needed(
    playback_started: bool,
    voice_started_emitted: bool,
    epoch: u64,
    emit: &Emit,
) {
    if playback_started || voice_started_emitted {
        emit(AppEvent::Conversation(ConversationUiEvent::VoiceEnded {
            epoch,
        }));
    }
}

/// root Effect → AppEvent / 声卡副作用的唯一翻译处。
/// StopPlayback 在此真实执行打断（它只来自 stop 事务路径）。
///
/// 2026-09-11：`RootEffect::Action(..)` 四条投影分支已删除（动作系统整体移除，
/// 见文件头注）。core 若产出 Action 效果（例如 dormant 的 action reducer 被
/// 外部直接驱动），desktop 侧不再有对应副作用——这正是「不接线」的含义。
pub(crate) fn forward_effects(
    epoch: u64,
    effects: &[RootEffect],
    audio: Option<&dyn crate::audio::PcmProducer>,
    emit: &Emit,
) {
    for effect in effects {
        match effect {
            RootEffect::StartThinking { .. } => {}
            RootEffect::StopPlayback => {
                if let Some(a) = audio {
                    a.stop_and_clear();
                }
            }
            RootEffect::TurnAborted { .. } => {}
            RootEffect::GoIdle => {}
            RootEffect::TurnCompleted { outcome, .. } => {
                tracing::debug!(?outcome, "TurnCompleted（双闩锁收口）");
                emit(AppEvent::RootAudit(
                    crate::app_event::RootFact::TurnCompleted {
                        epoch,
                        outcome_completed: matches!(outcome, GenerationOutcome::Completed),
                    },
                ));
            }
            // 动作效果：2026-09-11 起不再有桌面侧投影（无 AppEvent::Render）。
            // 保留显式分支（而非 `_` 兜底）以便未来新增效果时编译器点名。
            RootEffect::Action(_) => {}
            RootEffect::Dropped { reason } => {
                match reason {
                    CoreDropReason::StaleActionCompletion { finished, .. } => {
                        tracing::debug!(?finished, "迟到的动作自然完成已被丢弃");
                    }
                    other => tracing::debug!(?other, "root 事件被确定性丢弃"),
                }
                emit(AppEvent::RootAudit(crate::app_event::RootFact::Dropped));
            }
        }
    }
}

pub(crate) fn drain_residual_events(rx: &mut mpsc::Receiver<EngineEvent>) {
    // 上限防御：正常 close 后至多几个事件；异常时也不无限自旋。
    for _ in 0..256 {
        if rx.try_recv().is_err() {
            break;
        }
    }
}

pub(crate) fn gen_outcome(status: TurnStatus) -> GenerationOutcome {
    match status {
        TurnStatus::Completed => GenerationOutcome::Completed,
        TurnStatus::Failed => GenerationOutcome::Failed,
        TurnStatus::Cancelled => GenerationOutcome::Cancelled,
    }
}
