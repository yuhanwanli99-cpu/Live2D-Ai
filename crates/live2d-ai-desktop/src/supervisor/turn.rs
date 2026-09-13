//! 一轮完整生命周期 [`run_one_turn`]（节点 B 第二轮修复版）。
//!
//! 本文件从 `supervisor.rs` 整体迁出（1 号裁决：do_stop! / on_action_finished! /
//! on_any_pcm_progress! 三宏必须与 run_one_turn 同文件；只剪切、不改写宏）。
//!
//! # 行数豁免说明（用户硬约束 ≤500；本文件允许 ≤1000 头注豁免）
//!
//! 本文件当前 533 行（≤1000 区间）。曾尝试把 Stage B `pump_pending` 与
//! Stage D `drain_watch` 抽成独立 `pub(crate) async fn`，但三宏
//! `do_stop!` / `on_action_finished!` / `on_any_pcm_progress!` 引用
//! `run_one_turn` 局部状态（`root` / `pending_pcm` / `stopped` / `cancel` /
//! `say_rx` / `audio` / `emit` / `turn_id` / `playback_started` /
//! `voice_started_emitted` / `control_rx` / `finish_rx` / `quitting`），抽函数
//! 须把这些可变借用全部作为参数传入，会与 `biased select!` 内部的「同一
//! 借用上既读又写」语义发生 borrow checker 冲突；尤其 `control_rx` 与
//! `say_rx` 的 `&mut` 借用在 select 多臂上必须共存，抽函数后无法在不引入
//! `Pin` / 自引用结构的前提下保留当前时序铁律（B-P0-2/B2-P0-3/B-P0-5）。
//! 保留单文件是保证「三宏同文件、零语义变化」的唯一解。
//!
//! 时序铁律（B2-P0-1/2/3 + 复审裁决）：
//! ```text
//! gen_fut 返回 → pending 泵完成或致命退出
//!   → GenerationFinished（恰一次，只发一次且不在泵前）
//!   → Started/Drained/Cleared 按真实顺序落地
//!   → TurnCompleted 只可能由正确时序产生
//! ```
//! - **B-P0-2**：首样本入环即报 Started——`WouldBlock{accepted>0}` 也是事实；
//! - **B2-P0-3**：停滞/fault 置位后立刻退出泵循环进入清环；
//! - **B-P0-5/B2-P0-4**：动作完成事实统一经 [`on_action_finished`] 处理，
//!   四阶段逐字一致地使用事实自带原始 epoch；
//! - **P1-2/P0-4**：提交资格以 root 发出的 TurnCompleted(Completed) 效果为准；
//! - stop 不给 aborted 轮发送任何生成侧事实；
//! - root 关键事实经 [`AppEvent::RootAudit`] 外发（复审要求的可观察性）。

use std::time::{Duration, Instant};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use live2d_ai_core::{
    Effect as RootEffect, Event as RootEvent, GenerationOutcome, State as RootState,
};
use live2d_ai_runtime::{ConversationEngine, EngineEvent, TurnReport};

use crate::app_event::{AppEvent, ConversationUiEvent, TurnStageTimings};
use crate::audio::{PreparedPcm, TryEnqueue};

use super::handlers::{
    drain_residual_events, emit_voice_ended_if_needed, ev_epoch, ev_ts_ms, forward_effects,
    gen_outcome, handle_engine_event,
};
use super::{ActionFinishedFact, ControlCommand, Emit, root_apply};

/// 排空观察与 pending PCM 重试泵的轮询间隔：远低于可感知延迟，足够轻量
///（每 tick 主体是一次原子读）。
const SUPERVISOR_TICK: Duration = Duration::from_millis(20);

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_one_turn(
    root: &mut RootState,
    engine: &mut ConversationEngine,
    audio: &mut Option<Box<dyn crate::audio::PcmProducer>>,
    say_rx: &mut mpsc::Receiver<String>,
    control_rx: &mut mpsc::UnboundedReceiver<ControlCommand>,
    finish_rx: &mut mpsc::UnboundedReceiver<ActionFinishedFact>,
    current_epoch: &std::sync::atomic::AtomicU64,
    turn_id: u64,
    epoch: u64,
    user_text: String,
    quitting: &mut bool,
    emit: &Emit,
) -> bool {
    let mut pending_pcm: Option<PreparedPcm> = None;
    let mut playback_started = false;
    let mut voice_started_emitted = false;
    let mut saw_fatal_kind = false;
    let mut saw_llm_error = false;
    let mut audio_stalled = false;
    let mut stopped = false;

    // ---- 节点 D 可观测性：链路阶段耗时埋点（不侵入 core reducer）。
    // 全部以 `turn_submit = Instant::now()` 为零点；每个阶段记录首个/最末
    // 事件的真实 `Instant`，最终在收口时转换为相对毫秒。
    let turn_submit = Instant::now();
    let mut ts_first_text_delta: Option<Instant> = None;
    let mut ts_terminal: Option<Instant> = None;
    let mut ts_first_audio_chunk: Option<Instant> = None;
    let mut ts_playback_started: Option<Instant> = None;
    let mut ts_playback_drained: Option<Instant> = None;
    let mut ts_turn_completed: Option<Instant> = None;

    // B4-P0：句柄被 drop ⇒ sender 全部销毁 ⇒ receiver 永久就绪 None。
    // biased select 中若不给已关闭的通道摘臂，control 臂会每轮必赢，
    // gen_fut 永无轮询机会 → supervisor 线程忙循环无法回收。
    // 跨 select 迭代的「守卫读 / None 分支写」静态分析不可见，故允许。
    #[allow(unused_assignments)]
    let mut control_closed = false;
    #[allow(unused_assignments)]
    let mut finish_closed = false;

    // 提交资格唯一来源：root 发出的 TurnCompleted{Completed} 效果（B2 复审裁决）。
    let mut completed_completed_effect_seen = false;

    // 当前轮取消令牌：stop/quit 在推进 root epoch 后立即触发。
    let cancel = CancellationToken::new();

    // ---- B2-P0-4：动作完成事实的唯一处理器（四阶段共用；原始 epoch 原样上报）。
    macro_rules! on_action_finished {
        ($fact:expr) => {{
            let effects = root_apply(
                root,
                RootEvent::ActionPlaybackFinished {
                    epoch: $fact.epoch.into(),
                    action: $fact.action,
                },
            );
            forward_effects(root.epoch.get(), &effects, audio.as_deref(), emit);
        }};
    }

    // ---- stop 事务（D7 序）：①root epoch → ②cancel → ③clear pending →
    // ④效果执行 → ⑤VoiceEnded → ⑥清排队 Say（B-P0-6）。
    macro_rules! do_stop {
        () => {{
            if !stopped {
                let effects = root_apply(root, RootEvent::StopRequested);
                cancel.cancel();
                let _stale = pending_pcm.take();
                // P1WS-1：StopRequested 推进 epoch，镜像同步。外部 HTTP/Status
                // 端读 `current_epoch` 即可在 stop 后看到新代次。
                current_epoch.store(root.epoch.get(), std::sync::atomic::Ordering::Release);
                forward_effects(root.epoch.get(), &effects, audio.as_deref(), emit);
                emit(AppEvent::Conversation(ConversationUiEvent::NewEpoch {
                    epoch: root.epoch.get(),
                }));
                emit(AppEvent::Conversation(ConversationUiEvent::VoiceEnded {
                    epoch: root.epoch.get(),
                }));
                while say_rx.try_recv().is_ok() {}
                stopped = true;
            }
        }};
    }

    // ---- B-P0-2 核心：任何样本入环（含部分写入）⇒ 立刻报 PlaybackStarted
    // 并发 VoiceStarted；幂等。
    macro_rules! on_any_pcm_progress {
        ($result:expr) => {{
            match &$result {
                r if r.any_accepted() && !playback_started => {
                    playback_started = true;
                    // 节点 D：首样本入环时间戳（供 audio_play_ms）。
                    ts_playback_started = Some(Instant::now());
                    let cur = root.epoch;
                    let _fx = root_apply(
                        root,
                        RootEvent::PlaybackStarted {
                            epoch: cur,
                            turn_id: turn_id.into(),
                        },
                    );
                    emit(AppEvent::RootAudit(
                        crate::app_event::RootFact::PlaybackStarted { epoch: cur.get() },
                    ));
                    if !voice_started_emitted {
                        voice_started_emitted = true;
                        emit(AppEvent::Conversation(ConversationUiEvent::VoiceStarted {
                            epoch: cur.get(),
                        }));
                    }
                }
                _ => {}
            }
        }};
    }

    let (event_tx, mut event_rx) =
        mpsc::channel::<EngineEvent>(engine.config().tts_queue_capacity.max(4) + 8);

    // ================= 阶段 A：生成 + 即时 PCM 泵（engine 独占借用作用域） ======
    struct GenOut {
        report: TurnReport,
        stopped: bool,
        saw_llm_error: bool,
        saw_fatal_kind: bool,
    }
    let turn_phase: GenOut = {
        // gen 臂是唯一 break 出口且必先赋值；跨 select 静态不可见。
        #[allow(unused_assignments)]
        let mut report_slot: Option<TurnReport> = None;
        // 写（None 分支）/读（守卫条件）跨 select 迭代，静态分析不可见。
        #[allow(unused_assignments)]
        let mut events_closed = false;

        let gen_fut = engine.run_turn(epoch, &user_text, event_tx, cancel.child_token());
        tokio::pin!(gen_fut);

        'generating: loop {
            tokio::select! {
                biased;
                maybe_ctrl = control_rx.recv(), if !control_closed => {
                    // B4-P0：通道关闭（句柄被 drop）与 Quit 同义；摘臂后继续
                    // 轮询 gen_fut 至自然返回，绝不 abandon（D2 契约）。
                    match maybe_ctrl {
                        Some(ControlCommand::Stop) => do_stop!(),
                        Some(ControlCommand::Quit) => {
                            *quitting = true;
                            do_stop!();
                        }
                        // 在飞 turn 中到达：丢弃——空闲态 select 会自然处理。
                        // 最小实现：直接 no-op（rebuild 仅在 idle 发生）。
                        Some(ControlCommand::Reload) => {}
                        None => {
                            *quitting = true;
                            control_closed = true;
                            do_stop!();
                        }
                    }
                }
                maybe_fact = finish_rx.recv(), if !finish_closed => {
                    match maybe_fact {
                        Some(fact) => on_action_finished!(fact),
                        None => finish_closed = true, // 摘臂防 biased 饿死
                    }
                }
                _ = tokio::time::sleep(SUPERVISOR_TICK) => {
                    // B-P0-4：常驻健康观察优先；fault ⇒ 断掉 LLM/TTS 等待链。
                    if audio.as_deref().is_some_and(|a| !a.healthy()) {
                        saw_fatal_kind = true;
                        cancel.cancel();
                    } else if let (Some(a), Some(mut prep)) =
                        (audio.as_mut(), pending_pcm.take())
                    {
                        let r = a.try_enqueue_prepared(&mut prep);
                        on_any_pcm_progress!(r);
                        if matches!(r, TryEnqueue::WouldBlock { .. }) {
                            pending_pcm = Some(prep);
                        }
                    }
                }
                maybe_ev = event_rx.recv(),
                  if pending_pcm.is_none() && !events_closed =>
                {
                    match maybe_ev {
                        Some(ev) => {
                            // 代次防御闸门（权威闸在 core）。
                            if ev_epoch(&ev) == root.epoch.get() {
                                // 节点 D：链路阶段埋点——记录首次/末次时间戳。
                                // ts_ms 相对 engine `turn_started`（= supervisor
                                // `turn_submit`），故用 `now = turn_submit + ts_ms` 推
                                // 回真实 Instant，再与其它阶段在同一时间轴上做差。
                                if let Some(ts) = ev_ts_ms(&ev) {
                                    let at = turn_submit + Duration::from_millis(ts);
                                    match &ev {
                                        EngineEvent::TextDelta { .. } => {
                                            if ts_first_text_delta.is_none() {
                                                ts_first_text_delta = Some(at);
                                            }
                                        }
                                        EngineEvent::AudioChunk { .. } => {
                                            if ts_first_audio_chunk.is_none() {
                                                ts_first_audio_chunk = Some(at);
                                            }
                                        }
                                        EngineEvent::Terminal { .. } => {
                                            ts_terminal = Some(at);
                                        }
                                        _ => {}
                                    }
                                }
                                handle_engine_event(
                                    &ev, root, audio, turn_id,
                                    &mut pending_pcm,
                                    &mut playback_started, &mut voice_started_emitted,
                                    &mut saw_fatal_kind, &mut saw_llm_error, emit,
                                );
                            }
                        }
                        None => events_closed = true, // 摘臂防 biased 饿死
                    }
                }
                rep = &mut gen_fut => {
                    report_slot = Some(rep);
                    // 节点 D 收口兜底：`gen_fut` 返回意味着 engine 已 send
                    // 完 Terminal（`send_terminal` 在 `run_turn` 末尾）——
                    // 若上面的 `event_rx.recv()` 分支未抢到该事件，**兜底**
                    // 记录近似时间戳，避免 `llm_total_ms` 误为 0。
                    if ts_terminal.is_none() {
                        ts_terminal = Some(Instant::now());
                    }
                    break 'generating;
                }
            }
        }
        // ---- 生成返回后**必须先把通道里剩下的事件按正常路径处理掉**
        //      （2026-09-13 rc.3 N0 修，真机抓到的缺陷）----
        //
        // `gen_fut` 的**最后一次轮询**可能在返回前同步塞进若干事件
        // （`send_event` 在缓冲未满时立即完成）。而 biased select 一旦选中
        // `gen_fut` 臂就立刻返回，**不会**回头再 poll 上面的 `event_rx` 臂——
        // 于是这批事件留在通道里，被后面的 `drain_residual_events` **静默丢掉**。
        //
        // 真机实测（TTS 传输失败）：`EngineEvent::TextFallback`（**已经生成的
        // 整轮正文**）正是这样被丢掉的——用户看到「（生成失败）」而不是正文。
        // 顺序不变、语义不变：仍走同一个 `handle_engine_event`。
        for _ in 0..256 {
            let Ok(ev) = event_rx.try_recv() else { break };
            if ev_epoch(&ev) == root.epoch.get() {
                handle_engine_event(
                    &ev,
                    root,
                    audio,
                    turn_id,
                    &mut pending_pcm,
                    &mut playback_started,
                    &mut voice_started_emitted,
                    &mut saw_fatal_kind,
                    &mut saw_llm_error,
                    emit,
                );
            }
        }
        GenOut {
            report: report_slot.take().expect("生成阶段必须以 TurnReport 返回"),
            stopped,
            saw_llm_error,
            saw_fatal_kind,
        }
    }; // ← engine 借用随块结束释放

    if turn_phase.stopped {
        drain_residual_events(&mut event_rx);
        return false;
    }

    // ================= 阶段 B：pending PCM 收尾泵（B2-P0-3 可取消） ============
    // 生成返回≠全部 PCM 入环；此阶段不读引擎事件、不发任何 root 生成侧事实，
    // 只负责把最后一段 prepared 写完或判定停滞/故障后退出。
    const MAX_STALL: u32 = 250; // 连续 5s @20ms 零进展 ⇒ AudioStalled 致命
    let mut stall_ticks = 0u32;
    while let Some(mut prep) = pending_pcm.take() {
        let r = audio.as_mut().map(|a| a.try_enqueue_prepared(&mut prep));
        let accepted_now = match &r {
            Some(TryEnqueue::Enqueued { accepted }) => *accepted,
            Some(TryEnqueue::WouldBlock { accepted }) => *accepted,
            None => 0,
        };
        on_any_pcm_progress!(match &r {
            Some(r) => *r,
            None => crate::audio::TryEnqueue::Enqueued { accepted: 0 },
        });
        // B3-P0-1：WouldBlock{accepted} 的剩余样本**必须放回**下一轮继续泵送，
        // 否则尾段被静默截断；仅 Enqueued/None（dry-run）才结束本 prepared。
        match r {
            Some(TryEnqueue::WouldBlock { .. }) => {
                if accepted_now > 0 {
                    stall_ticks = 0; // 有真实进展：重置「连续零进展」计时
                }
                pending_pcm = Some(prep);
            }
            _ => break,
        }
        // 可取消等待：control / finish / 健康 / 停滞哨兵全部在场（B2-P0-3）。
        tokio::select! {
            biased;
            maybe_ctrl = control_rx.recv(), if !control_closed => {
                // B4-P0：通道关闭按 Quit 处理。此处 break 后即 return（stopped
                // 检查），无需摘臂——写 control_closed 是无用赋值（clippy）。
                match maybe_ctrl {
                    Some(ControlCommand::Stop) => do_stop!(),
                    Some(ControlCommand::Quit) => {
                        *quitting = true;
                        do_stop!();
                    }
                    // 在飞 turn（Stage B 泵期）收到 Reload：丢弃，留在 idle 重建。
                    Some(ControlCommand::Reload) => {}
                    None => {
                        *quitting = true;
                        do_stop!();
                    }
                }
                break;
            }
            maybe_fact = finish_rx.recv(), if !finish_closed => {
                match maybe_fact {
                    Some(fact) => on_action_finished!(fact),
                    None => finish_closed = true, // 摘臂防 biased 饿死
                }
            }
            _ = tokio::time::sleep(SUPERVISOR_TICK) => {
                stall_ticks += 1;
                if audio.as_deref().is_some_and(|a| !a.healthy())
                    || stall_ticks >= MAX_STALL
                {
                    saw_fatal_kind = true;
                    audio_stalled = true;
                    break;
                }
            }
        }
        if stopped {
            break;
        }
    }
    if stopped {
        drain_residual_events(&mut event_rx);
        return false;
    }

    // ================= 阶段 C：生成侧终态落地（恰一次；放最晚＝根因修复） ======
    // B3-P0-2：事实权威必须与 root 一致——声卡 fault/stall 是**内部致命**，
    // 无论引擎自身返回什么都归一化为 Failed；仅用户取消才允许 Cancelled，
    // 仅 LLM 中途失败才沿用引擎 Failed，真实成功才 Completed。
    //
    // 补强（复审 P0-2 场景 B）：生成期声卡 fault 可能恰好卡在 gen_fut 完成
    // 的瞬间，20ms tick 观察尚未置位 saw_fatal_kind 就到 Stage C——若此时
    // 直接沿用引擎 status 会错误定格为 Completed，Stage D 补发 Cleared 也
    // 无法纠正 root 的权威 outcome。因此 Stage C 判定前**同步健康检查**一次
    // （原子读，零成本），把 fault/stall 在终态定格前归一化为 Failed。
    if audio.as_deref().is_some_and(|a| !a.healthy()) {
        saw_fatal_kind = true;
        audio_stalled = true;
    }
    let outcome = if saw_fatal_kind || audio_stalled {
        GenerationOutcome::Failed
    } else {
        gen_outcome(turn_phase.report.status)
    };
    {
        let cur = root.epoch;
        let fx = root_apply(
            root,
            RootEvent::GenerationFinished {
                epoch: cur,
                turn_id: turn_id.into(),
                outcome,
            },
        );
        for e in &fx {
            if let RootEffect::TurnCompleted { outcome: oc, .. } = e
                && matches!(oc, GenerationOutcome::Completed)
            {
                completed_completed_effect_seen = true;
                ts_turn_completed = Some(Instant::now());
            }
        }
        forward_effects(root.epoch.get(), &fx, audio.as_deref(), emit);
        emit(AppEvent::RootAudit(
            crate::app_event::RootFact::GenerationFinished {
                epoch: cur.get(),
                completed: matches!(outcome, GenerationOutcome::Completed),
            },
        ));
    }

    // ================= 阶段 D：排空观察（审批补充裁决的四条件门控） ============
    'drain_watch: loop {
        let audio_faulted = audio.as_deref().is_some_and(|a| !a.healthy());
        // fatal 证据聚合：显式错误类 ∨ 故障 ∨ stalled ∨ 泵期破坏。
        let fatal_now = saw_fatal_kind || audio_faulted || audio_stalled;

        // Cleared 规则修订：只要曾报过 Started，致命收尾必恰一次 PlaybackCleared
        //（与 ring 是否恰好已空无关）。
        if fatal_now {
            if let Some(a) = audio.as_deref() {
                a.stop_and_clear();
            }
            if playback_started {
                let cur = root.epoch;
                let fx = root_apply(
                    root,
                    RootEvent::PlaybackCleared {
                        epoch: cur,
                        turn_id: turn_id.into(),
                    },
                );
                for e in &fx {
                    if let RootEffect::TurnCompleted { outcome: oc, .. } = e
                        && matches!(oc, GenerationOutcome::Completed)
                    {
                        completed_completed_effect_seen = true;
                        ts_turn_completed = Some(Instant::now());
                    }
                }
                // 事实投影顺序：先 Cleared 物理事实，再 TurnCompleted 逻辑收口
                //（同 Drained 路径的对称约定）。
                emit(AppEvent::RootAudit(
                    crate::app_event::RootFact::PlaybackCleared { epoch: cur.get() },
                ));
                forward_effects(root.epoch.get(), &fx, audio.as_deref(), emit);
            }
            emit_voice_ended_if_needed(
                playback_started,
                voice_started_emitted,
                root.epoch.get(),
                emit,
            );
            break 'drain_watch;
        }

        let ring_drained = audio.as_deref().is_some_and(|a| a.is_drained());
        let healthy = audio.as_deref().map(|a| a.healthy()).unwrap_or(true);
        // 最终 Drained 四条件（审批补充裁决）：缺一不可。
        if pending_pcm.is_none() && playback_started && ring_drained && healthy {
            let cur = root.epoch;
            // 节点 D：排空时间戳（供 audio_play_ms 与 turn_total_ms）。
            ts_playback_drained = Some(Instant::now());
            let fx = root_apply(
                root,
                RootEvent::PlaybackDrained {
                    epoch: cur,
                    turn_id: turn_id.into(),
                },
            );
            for e in &fx {
                if let RootEffect::TurnCompleted { outcome: oc, .. } = e
                    && matches!(oc, GenerationOutcome::Completed)
                {
                    completed_completed_effect_seen = true;
                    ts_turn_completed = Some(Instant::now());
                }
            }
            // 事实投影顺序：先 Drained 物理事实，再 TurnCompleted 逻辑收口——
            // root Effect 在 forward_effects 内统一转发，遵循「先事实后效果」
            // 的可观察性契约。
            emit(AppEvent::RootAudit(
                crate::app_event::RootFact::PlaybackDrained { epoch: cur.get() },
            ));
            forward_effects(root.epoch.get(), &fx, audio.as_deref(), emit);
            emit_voice_ended_if_needed(
                playback_started,
                voice_started_emitted,
                root.epoch.get(),
                emit,
            );
            break 'drain_watch;
        }
        if !playback_started && pending_pcm.is_none() {
            break 'drain_watch; // dry-run 文本模式 / 空回答：NeverStarted 已是终态
        }

        tokio::select! {
            biased;
            maybe_ctrl = control_rx.recv(), if !control_closed => {
                // B4-P0：通道关闭按 Quit 处理并摘臂。
                match maybe_ctrl {
                    Some(ControlCommand::Stop) => do_stop!(),
                    Some(ControlCommand::Quit) => {
                        *quitting = true;
                        do_stop!();
                    }
                    // Stage D 排空期间收到 Reload：丢弃，留在 idle 重建。
                    Some(ControlCommand::Reload) => {}
                    None => {
                        // B4-P0：通道关闭按 Quit 处理。break 后即 return，
                        // 无需摘臂——写 control_closed 是无用赋值（clippy）。
                        *quitting = true;
                        do_stop!();
                    }
                }
                break 'drain_watch;
            }
            maybe_fact = finish_rx.recv(), if !finish_closed => {
                match maybe_fact {
                    Some(fact) => on_action_finished!(fact),
                    None => finish_closed = true, // 摘臂防 biased 饿死
                }
            }
            _ = tokio::time::sleep(SUPERVISOR_TICK) => {}
        }
    }
    drain_residual_events(&mut event_rx);

    // B3-P0-3：stop 事务完成后旧 turn 即刻终结——禁止提交、fallback、任何
    // 生成侧副作用（stop 后 root 已推进到新 epoch，旧 turn 的 fallback 若
    // 继续执行会被盖上新代次，形成「stop 又复活动作」的竞态）。
    if stopped {
        return false;
    }

    if turn_phase.saw_llm_error && !turn_phase.saw_fatal_kind && !turn_phase.stopped {
        println!("[turn] LLM 中途失败；已生成的语音继续播放完毕，本轮不进入记忆。");
    }

    tracing::debug!(turn = turn_id, ?outcome, stopped, "turn 收口");

    // ================= P0-4 提交资格（简单直接；B3 复审裁决） ==================
    // stop 权威状态取**当前** `stopped`（do_stop! 兜底也可能在此前置位），
    // 不依赖 Stage A 快照 `turn_phase.stopped`。
    let commit_allowed = !stopped
        && matches!(outcome, GenerationOutcome::Completed)
        && completed_completed_effect_seen
        && audio.as_deref().map(|a| a.healthy()).unwrap_or(true)
        && !turn_phase.saw_fatal_kind
        && !audio_stalled;
    if commit_allowed {
        engine.commit_completed_turn(&user_text, &turn_phase.report.assistant_text);
    }

    // 2026-09-11 用户裁决：LLM 不暴露任何工具、只做对话——动作系统整体移除。
    // 这里原有两块动作触发路径已**整块删除**：
    //   1. `EngineEvent::ToolAction` → `tool_adapter::wire_to_semantic` → core 仲裁；
    //   2. **D11 文本规则 fallback**（LLM 没调工具时按正文规则 `rule_fallback`
    //      触发动作，`ActionSource::RuleFallback`）——它是第二条触发源，会绕过
    //      工具适配器直接驱动动作，故一并拆除（连带
    //      `handlers::content_worth_fallback` 判据与 `accepted_tool_seen` 门闩）。
    // core 的 action 子系统本身保留在 `live2d-ai-core`（内部自洽、有单测），
    // 但不再被本链路驱动——它是休眠能力，不属于「文本 → LLM → TTS → 口型 →
    // 渲染」这条成品链路。

    // 节点 D 链路耗时埋点收口：把所有阶段时间戳转换为相对 `turn_submit`
    // 的毫秒数；阶段未触达则保持 0（与 `Default` 一致）。stopped 路径
    // 已 early return，本分支是「收尾成功」语义——非 stop 路径，**纯附加**
    // 不改变 TurnCompleted/双闩锁/epoch 判定。
    let elapsed_ms = |at: Option<Instant>| -> u64 {
        at.map(|t| t.saturating_duration_since(turn_submit).as_millis() as u64)
            .unwrap_or(0)
    };
    let tts_synth_ms = if let (Some(t_audio), Some(t_term)) = (ts_first_audio_chunk, ts_terminal) {
        t_term.saturating_duration_since(t_audio).as_millis() as u64
    } else {
        0
    };
    let audio_play_ms = match (ts_playback_started, ts_playback_drained) {
        (Some(s), Some(d)) => d.saturating_duration_since(s).as_millis() as u64,
        _ => 0,
    };
    let timings = TurnStageTimings {
        llm_first_token_ms: elapsed_ms(ts_first_text_delta),
        llm_total_ms: elapsed_ms(ts_terminal),
        tts_synth_ms,
        tts_first_chunk_ms: elapsed_ms(ts_first_audio_chunk),
        audio_play_ms,
        turn_total_ms: elapsed_ms(ts_turn_completed),
    };
    emit(AppEvent::RootAudit(
        crate::app_event::RootFact::TurnStages {
            epoch: root.epoch.get(),
            timings,
        },
    ));

    // 正常完成**不**发 NewEpoch：epoch 未变，前端已由
    // `turn_state{completed}` 收口当前 assistant bubble；
    // 再发 new_epoch 会让前端误开一个空 bubble。
    // NewEpoch 只由 stop 路径（`do_stop!`）在真正推进 epoch 时发送。
    !stopped
}
