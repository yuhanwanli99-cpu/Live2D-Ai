//! supervisor 线程装配：通道、线程局部回调与 spawn_supervisor 家族。
//!
//! 从 supervisor.rs 拆出（2026-10-16，D3「先只搬家」）：该文件顶在 1000 行的
//! 豁免上，而 RATCHET_SRC_RS_500 不能涨。**行为一行未改**——只是宿主从父模块
//! 换成 super::spawn；父模块把 ModEventSink / AudioCb / 两个线程局部与两个
//! spawn_supervisor* 原样再导出，所以 crate::supervisor::X 与 handlers.rs 的
//! super::MOD_EVENT_CB 路径全部不变。

use std::sync::Arc;

use tokio::sync::mpsc;

use live2d_ai_mod_system::ModEventTopic;

use crate::app_event::AppEvent;
use crate::session_scope::SessionScopeStore;

use super::{
    Emit, PerformanceStatsSlot, ReloadAck, SupervisorConfig, SupervisorHandle,
    project_conversation_to_mod, run::run_forever,
};

/// host→Mod 事件投递回调（`SupervisorConfig.mod_events`）。
///
/// 由 supervisor 在 turn 生命周期点调用（`dispatch_event` 非阻塞，满则丢弃），
/// 不携带密钥；payload 为 turn/action/voice 等非敏感字符串。引入 Type alias
/// 以消解 clippy「very complex type」告警。
/// 第三个参数是**会话 id**（L1 基座）：`Some(id)` = 该事件属于某会话；
/// `None` = 调用方没带会话。Mod 侧经 `ModRuntime::on_scoped_event` 收到它。
pub type ModEventSink = Arc<dyn Fn(ModEventTopic, &str, Option<&str>) + Send + Sync>;

/// 音频帧广播回调的线程局部载体。
///
/// F6-T2 接线（WS 音频广播）：**PCM 绝不经 AppEvent**（见本模块头注），因此
/// 不能走 `emit` 闭包。为把 `emit_audio` 植入 `handle_engine_event`（其签名由
/// `turn.rs`/test 共享，不能改动）之内，`spawn_supervisor_impl` 在**子线程入
/// 口**把回调存入本线程局部；`handlers::handle_engine_event` 在 `AudioChunk`
/// 分支读取之。生产装配（cli_entry）在 `LIVE2D_AI_WS_AUDIO=1` 时传入 Some，否则
/// 传 None——默认行为与 `emit` 分支一致（关闭 WS 音频，仅 cpal 播放）。
/// 末尾三个参数是**句子边界**：`sentence_seq` / `first_chunk` / `final_chunk`
/// （2026-09-11 新增）。WS 帧的 `start`/`end` 必须按**句**给出——见
/// [`crate::web_api::ws::build_audio_frames`]。
pub(crate) type AudioCb =
    dyn Fn(u64, &[f32], live2d_ai_runtime::AudioSpec, u64, bool, bool) + Send + Sync;
thread_local! {
    pub(crate) static AUDIO_CB: std::cell::RefCell<Option<Arc<AudioCb>>> = const { std::cell::RefCell::new(None) };
    /// P1-2（2026-09-16）：Mod 事件桥的线程局部载体。
    ///
    /// `handlers::handle_engine_event` 在 `EngineEvent::SentenceReady` 分支读它，
    /// 把「该句即将送 TTS」投给 Mod（导演按句投 cue）。**非阻塞**（dispatch_event
    /// 是 try_send）：主链绝不等待导演。与 AUDIO_CB 同款：签名由 turn.rs/test
    /// 共享，不能改，所以走线程局部而不是加参数。
    pub(crate) static MOD_EVENT_CB: std::cell::RefCell<Option<ModEventSink>> = const { std::cell::RefCell::new(None) };
}

/// 启动 supervisor：独立 OS 线程 + 独立 Tokio current-thread runtime（D1）。
///
/// **向后兼容**：保持原 2-arg 签名 ——不设置 `emit_audio`（默认 `None`，
/// 仅 cpal 播放）。F6-T2 接线需 WS 音频广播时，用
/// [`spawn_supervisor_with_audio`]（传入 `Some` 闭包）。
pub fn spawn_supervisor(
    config: SupervisorConfig,
    emit: impl Fn(AppEvent) + Send + Sync + 'static,
) -> SupervisorHandle {
    spawn_supervisor_impl(config, emit, None)
}

/// 启动 supervisor（带 WS 音频广播回调）——F6-T2 接线。
///
/// `emit_audio` 为 `Some` 时，PCM 经 cpal 播放的同时被推送到 WS 广播
/// （`Broadcaster::broadcast_audio_frames`）；为 `None` 时仅 cpal 播放
/// （等价于 [`spawn_supervisor`]）。双声问题消解见 `cli_entry::build_web_supervisor`
/// 文档 ——默认闭 `LIVE2D_AI_WS_AUDIO=1` 时才开启。
pub fn spawn_supervisor_with_audio(
    config: SupervisorConfig,
    emit: impl Fn(AppEvent) + Send + Sync + 'static,
    emit_audio: Arc<AudioCb>,
) -> SupervisorHandle {
    spawn_supervisor_impl(config, emit, Some(emit_audio))
}

/// 共享子线程入口：`emit_audio` 以 `Option<Arc<…>>` 传入。
/// 子线程入口安装线程局部回调，供 `handlers::handle_engine_event` 在
/// `AudioChunk` 分支读取（PCM 绝不经 AppEvent，详见本模块头注）。
fn spawn_supervisor_impl(
    config: SupervisorConfig,
    emit: impl Fn(AppEvent) + Send + Sync + 'static,
    emit_audio: Option<Arc<AudioCb>>,
) -> SupervisorHandle {
    let (say_tx, say_rx) = mpsc::channel(1);
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    let (finish_tx, finish_rx) = mpsc::unbounded_channel();
    let reload_pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // L1 基座：会话作用域表。**supervisor 线程与 handle 共享同一个 Arc**——
    // HTTP/mod 侧写、supervisor 每轮读，因此「切会话立刻生效（下一轮）」。
    let session_scopes = SessionScopeStore::new();
    let session_scopes_for_thread = session_scopes.clone();
    // P1WS-1：当前 epoch 镜像（外部 HTTP/Status 端读这里）；启动时 = 0
    // （与 `RootState::default().epoch.get()` 一致）。
    let current_epoch = Arc::new(std::sync::atomic::AtomicU64::new(0));
    // 表演层计数槽（2026-09-22）：supervisor 线程写（装配 / 热重载），
    // handle 读（`GET /api/v1/app/status`）。
    let performance_stats: PerformanceStatsSlot = Arc::new(std::sync::Mutex::new(None));
    let performance_stats_for_thread = Arc::clone(&performance_stats);
    // S-001（2026-10-16）：热重载结果的回话槽（handle 装、supervisor 线程回）。
    let reload_ack: Arc<std::sync::Mutex<Option<ReloadAck>>> =
        Arc::new(std::sync::Mutex::new(None));
    let reload_ack_for_thread = Arc::clone(&reload_ack);
    // 3.0 队首第一片（2026-10-17）：一轮在飞标志。外部文本端点据此把忙时
    // 文本收进合并窗口、先不注入；supervisor 线程在 `run_one_turn` 前后置位。
    let turn_in_flight = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let turn_in_flight_for_thread = Arc::clone(&turn_in_flight);

    let emit: Emit = Arc::new(emit);
    // **Mod 事件桥（d2）**：包裹 AppEvent 发射——转发给 AppEvent 消费端的
    // 同时，把 `AppEvent::Conversation` 三变体投影为对应 `ModEventTopic`
    // 投递给 Mod worker（`dispatch_event` 非阻塞，满则丢弃）。`mod_events`
    // 为 `Option<Arc<F>>`，测试/无 Mod 时传 None（包裹闭包内 no-op）。
    let mod_events_for_emit = config.mod_events.clone();
    let emit: Emit = Arc::new(move |ev: AppEvent| {
        emit(ev.clone());
        if let Some(f) = &mod_events_for_emit
            && let Some((topic, payload)) = project_conversation_to_mod(&ev)
        {
            // 会话无关的投影（TextDelta / VoiceStarted / VoiceEnded）不带会话：
            // 它们描述的是「这一轮正在发生什么」，而轮次与会话的绑定在
            // TurnStarted/TurnPrompt/TurnEnded 三条上已经给全了。
            f(topic, &payload, None);
        }
    });
    let mod_events_for_thread = config.mod_events.clone();
    let mod_events_for_tls = config.mod_events.clone();
    let reload_pending_for_thread = Arc::clone(&reload_pending);
    let current_epoch_for_thread = Arc::clone(&current_epoch);
    let join = std::thread::Builder::new()
        .name("l2d-supervisor".to_owned())
        .spawn(move || {
            // F6-T2：在子线程入口安装线程局部音频回调，供 handlers::handle_engine_event
            // 在 AudioChunk 分支读取。线程退出时该线程局部随线程灭亡，无需显式清理。
            AUDIO_CB.with(|cell| *cell.borrow_mut() = emit_audio);
            // P1-2：安装 Mod 事件桥线程局部（SentenceReady 用）。
            MOD_EVENT_CB.with(|cell| *cell.borrow_mut() = mod_events_for_tls);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("supervisor runtime");
            rt.block_on(run_forever(
                config,
                say_rx,
                control_rx,
                finish_rx,
                reload_pending_for_thread,
                current_epoch_for_thread,
                emit,
                mod_events_for_thread,
                session_scopes_for_thread,
                performance_stats_for_thread,
                reload_ack_for_thread,
                turn_in_flight_for_thread,
            ));
        })
        .expect("启动 supervisor 线程");

    SupervisorHandle {
        say_tx,
        control_tx,
        finish_tx,
        join: Some(join),
        reload_pending,
        current_epoch,
        session_scopes,
        performance_stats,
        reload_ack,
        turn_in_flight,
    }
}
