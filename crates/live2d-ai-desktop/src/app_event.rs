//! 统一应用事件 [`AppEvent`]：Web 主路径的**唯一** UI 事件投影源。
//!
//! 生产者只剩 **supervisor 线程**（Tokio 后台线程）：下发对话侧状态、审计事实与错误；
//! 消费端是 WS 投影（[`crate::web_api::ws::events::app_event_to_ws_frame`]）与
//! `live2d-ai.toml` 之外的表层镜像（`conversation` 事件 → Mod 投影）。
//!
//! 2026-10-01（W2-B / D1 第二段）：原生壳（winit 事件循环 + `EventLoopProxy`）、
//! `--chat` 的 REPL stdin 线程、托盘线程随壳移出构建 ⇒ `AppEvent::Tray(PetUserEvent)`
//! 变体与 `send_app_event()` helper 一并删除（该变体在 WS 投影里本来就不发帧，
//! 零帧变化）。恢复条件见 `docs/architecture/ARCHIVED-native-shell.md`。
//!
//! 类型边界（D4/D14 裁决原文）：
//! - **PCM 绝不进入 AppEvent**——音频块由 supervisor 直接写入声卡 facade；
//! - **Say 文本绝不进入 AppEvent**——聊天输入走有界命令通道进 supervisor；
//! - 每个 [`ConversationUiEvent`] 都携带产生它的业务 **epoch**；表层镜像
//!   （只能被下发的最新值更新，绝不自增），与事件上的 epoch 不符即丢弃——
//!   这是副作用执行端的**防御性闸门**（权威闸门仍在 core reducer，见 D9）。
//!
//! 2026-09-11 用户裁决：LLM 不暴露任何工具、只做对话——动作系统（含
//! `AppEvent::Render(RenderCommand::…)` 表演投影与 `SemanticAction` 依赖）
//! 已整体移除；本枚举只承载对话状态、root 审计事实、错误与生命周期。

use live2d_ai_runtime::ErrorKind;

/// 发往事件循环的统一用户事件。
#[derive(Debug, Clone, PartialEq)]
pub enum AppEvent {
    /// 对话侧状态通知（口型门控与 epoch 镜像维护用）。
    Conversation(ConversationUiEvent),
    /// root 侧关键事实的审计投影（节点 B 复审要求：UI 事件之外，
    /// 测试必须能观察 root 事实顺序——Started/GenerationFinished/Drained/
    /// TurnCompleted/Cleared 与丢弃原因）。
    RootAudit(RootFact),
    /// **链路错误**（LLM / TTS / 解码 / 背压）的对外投影。
    ///
    /// 2026-09-11 补：此前 `AppEvent` **没有**错误变体，引擎错误只在
    /// `handlers::handle_engine_event` 里 `println!` 上屏——于是
    /// 「UI 只知道本轮失败，不知道为什么」（WS 从来不发 `error` 帧），
    /// 后端日志也只有散文没有码。本变体是那条断线的修复点：
    /// 同一份 `AppErrorEvent` 同时进 `tracing`（落盘）与 WS（上屏）。
    Error(AppErrorEvent),
    /// supervisor 已完成停机清理（关闭流、释放引擎）——事件循环此刻才允许退出。
    ///
    /// `/quit`、`CloseRequested`、`TrayExit` 三者共用同一 shutdown handshake：
    /// 主线程收到请求后置 quitting 并给 supervisor 发 Quit，同时启动 500ms
    /// 兜底计时；提前收到本变体即立刻退出，否则超时强退并打警告。
    ShutdownReady,
}

/// 一条链路错误的**可观测快照**（错误码 + 阶段 + 文案 + 提示）。
///
/// 字段全部是「给人看 / 给机器分支」的纯数据，不含密钥、不含请求头。
/// `code` 的取值契约见 `live2d_ai_runtime::conversation::ErrorKind::code`
/// 所在模块的头注（`<stage>_<suffix>`，例如 `llm_upstream_401`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppErrorEvent {
    /// 稳定机器码，例如 `llm_upstream_401` / `tts_transport`。
    pub code: String,
    /// 出错阶段：`llm` / `tts` / `decode` / `tools`（= `code` 的前缀）。
    pub stage: String,
    /// 人类可读描述（通常含上游状态码与响应体片段）。
    pub message: String,
    /// 一句可执行的处置提示（由后端给出，前端只显示）。
    pub hint: Option<String>,
    /// 产生该错误的业务代次（前端据此判断「这是不是我已经作废的那一轮」）。
    pub epoch: u64,
    /// 是否致命：`false` = 已生成的语音仍会播完（LLM 阶段失败）；
    /// `true` = 剩余排队句子被丢弃、本轮立即结束。
    pub fatal: bool,
}

impl AppErrorEvent {
    /// 由引擎错误构造（**唯一**投影入口，避免各处各写一份映射）。
    ///
    /// `epoch` 取自 `EngineEvent::Error` 自带字段，不另行推断。
    pub fn from_kind(kind: &ErrorKind, epoch: u64) -> Self {
        Self {
            code: kind.code(),
            stage: kind.stage().to_string(),
            message: kind.to_string(),
            hint: kind.hint(),
            epoch,
            fatal: kind.is_fatal(),
        }
    }

    /// 配置热重载失败（`config_reload_failed`）的**唯一**构造点。
    ///
    /// 两个生产出口共用它，保证 code / stage / hint / epoch / fatal 五字段
    /// 逐字一致（用户拿界面上的码去日志里搜，两处必须同一个字符串）：
    /// 1. `supervisor::reload::apply_reload` —— **有** supervisor 时重建引擎失败；
    /// 2. `cli_entry::config_watch_post_reload` —— **没有** supervisor 时手改
    ///    `live2d-ai.toml` 解析失败（S-007：否则界面完全没有提示）。
    ///
    /// `message` 由调用方给出（含具体解析错误）；`epoch` 恒 0（这不是某一轮
    /// 对话的错误，只用于迟到帧判定）；`fatal = false`（旧配置仍在跑，
    /// 不是本轮对话失败——前端因此只显示横幅、不终止任何一轮）。
    pub fn config_reload_failed(message: String) -> Self {
        Self {
            code: "config_reload_failed".to_string(),
            stage: "config".to_string(),
            message,
            hint: Some("修好 live2d-ai.toml 后再保存一次，或重启服务".to_string()),
            epoch: 0,
            fatal: false,
        }
    }

    /// 单行日志文本（`code` 在最前，便于 `grep '^\[error\] llm_upstream_401'`）。
    ///
    /// 与 WS 帧用**同一个** `code`——「日志里搜到的码」和「界面上看到的码」
    /// 必须是同一个字符串，否则对不上号。
    pub fn log_line(&self) -> String {
        match &self.hint {
            Some(h) => format!(
                "[error] {} {}（epoch={}）: {} | 处置: {h}",
                self.code, self.stage, self.epoch, self.message
            ),
            None => format!(
                "[error] {} {}（epoch={}）: {}",
                self.code, self.stage, self.epoch, self.message
            ),
        }
    }
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
    /// 提交 → `TurnCompleted`（端到端 turn 总耗时）。
    pub turn_total_ms: u64,
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
    /// **思考**增量（推理模型的 `reasoning_content`；2026-09-13）。
    ///
    /// 与 [`Self::TextDelta`] 的重要区别：`TextDelta` 走的是
    /// 「该句**语音已合成完毕**」（`SentenceVoiced`）→ 文字与声音同拍；
    /// 而思考**没有声音可对**，所以它直达 UI，不做任何音频同步。
    ///
    /// 用途：web 路径投影为 `reasoning_delta` 帧，前端挂在气泡的「思考」折叠区。
    /// **不得**把它当正文用（不合成、不进句子装配器、不进历史）。
    ReasoningDelta {
        /// 所属业务代次。
        epoch: u64,
        /// 引擎记录的产生时刻（相对本轮起点的毫秒数）。
        ts_ms: u64,
        /// 思考增量片段。
        text: String,
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
    /// LLM 正文**整句**（真实 text payload；P1WS-1 引入；2026-08-29）。
    ///
    /// 由 supervisor 在 `EngineEvent::SentenceVoiced` 处投影——**不是**在
    /// `EngineEvent::TextDelta` 处（那一条只做控制台回显，见
    /// `supervisor/handlers.rs`）。这是「一句一单元：先完整合成再显示」契约的
    /// 落点：文字与声音同拍，而不是领先好几秒。
    ///
    /// 壳侧收到后可选渲染（winit 路径无需显示文本——只维护 render_epoch；web 路径
    /// 经 `ws::app_event_to_ws_frame` 投影为 `text_delta` 帧给前端气泡追加）。
    TextDelta {
        /// 所属业务代次。
        epoch: u64,
        /// 引擎记录的产生时刻（相对本轮起点的毫秒数；可观测性用途）。
        ts_ms: u64,
        /// 句子序号，与 `SentenceVoiced` / 音频帧 `sentence_seq` 同源（从 1 起）。
        sentence_seq: u64,
        /// 该句完整文本（与送 TTS 的那一句逐字相同）。
        text: String,
    },
    /// **正文兜底**：失败轮把已经生成的正文整段交给 UI（rc.3 N0，2026-09-13）。
    ///
    /// 投影自 `EngineEvent::TextFallback`，web 路径为 `text_fallback` 帧。
    /// 与 [`Self::TextDelta`] 的分工：
    /// - `TextDelta` 是**健康路径**的上屏（与声音同拍，前端**追加**）；
    /// - `TextFallback` 是**失败路径**的一次性兜底（前端**整段设置**并标注
    ///   「未收尾」），只在 `TurnStatus::Failed` 且已有正文时出现——
    ///   健康轮永远不发。
    ///
    /// 为什么必须整段而不是只有残余：残余的边界受切句器 trim 影响，前端用字符串
    /// 前缀推导会算错；整段设置天然幂等（详见 `EngineEvent::TextFallback`）。
    TextFallback {
        /// 所属业务代次。
        epoch: u64,
        /// 引擎记录的产生时刻（相对本轮起点的毫秒数；可观测性用途）。
        ts_ms: u64,
        /// 整轮正文（覆盖式设置，非增量）。
        text: String,
    },
    /// **按句 cue**（2026-09-22）：投影自 `EngineEvent::ActionCue`，
    /// web 路径为 **既有** `action_cue` 帧（前端 `ActionCueEvent` 不需要改）。
    ///
    /// wire 契约不变：`cues[]` 单条
    /// `{sentence_seq, preset_id, intensity, ttl_ms, priority}`，前端在该句音频
    /// `first_chunk` 时应用。rc4 起产出口**只有一个**：核心本地规则表
    /// （默认走按句清洗；`[performance].enabled=true` 时按**每一段原文**出 cue，
    /// 表演层 `resolution.cues` 不投递）——导演 Mod 已退役，host 不再转发
    /// `SentenceReady`。
    /// 驱动舞台的**唯一**通道就是本帧；`cues[].preset_id=="none"` = 撤销两槽，
    /// `cues: []` = 本轮不动（D10）。
    ActionCue {
        /// 所属业务代次。
        epoch: u64,
        /// 引擎记录的产生时刻（相对本轮起点的毫秒数）。
        ts_ms: u64,
        /// 这份计划覆盖到的最大句序号。
        covers_upto_seq: u64,
        /// 按句 cue（已校验 / 钳位）。
        cues: Vec<live2d_ai_runtime::performance::PerformanceCue>,
    },
    /// **本轮输入正文**（用户那一句原文，2026-10-14）：web 路径投影为
    /// `user_line` 帧。
    ///
    /// 为什么要这一帧：外部注入（弹幕 sidecar 直连
    /// `POST /api/v1/external/chat`）时**浏览器没参加这次发送**，前端只有助手侧
    /// 的 `text_delta` 可收——历史里于是缺了用户那一句（语音路径靠前端在
    /// `ok:true` 后自己 `acceptInjectedUserTurn` 补，外部注入没有这个回调）。
    /// 本事件把**喂给模型的那一句**原样告诉页面，前端据此补上用户气泡并存档。
    ///
    /// 与 [`Self::TextDelta`] 的分工：本事件是**用户侧**输入，发点在 supervisor
    /// 的 say 分支、**必须先于该轮任何 `TextDelta`**；`TextDelta` 是助手侧正文、
    /// 按句随音频上屏。空文本仍会照发，前端对空文本不写（见执行单 §4）。
    UserLine {
        /// 所属业务代次（与同轮 `TextDelta` 的 epoch 相同）。
        epoch: u64,
        /// 本轮输入正文（与喂给模型的那一句逐字相同）。
        text: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use live2d_ai_runtime::Error as RtError;

    fn llm_401() -> ErrorKind {
        ErrorKind::Llm(RtError::Status {
            status: reqwest::StatusCode::UNAUTHORIZED,
            body: "Authentication Fails (governor)".to_string(),
        })
    }

    /// 投影入口是**唯一**映射点：码/阶段/致命性三者一次算清。
    #[test]
    fn from_kind_carries_code_stage_and_fatality() {
        let e = AppErrorEvent::from_kind(&llm_401(), 5);
        assert_eq!(e.code, "llm_upstream_401");
        assert_eq!(e.stage, "llm");
        assert_eq!(e.epoch, 5);
        assert!(!e.fatal);
        assert!(e.hint.as_deref().unwrap_or("").contains("api_key_env"));
        assert_eq!(
            e.message,
            "LLM 阶段错误: 上游非成功状态 401 Unauthorized: Authentication Fails (governor)"
        );
    }

    /// 日志行与 WS 帧必须是**同一个** code（用户拿界面上的码去日志里搜）。
    #[test]
    fn log_line_starts_with_code_and_includes_hint() {
        let e = AppErrorEvent::from_kind(&llm_401(), 5);
        let line = e.log_line();
        assert!(line.starts_with("[error] llm_upstream_401"), "line: {line}");
        assert!(line.contains("epoch=5"), "line: {line}");
        assert!(line.contains("处置:"), "line: {line}");
    }

    /// 致命错误在窗口侧也带 fatal（决定前端是否还要等这一轮）。
    #[test]
    fn fatal_kinds_are_flagged() {
        let e = AppErrorEvent::from_kind(
            &ErrorKind::Backpressure {
                message: "队列满".into(),
            },
            1,
        );
        assert_eq!(e.code, "tts_backpressure");
        assert_eq!(e.stage, "tts");
        assert!(e.fatal);
    }
}
