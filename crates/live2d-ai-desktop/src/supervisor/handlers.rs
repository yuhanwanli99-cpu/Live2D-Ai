//! supervisor 的「事件→根 effect→副作用」翻译层（节点 B 第二轮裁决后拆分）。
//!
//! 本文件是 [`crate::supervisor`] 私有子模块：保留 `pub(crate)` 可见性使
//! `tests.rs`（同级）可直接调用 `handle_engine_event` / `forward_effects`
//! 完成 6 条 P0 真实闭环测试；外部 crate 仍只看到 `spawn_supervisor` /
//! [`SupervisorHandle`]。**禁止展开宏**（1 号裁决）—— 共享宏
//! `do_stop!` / `on_action_finished!` / `on_any_pcm_progress!` 留于
//! `turn.rs` 内的 [`run_one_turn`] 局部作用域。

use tokio::sync::mpsc;

use live2d_ai_core::{
    ActionCommand, ActionEffect, DropReason as CoreDropReason, Effect as RootEffect,
    Event as RootEvent, GenerationOutcome,
};
use live2d_ai_runtime::{EngineEvent, ErrorKind, TurnStatus};

use crate::app_event::{AppEvent, ConversationUiEvent, RenderCommand};
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
    accepted_tool_seen: &mut bool,
    assistant_text_len: &mut usize,
    saw_fatal_kind: &mut bool,
    saw_llm_error: &mut bool,
    emit: &Emit,
) {
    match ev {
        EngineEvent::TextDelta { epoch, ts_ms, text } => {
            *assistant_text_len += text.chars().count();
            print!("{text}");
            use std::io::Write as _;
            let _ = std::io::stdout().flush();
            // P1WS-1：把真实 LLM 流式文本透传给 UI/WS。
            // - 不变 core reducer（只走 emit）；
            // - 不变 TurnCompleted/双闩锁语义（TextDelta 不触发任何 root 事件）；
            // - `epoch` 来自引擎事件本身（与 `ev_epoch(&ev)` 一致），便于
            //   前端按 epoch 分组气泡；
            // - `ts_ms` 保留供节点 D 链路耗时可视化（ws 投影可选携带）。
            emit(AppEvent::Conversation(ConversationUiEvent::TextDelta {
                epoch: *epoch,
                ts_ms: *ts_ms,
                text: text.clone(),
            }));
        }
        EngineEvent::ToolAction { args, .. } => {
            // D11 第一步：wire 合法性（非法 strength 直接拒绝、绝不 clamp）。
            match crate::tool_adapter::wire_to_semantic(args.clone()) {
                Ok(sa) => {
                    let cur = root.epoch;
                    let effects = root_apply(
                        root,
                        RootEvent::Action {
                            epoch: cur,
                            command: ActionCommand::Play { action: sa },
                        },
                    );
                    // 第二步：「被接受」＝根产出 Start/Transition；capability
                    // 缺失/幂等/低优先级拒绝都不算接受、也不抑制 fallback。
                    let accepted = effects.iter().any(|e| {
                        matches!(
                            e,
                            RootEffect::Action(ActionEffect::Start { .. })
                                | RootEffect::Action(ActionEffect::Transition { .. })
                        )
                    });
                    if accepted {
                        *accepted_tool_seen = true;
                    }
                    forward_effects(root.epoch.get(), &effects, audio.as_deref(), emit);
                }
                Err(reason) => println!("\n[tool] {reason}"),
            }
        }
        EngineEvent::Error { kind, .. } => {
            println!("\n[error] {kind}");
            // 失败归因观察（步骤 7/D8）：二者分别驱动排空策略的分支判定。
            match kind {
                ErrorKind::Llm(_) => *saw_llm_error = true,
                ErrorKind::Tts(_) | ErrorKind::Decode(_) | ErrorKind::Backpressure { .. } => {
                    *saw_fatal_kind = true;
                }
                ErrorKind::IncompleteTools(_) => {}
            }
        }
        EngineEvent::AudioChunk { samples, spec, .. } => {
            if samples.is_empty() {
                return; // 句尾 final_chunk 允许为空：无音频语义
            }
            if let Some(producer) = audio {
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
            // dry-run（无声卡）：跳过入环、永不报 Started。
        }
        // 终态事件：supervisor 以 gen_fut 返回的 TurnReport 为权威，忽略之。
        EngineEvent::Terminal { .. } => {}
    }
}

pub(crate) fn ev_epoch(ev: &EngineEvent) -> u64 {
    match ev {
        EngineEvent::TextDelta { epoch, .. }
        | EngineEvent::ToolAction { epoch, .. }
        | EngineEvent::AudioChunk { epoch, .. }
        | EngineEvent::Error { epoch, .. }
        | EngineEvent::Terminal { epoch, .. } => *epoch,
    }
}

/// 节点 D 链路耗时埋点：从 `EngineEvent` 取出 `ts_ms`（相对本轮起点的
/// 毫秒数）。所有变体均带 `ts_ms`；返回 `Some` 即可。
pub(crate) fn ev_ts_ms(ev: &EngineEvent) -> Option<u64> {
    match ev {
        EngineEvent::TextDelta { ts_ms, .. }
        | EngineEvent::ToolAction { ts_ms, .. }
        | EngineEvent::AudioChunk { ts_ms, .. }
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
            RootEffect::Action(ActionEffect::Start { action }) => {
                emit(AppEvent::Render(RenderCommand::Perform {
                    epoch,
                    action: *action,
                }));
            }
            RootEffect::Action(ActionEffect::Transition { to, .. }) => {
                emit(AppEvent::Render(RenderCommand::Perform {
                    epoch,
                    action: *to,
                }));
            }
            RootEffect::Action(ActionEffect::End { .. }) => {
                emit(AppEvent::Render(RenderCommand::CeasePerforming { epoch }));
            }
            RootEffect::Action(ActionEffect::Dropped { reason }) => {
                tracing::debug!(?reason, "动作指令被 core 仲裁拒绝");
            }
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

pub(crate) fn content_worth_fallback(
    outcome: GenerationOutcome,
    assistant_text_len: usize,
) -> bool {
    matches!(
        outcome,
        GenerationOutcome::Completed | GenerationOutcome::Failed
    ) && assistant_text_len > 0
}
