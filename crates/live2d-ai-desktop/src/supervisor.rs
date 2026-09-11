//! 最终接线中枢 [`spawn_supervisor`]：把「终端输入 → LLM → TTS → PCM → 声卡
//! → 动作/口型事实回报」串成真实闭环（节点 A 步骤 8 批准范围）。
//!
//! **行数豁免（≤1000 头注）**：F6-T2 接线（WS 音频广播）新增 thread-local
//! 载体 `AUDIO_CB` + `spawn_supervisor_with_audio`，当前 577 行（≤1000）。
//!
//! # 进程结构与所有权（D1/D2/D3 裁决落地）
//!
//! ```text
//! 主线程 winit          │ stdin 线程            │ supervisor OS 线程（Tokio current-thread）
//! ─────────────────────┼──────────────────────┼─────────────────────────────────────────
//! Window/Surface/      │ repl::parse_line      │ root State（唯一业务 epoch owner）
//! ModelRendererCore    │ ├ Say ──say_tx(有界)──▶ ConversationEngine（单 owner）
//! PerformancePlayer    │ ├ Stop ─┬ control_tx ▶ CancellationToken（当前轮）
//! render_epoch 镜像    │ └ Quit ─┘             │ pending PreparedPcm（至多一）
//! MouthSnapshot 只读   │ ◀──── proxy(AppEvent) ─┴─ select!{control|finish|gen|event(+守卫)|pump}
//! ```
//!
//! - supervisor **独占** root reducer、引擎与声卡 facade；主线程只执行副作用；
//! - **PCM 绝不经 AppEvent**：AudioChunk 在本线程内做代次检查 → prepared
//!   转换（恰一次）→ 有界入环；`WouldBlock` 完整保留重试，同时以通道守卫
//!   停止消费下一引擎事件（背压原路传回引擎的有界队列）；
//! - **Say 文本绝不进 AppEvent**：走有界命令通道。
//!
//! # 关键裁决的落点
//!
//! - **PlaybackDrained 报告时机**（审批补充裁决）：生成中途 ring 短暂排空
//!   ≠ 播放结束。仅当
//!   `generation_terminal_seen && pending_pcm.is_none() && playback_started
//!    && healthy && is_drained()`
//!   才报告最终 Drained；空音频轮保持 NeverStarted，由 GenerationFinished
//!   直接收口。
//! - **P0-4**：history 仅在 Completed 且真实排空（或从未起播）、未被取消、
//!   声卡全程健康时，经 [`ConversationEngine::commit_completed_turn`] 提交。
//! - **D8 失败策略**：失败归因 = 生成期观察到的 ErrorKind——仅 `Llm` →
//!   播完已入队内容再收尾；出现任一致命类（Tts/Decode/Backpressure）或声卡
//!   故障 → 立即清环并报 PlaybackCleared。
//! - **D11**：工具强度非法即拒绝不 clamp；仅当根 Play 被接受（Start/
//!   Transition 效果）才抑制 fallback；fallback 输入是完整 assistant_text，
//!   在生成终态判定一次。
//! - **D13 / stop 事务（D7 顺序）**：①root `StopRequested` 先推进 epoch →
//!   ②cancel 当前轮令牌 → ③清 pending PreparedPcm → ④执行 StopPlayback
//!   （ring 打断）→ ⑤口型归零协议（VoiceEnded 下发，渲染端强制电平 0）。
//!   生成 future **总会被 await 到自然返回**——中途放弃会让内部 TTS worker
//!   脱管泄漏（审计 D2 明令禁止）。排队中的 Say 由容量 1 的通道天然只留一条，
//!   stop 后调用方 clear。
//! - **Say 队列容量取 1**：D14 要求有界 Say，「实施顺序」要求至多一条 pending
//!   ——两条裁决取交集从严；stdin 端满即提示忙碌。
//!
//! # 测试性
//!
//! 对外只依赖 `emit: Fn(AppEvent)` 回调而非 EventLoopProxy 具体类型：
//! 生产装配传 proxy 包装闭包；集成测试传事件收集器，即可在无窗口环境验证
//! 完整闭环。无音频设备时自动进入 dry-run（跳过入环、永不报 Started）。
//!
//! # 文件结构
//!
//! - `super::turn`：本文件把 `run_one_turn` + `GenOut` 整体迁出，连同三宏
//!   `do_stop!` / `on_action_finished!` / `on_any_pcm_progress!`（必须同
//!   文件——1 号裁决）。`run_forever` 经 `turn::run_one_turn` 调度。
//! - `super::handlers`：`handle_engine_event` / `forward_effects` /
//!   `ev_epoch` / `ev_ts_ms` / `drain_residual_events` / `gen_outcome` /
//!   `emit_voice_ended_if_needed` 七个翻译层纯函数。`pub(crate)` 使同级
//!   `tests_loop.rs` / `tests_pcm.rs` / `tests_fault.rs` 可直接调用。
//!   （原第八个 `content_worth_fallback` 已随 2026-09-11 动作系统移除而删除。）
//! - `super::tests_loop` / `super::tests_pcm` / `super::tests_fault`
//!   （cfg(test)）：闭环端到端测试 + 6 条 P0 真实闭环用例；共用基建在
//!   `super::support`。
//!
//! 本文件留下：`ActionFinishedFact` / `ControlCommand` / `SupervisorConfig` /
//! `SupervisorHandle` / `Emit` / `spawn_supervisor` / `run_forever` /
//! `root_apply` 公共装配与事件循环入口。`SUPERVISOR_TICK` 常量随 `run_one_turn`
//! 迁至 `turn.rs`（仅 turn 内部使用）。

use std::sync::Arc;

use tokio::sync::mpsc;

use live2d_ai_core::{
    ActionCommand, Effect as RootEffect, Event as RootEvent, ModelCapabilities, SemanticAction,
    State as RootState,
};

/// root 事件的唯一入口（core 的自由函数 `apply` 的薄包装；保证调用点风格统一）。
pub(crate) fn root_apply(root: &mut RootState, event: RootEvent) -> Vec<RootEffect> {
    live2d_ai_core::apply(root, event)
}
use live2d_ai_mod_system::ModEventTopic;
use live2d_ai_runtime::{AppSettings, ConversationConfig, ConversationEngine, OpenAiClient};

use crate::app_event::{AppEvent, ConversationUiEvent};

mod handlers;
mod turn;

/// 动作自然完成事实：**携带产生它的原始业务 epoch**（B-P0-5 裁决）。
///
/// winit 侧在 Perform 下发时记住 `(epoch, action)`，自然完成时原样回报；
/// supervisor **不得**重新盖上当前 epoch——跨代复用同一动作身份时，
/// 迟到的旧完成必须被 core 以 StaleActionCompletion 丢弃而不是误伤新代次。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionFinishedFact {
    pub epoch: u64,
    pub action: SemanticAction,
}

/// REPL/控制命令（无界通道：控制命令不允许被 Say 积压饿死，D14）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlCommand {
    /// 取消当前轮并清理（等同终端 `/stop`）。
    Stop,
    /// 开始关机 handshake（等同 `/quit`、窗口关闭、托盘退出）。
    Quit,
    /// 配置已保存：重建 LLM/TTS client（热重载；失败保留旧配置继续运行）。
    ///
    /// 由 Web `PATCH /api/v1/settings` 与 egui 保存按钮在写盘成功后发送；
    /// supervisor 在空闲态收到本命令即重读 `live2d-ai.toml`、解析、构造新
    /// [`OpenAiClient`] 与 [`ConversationConfig`] 并替换引擎，**不**重写文件
    /// （文件已由调用方写回）。在飞 turn 中到达时，本实现不抢占——turn 收口
    /// 回到空闲态后由 PATCH 再次触发或保留旧配置（最小可行：空闲态 select
    /// 即可，不在 `run_one_turn` 内桥接）。
    Reload,
}

/// supervisor 装配参数（所有权转移；字段均为不可变视图或独占资源）。
pub struct SupervisorConfig {
    /// 已按 settings 解析好的统一客户端。
    pub client: OpenAiClient,
    /// 对话配置（persona/历史上限等）。
    pub conversation: ConversationConfig,
    /// 可信能力集（皮套加载成功后由装配方显式给定；绝不默认空集）。
    pub capabilities: ModelCapabilities,
    /// 声卡输出。`None` = 无设备环境 dry-run（P1-1 明示语义：**文本成功模式**
    /// ——生成与切句全链路照常验证，PCM 在 supervisor 内丢弃且永不报
    /// Started；Completed 轮仍允许提交 history，core 文档的 NeverStarted 语境
    /// 由本策略显式收窄）。真实链路用 [`AudioOutputFacade`]，闭环竞态测试注入
    /// detached 的 `PlaybackHandle`（同 trait，消费节奏可控）。
    pub audio: Option<Box<dyn crate::audio::PcmProducer>>,
    /// 配置文件路径（`live2d-ai.toml`）。`Some` 时 [`ControlCommand::Reload`]
    /// 在空闲态会重读该路径以重建 client；`None` 时 Reload 直接 no-op
    /// （测试或 `--web` 模式可借此禁用热重载）。
    pub config_path: Option<String>,
    /// host→Mod 事件桥（None = 不投递；测试 / 无 Mod 场景）。
    ///
    /// 由 supervisor 在 turn 生命周期点调用（非阻塞）。回调内部经
    /// `ModRegistry::dispatch_event` 投递到 Mod worker（有界 channel，
    /// 满则丢弃计数）；**不携带密钥**，payload 为 turn/action/voice 等
    /// 非敏感字符串。设计见 docs/plans/integrity-takeover-fix-plan-2026-09-07.md §3。
    pub mod_events: Option<ModEventSink>,
}

/// supervisor 对外句柄：三条命令入口 + 关机回收。
pub struct SupervisorHandle {
    say_tx: mpsc::Sender<String>,
    control_tx: mpsc::UnboundedSender<ControlCommand>,
    finish_tx: mpsc::UnboundedSender<ActionFinishedFact>,
    /// **Mod 边界 / 外部触发**：进 core 仲裁的用户动作通道。
    /// 容量无界（控制命令类不阻塞；与 `control_tx` 同等待遇——D14）。
    /// 经此通道进入的 action 走 core `RootEvent::Action` 路径，
    /// capability gate / 优先级仲裁 / 幂等仍由 core 决定——**禁止**
    /// handler 直接调 `core::apply` 或写 `ActionState::current`。
    ///
    /// 2026-09-11：原 `D4 命令注册协议` 的 web 命令面板入口已随
    /// `/api/v1/commands` 端点删除；本通道现仅由 Mod host channels 使用。
    action_tx: mpsc::UnboundedSender<SemanticAction>,
    join: Option<std::thread::JoinHandle<()>>,
    /// 「reload 待处理」标志：handle.reload() 置位，supervisor 在 idle
    /// select / turn 收口兜底时消费。**绕过 turn.rs 的 channel recv**——
    /// 否则 turn.rs 的 `Some(ControlCommand::Reload) => {}` 会吞掉 in-flight
    /// 阶段的 Reload，导致 PATCH 后下一轮仍用旧 client。原子标志保证
    /// 「在飞阶段到达的 reload 必被 turn 收口后的 try_apply_reload 捕获」。
    reload_pending: Arc<std::sync::atomic::AtomicBool>,
    /// P1WS-1：当前 epoch 镜像（`AtomicU64`）。supervisor 线程在每次推进
    /// `root.epoch`（`UserSubmitted` 之前 `cur_epoch = root.epoch`）与
    /// stop 事务（`StopRequested` 推进）后回写到这里；外部 `/api/v1/chat`
    /// 与 `/api/v1/app/status` 经此读出**真实**当前代次——避免把 root
    /// 状态在多线程间共享。
    current_epoch: Arc<std::sync::atomic::AtomicU64>,
}

impl SupervisorHandle {
    /// 提交聊天输入（容量 1：至多一条 pending Say）。满则返回 `false`，
    /// 由调用方提示「忙碌」，绝不阻塞 stdin。
    pub fn say(&self, text: impl Into<String>) -> bool {
        self.say_tx.try_send(text.into()).is_ok()
    }

    /// 请求停止当前轮（非阻塞；幂等）。
    pub fn stop(&self) {
        let _ = self.control_tx.send(ControlCommand::Stop);
    }

    /// 请求关机（非阻塞；幂等）。
    pub fn quit(&self) {
        let _ = self.control_tx.send(ControlCommand::Quit);
    }

    /// 请求热重载配置（非阻塞；幂等；supervisor 空闲态会重读 `live2d-ai.toml`）。
    ///
    /// 由 `PATCH /api/v1/settings` 与 egui 保存按钮在写盘成功后调用；失败时
    /// supervisor 内部保留旧 client + conversation（旧 engine 继续工作），
    /// 并通过日志/事件通道回报错误，前端可显示「保存失败，仍用旧配置」。
    ///
    /// **实现**：同时置位 [`SupervisorHandle::reload_pending`] 原子标志 +
    /// 经 channel 发送 `ControlCommand::Reload`。通道是「在 idle select 走
    /// 的快速路径」；原子标志是「turn 在飞时被 turn.rs 吞掉的兜底」——
    /// turn.rs 的 no-op 分支不会清标志，supervisor 收口后由 `try_apply_reload`
    /// 读取。**两条路径至少一条生效**。
    pub fn reload(&self) {
        self.reload_pending
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = self.control_tx.send(ControlCommand::Reload);
    }

    /// winit 侧回报「某动作自然播放完毕」（P0-6 身份事实；非阻塞；
    /// B-P0-5：epoch 必须是**该动作开始时**的业务代次，由调用方保存并原样回传）。
    pub fn report_action_finished(&self, epoch: u64, action: SemanticAction) {
        let _ = self.finish_tx.send(ActionFinishedFact { epoch, action });
    }

    /// P1WS-1：读出当前 epoch（由 supervisor 线程在 epoch 推进处镜像更新）。
    ///
    /// `AtomicU64::Ordering::Acquire` 读；启动时默认 0（与 root 默认一致）。
    /// 外部 `/api/v1/chat` 与 `/api/v1/app/status` 走此接口获取真实代次，
    /// 无 supervisor 句柄时由 caller 自行回退到 0。
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
            .load(std::sync::atomic::Ordering::Acquire)
    }

    /// 回收 supervisor 线程（shutdown handshake 完成后近乎即时返回）。
    pub fn join(mut self) {
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }

    /// **D4（命令注册协议）**：外部（Mod host channels 等）触发的用户动作。
    ///
    /// 走独立 `action_tx` 通道（unbounded）→ supervisor 空闲态 `select!`
    /// 收到后 → `root_apply(RootEvent::Action { ..., command: Play })`
    /// → 经 core 仲裁（capability gate / 优先级 / 幂等）→ `forward_effects`。
    /// **禁止**任何 handler 直接调 `core::apply`（§7.3 grep 审计）。
    ///
    /// 非阻塞、幂等；动作重复或 capability 缺失时由 core 端发 `Dropped`
    /// 效果并通过 `AppEvent::RootAudit` 下发。
    ///
    /// 2026-09-11 用户裁决：LLM 工具与 `/api/v1/commands` 端点已删除，
    /// `AppEvent::Render` 动作投影也随之移除——本通道仍可把动作送进 core
    /// reducer（Mod 边界用量），但桌面侧不再有任何表演副作用。
    ///
    /// 返回 `true` = 已入队；`false` = supervisor 已关（unbounded 通道
    /// 实际不会满，仅在所有 receiver 都 drop 时返回 `false`）。
    pub fn trigger_action(&self, action: SemanticAction) -> bool {
        self.action_tx.send(action).is_ok()
    }

    /// 测试专用：访问 `reload_pending` 原子标志的只读视图。
    ///
    /// 用于 `web_api` / `tests_reload` 断言「PATCH 写盘成功后 Reload 抵
    /// 达 supervisor」——标志被 `apply_reload` swap 清零即为「已被处理」。
    /// **仅供 `#[cfg(test)]` 路径调用**；生产代码不应直接读此标志。
    #[cfg(test)]
    pub fn reload_pending_for_test(&self) -> &std::sync::atomic::AtomicBool {
        &self.reload_pending
    }
}

type Emit = Arc<dyn Fn(AppEvent) + Send + Sync>;

/// host→Mod 事件投递回调（`SupervisorConfig.mod_events`）。
///
/// 由 supervisor 在 turn 生命周期点调用（`dispatch_event` 非阻塞，满则丢弃），
/// 不携带密钥；payload 为 turn/action/voice 等非敏感字符串。引入 Type alias
/// 以消解 clippy「very complex type」告警。
pub type ModEventSink = Arc<dyn Fn(ModEventTopic, &str) + Send + Sync>;

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
    static AUDIO_CB: std::cell::RefCell<Option<Arc<AudioCb>>> = const { std::cell::RefCell::new(None) };
}

/// 从磁盘最新配置构造新的 [`OpenAiClient`] + [`ConversationConfig`]（热重载核心）。
///
/// **不**触碰 [`ConversationEngine::history`]（旧 engine 跨重建保留历史不现实——
/// `ConversationEngine` 是持 client 的；新 engine 从空历史开始，旧 engine 直接 drop，
/// 已 `commit_completed_turn` 写入的内容**不会**跨 reload 保留——这是公开的语义）。
///
/// 失败路径（文件 IO / TOML 解析 / `base_url` 非法 / env 解析 / reqwest 构造）
/// 全部以 [`String`] 形式回传，**不**修改调用方的旧 engine（保留旧配置继续运行）。
fn rebuild_engine_from_config(
    config_path: &str,
    _emit: &Emit,
) -> Result<(OpenAiClient, ConversationConfig), String> {
    // 1) 读盘 + 解析。
    let settings =
        AppSettings::load_from_path(config_path).map_err(|e| format!("读取/解析配置失败: {e}"))?;
    // 2) 解析环境变量（用生产 lookup，与 chat 装配同语义）。
    let resolved = settings
        .resolve_with(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .map_err(|e| format!("配置解析失败（URL/环境变量名）: {e}"))?;
    // 3) 构造新 client。
    let client = OpenAiClient::new(resolved.llm.clone(), resolved.tts.clone())
        .map_err(|e| format!("构建 LLM/TTS 客户端失败: {e}"))?;
    Ok((client, resolved.conversation))
}

/// 应用 Reload：从最新配置重建 engine；失败保留旧 engine。
///
/// 抽成独立函数供两处复用：
/// 1. `run_forever` 空闲态 select 直接处理（无 in-flight turn 时）；
/// 2. `run_one_turn` 收口后兜底（处理在飞期间积压的 Reload）。
///
/// `config_path: None` 时静默 no-op（web 模式 / 测试装配）。
///
/// `reload_pending: Some(&AtomicBool)` 时**消费**标志：成功读置 false；
/// 读置 false 后**才**执行 rebuild（避免在 idle select 之外做无谓 IO）。
fn apply_reload(
    engine: &mut ConversationEngine,
    config_path: Option<&str>,
    reload_pending: Option<&std::sync::atomic::AtomicBool>,
    emit: &Emit,
) {
    // 消费标志：仅在需要应用时才清。失败路径不重置——保留以便下次再试。
    let should_apply = reload_pending
        .map(|f| f.swap(false, std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(true);
    if !should_apply {
        return;
    }
    let Some(path) = config_path else {
        tracing::debug!("Reload 收到但 config_path 未设置（web 模式 / 测试），no-op");
        return;
    };
    match rebuild_engine_from_config(path, emit) {
        Ok((new_client, new_conversation)) => {
            *engine = ConversationEngine::new(new_client, new_conversation);
            tracing::info!(path, "热重载成功：已重建 LLM/TTS client");
        }
        Err(e) => {
            // 失败回退：旧 engine 不动；前端可由日志/事件
            // 显示「保存失败，仍用旧配置」。进程继续运行。
            // 标志已被清——但 PATCH 端拿不到错误返回（web 模式仅 HTTP
            // 200 + 状态：旧值 GET 仍可见），必要时 PATCH 端发 Reload
            // 之前可先做 server-side validate。
            tracing::error!(error = %e, path, "热重载失败：保留旧配置继续运行");
        }
    }
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
    // **D4**：web 端命令面板动作通道（unbounded；与 control_tx 同类）。
    let (action_tx, action_rx) = mpsc::unbounded_channel::<SemanticAction>();
    let reload_pending = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // P1WS-1：当前 epoch 镜像（外部 HTTP/Status 端读这里）；启动时 = 0
    // （与 `RootState::default().epoch.get()` 一致）。
    let current_epoch = Arc::new(std::sync::atomic::AtomicU64::new(0));

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
            f(topic, &payload);
        }
    });
    let mod_events_for_thread = config.mod_events.clone();
    let reload_pending_for_thread = Arc::clone(&reload_pending);
    let current_epoch_for_thread = Arc::clone(&current_epoch);
    let join = std::thread::Builder::new()
        .name("l2d-supervisor".to_owned())
        .spawn(move || {
            // F6-T2：在子线程入口安装线程局部音频回调，供 handlers::handle_engine_event
            // 在 AudioChunk 分支读取。线程退出时该线程局部随线程灭亡，无需显式清理。
            AUDIO_CB.with(|cell| *cell.borrow_mut() = emit_audio);
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("supervisor runtime");
            rt.block_on(run_forever(
                config,
                say_rx,
                control_rx,
                finish_rx,
                action_rx,
                reload_pending_for_thread,
                current_epoch_for_thread,
                emit,
                mod_events_for_thread,
            ));
        })
        .expect("启动 supervisor 线程");

    SupervisorHandle {
        say_tx,
        control_tx,
        finish_tx,
        action_tx,
        join: Some(join),
        reload_pending,
        current_epoch,
    }
}

/// 把 `AppEvent::Conversation` 投影为对应的 Mod 事件（纯函数，便于单测）。
///
/// 返回 `Some((ModEventTopic, payload))` 时，调用方经 `mod_events` 桥（`dispatch_event`）
/// 投递给 Mod worker。三变体：
/// - `TextDelta` → `ModEventTopic::TextDelta`（payload = 增量文本）;
/// - `VoiceStarted` → `ModEventTopic::VoiceStarted`（payload = epoch 字符串）;
/// - `VoiceEnded` → `ModEventTopic::VoiceEnded`（payload = epoch 字符串）。
///
/// 非 Conversation 变体（Render / RootAudit / Tray / ShutdownReady）返回 `None`，
/// 因为 Mod 事件主题只覆盖对话/动作生命周期（见 topics.rs）。
fn project_conversation_to_mod(ev: &AppEvent) -> Option<(ModEventTopic, String)> {
    match ev {
        AppEvent::Conversation(ConversationUiEvent::TextDelta { text, .. }) => {
            Some((ModEventTopic::TextDelta, text.clone()))
        }
        AppEvent::Conversation(ConversationUiEvent::VoiceStarted { epoch }) => {
            Some((ModEventTopic::VoiceStarted, epoch.to_string()))
        }
        AppEvent::Conversation(ConversationUiEvent::VoiceEnded { epoch }) => {
            Some((ModEventTopic::VoiceEnded, epoch.to_string()))
        }
        _ => None,
    }
}

// **D4**：新增 `action_rx` 第 4 个 receiver 通道，函数参数从 7 增至 8
// 超过 clippy 默认阈值；显式 `allow` 维持与既有一致风格（参数按
// 所有权通道顺序排列，非可拆分组）。
#[allow(clippy::too_many_arguments)]
async fn run_forever(
    config: SupervisorConfig,
    mut say_rx: mpsc::Receiver<String>,
    mut control_rx: mpsc::UnboundedReceiver<ControlCommand>,
    mut finish_rx: mpsc::UnboundedReceiver<ActionFinishedFact>,
    mut action_rx: mpsc::UnboundedReceiver<SemanticAction>,
    reload_pending: Arc<std::sync::atomic::AtomicBool>,
    current_epoch: Arc<std::sync::atomic::AtomicU64>,
    emit: Emit,
    mod_events: Option<ModEventSink>,
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
    let mut engine = ConversationEngine::new(client, conversation);
    let mut next_turn_id: u64 = 0;
    let mut quitting = false;

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
                    apply_reload(
                        &mut engine,
                        config_path.as_deref(),
                        Some(reload_pending.as_ref()),
                        &emit,
                    );
                }
            },
            fact = finish_rx.recv() => {
                if let Some(fact) = fact {
                    // **Mod 事件桥（d2）**：动作自然完成 → `ActionFinished`。
                    // payload = 动作协议名（`ActionId::name()`），director 可据此推进编序。
                    if let Some(f) = &mod_events {
                        f(ModEventTopic::ActionFinished, fact.action.action.name());
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
            // **Mod 边界 / 外部触发**：经 `action_tx` 到达的用户动作。
            // 走 core 仲裁路径：
            //   RootEvent::Action → core 闸门（capability gate / 优先级 / 幂等）
            //   → forward_effects。handler **不**直接调 core::apply（§7.3）。
            //
            // 2026-09-11：原 D4 web 命令面板与本条注释里的
            // `AppEvent::Render` 投影均已随动作系统删除；core reducer 仍在，
            // 但桌面侧不再有表演副作用。
            action = action_rx.recv() => {
                if let Some(action) = action {
                    let cur = root.epoch;
                    let effects = root_apply(
                        &mut root,
                        RootEvent::Action {
                            epoch: cur,
                            command: ActionCommand::Play { action },
                        },
                    );
                    handlers::forward_effects(root.epoch.get(), &effects, audio.as_deref(), &emit);
                }
            }
            text = say_rx.recv() => match text {
                Some(text) => {
                    next_turn_id += 1;
                    let cur_epoch = root.epoch;
                    let epoch = cur_epoch.get();
                    // **Mod 事件桥（d2）**：新 turn 提交 → `TurnStarted`。
                    // payload = 当前 turn id；director 据此按 config.sequence 自动编序。
                    if let Some(f) = &mod_events {
                        f(ModEventTopic::TurnStarted, &next_turn_id.to_string());
                    }
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
                    turn::run_one_turn(
                        &mut root, &mut engine, &mut audio, &mut say_rx, &mut control_rx,
                        &mut finish_rx, current_epoch.as_ref(), next_turn_id, epoch, text,
                        &mut quitting, &emit,
                    )
                    .await;
                    // 兜底：「在飞 turn 中到达的 Reload」走原子标志
                    // [`SupervisorHandle::reload_pending`]，turn.rs no-op
                    // channel 消息但**不清标志**。turn 收口后统一消费。
                    // **不 drain control_rx**：turn 内到达的 Quit/Stop 已在
                    // turn.rs select 内处理（quitting=true / do_stop），
                    // turn 收尾后仍积压的命令由外层空闲态 select 自然消费
                    // （删除旧的无条件 `while try_recv().is_ok(){}` drain——
                    // 它会吞掉恰好落在收尾边界的 Quit，导致应用不退出）。
                    apply_reload(
                        &mut engine,
                        config_path.as_deref(),
                        Some(reload_pending.as_ref()),
                        &emit,
                    );
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

// ---------------------------------------------------------------- 闭环集成测试

/// `project_conversation_to_mod` 投影语义（AppEvent::Conversation → ModEventTopic）。
#[cfg(test)]
mod tests_mod_projections {
    use super::*;

    #[test]
    fn text_delta_projects_to_text_delta_with_payload() {
        let ev = AppEvent::Conversation(ConversationUiEvent::TextDelta {
            epoch: 3,
            ts_ms: 42,
            text: "你好".to_string(),
        });
        assert_eq!(
            project_conversation_to_mod(&ev),
            Some((ModEventTopic::TextDelta, "你好".to_string()))
        );
    }

    #[test]
    fn voice_started_projects_with_epoch_payload() {
        let ev = AppEvent::Conversation(ConversationUiEvent::VoiceStarted { epoch: 7 });
        assert_eq!(
            project_conversation_to_mod(&ev),
            Some((ModEventTopic::VoiceStarted, "7".to_string()))
        );
    }

    #[test]
    fn voice_ended_projects_with_epoch_payload() {
        let ev = AppEvent::Conversation(ConversationUiEvent::VoiceEnded { epoch: 9 });
        assert_eq!(
            project_conversation_to_mod(&ev),
            Some((ModEventTopic::VoiceEnded, "9".to_string()))
        );
    }

    #[test]
    fn non_conversation_events_yield_none() {
        use crate::user_event::PetUserEvent;
        // 2026-09-11：`AppEvent::Render` 已随动作系统删除，这里改用剩下的
        // 非 Conversation 变体覆盖「不应投影」的语义。
        let audit = AppEvent::RootAudit(crate::app_event::RootFact::Dropped);
        let tray = AppEvent::Tray(PetUserEvent::TrayExit);
        let shutdown = AppEvent::ShutdownReady;
        for ev in [audit, tray, shutdown] {
            assert_eq!(
                project_conversation_to_mod(&ev),
                None,
                "非 Conversation 事件不应投影"
            );
        }
    }
}

#[cfg(test)]
mod support;
#[cfg(test)]
mod tests_fault;
#[cfg(test)]
mod tests_loop;
#[cfg(test)]
mod tests_pcm;
#[cfg(test)]
mod tests_reload;
#[cfg(test)]
mod tests_stall;
// P1WS-1：真实 LLM 流式文本透传（text_delta emit）+ epoch 镜像单元测试。
#[cfg(test)]
mod tests_stream;
#[cfg(test)]
mod tests_timing;
