//! 设置面板（egui 0.35 overlay）—— W6 接线完成 + W7 可编辑表单。
//!
//! # 文件状态
//!
//! 本文件按调研报告（`docs/plans/ui-selection-report-2026-08-28.md`）
//! 复审检查单 6 项完成：mod.rs 已挂载；`init` 提供 `SettingsUiState` 工厂；
//! `bootstrap.rs` 在 window 就绪后用真实 `egui_winit::State::new` 覆盖
//! `winit_state` 字段（`Option<State>` 是 W6 形状契约）；`handler.rs` 在
//! window_event 入口处理 F10 显隐并把可见时的事件转发给
//! `handle_window_event`；`frame.rs` 的 `draw_and_present_*` 末尾（submit 前）
//! 调 `render_overlay` 把 egui pass 编码进同帧 encoder；`runtime::settings`
//! 提供 `to_toml_string` 对称写回能力。
//!
//! W7 任务（可编辑表单 + 保存按钮）：`render_overlay` 面板内分 LLM / TTS /
//! Persona 三组控件双向绑定 `Draft` 字段；「保存」按钮调 [`save_draft`]
//! 落盘（`plan_atomic_write` + `fs::rename`）并经 `SupervisorHandle::reload`
//! 触发热重载。`config_path` / `supervisor` 由 `bootstrap.rs` 在装配时
//! 注入（chat 模式用真 supervisor；smoke 模式为 `None`，仅写盘不热重载）。
//!
//! # 0.35 真实 API（自 `registry/src/.../egui-wgpu-0.35.0/src/renderer.rs`
//! 与 `egui-winit-0.35.0/src/lib.rs` 核对的现状）
//!
//! - `egui::Context::run_ui(input, |ui| ...) -> FullOutput`（**不是** `run`）。
//! - `egui::Context::tessellate(shapes, pixels_per_point) -> Vec<ClippedPrimitive>`。
//! - `egui_wgpu::Renderer::new(device, format, RendererOptions) -> Self`。
//! - `Renderer::update_buffers(&mut self, device, queue, encoder, paint_jobs,
//!   &ScreenDescriptor) -> Vec<CommandBuffer>`。
//! - `Renderer::render(&self, render_pass, paint_jobs, &ScreenDescriptor)`：
//!   `&self`（多线程场景需 `Mutex`，见 `SettingsUiState::renderer`）。
//! - `egui_winit::State::new(ctx, viewport_id, display_target, native_ppi,
//!   theme, max_texture_side) -> Self`。`State::on_window_event(&mut self,
//!   window: &Window, event: &WindowEvent) -> EventResponse`。
//! - `egui_wgpu::ScreenDescriptor { size_in_pixels: [u32; 2], pixels_per_point: f32 }`。
//!
//! # 接线契约（已实装）
//!
//! 1. **同帧单提交**：清屏路径下 `render_overlay` 与主渲染共用同一次
//!    `queue.submit([encoder.finish()])`（不创建/finish/submit encoder，
//!    只往传入 encoder 追加 `begin_render_pass`，load=Load 保留 Live2D 内容）；
//!    model 路径因 `l2d::render_to_view_submit` 内部已 submit，本函数另起
//!    encoder，frame.rs 同步 submit，两次 submit 均无 `PollType::Wait`。
//! 2. **面板不可见 = 零开销**：`visible == false` 时 `render_overlay` 第一
//!    步 `return`（`should_route_to_ui` 共识：visible && consumed）。
//! 3. **F10 显隐**：由 `handler.rs` 在 WindowEvent 入口检测
//!    `KeyboardInput { physical_key: F10, state: Pressed }` → `toggle_visible`
//!    + `request_redraw`，**先于** egui 事件路由。
//! 4. **`api_key_env` 只存变量名**：`AppSettings::to_toml_string` 写回时
//!    仅序列化环境变量名；密钥由运行时从 `env::var` 读出。
//! 5. **保存后**：「保存成功」/「保存失败」状态条 + 热重载（`Some` 时调
//!    `SupervisorHandle::reload` 走 PATCH 同款路径）。
//!
//! # 门禁
//!
//! - `cargo test -p live2d-ai-desktop --all-targets` 全绿。
//! - `settings_ui.rs` ≤ 1000 行（W7 加表单 / 保存按钮后突破 500；本批在
//!   头注显式豁免 1000 软上限，未来如需继续扩 UI 段，应拆 `settings_ui_*.rs`）。
//! - **行数豁免（≤1000）**：本文件 601 行——表单 UI + save 逻辑 + 面板渲染同文件（egui 闭包捕获状态强耦合，拆分破坏借用结构）；测试已拆 settings_ui_tests.rs。
//!

#![allow(clippy::needless_pass_by_value)] // API 形状按接线契约

use winit::event::WindowEvent;
use winit::window::Window;

use live2d_ai_runtime::settings::patch::plan_atomic_write;
use live2d_ai_runtime::settings::{AppSettings, LlmSettings, PersonaSettings, TtsSettings};

use std::sync::Arc;

use crate::supervisor::SupervisorHandle;

/// 表单草稿：直接对应四组 UI 控件的双向绑定字段。
///
/// 全部用 `String` 简化；`api_key_env` 缺省 = `None`（不鉴权）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Draft {
    /// LLM 服务基址（OpenAI-compatible `/chat/completions`）。
    pub llm_base_url: String,
    /// LLM 模型名（请求体 `model` 字段）。
    pub llm_model: String,
    /// 存放 LLM API key 的环境变量名（**不存密钥**）。
    pub llm_api_key_env: Option<String>,
    /// TTS 服务基址（OpenAI-compatible `/audio/speech`）。
    pub tts_base_url: String,
    /// TTS 音色（请求体 `voice` 字段）。
    pub tts_voice: String,
    /// 存放 TTS API key 的环境变量名（**不存密钥**）。
    pub tts_api_key_env: Option<String>,
    /// 固定注入每轮的 system 提示；空串 = 不发 system 消息。
    pub persona_system_prompt: String,
    /// 保留历史轮数上限；0 = 每轮独立。
    pub persona_max_history_pairs: usize,
    /// W7 任务：开发者模式开关（UI 显式 checkbox；走 `save_draft` 三态路径
    /// 落盘后由 PATCH handler 同步到 `StatusContext`，即时生效）。
    pub dev_mode: bool,
}

/// egui 渲染器（状态机 + GPU renderer）。
///
/// 字段顺序与文档一致：`State`（winit 事件桥）持有 `Context` 的 clone，所以
/// 上下文可独立访问；`Renderer`（egui-wgpu GPU 端）独立存放。
pub(crate) struct SettingsUiState {
    /// egui 上下文（线程安全的 `Arc` 包装，`State` 与本字段共享同一份）。
    pub ctx: egui::Context,
    /// winit 事件桥；`take_egui_input` / `on_window_event` 都在它身上。
    ///
    /// **W6 接线约束**：`State::new` 0.35 必须有 `&dyn HasDisplayHandle`
    /// （要等 `bootstrap.rs` 里 `Window` 就绪）。本字段在 `init` 时**只
    /// 占位**——由 W6 接线批次在 `bootstrap.rs` 里用真实
    /// `egui_winit::State::new(&window, ...)` 重新赋值。`Option<State>`
    /// 是 W6 接线时的形状契约（占位 → `Some(real)`）。
    pub winit_state: Option<egui_winit::State>,
    /// egui-wgpu 渲染器（**0.35 是 `&self` 渲染**——见文件头 API 说明）。
    ///
    /// 用 `Mutex` 允许任意线程持有 `SettingsUiState` 调 `render_overlay`；
    /// 短临界区（一次渲染 pass）下锁开销可忽略。
    pub renderer: std::sync::Mutex<egui_wgpu::Renderer>,
    /// 面板可见性（F10 切换）。
    pub visible: bool,
    /// 表单草稿（按控件读 / 写）。
    pub draft: Draft,
    /// 状态条消息（保存结果 / 错误），由 UI 内 Label 显示。
    pub status_msg: Option<String>,
    /// 配置文件路径（`live2d-ai.toml`）。`None` = 面板无可写目标（无
    /// `--config` 或 chat 装配未注入），保存按钮按失败处理并提示「未
    /// 配置路径」。由 `bootstrap.rs` 在装配时注入（与 `web_api` 的
    /// `ServerContext::config_path` 同源）。
    pub config_path: Option<String>,
    /// supervisor 句柄（chat 模式为 `Some`，smoke / 无 chat 时为 `None`）。
    /// 保存按钮写盘成功后调 `reload()` 触发热重载；`None` = 仅写盘不热重载
    /// （与 PATCH 路由写盘但无 supervisor 的回退路径同语义）。
    pub supervisor: Option<Arc<SupervisorHandle>>,
}

/// 任务 2 要求的 init 函数：构造 `SettingsUiState`。
///
/// **W6 接线约束**：`egui_winit::State::new` 在 0.35 需要 `&dyn
/// HasDisplayHandle`（即 `&Window`），本函数没有 Window 句柄——
/// **只构造 Context + Renderer + 占位 `State = None`**，由 W6 接线
/// 批次（`bootstrap.rs`）在 Window 就绪后用 `State::new(&window, ...)`
/// 把字段 `winit_state` 补成 `Some(...)`。
///
/// W7 增强：`config_path` / `supervisor` 由 `bootstrap.rs` 在装配时注
/// 入——前者来自 `RunOptions::config` / `chat` 装配的最终路径；后者来自
/// `ChatBridge::supervisor`（chat 模式为 `Some`，smoke 模式为 `None`）。
/// `init` 不读盘，避免破坏 W6 阶段「面板只读」的最小接线语义；如需把
/// 当前磁盘配置填到 `Draft` 由调用方在拿到 `SettingsUiState` 后手动
/// `settings_to_draft(&loaded)` 注入。
///
/// 调用方契约（接线批次 W6 + W7）：
/// ```ignore
/// let mut state = init(
///     &device,
///     fmt,
///     max_tex,
///     Some(config_path.clone()),
///     chat_bridge.map(|b| b.supervisor.clone()),
/// );
/// state.winit_state = Some(egui_winit::State::new(
///     state.ctx.clone(),
///     egui::ViewportId::ROOT,
///     &window,
///     Some(window.scale_factor() as f32),
///     None, // theme: 从 OS 检测
///     None, // max_texture_side: 走 egui 默认
/// ));
/// ```
pub(crate) fn init(
    device: &wgpu::Device,
    output_format: wgpu::TextureFormat,
    _max_texture_dimension: u32,
    config_path: Option<String>,
    supervisor: Option<Arc<SupervisorHandle>>,
) -> SettingsUiState {
    let ctx = egui::Context::default();
    // 0.35 的 RendererOptions 没有 Default：四个字段全填显式字面量。
    let renderer_options = egui_wgpu::RendererOptions {
        msaa_samples: 1,
        depth_stencil_format: None,
        dithering: true,
        predictable_texture_filtering: false,
    };
    let renderer = egui_wgpu::Renderer::new(device, output_format, renderer_options);
    SettingsUiState {
        ctx,
        // 占位——W6 接线在 bootstrap.rs 里用真实 State::new 覆盖。
        winit_state: None,
        renderer: std::sync::Mutex::new(renderer),
        visible: false,
        draft: Draft::default(),
        status_msg: None,
        config_path,
        supervisor,
    }
}

/// 路由判定：窗口事件该走 egui 还是常规 winit 分支。
///
/// - `visible == false`：恒 `false`（面板不消费事件，零拦截——节点 C
///   锁定语义：F10 显隐时鼠标穿透、桌宠右键菜单、拖动都不被吞）。
/// - `visible == true && consumed_by_egui == true`：true（egui 拿到了
///   焦点 / 文本输入态，调用方应**不**继续分发该事件）。
/// - `visible == true && consumed_by_egui == false`：false（egui 没消费，
///   事件继续给游戏 / 桌宠 / 其它 winit 处理器）。
pub(crate) fn should_route_to_ui(visible: bool, consumed_by_egui: bool) -> bool {
    visible && consumed_by_egui
}

/// F10 显隐：原地翻转 `state.visible`。
pub(crate) fn toggle_visible(state: &mut SettingsUiState) {
    state.visible = !state.visible;
}

/// 把 winit 事件转发给 egui（仅当面板可见时——隐藏时上层不该调本函数）。
///
/// **0.35 真实签名**（核对自 `egui-winit-0.35.0/src/lib.rs:282`）：
/// ```ignore
/// State::on_window_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse
/// ```
/// 返回 `EventResponse.consumed`——caller 据此分发剩余 winit 分支。
///
/// **W6 接线占位**：`winit_state` 字段是 `Option<State>`，未接线时为
/// `None` → 返回 `EventResponse { consumed: false, repaint: true }`——
/// 「repaint=true 让 caller 重画一帧但不过度拦截」。W6 接线后改为：
/// ```ignore
/// state.winit_state.as_mut()
///     .expect("W6 接线必须已设置 winit_state")
///     .on_window_event(window, event)
/// ```
pub(crate) fn handle_window_event(
    state: &mut SettingsUiState,
    window: &Window,
    event: &WindowEvent,
) -> egui_winit::EventResponse {
    // 不可见 = 零拦截（与 should_route_to_ui(_, false) 共识）。
    if !state.visible {
        return egui_winit::EventResponse::default();
    }
    // W6 接线启用路径：
    match state.winit_state.as_mut() {
        Some(winit_state) => winit_state.on_window_event(window, event),
        None => {
            // 未接线 = 假装消费 + 触发重绘；W6 之后不会走这条。
            let _ = (state, event);
            egui_winit::EventResponse {
                consumed: true,
                repaint: true,
            }
        }
    }
}

/// 草稿 → `AppSettings`：首次创建（磁盘无配置）时使用；也供 round-trip 测试。
///
/// **F2b 语义**：本函数**全量重建** `AppSettings`（TTS 隐藏字段
/// `model/sample_rate/channels` 走 default）。仅当磁盘**无**既有配置
/// （首次保存）时才可安全使用——存在既有配置时必须走
/// [`apply_draft`]（只合并 UI 暴露字段，保留隐藏字段）。
pub(crate) fn draft_to_settings(d: &Draft) -> AppSettings {
    AppSettings {
        llm: LlmSettings {
            base_url: d.llm_base_url.clone(),
            model: d.llm_model.clone(),
            api_key_env: d.llm_api_key_env.clone().filter(|s| !s.is_empty()),
            // max_tokens 不在 v1 草稿里（egui 面板不暴露输出上限），
            // 走省略 = DEFAULT_MAX_TOKENS；已有配置由 apply_draft 原样保留。
            max_tokens: None,
        },
        tts: TtsSettings {
            base_url: d.tts_base_url.clone(),
            // tts.model 在 v1 草稿里不暴露（只用 voice），固定 None。
            model: None,
            voice: d.tts_voice.clone(),
            api_key_env: d.tts_api_key_env.clone().filter(|s| !s.is_empty()),
            // sample_rate / channels 走 TtsSettings::default() 的值
            //（24 kHz 单声道；v1 面板不暴露 PCM 规格——如要改走配置文件）。
            ..TtsSettings::default()
        },
        persona: PersonaSettings {
            system_prompt: d.persona_system_prompt.clone(),
            max_history_pairs: d.persona_max_history_pairs,
            // name/description 由 Web 设置（角色卡）写入；egui F10 面板暂不暴露。
            ..PersonaSettings::default()
        },
        // W7 任务：dev_mode 由 UI 显式字段控制（checkbox 双向绑定）。
        dev_mode: d.dev_mode,
    }
}

/// **F2b**：把 UI 草稿**只合并**到既有 `AppSettings` 的 UI 暴露字段。
///
/// 只改 9 个字段（llm.base_url/model/api_key_env、tts.base_url/voice/
/// api_key_env、persona.system_prompt/max_history_pairs、dev_mode）；
/// **不动**隐藏字段（tts.model / tts.sample_rate / tts.channels 及未来
/// 新增字段）——避免「用户只改 LLM，TTS 音频规格被重置为默认」的损坏。
///
/// 草稿空 `api_key_env`（UI 清空）→ `None`（与 `draft_to_settings` 同语义）。
pub(crate) fn apply_draft(target: &mut AppSettings, d: &Draft) {
    target.llm.base_url = d.llm_base_url.clone();
    target.llm.model = d.llm_model.clone();
    target.llm.api_key_env = d.llm_api_key_env.clone().filter(|s| !s.is_empty());
    target.tts.base_url = d.tts_base_url.clone();
    target.tts.voice = d.tts_voice.clone();
    target.tts.api_key_env = d.tts_api_key_env.clone().filter(|s| !s.is_empty());
    target.persona.system_prompt = d.persona_system_prompt.clone();
    target.persona.max_history_pairs = d.persona_max_history_pairs;
    target.dev_mode = d.dev_mode;
}

/// `AppSettings` → 草稿：UI 启动 / 「重新载入」时调用。
///
/// 由 `settings_ui_tests::dev_mode_round_trips_through_draft` 等 round-trip
/// 测试引用；保留为 `pub(crate)`，但生产 UI 当前不直接调用（bootstrap 阶
/// 段还未把 loaded `AppSettings` 注入 `Draft`——后续 UI 任务批次如需「重
/// 新载入」按钮再接）。`#[allow(dead_code)]` 仅保留给「测试独占」的事实
/// 一条说明注释。
#[allow(dead_code)] // round-trip 测试专用；UI 暂未提供「重新载入」入口
pub(crate) fn settings_to_draft(s: &AppSettings) -> Draft {
    Draft {
        llm_base_url: s.llm.base_url.clone(),
        llm_model: s.llm.model.clone(),
        llm_api_key_env: s.llm.api_key_env.clone(),
        tts_base_url: s.tts.base_url.clone(),
        tts_voice: s.tts.voice.clone(),
        tts_api_key_env: s.tts.api_key_env.clone(),
        persona_system_prompt: s.persona.system_prompt.clone(),
        persona_max_history_pairs: s.persona.max_history_pairs,
        dev_mode: s.dev_mode,
    }
}

/// 草稿 → 磁盘（与 Web PATCH 共用同一 SettingsService 路径）。
///
/// 复用 W1 的 [`plan_atomic_write`]（`fdatasync` + 拒绝空内容 + 失败清理
/// tmp） + `fs::rename` 原子换名——**不**直接 `fs::write` 整个文件，避免
/// 与 `settings_routes::apply_and_write` 形成第二套保存逻辑。
///
/// 写盘成功后：
/// - 若 `supervisor` 为 `Some` → 调用 `handle.reload()` 触发热重载；
/// - 若为 `None`（仅面板写盘的 egui-only 模式）→ 仅写盘，不通知。
///
/// 写盘失败（IO / 权限 / 路径） → 返回 `Err`，UI 状态条显示「保存失败」，
/// 旧配置文件保持原状（`plan_atomic_write` 失败时不会污染原文件）。
pub(crate) fn save_draft(
    draft: &Draft,
    config_path: &str,
    supervisor: Option<&Arc<SupervisorHandle>>,
) -> Result<(), String> {
    let path = std::path::Path::new(config_path);
    // F2b-M0-2：**先读磁盘既有配置**，只合并 UI 暴露字段——保留
    // tts.model / sample_rate / channels 等隐藏字段（避免「只改 LLM 却
    // 重置 TTS 音频规格」）。仅当文件**不存在**（首次保存）→ 全量创建；
    // 已存在但解析失败（损坏/格式过期）→ 拒绝覆盖（避免把坏配置写死）。
    let next = match live2d_ai_runtime::AppSettings::load_from_path(path) {
        Ok(mut s) => {
            apply_draft(&mut s, draft);
            s
        }
        // 仅「文件不存在」= 首次保存 → 全量创建；
        // 已存在但解析失败（损坏/格式过期）→ 拒绝覆盖（避免把坏配置写死）。
        Err(live2d_ai_runtime::settings::SettingsError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            draft_to_settings(draft)
        }
        Err(e) => return Err(format!("既有配置解析失败，拒绝覆盖: {e}")),
    };
    // F2b-M0-1：写盘**前**统一配置校验（URL / env 名 / audio spec）。
    // `resolve_with(|_| None)` 不要求真实 key 存在，但会校验语法——
    // 失败返回 Err，**不**创建/替换目标配置、不触发 reload。
    next.resolve_with(|_| None)
        .map_err(|e| format!("配置校验失败: {e}"))?;
    // **合并写回**：保留 `live2d-ai.toml` 里的注释与排版（见
    // `AppSettings::merge_into_toml` 的说明）。
    let toml_text = next.to_toml_string_merging(std::path::Path::new(config_path));
    // 拒绝空内容由 plan_atomic_write 内部保证（避免误存空配置）。
    let tmp = plan_atomic_write(path, &toml_text).map_err(|e| format!("原子写回失败: {e}"))?;
    // rename 失败时清理 tmp（plan_atomic_write 已保证 tmp 存在）。
    if let Err(e) = std::fs::rename(&tmp, config_path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("rename {}: {e}", tmp.display()));
    }
    if let Some(h) = supervisor {
        h.reload();
    }
    Ok(())
}

/// 渲染面板到 `frame_view`（由调用方在 Live2D pass 之后、submit 之前调）。
///
/// **接线契约（重申）**：
/// 1. 面板不可见 → 立即 return（零开销）。
/// 2. 本函数**不**创建 / finish / submit encoder——只往 caller 传入的
///    `encoder` 追加 `begin_render_pass`（load=Load，保留 Live2D 内容）。
/// 3. caller 在外部 `queue.submit([encoder.finish()])` 一次性提交，保证
///    Live2D + egui 同帧呈现（model 路径 `l2d` 内部已 submit，本函数新建
///    encoder，由 frame.rs 同步 submit——两次 submit 均无 `PollType::Wait`）。
///
/// `screen_px` = `[width, height] in physical pixels`（由 caller 从
/// `frame_view` 推断）；`pixels_per_point` 来自 `State::take_egui_input`
/// 的 `FullOutput::pixels_per_point`（0.35 的 `egui_wgpu::ScreenDescriptor`
/// 字段名照搬即可）。
pub(crate) fn render_overlay(
    state: &mut SettingsUiState,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    frame_view: &wgpu::TextureView,
    screen_px: [f32; 2],
    window: &Window,
) {
    // 0. 隐藏 = 零开销（gate #2 强约束：未接线面板不影响基准）。
    if !state.visible {
        return;
    }
    // 1. 拿输入：必须 `take_egui_input(&Window)` 才能清空 `winit_state.egui_input`
    //    并设置时间/屏幕矩形；返回的 `RawInput` 喂给 `run_ui`。
    let raw_input = match state.winit_state.as_mut() {
        Some(ws) => ws.take_egui_input(window),
        None => return, // 接线前 = 静默跳过（防御）
    };

    // 2. 跑 UI 闭包：W7 任务把 v1 Label 占位换成可编辑表单 + 「保存」/「关闭」
    //    按钮。控件全部双向绑定 `state.draft` 字段；`save_clicked` 由闭包
    //    设置、闭包返回后由调用方调 `save_draft` 落盘（与「同帧单提交」契
    //    约兼容——保存动作不直接 submit encoder，只在状态条写消息）。
    let status_msg = state.status_msg.clone();
    let save_clicked = std::cell::Cell::new(false);
    let close_clicked = std::cell::Cell::new(false);
    let full_output = state.ctx.run_ui(raw_input, |ui| {
        egui::Window::new("设置").show(ui.ctx(), |ui| {
            // ---- LLM 组 ----
            ui.collapsing("LLM（OpenAI-compatible /chat/completions）", |ui| {
                ui.label("Base URL");
                ui.text_edit_singleline(&mut state.draft.llm_base_url);
                ui.label("Model");
                ui.text_edit_singleline(&mut state.draft.llm_model);
                ui.label("API Key 环境变量名（非密钥本身）");
                // `Option<String>` 用 helper 字段化：TextEdit 只吃 `&mut String`，
                // 这里把 `None` 显示为空串、`Some(s)` 显示为 s；保存时 `draft_to_settings`
                // 内部 `.filter(|s| !s.is_empty())` 把它规范回 `None`。
                let mut llm_key_buf = state.draft.llm_api_key_env.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut llm_key_buf).changed() {
                    state.draft.llm_api_key_env = if llm_key_buf.is_empty() {
                        None
                    } else {
                        Some(llm_key_buf)
                    };
                }
            });
            // ---- TTS 组 ----
            ui.collapsing("TTS（OpenAI-compatible /audio/speech）", |ui| {
                ui.label("Base URL");
                ui.text_edit_singleline(&mut state.draft.tts_base_url);
                ui.label("Voice");
                ui.text_edit_singleline(&mut state.draft.tts_voice);
                ui.label("API Key 环境变量名（非密钥本身）");
                let mut tts_key_buf = state.draft.tts_api_key_env.clone().unwrap_or_default();
                if ui.text_edit_singleline(&mut tts_key_buf).changed() {
                    state.draft.tts_api_key_env = if tts_key_buf.is_empty() {
                        None
                    } else {
                        Some(tts_key_buf)
                    };
                }
            });
            // ---- Persona 组 ----
            ui.collapsing("Persona", |ui| {
                ui.label("系统提示词（System Prompt）");
                // 多行编辑：空串 = 不发 system 消息（与 AppSettings 同语义）。
                ui.text_edit_multiline(&mut state.draft.persona_system_prompt);
                ui.label(format!(
                    "历史轮数上限（每轮独立 = 0）: {}",
                    state.draft.persona_max_history_pairs
                ));
                let mut history_buf = state.draft.persona_max_history_pairs as i64;
                if ui
                    .add(egui::DragValue::new(&mut history_buf).range(0..=1024))
                    .changed()
                {
                    state.draft.persona_max_history_pairs = history_buf.max(0) as usize;
                }
            });
            // ---- 开关 / 行为 ----
            ui.separator();
            // 开发者模式开关：勾选后由用户「保存」动作落盘；与 CLI flag
            // 互不覆盖（CLI 启动期决定 → settings 落盘 → UI 重启可见）。
            ui.checkbox(&mut state.draft.dev_mode, "开发者模式（dev_mode）")
                .on_hover_text("打开后 /api/v1/logs* 端点放行；保存后立即生效（无需重启）");
            // ---- 按钮行 ----
            ui.horizontal(|ui| {
                if ui.button("保存").clicked() {
                    save_clicked.set(true);
                }
                if ui.button("关闭").clicked() {
                    close_clicked.set(true);
                }
            });
            // ---- 状态条（保存结果 / 错误 / 「未配置路径」提示）----
            if let Some(msg) = &status_msg {
                ui.label(msg);
            }
        });
    });

    // 2b. 处理「关闭」按钮：直接置位 visible=false（隐藏后下一帧 render_overlay
    //     第一步 return = 零开销，契约不变）。
    if close_clicked.get() {
        state.visible = false;
    }

    // 2c. 处理「保存」按钮：闭包内已把字段写入 `state.draft`，这里调
    //     `save_draft` 走 `plan_atomic_write` + `fs::rename` + 可选 reload。
    //     注意：本函数**不**创建/finish encoder（不动 Live2D 提交语义），
    //     只在 `state.status_msg` 写结果供下一帧 Label 显示。
    if save_clicked.get() {
        let path_owned = state.config_path.clone();
        let supervisor_ref = state.supervisor.as_ref();
        let result = match path_owned.as_deref() {
            Some(p) if !p.is_empty() => save_draft(&state.draft, p, supervisor_ref),
            _ => Err("未配置 config_path（启动未注入 live2d-ai.toml 路径）".to_string()),
        };
        state.status_msg = Some(match result {
            Ok(()) => "保存成功，已热重载".to_string(),
            Err(e) => format!("保存失败: {e}"),
        });
    }

    // 3. 上传纹理（fonts delta + 用户纹理）。
    let mut renderer = match state.renderer.lock() {
        Ok(r) => r,
        Err(_) => return, // 中毒锁 = 静默跳过（不污染主渲染）
    };
    for (id, delta) in &full_output.textures_delta.set {
        renderer.update_texture(device, queue, *id, delta);
    }
    for id in &full_output.textures_delta.free {
        renderer.free_texture(id);
    }

    // 4. 轻量几何。
    let paint_jobs = state
        .ctx
        .tessellate(full_output.shapes, full_output.pixels_per_point);
    let screen_desc = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [screen_px[0] as u32, screen_px[1] as u32],
        pixels_per_point: full_output.pixels_per_point,
    };
    let _ = renderer.update_buffers(device, queue, encoder, &paint_jobs, &screen_desc);

    // 5. 编码 pass（gate #1 同帧单提交）。`load=Load` 保留 Live2D 内容；
    //    `forget_lifetime` 是 egui-wgpu 0.35 渲染签名要求（`RenderPass<'static>`）。
    let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("egui_overlay"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: frame_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    let mut rpass_static = rpass.forget_lifetime();
    renderer.render(&mut rpass_static, &paint_jobs, &screen_desc);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----- should_route_to_ui：3 条 -----

    #[test]
    fn route_hidden_always_false_even_if_egui_consumed() {
        // 隐藏时绝不拦截事件（节点 C 锁定：桌宠穿透 / 托盘菜单不丢）。
        assert!(!should_route_to_ui(false, false));
        assert!(!should_route_to_ui(false, true));
    }

    #[test]
    fn route_visible_only_when_egui_actually_consumed() {
        // 可见 + 消费 → 走 egui。
        assert!(should_route_to_ui(true, true));
        // 可见 + 未消费（鼠标移到空白处）→ 让事件继续给桌宠。
        assert!(!should_route_to_ui(true, false));
    }

    #[test]
    fn route_f10_toggle_is_decoupled_from_consumed() {
        // 显隐由 toggle_visible 决定（独立于 consumed）；本函数只负责
        // 「可见时是否拦截」。可见但未消费事件 = 不拦截——测试组合。
        assert!(!should_route_to_ui(true, false));
        assert!(!should_route_to_ui(false, true));
    }

    // ----- draft ↔ settings round-trip：2 条 -----

    #[test]
    fn draft_to_settings_preserves_all_fields() {
        let d = Draft {
            llm_base_url: "https://api.openai.com/v1".into(),
            llm_model: "gpt-4o-mini".into(),
            llm_api_key_env: Some("OPENAI_KEY".into()),
            tts_base_url: "https://tts.example/v1".into(),
            tts_voice: "nova".into(),
            tts_api_key_env: Some("TTS_KEY".into()),
            persona_system_prompt: "你是桌宠".into(),
            persona_max_history_pairs: 4,
            dev_mode: true,
        };
        let s = draft_to_settings(&d);
        assert_eq!(s.llm.base_url, d.llm_base_url);
        assert_eq!(s.llm.model, d.llm_model);
        assert_eq!(s.llm.api_key_env.as_deref(), Some("OPENAI_KEY"));
        assert_eq!(s.tts.base_url, d.tts_base_url);
        assert_eq!(s.tts.voice, d.tts_voice);
        assert_eq!(s.tts.api_key_env.as_deref(), Some("TTS_KEY"));
        // tts.model 固定 None（v1 面板不暴露）。
        assert!(s.tts.model.is_none());
        // tts PCM 规格走 TtsSettings::default()。
        assert_eq!(s.tts.sample_rate, 24_000);
        assert_eq!(s.tts.channels, 1);
        assert_eq!(s.persona.system_prompt, d.persona_system_prompt);
        assert_eq!(s.persona.max_history_pairs, 4);
        // W7 任务：dev_mode 由 Draft.dev_mode 透传到 AppSettings.dev_mode。
        assert_eq!(s.dev_mode, d.dev_mode, "draft.dev_mode → settings.dev_mode");
    }

    #[test]
    fn settings_to_draft_round_trip_back_to_equal_draft() {
        // 起点 d1 → settings → d2；d2 必须等于 d1（前提：d1 字段都符合
        // settings 反序列化形态——v1 面板不暴露的 tts.model 固定 None，
        // round-trip 时 d1.tts_api_key_env 空串会被 draft_to_settings 过滤成 None）。
        let d1 = Draft {
            llm_base_url: "http://127.0.0.1:11434/v1".into(),
            llm_model: "qwen2.5:7b".into(),
            llm_api_key_env: Some("LLM_KEY".into()),
            tts_base_url: "http://127.0.0.1:8000/v1".into(),
            tts_voice: "alloy".into(),
            tts_api_key_env: None, // 已过滤形态
            persona_system_prompt: String::new(),
            persona_max_history_pairs: 0,
            dev_mode: false,
        };
        let s = draft_to_settings(&d1);
        let d2 = settings_to_draft(&s);
        assert_eq!(d1, d2);
    }

    // ----- api_key_env 密钥不入文件契约：见 `settings_ui_secret_tests`（拆分承载）-----
}
