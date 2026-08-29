//! 统一应用事件 [`AppEvent`]：winit 事件循环的**唯一**跨线程入口（D4 裁决）。
//!
//! 三类生产者全部只经 [`winit::event_loop::EventLoopProxy`] 发送本枚举：
//! - **supervisor 线程**（Tokio 后台线程）：下发渲染命令与对话侧状态；
//! - **REPL stdin 线程**：转发显式退出请求；
//! - **托盘线程**：沿用既有轻量 [`PetUserEvent`] 包装（不变）。
//!
//! 类型边界（D4/D14 裁决原文）：
//! - **PCM 绝不进入 AppEvent**——音频块由 supervisor 直接写入声卡 facade；
//! - **Say 文本绝不进入 AppEvent**——聊天输入走有界命令通道进 supervisor；
//! - 每个 [`RenderCommand`] / [`ConversationUiEvent`] 都携带产生它的业务
//!   **epoch**；ShellApp 维护 `render_epoch` 镜像（只能被下发的最新值更新，
//!   绝不自增），与命令上的 epoch 不符即丢弃——这是副作用执行端的
//!   **防御性闸门**（权威闸门仍在 core reducer，见 D9）。

use live2d_ai_core::SemanticAction;

use crate::user_event::PetUserEvent;

/// 发往事件循环的统一用户事件。
#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    /// 托盘菜单命令（既有路径原样包装）。
    Tray(PetUserEvent),
    /// 表演侧渲染命令（root Effect 的副作用投影）。
    Render(RenderCommand),
    /// 对话侧状态通知（口型门控与 render_epoch 镜像维护用）。
    Conversation(ConversationUiEvent),
    /// root 侧关键事实的审计投影（节点 B 复审要求：UI 事件之外，
    /// 测试必须能观察 root 事实顺序——Started/GenerationFinished/Drained/
    /// TurnCompleted/Cleared 与丢弃原因）。
    RootAudit(RootFact),
    /// supervisor 已完成停机清理（关闭流、释放引擎）——事件循环此刻才允许退出。
    ///
    /// `/quit`、`CloseRequested`、`TrayExit` 三者共用同一 shutdown handshake：
    /// 主线程收到请求后置 quitting 并给 supervisor 发 Quit，同时启动 500ms
    /// 兜底计时；提前收到本变体即立刻退出，否则超时强退并打警告。
    ShutdownReady,
}

/// root 关键事实的枚举化审计记录（只读投影；专供测试断言与诊断日志）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootFact {
    /// PlaybackStarted 被根接受。
    PlaybackStarted { epoch: u64 },
    /// GenerationFinished 被根接受（每轮恰一次；B2-P0-1 锁定项）。
    GenerationFinished { epoch: u64, completed: bool },
    /// PlaybackDrained 被根接受。
    PlaybackDrained { epoch: u64 },
    /// PlaybackCleared 被根接受。
    PlaybackCleared { epoch: u64 },
    /// TurnCompleted 效果出现（唯一收口出口）。
    TurnCompleted { epoch: u64, outcome_completed: bool },
    /// root 对某事实返回了确定性丢弃。
    Dropped,
    /// 链路阶段耗时埋点（节点 D 开发者模式）。**纯附加字段**，随
    /// `TurnCompleted` 之后下发一次；不改变双闩锁/epoch 语义。
    TurnStages {
        epoch: u64,
        timings: TurnStageTimings,
    },
}

/// 链路阶段耗时（毫秒；相对 `run_one_turn` 提交时刻）。
///
/// 由 supervisor 在 `run_one_turn` 内逐步填写；未触达该阶段时字段保持
/// `0`（与 `Default` 行为一致）——便于「阶段未发生」与「阶段瞬时完成」
/// 在断言中区分（前者为 0、后者通常 ≥ 0 但**极小**）。
///
/// 节点 D 需求（链路每步耗时可视化）+ D1 契约 §WS 事件：所有阶段经
/// [`RootFact::TurnStages`] 经 AppEvent 暴露，**不侵入 core reducer**
///（live2d-ai-core/ 严禁动；纯计时在 runtime/supervisor 层）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TurnStageTimings {
    /// LLM 提交 → 首个 `TextDelta` 到达 supervisor。
    pub llm_first_token_ms: u64,
    /// LLM 提交 → `Terminal` 事件（含 TTS 排空）。
    pub llm_total_ms: u64,
    /// 单句切句 → 该句 TTS 完成（按句累加：所有句耗时之和；测试断言
    /// `> 0` 即代表 TTS 实际跑了）。
    pub tts_synth_ms: u64,
    /// TTS 请求 → 首个 `AudioChunk`（按首句计）。
    pub tts_first_chunk_ms: u64,
    /// 首 chunk 入环 → `PlaybackDrained`（端到端播放耗时）。
    pub audio_play_ms: u64,
    /// `ToolAction` → 仲裁 → `Perform` 效果（按动作累加）。
    pub action_dispatch_ms: u64,
    /// 提交 → `TurnCompleted`（端到端 turn 总耗时）。
    pub turn_total_ms: u64,
}

/// 渲染命令（root Effect 投影；epoch 为该效果所属代次）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderCommand {
    /// 表演指定动作（`Play` 与抢占 `Transition` 统一为「改为表演新动作」；
    /// capability gate 与优先级仲裁已在 core reducer 完成）。
    Perform {
        /// 所属业务代次。
        epoch: u64,
        /// 要表演的动作（含强度与来源；来源仅观测用）。
        action: SemanticAction,
    },
    /// 立即停止表演（stop / interrupt / End 效果共用；对空闲 player 幂等无害）。
    CeasePerforming {
        /// 所属业务代次。
        epoch: u64,
    },
}

/// 对话侧状态通知（携带业务 epoch；不含任何音频载荷）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversationUiEvent {
    /// 业务代次推进提示（stop 后必然发生；新 turn 开启时若值变化也发）。
    /// ShellApp 收到后无条件刷新 `render_epoch` 镜像。
    NewEpoch {
        /// 新的当前代次。
        epoch: u64,
    },
    /// 本轮开始发声：首个非空 PCM 成功入环（声卡回调即将消费）。
    /// ShellApp 进入「口型活跃」状态：按 MouthSnapshot 电平驱动口型。
    VoiceStarted {
        /// 所属业务代次。
        epoch: u64,
    },
    /// 声卡侧静默收尾（排空 / 被清 / 被 stop / 声卡故障兜底）。
    /// ShellApp 必须此后把口型电平强制按 0 使用——不能等待下一次回调自然归零。
    VoiceEnded {
        /// 所属业务代次。
        epoch: u64,
    },
    /// LLM 正文增量（真实 text payload；P1WS-1 引入；2026-08-29）。
    ///
    /// 由 supervisor 在 `EngineEvent::TextDelta` 处投影；携带**原始增量文本**
    /// 与产生它时引擎内部记录的 `ts_ms`（用于链路耗时可视化）。壳侧收到后
    /// 可选渲染（winit 路径无需显示文本——只维护 render_epoch；web 路径
    /// 经 `ws::app_event_to_ws_frame` 投影为 `text_delta` 帧给前端气泡追加）。
    TextDelta {
        /// 所属业务代次。
        epoch: u64,
        /// 引擎记录的产生时刻（相对本轮起点的毫秒数；可观测性用途）。
        ts_ms: u64,
        /// 文本增量片段（来自 LLM stream chunk；未做切句假设）。
        text: String,
    },
}

/// 发布 helper：经代理发送 [`AppEvent`]（发送失败仅见于调试日志：
/// 事件循环已退出属于正常关机竞态）。
pub fn send_app_event(proxy: &winit::event_loop::EventLoopProxy<AppEvent>, event: AppEvent) {
    if let Err(e) = proxy.send_event(event) {
        tracing::debug!(error = %e, "AppEvent 投递失败（事件循环可能已退出）");
    }
}
