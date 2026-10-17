//! supervisor 主事件循环 run_forever：空闲态 select（控制 / 迟到动作事实 /
//! 用户输入）+ 每轮收口后的兜底热重载。
//!
//! 从 supervisor.rs 拆出（2026-10-16，D3「先只搬家」）。run_forever 改成
//! pub(super)——唯一调用方是同级的 spawn.rs。**行为一行未改**。

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::mpsc;

use live2d_ai_core::{Event as RootEvent, State as RootState};
use live2d_ai_mod_system::ModEventTopic;
use live2d_ai_runtime::{AppSettings, ConversationEngine};

use crate::app_event::AppEvent;
use crate::session_scope::SessionScopeStore;

use super::dialogue_history;
use super::handlers;
use super::performance_wiring::{build_performance_runtime, load_motion_bias};
use super::reload::{apply_reload, build_sentence_cleaner};
use super::turn;
use super::{
    ActionFinishedFact, ControlCommand, Emit, ModEventSink, PerformanceStatsSlot, ReloadAck,
    SayRequest, SupervisorConfig, root_apply,
};

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_forever(
    config: SupervisorConfig,
    mut say_rx: mpsc::Receiver<SayRequest>,
    mut control_rx: mpsc::UnboundedReceiver<ControlCommand>,
    mut finish_rx: mpsc::UnboundedReceiver<ActionFinishedFact>,
    reload_pending: Arc<std::sync::atomic::AtomicBool>,
    current_epoch: Arc<std::sync::atomic::AtomicU64>,
    emit: Emit,
    mod_events: Option<ModEventSink>,
    session_scopes: SessionScopeStore,
    performance_stats: PerformanceStatsSlot,
    // S-001（2026-10-16）：reload 结果的回话槽（谁真执行了 reload，谁回话）。
    reload_ack: Arc<std::sync::Mutex<Option<ReloadAck>>>,
    // 3.0 队首第一片（2026-10-17）：一轮在飞标志（外部文本忙时合并用）。
    turn_in_flight: Arc<std::sync::atomic::AtomicBool>,
) {
    let SupervisorConfig {
        client,
        conversation,
        capabilities,
        mut audio,
        config_path,
        mod_events: _,
    } = config;

    let mut root = RootState::default();
    // 能力集显式注入（附带发现 #5）：Bai 支持全部六动作；默认空集会导致
    // 一切 Play 被 capability gate 拒绝——那必须是装配错误而不是静默行为。
    root.action.capabilities = capabilities;
    // 表演倾向（2026-10-17 rc4）：只读 mods.json 的
    // `mods.director.config.motion_bias`（既有键，缺省平常）；导演 Mod 的
    // enabled / 导演二路端点已不再是开关。config_path 为 None（测试 / 无配置装配）
    // → 平常。
    let motion_bias = config_path
        .as_deref()
        .map(load_motion_bias)
        .unwrap_or_default();
    // 核心第二路按句清洗（**常驻**；单一 cue 写者）：要在把 client 移进引擎**之前**
    // 装配。测试 / 手工装配用 `LlmConfig::new` → clean_tts=false → 这里 `None`，
    // 引擎走确定性清洗 + 规则表。
    let cleaner = build_sentence_cleaner(&client, &motion_bias);
    let mut engine = ConversationEngine::new(client, conversation);
    engine.set_cleaner(cleaner);
    // ---- 整段表演层装配（T8，2026-10-07；2026-10-08 起显式 opt-in）----
    // 开关 = `[performance].enabled`（缺省 false → 主链走常驻二路按句清洗）；
    // 打开后只读 `[performance]` 自己的 base_url/model（导演二路端点已删除），
    // 且本轮 0 条按句 action_cue（单 cue 写者）。
    // config_path 为 None（测试 / 无配置装配）时不装配——与「从来没有表演层」逐字一致。
    if let Some(path) = config_path.as_deref()
        && let Ok(settings) = AppSettings::load_from_path(path)
    {
        let performance = build_performance_runtime(&settings.performance);
        engine.set_performance(performance.clone());
        if let Ok(mut slot) = performance_stats.lock() {
            *slot = performance.map(|rt| rt.stats());
        }
    }
    let mut next_turn_id: u64 = 0;
    let mut quitting = false;
    let mut histories = dialogue_history::DialogueHistories::default();

    while !quitting {
        // ---- 空闲态：受理输入与控制。此时到达的「动作自然完成」事实属于迟到，
        // 直接交给 core 权威闸门产出确定性丢弃（状态不变），保持单一事实路径。
        tokio::select! {
            ctrl = control_rx.recv() => match ctrl {
                Some(ControlCommand::Quit) | None => quitting = true,
                Some(ControlCommand::Stop) => {}
                Some(ControlCommand::Reload) => {
                    // 热重载：仅在空闲态（`run_one_turn` 不在飞）处理；
                    // 失败保留旧 client 继续运行，旧 engine 不会被破坏。
                    // S-001：结果回给等待中的 HTTP 请求（没有等待者就 no-op）。
                    let outcome = apply_reload(
                        &mut engine,
                        config_path.as_deref(),
                        Some(reload_pending.as_ref()),
                        &performance_stats,
                        &mut histories,
                        &emit,
                    );
                    super::reply_reload(&reload_ack, &outcome);
                }
            },
            fact = finish_rx.recv() => {
                if let Some(fact) = fact {
                    // **Mod 事件桥（d2）**：动作自然完成 → `ActionFinished`。
                    // payload = 动作协议名（`ActionId::name()`）。
                    // 2026-09-12：树内已无消费者（director Mod 已删除）——话题保留，
                    // 因为它是 Mod API 的一部分；**不代表**有任何东西在驱动动作。
                    if let Some(f) = &mod_events {
                        // 动作事实与「哪一轮」绑定，而轮次已随事件流带过会话；
                        // 这里不需要重复携带（动作在产品路径上本就休眠）。
                        f(ModEventTopic::ActionFinished, fact.action.action.name(), None);
                    }
                    // B-P0-5（关键）：使用**事实自带的原始 epoch**原样上报；
                    // 重盖当前值会让跨代复用的迟到完成误伤新代次的同名动作。
                    let effects = root_apply(&mut root, RootEvent::ActionPlaybackFinished {
                        epoch: fact.epoch.into(),
                        action: fact.action,
                    });
                    handlers::forward_effects(root.epoch.get(), &effects, audio.as_deref(), &emit);
                }
            }
            // 2026-09-12（rc.2）：原「Mod 边界 / 外部触发」的 `action_rx` 分支已删除——
            // 它是**唯一**会把 `RootEvent::Action` 送进 core reducer 的实现，而动作在
            // 产品路径上不存在（`core-chain-baseline.md` §3.3）；core 的 action/performance
            // 子系统仍保留但**无驱动方**（待机生命体征 `IdleState` 是另一套机制）。
            request = say_rx.recv() => match request {
                Some(request) => {
                    let SayRequest { text, session } = request;
                    // 3.0 队首第一片（2026-10-17）：从这一刻到本轮收口，外部文本
                    // 端点会把新到的文本收进合并窗口、先不注入（等这一轮结束再
                    // 把整窗合成一条推一次）。置位点在本轮任何事件之前。
                    turn_in_flight.store(true, Ordering::Release);
                    next_turn_id += 1;
                    let cur_epoch = root.epoch;
                    let epoch = cur_epoch.get();
                    // **Mod 事件桥（d2）**：新 turn 提交 → `TurnStarted`。
                    // payload = 当前 turn id（话题保留给 Mod；树内无消费者）。
                    if let Some(f) = &mod_events {
                        f(
                            ModEventTopic::TurnStarted,
                            &next_turn_id.to_string(),
                            session.as_deref(),
                        );
                        // **Wave 2**：紧随其后把**本轮输入正文**交给 Mod。
                        // `TurnStarted` 的 payload 只是序号，记忆 / 导演类 Mod 需要
                        // 正文才能检索 / 判情绪（见 `topics.rs::TurnPrompt` 头注）。
                        //
                        // **L1**：第三条参数是本轮会话 id；memory 用它分桶，
                        // persona 用它决定「这张卡属于哪个会话」。
                        //
                        // **同轮生效（2026-09-15）**：这一行必须在下面那行
                        // `set_system_prompt_override` **之前**。sink 对
                        // `TurnPrompt` 走同步投递（`mod_event_sink` →
                        // `dispatch_event_and_flush`），所以走到下一行时 Mod 写的
                        // 会话注入槽（memory 的 MEMORY 块 / persona 的卡）一定已经
                        // 落表——本轮请求体就带得上它。
                        // 旧实现把这一行放在 `set_system_prompt_override` 之后，
                        // 于是注入只能等下一轮（`dispatch_event` 是 try_send，
                        // 哪个线程先跑到读表点是竞态）。
                        f(ModEventTopic::TurnPrompt, &text, session.as_deref());
                    }
                    // 本轮输入正文先于任何 text_delta 给前端（执行单 2026-10-14 §3）。
                    handlers::emit_user_line(&emit, epoch, &text);

                    // **L1 会话绑定**：本轮 system_prompt 的**唯一**决议点。
                    //
                    // 每轮都重新决议（而不是在切会话时改一次）：
                    // - 热重载会整体重建引擎，覆盖槽随之清空——每轮 set 天然自愈；
                    // - 会话表按 owner 分槽（set_owned），每轮重读保证看到最新值；
                    // - 上面刚投递过 `TurnPrompt`（同步），所以本行读到的是
                    //   **本轮的** memory / persona 写入，不是上一轮的残留。
                    //
                    // 表里没有该会话 → None → 引擎回落配置里的全局
                    // `persona.system_prompt`（降级语义，见 session_scope 头注）。
                    engine.set_system_prompt_override(
                        session_scopes.prompt_for(session.as_deref()),
                    );
                    // 对话历史按会话换桶（前为进程一份，换会话会串上一会话轮次）。
                    histories.install(&mut engine, session.as_deref());
                    // P1WS-1：开轮前镜像 epoch。Stop 事务推进 epoch 后会再次
                    // 同步写；UserSubmitted 不动 epoch（root 现状保持），但
                    // 仍把当前值同步给镜像以覆盖 boot 时 0。
                    current_epoch.store(epoch, std::sync::atomic::Ordering::Release);
                    root_apply(&mut root, RootEvent::UserSubmitted {
                        epoch: cur_epoch,
                        turn_id: next_turn_id.into(),
                        sentence_id: 1.into(),
                        text: String::new(), // 载荷不参与规则；正文走引擎请求体
                    });
                    let close = turn::run_one_turn(
                        &mut root, &mut engine, &mut audio, &mut say_rx, &mut control_rx,
                        &mut finish_rx, current_epoch.as_ref(), next_turn_id, epoch, text,
                        &mut quitting, &emit,
                    )
                    .await;
                    // 本轮收口：忙时合并窗口现在可以推出（在 TurnEnded /
                    // AssistantReplied 之前置位，因为那些是「轮末准备下一轮」，
                    // 合并线程只需要知道「上一条处理完了」）。
                    turn_in_flight.store(false, Ordering::Release);
                    // **Wave 3（2026-09-21）**：助手侧正文 → AssistantReplied。
                    //
                    // 这是「记忆摘要含助手侧」的唯一入口（见 topics.rs 该主题头注）：
                    // TurnPrompt 只有用户话、SentenceReady 是按句锚点，记忆类 Mod
                    // 需要一个「一轮的助手侧正文」。**投递非阻塞**（mod_event_sink
                    // 只对 TurnPrompt 走 dispatch_event_and_flush），慢 / 失败的
                    // Mod 绝不阻塞主链；正文先做**确定性清洗**，交给 Mod 的是上屏
                    // 口径（与送 TTS 同源）。
                    if let Some(f) = &mod_events
                        && !close.assistant_text.trim().is_empty()
                    {
                        let display =
                            live2d_ai_runtime::clean_for_tts(&close.assistant_text);
                        if !display.is_empty() {
                            let payload = serde_json::json!({
                                "turn": next_turn_id,
                                "role": "assistant",
                                "text": display,
                                "interrupted": close.interrupted,
                            })
                            .to_string();
                            f(
                                ModEventTopic::AssistantReplied,
                                &payload,
                                session.as_deref(),
                            );
                        }
                    }
                    // **Wave 3**：本轮收口 → `TurnEnded`（payload = turn id）。
                    // 发点在 `run_one_turn` 返回之后，**无论**该轮是正常收口还是
                    // `TurnStatus::Failed`——语义是「这一轮结束了」，不是「成功」。
                    // 与 `TurnPrompt` **不同**：TurnEnded 是轮末，它的语义本来就是
                    // 「为下一轮准备」——这里做的 `apply_settings` 对下一轮生效。
                    // （TurnPrompt 的会话注入是**同轮**生效，见上面的注释。）
                    if let Some(f) = &mod_events {
                        f(
                            ModEventTopic::TurnEnded,
                            &next_turn_id.to_string(),
                            session.as_deref(),
                        );
                    }
                    // 兜底：「在飞 turn 中到达的 Reload」走原子标志
                    // [`SupervisorHandle::reload_pending`]，turn.rs no-op
                    // channel 消息但**不清标志**。turn 收口后统一消费。
                    // **不 drain control_rx**：turn 内到达的 Quit/Stop 已在
                    // turn.rs select 内处理（quitting=true / do_stop），
                    // turn 收尾后仍积压的命令由外层空闲态 select 自然消费
                    // （删除旧的无条件 `while try_recv().is_ok(){}` drain——
                    // 它会吞掉恰好落在收尾边界的 Quit，导致应用不退出）。
                    // S-001：在飞期间积压的 Reload 在这里落地——同样回话
                    // （请求可能还在等；超时走了的 try_send 失败即丢弃）。
                    let outcome = apply_reload(
                        &mut engine,
                        config_path.as_deref(),
                        Some(reload_pending.as_ref()),
                        &performance_stats,
                        &mut histories,
                        &emit,
                    );
                    super::reply_reload(&reload_ack, &outcome);
                }
                None => quitting = true,
            },
        }
    }

    // ---- 关机收尾：先释放声卡（Drop 即停流），再通知主线程可以退出。
    tracing::info!("supervisor 已完成停机清理");
    drop(audio);
    emit(AppEvent::ShutdownReady);
}
