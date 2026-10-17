//! 最终接线中枢 [`spawn_supervisor`]：把「终端输入 → LLM → TTS → PCM → 声卡
//! → 动作/口型事实回报」串成真实闭环（节点 A 步骤 8 批准范围）。
//!
//! **行数（AGENTS.md「源码 ≤500 行，豁免 ≤1000 需头注理由」）**：F6-T2 接线
//! （WS 音频广播）新增 thread-local 载体 `AUDIO_CB` + `spawn_supervisor_with_audio`。
//! T8（2026-10-07）把表演层装配面整体迁到 `supervisor/performance_wiring.rs`，
//! 本文件回到 1000 行以内（`code_stats` 的 `over-1000` 棘轮是 0）。
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
    Effect as RootEffect, Event as RootEvent, ModelCapabilities, SemanticAction, State as RootState,
};

/// root 事件的唯一入口（core 的自由函数 `apply` 的薄包装；保证调用点风格统一）。
pub(crate) fn root_apply(root: &mut RootState, event: RootEvent) -> Vec<RootEffect> {
    live2d_ai_core::apply(root, event)
}
use live2d_ai_runtime::performance::PerformanceStats;
use live2d_ai_runtime::{ConversationConfig, OpenAiClient};

/// 表演层计数槽：**当前**运行时的 `Arc<PerformanceStats>`（换运行时即换表）。
///
/// 为什么用槽而不是把计数直接挂在 handle 上：热重载会整体重建引擎与表演层
/// 运行时，计数表随之更换；status 端点每次读**当下**那份即可，不必让 runtime
/// 认识宿主。
pub type PerformanceStatsSlot = Arc<std::sync::Mutex<Option<Arc<PerformanceStats>>>>;

use crate::app_event::AppEvent;
use crate::session_scope::SessionScopeStore;

mod dialogue_history;
mod handlers;
// 2026-10-17：从本文件拆出（`turn_in_flight` 让本文件越过 500 行棘轮）。
mod mod_projection;
mod performance_wiring;
mod reload;
mod run;
mod spawn;
mod turn;

// 2026-10-16（D3）：以下四项搬家后**原样再导出**——`crate::supervisor::X` 与
// `handlers.rs` 的 `super::AUDIO_CB` / `super::MOD_EVENT_CB` 路径不变。
pub(crate) use spawn::{AUDIO_CB, AudioCb, MOD_EVENT_CB};
pub use spawn::{ModEventSink, spawn_supervisor, spawn_supervisor_with_audio};
// 拆出后原样再导出：`spawn.rs` 的 `super::project_conversation_to_mod` 路径不变。
pub(crate) use mod_projection::project_conversation_to_mod;

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

/// 一条待处理的用户输入（L1 基座：带**会话 id**）。
///
/// 为什么把 session 放进消息体而不是在 supervisor 里读「当前活动会话」：
/// 入队与出队之间用户可能切了会话（容量 1 的通道 + 在飞一轮足够产生这个窗口）。
/// 在**入队那一刻**解析会话，语义才是「这句话属于哪个会话」而不是「轮到它时
/// 界面上是哪个会话」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SayRequest {
    /// 用户输入正文。
    pub text: String,
    /// 该输入所属的会话（已归一化）；`None` = 不带会话（全局桶）。
    pub session: Option<String>,
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

/// supervisor 对外句柄：两条命令入口 + 动作完成事实 + 关机回收。
pub struct SupervisorHandle {
    say_tx: mpsc::Sender<SayRequest>,
    control_tx: mpsc::UnboundedSender<ControlCommand>,
    // 2026-10-01（W2-B / D1 第二段）：动作完成回报的**壳半边**——原调用方是
    // winit 壳的事件循环（`app/handler.rs`）。AGENTS「动作与表演的休眠台账」把
    // `action_tx`/完成回报列为**休眠契约**（不得私自接回真通道，也不随手删），
    // 故此处保留字段并在非测试构建下放行 dead_code；恢复条件见
    // `docs/architecture/ARCHIVED-native-shell.md`。
    #[allow(dead_code)]
    finish_tx: mpsc::UnboundedSender<ActionFinishedFact>,
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
    /// 会话级 system_prompt 覆盖 + 活动会话游标（L1 基座）。
    ///
    /// 与 supervisor 线程**共享同一张表**：HTTP/mod 侧写入，supervisor 每轮读。
    session_scopes: SessionScopeStore,
    /// 表演层计数槽（2026-09-22）：`GET /api/v1/app/status` 的
    /// `performance` 块从这里读「成功几轮 / 回退几轮 / 最近原因码」。
    performance_stats: PerformanceStatsSlot,
    /// 热重载结果的**回话槽**（S-001，2026-10-16）。
    ///
    /// `reload()` 一直是「发出去就不管」：supervisor 线程里那次 reload 成功还是
    /// 失败，PATCH 端看不到，于是界面永远写「已保存并生效」。这里放一个一次性
    /// 回话口：谁真正执行了 reload（空闲态 select 或 turn 收口后的兜底），谁就
    /// 把结果 `try_send` 回来。**没有等待者时是 None**（巡检/手改 toml 那条路）。
    reload_ack: Arc<std::sync::Mutex<Option<ReloadAck>>>,
    /// 「一轮是否在飞」（3.0 队首第一片，2026-10-17）。
    ///
    /// 外部文本端点用它判断「上一条还没处理完」：在飞时把文本收进忙时合并
    /// 窗口、先不注入；本轮收口后再把整窗合并成一条注入一次。supervisor 线程
    /// 在 `run_one_turn` 前后置位 / 清零，HTTP 线程只读（Release / Acquire）。
    turn_in_flight: Arc<std::sync::atomic::AtomicBool>,
}

impl SupervisorHandle {
    /// 提交聊天输入（容量 1：至多一条 pending Say）。满则返回 `false`，
    /// 由调用方提示「忙碌」，绝不阻塞 stdin。
    ///
    /// **会话归属**：用宿主记录的**当前活动会话**（由 `say_scoped` 或
    /// `set_active_session` 设置）。裸 HTTP / external-input / voice-input
    /// 走这条——它们的语义是「接在当前这段对话上」，正是活动会话。
    pub fn say(&self, text: impl Into<String>) -> bool {
        let session = self.session_scopes.active();
        self.say_tx
            .try_send(SayRequest {
                text: text.into(),
                session,
            })
            .is_ok()
    }

    /// 提交聊天输入并**显式指定会话**（`POST /api/v1/chat` 的 `session_id`）。
    ///
    /// 同时把宿主的活动会话游标推到该 id（`None` 也显式推——「这次没带会话」
    /// 是一个事实，不该悄悄沿用上一个）。
    pub fn say_scoped(&self, text: impl Into<String>, session: Option<String>) -> bool {
        let normalized = session.and_then(|s| live2d_ai_mod_system::sanitize_session_id(&s));
        self.session_scopes.set_active(normalized.as_deref());
        self.say_tx
            .try_send(SayRequest {
                text: text.into(),
                session: normalized,
            })
            .is_ok()
    }

    /// 只更新活动会话游标（不提交输入）。
    ///
    /// 前端切会话时调用：让随后由 Mod 发起的注入（external-input 弹幕、
    /// voice-input 转写）落到**用户正在看的**那个会话上。
    pub fn set_active_session(&self, session: Option<&str>) {
        self.session_scopes.set_active(session);
    }

    /// 会话作用域表的句柄（面板状态 / Mod 注入 / 测试断言用）。
    pub fn session_scopes(&self) -> &SessionScopeStore {
        &self.session_scopes
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
    // 同上：壳移出后无生产调用方（休眠契约的壳半边，A1–A8 相关语义不动）。
    #[allow(dead_code)]
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

    /// 请求热重载**并等它的结果**（S-001，2026-10-16）。
    ///
    /// 与 [`SupervisorHandle::reload`] 只差一条：这里装一个一次性回话口，
    /// 拿回「引擎到底装上没有」——不拿回结果就没法诚实地说「已生效」。
    ///
    /// - `Some(Ok(()))` = supervisor 在空闲态把新配置装配好了；
    /// - `Some(Err(e))` = 装配失败（解析不过 / client 构造失败）：引擎**仍是旧配置**；
    /// - `None` = `wait` 内没等到：正在跑一轮，reload 按既有语义延后到该轮收口
    ///   （`reload_pending` 兜底）——**不得**当成已生效。
    ///
    /// `wait` 必须有界：HTTP 请求循环是单线程的，等太久等于把整个 API 占住。
    pub fn reload_reported(&self, wait: std::time::Duration) -> Option<Result<(), String>> {
        let (tx, rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
        if let Ok(mut slot) = self.reload_ack.lock() {
            *slot = Some(tx);
        }
        self.reload();
        rx.recv_timeout(wait).ok()
    }

    /// 回收 supervisor 线程（shutdown handshake 完成后近乎即时返回）。
    pub fn join(mut self) {
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }

    /// **表演层当前计数**（`None` = 本次运行没有装配表演层）。
    ///
    /// 只读快照：锁毒化时回 `None`（status 端点必须始终可用）。
    pub fn performance_stats(&self) -> Option<Arc<PerformanceStats>> {
        self.performance_stats.lock().ok().and_then(|g| g.clone())
    }

    /// 主链是否正在处理一轮（`true` = 上一条还没结束）。
    ///
    /// 外部文本端点（`POST /api/v1/external/chat`）在忙时把文本收进合并窗口、
    /// 先不注入；常驻合并线程在本方法返回 `false` 后才把整窗注入一次。
    pub fn turn_in_flight(&self) -> bool {
        self.turn_in_flight
            .load(std::sync::atomic::Ordering::Acquire)
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

/// 热重载结果的**一次性回话口**（S-001，2026-10-16）。
///
/// 用 `std::sync::mpsc` 而不是 tokio：等待方是同步的 HTTP 请求线程，
/// 只有 `recv_timeout` 才谈得上「有界等待」。容量 1（结果只有一个）。
pub type ReloadAck = std::sync::mpsc::SyncSender<Result<(), String>>;

/// 把这次热重载的结果回给等待者（S-001）。
///
/// 没有等待者（`None`）= 手改 toml / 巡检路径，no-op；等待者已经走了
/// （超时返回 Queued）= `try_send` 失败，同样丢弃。**绝不阻塞** supervisor 线程。
pub(crate) fn reply_reload(
    ack: &Arc<std::sync::Mutex<Option<ReloadAck>>>,
    outcome: &Result<(), String>,
) {
    let taken = ack.lock().ok().and_then(|mut slot| slot.take());
    if let Some(tx) = taken {
        let _ = tx.try_send(outcome.clone());
    }
}

#[cfg(test)]
mod support;
// Wave 3（2026-09-21）：助手侧正文事件，单列守住「测试文件 ≤800 行」。
#[cfg(test)]
mod tests_assistant_event;
#[cfg(test)]
mod tests_fault;
#[cfg(test)]
mod tests_loop;
#[cfg(test)]
mod tests_pcm;
#[cfg(test)]
mod tests_reload;
// L1 会话绑定（2026-09-15）：从 tests_loop.rs 拆出，守住「测试文件 ≤800 行」。
#[cfg(test)]
mod tests_session;
#[cfg(test)]
mod tests_stall;
// 2026-09-22：表演层端到端冒烟（真实 [performance] 配置 → mock 表演层）。
#[cfg(test)]
mod tests_performance;
// P1WS-1：真实 LLM 流式文本透传（text_delta emit）+ epoch 镜像单元测试。
#[cfg(test)]
mod tests_dialogue_history;
#[cfg(test)]
mod tests_stream;
#[cfg(test)]
mod tests_timing;
