//! 渲染循环（仅 `target_arch = "wasm32"` 下编译）：取交换链纹理 →
//! **`render_to_view_submit`** → present，以及 surface 生命周期契约（C4）与运行时 HUD。
//!
//! 单帧状态机契约见 [`tick`]：Suboptimal 先 render/present 再 configure；Outdated 无
//! texture 直接 configure；Lost configure + 同步视口；Timeout/Occluded 跳过；
//! Validation 连续两次视为 fatal 并停循环（P0-2-2）。

use std::{cell::RefCell, rc::Rc, sync::Arc};

use glam::f32::Affine2;
use l2d::renderer::{FIXED_DT_60HZ, GpuContext, ModelRendererCore};
use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{HtmlCanvasElement, Window};

use super::gpu::{DPR_CAP, canvas_pixel_size};
use super::idle::{self, IdleState};
use super::input::{BridgeState, apply_bridge_effects};
use crate::web::net::js_str;
use crate::web::status;

/// HUD 统计窗口长度（ms）。500ms 刷新一次 DOM，远低于每帧刷屏成本；
/// 同时能覆盖足够多的帧样本估出稳定 FPS。
const HUD_INTERVAL_MS: f64 = 500.0;

/// rAF 循环共享态：渲染核心 + surface + 配置 + 尺寸来源。
pub(crate) struct FrameState {
    pub(crate) core: ModelRendererCore,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) gpu: Arc<GpuContext>,
    pub(crate) canvas: HtmlCanvasElement,
    pub(crate) window: Window,
    /// 连续 `CurrentSurfaceTexture::Validation` 命中计数（P0-2-2）：
    /// `>= 2` 时判定为 fatal、写状态栏并停 rAF 调度，避免 GPU 校验错误
    /// 状态下无限重试。任意非 Validation 取帧结果会清零。
    pub(crate) validation_streak: u8,
    /// 运行时 HUD 统计（adapter 固定在首次写入时快照，其余每 500ms 刷新）。
    pub(crate) hud: HudState,
    pub bridge: BridgeState,
    /// RM6 待机生命体征采样器（呼吸/眨眼/微表情）。
    pub(crate) idle: IdleState,
    /// load_model 后立即捕获的 layout 初始 transform（Affine2）。
    /// 每帧 set_transform 都在此基础上右乘用户缩放/平移，保持 layout 构图
    /// 不变，仅叠加交互变换。
    /// 累积未模拟的真实时间（秒）：用于帧率无关的固定 dt 步进（R4）。
    /// rAF 时间戳到达时累加 real_dt，直到累积 >= FIXED_DT_60HZ 再消费一次
    /// core.update，避免 240Hz 时物理/姿态模拟跑到 4× 速度。
    pub(crate) last_layout_transform: Affine2,
}

/// 运行时性能 HUD 统计态。
///
/// 生命周期：rAF 循环内逐帧累积，错峰 500ms 刷屏一次 DOM。
/// - `window_start`：当前统计窗起点（performance.now()）；
/// - `frame_count`：本窗内已渲染帧数；
/// - `cpu_time_sum`：本窗内 CPU 渲染耗时总和（ms），滑动求平均帧耗时；
/// - `last_cpu_ms`：最近一次渲染耗时（ms），用于实时反馈；
/// - `adapter_line`：adapter 信息快照（一次性写入）；
/// - `pending`/`due`：500ms 刷新节拍标记。
pub(crate) struct HudState {
    window_start: f64,
    frame_count: u32,
    cpu_time_sum: f64,
    last_cpu_ms: f64,
    adapter_line: Option<String>,
    last_fps: f64,
    /// 上一帧 rAF 时间戳（performance.now()，毫秒）。用于计算真实帧间 dt。
    /// None 表示首帧，dt 使用 FIXED_DT_60HZ 兜底。
    last_frame_ms: Option<f64>,
    /// R4：最近一次 tick 消耗的模拟步数（core.update 调用次数）。
    /// 证明帧率无关：60fps 恒为 1，240fps 偶尔 2-4。
    last_sim_steps: u32,
}

impl Default for HudState {
    fn default() -> Self {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        Self {
            window_start: now,
            frame_count: 0,
            cpu_time_sum: 0.0,
            last_cpu_ms: 0.0,
            adapter_line: None,
            last_fps: 0.0,
            last_frame_ms: None,
            last_sim_steps: 0,
        }
    }
}

impl HudState {
    /// 一次性写入 adapter 信息（在 init_gpu 后、rAF 启动前调用）。
    pub(crate) fn snapshot_adapter(
        &mut self,
        backend: wgpu::Backend,
        adapter_name: &str,
        is_webgpu: bool,
    ) {
        // WebGPU 原生路 → "WebGPU"；GL 回退 → "WebGL2"。
        let api = if is_webgpu { "WebGPU" } else { "WebGL2" };
        self.adapter_line = Some(format!("{backend}/{api} ({adapter_name})"));
    }

    /// 记录一帧 CPU 渲染耗时（由 render_and_present 调用）。
    pub(crate) fn record_frame(&mut self, cpu_ms: f64) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.cpu_time_sum += cpu_ms;
        self.last_cpu_ms = cpu_ms;
    }

    /// 是否到了 500ms 刷新 DOM 的时机。
    pub(crate) fn due(&self) -> bool {
        let now = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        now - self.window_start >= HUD_INTERVAL_MS
    }

    /// 刷新统计窗：结算当前 FPS/CPU 均值，重置计数器。
    pub(crate) fn flush(&mut self) {
        let elapsed = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(self.window_start + HUD_INTERVAL_MS)
            - self.window_start;
        if elapsed > 0.0 && self.frame_count > 0 {
            self.last_fps = (self.frame_count as f64 / elapsed) * 1000.0;
        }
        self.window_start = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(self.window_start + HUD_INTERVAL_MS);
        self.frame_count = 0;
        self.cpu_time_sum = 0.0;
    }
}

pub(crate) type SharedState = Rc<RefCell<FrameState>>;
pub(crate) type FrameClosure = Closure<dyn FnMut(f64)>;
pub(crate) type FrameSlot = Rc<RefCell<Option<FrameClosure>>>;

/// resize 处理：重算物理尺寸 → 重配 surface → 同步渲染核心视口。
/// （窗口缩放/DPR 变化都会触发；尺寸不变时跳过。）
///
/// **C4 裁决 P0-3 WASM 同步**（与 desktop 同语义）：
/// - 同一尺寸重复触发由 `(width, height) == (st.config.width, st.config.height)`
///   守卫直接 return，等价于 desktop 的 latest-size 合并（last-wins）；
/// - **不**使用固定毫秒防抖：rAF 循环天然每帧重读 `canvas.client_*`，
///   `tick` 在每次 `get_current_texture` 前都按当前 config 出帧；
///   浏览器连续 resize 事件间无未 present 的 swapchain texture 存活
///   （`install_resize_handler` 与 `tick` 不并发），所以可立即 configure
///   而不违反"先 present 再 configure"契约；
/// - 因此**不**像 desktop 那样维护 `pending_surface_size`：WASM 路径
///   resize → configure → set_viewport 在 rAF 帧间是原子的，合并语义
///   已被 rAF 节流 + 同尺寸守卫覆盖。
pub(crate) fn install_resize_handler(state: &SharedState) {
    let handler: Closure<dyn FnMut()> = Closure::new({
        let state = Rc::clone(state);
        move || {
            sync_canvas_size(&state);
        }
    });
    let window = state.borrow().window.clone();
    if let Err(e) =
        window.add_event_listener_with_callback("resize", handler.as_ref().unchecked_ref())
    {
        status(&format!("注册 resize 监听失败：{}", js_str(&e)));
        return;
    }
    // 页面生命周期内常驻（demo 不做卸载路径），防止闭包被回收。
    std::mem::forget(handler);

    // **兜底自愈（2026-09-10）**：宿主把本页嵌进 iframe / Flutter `HtmlElementView` 时，
    // 槽位尺寸变化不保证在 `window` 上派发 `resize`；漏掉一次就会让 canvas 位图与 CSS
    // 显示盒尺寸不一致 → 浏览器**非等比**拉伸 → 模型看起来被压扁。
    // 这里每 500ms 比对一次（`canvas_pixel_size` 同口径），不一致就重配 surface + 视口。
    let resync_state = Rc::clone(state);
    let resync: Closure<dyn FnMut()> = Closure::new(move || {
        sync_canvas_size(&resync_state);
    });
    if let Some(window) = web_sys::window()
        && window
            .set_interval_with_callback_and_timeout_and_arguments_0(
                resync.as_ref().unchecked_ref(),
                500,
            )
            .is_ok()
    {
        std::mem::forget(resync);
    }
}

/// 把 canvas 位图（缓冲）尺寸对齐到当前 CSS 显示盒 × DPR。
///
/// 与 [`install_resize_handler`] 的 `resize` 回调、以及 500ms 兜底定时器共用同一逻辑：
/// 尺寸未变则 no-op（避免无谓的 `configure`）。
///
/// 返回是否发生了重配。
pub(crate) fn sync_canvas_size(state: &SharedState) -> bool {
    let mut st = state.borrow_mut();
    let (width, height) = canvas_pixel_size(&st.canvas, &st.window, st.bridge.tier);
    if (width, height) == (st.config.width, st.config.height) {
        return false;
    }
    st.config.width = width;
    st.config.height = height;
    st.surface.configure(st.gpu.device(), &st.config);
    if let Err(e) = st.core.set_viewport(width, height) {
        status(&format!("resize 失败：{e}"));
    }
    true
}

/// 诊断口径：当前 CSS 显示盒 `(宽, 高)` 与钳制后的 DPR。
///
/// 状态栏同时打印「缓冲」与「CSS」两组尺寸——两者宽高比不一致即为压扁根因。
pub(crate) fn canvas_css_metrics(canvas: &HtmlCanvasElement, window: &Window) -> (i32, i32, f64) {
    let dpr = window.device_pixel_ratio().clamp(1.0, DPR_CAP);
    (canvas.client_width(), canvas.client_height(), dpr)
}

/// 单帧：固定 dt 步进姿态/物理 → 取交换链纹理 → **render_to_view_submit** → present。
/// 致命错误写状态栏并停止调度；C4 裁决的 surface 生命周期契约见模块头注释。
///
/// **与 desktop `render_frame` 的契约对照**（C4 P0-3 同步，决策语义一致）：
/// - `Suboptimal(frame)` → 先 `render_and_present`，**再** `configure`
///   （texture 仍存活，先 configure 会 panic）；desktop 走
///   `FrameOutcome::PresentedThenReconfigure` + `reconfigure_current`，
///   同一合约；
/// - `Outdated` → 无 texture，直接 `configure` 跳过本帧；
/// - `Lost` → `configure` + `set_viewport`；desktop 走 `recreate_surface`
///   （重建 surface + configure + viewport），demo 简化为同 surface 上的
///   configure + viewport（wgpu web 端在 Lost 后只需重新协商 surface 配置）；
/// - `Timeout` / `Occluded` → 静默跳过本帧；
///   `Validation` → 首次提示后跳过本帧、连续第二次命中视为 fatal 并停循环
///   （见 P0-2-2 裁决）。
pub(crate) fn tick(slot: &FrameSlot, state: &SharedState, now_ms: f64) {
    // **P3.1 修正（R4 撤销）**：R4 的 60Hz accumulator 修好了"240Hz 四倍速"，
    // 但引入了"姿态每 4 帧跳一次"的视觉步进（240Hz 上 sim 0,0,0,1 轮转 → 摆动卡顿）。
    //
    // 实际核实：ayagami `PhysicsEngine::update(pose, dt)` **原生支持任意 dt**——
    // 内部按 min_fps 自动拆分 ticks（physics.rs:466-480），pose_stack 的 idle/呼吸
    // 是墙钟正弦（elapsed 累计真实 dt，帧率无关）。因此**直接用真实 dt 更新**：
    // - 每帧都推进姿态 → 渲染连续无跳变；
    // - 物理由引擎内部分步保稳定；
    // - 速度仍与墙钟一致（dt 真实）。
    // dt 钳制 [1/240, 1/15]s：防巨步（切后台回来）与零步。
    let dt_millis = {
        let mut st = state.borrow_mut();
        let real_dt = match st.hud.last_frame_ms {
            Some(prev) if now_ms > prev => {
                ((now_ms - prev) / 1000.0).clamp(1.0 / 240.0, 1.0 / 15.0)
            }
            _ => FIXED_DT_60HZ as f64,
        };
        st.hud.last_frame_ms = Some(now_ms);
        st.hud.last_sim_steps = 1; // 每帧恰好一次 update（HUD 口径保持）
        if let Err(e) = st.core.update(real_dt as f32) {
            drop(st);
            status(&format!("停止渲染循环：update({real_dt:.4}s) 失败：{e}"));
            return;
        }
        // real_dt 毫秒用于口型衰减（dt 归一化）。
        real_dt * 1000.0
    };

    // bridge 状态 → 可见效果（口型/缩放/背景），独立借用窗口。
    apply_bridge_effects(state, dt_millis);

    let mut st = state.borrow_mut();
    let keep_going = match st.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(frame) => {
            st.validation_streak = 0;
            render_and_present(&mut st, frame)
        }
        // C4 裁决：frame 仍存活时不得先 configure；先渲染并 present，
        // 让 frame 在 drop 前完成提交与呈现，再 reconfigure。
        wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
            st.validation_streak = 0;
            let presented = render_and_present(&mut st, frame);
            st.surface.configure(st.gpu.device(), &st.config);
            presented
        }
        // C4 裁决：surface 配置过期，无 texture 可渲，直接应用当前
        // config 后跳过本帧（下一帧按新 config 重新出帧）。
        wgpu::CurrentSurfaceTexture::Outdated => {
            st.validation_streak = 0;
            st.surface.configure(st.gpu.device(), &st.config);
            true
        }
        // C4 裁决：surface Lost 需重配并同步视口（configure 后
        // 渲染核心的 viewport 需对齐 surface 当前尺寸）。
        wgpu::CurrentSurfaceTexture::Lost => {
            st.validation_streak = 0;
            st.surface.configure(st.gpu.device(), &st.config);
            // 先取尺寸副本，避免与 set_viewport 的 mut 借用冲突。
            let (vw, vh) = (st.config.width, st.config.height);
            if let Err(e) = st.core.set_viewport(vw, vh) {
                status(&format!("Lost 后视口同步失败：{e}"));
            }
            true
        }
        // 超时/窗口被遮挡：正常瞬态，静默跳过。
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
            st.validation_streak = 0;
            true
        }
        // P0-2-2 裁决：首次 Validation 记录后跳过本帧并继续循环；连续
        // 第二次命中视为 fatal——写状态栏并返回 `false` 停 rAF 调度，
        // 避免 GPU 校验错误状态下无限重试。任意非 Validation 取帧结果
        // 在上列分支把 `validation_streak` 清零，所以仅"连续"两次才 fatal。
        wgpu::CurrentSurfaceTexture::Validation => {
            st.validation_streak = st.validation_streak.saturating_add(1);
            if st.validation_streak == 1 {
                status("surface 取帧校验错误（已跳过该帧，继续循环）");
                true
            } else {
                status(&format!(
                    "停止渲染循环：连续 {} 次 surface 取帧校验错误，判定为 fatal（GPU 校验错误状态下不再无限重试）。",
                    st.validation_streak
                ));
                false
            }
        }
    };

    drop(st);
    if keep_going {
        // 错峰 500ms 刷新一次 HUD（DOM 写操作），再重新调度下一帧。
        // write_hud_if_due/clone 已完成的 state borrow，新 borrow 互不冲突。
        write_hud_if_due(state);
        crate::web::schedule_frame(slot);
    }
}

/// 渲染一帧并 present。返回 `false` 表示致命错误、应停止循环。
///
/// 实时路径（wasm32 rAF）只调用 `render_to_view_submit`，**不得**调用阻塞的
/// `render_to_view`（C1/C6/C8 硬门禁「实时调用图不得出现 PollType::Wait」）。
/// 返回的 `SubmissionIndex` 在交换链路径下无须等待——浏览器自身的 `present()`
/// 负责把提交与呈现串行化。
pub(crate) fn render_and_present(st: &mut FrameState, frame: wgpu::SurfaceTexture) -> bool {
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    // CPU frame time：只统计提交命令缓冲所用时，排除了取 texture / present
    // 本身（后者由浏览器在合成线程完成）。这里衡量的是 JS↔GPU 命令桥开销，
    // 也就是骨骼 IK + draw 提交的本地开销——最能反馈渲染核心热点的指标。
    let t0 = performance_now();
    match st.core.render_to_view_submit(&view, st.config.format) {
        Ok(submission) => {
            let _ = submission; // 可选：记录 last_submission 供错误状态栏
            frame.present();
            let cpu_ms = performance_now() - t0;
            st.hud.record_frame(cpu_ms);
            true
        }
        Err(e) => {
            // 统计失败不计入 FPS：本帧未真正渲染出像素。
            status(&format!("停止渲染循环：render_to_view_submit 失败：{e}"));
            false
        }
    }
}

/// `performance.now()` 封装（wasm 单线程，不存在时回 0）。
fn performance_now() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

/// 每 500ms 刷新一次运行时 HUD（adapter / canvas / dpr / FPS / cpu ms）。
/// 由 `tick` 在错峰内调用，避免每帧刷 DOM。
pub(crate) fn write_hud_if_due(state: &SharedState) {
    // 先 immutable borrow 读数据；错峰判定与 DOM 写完成后 drop，再 mut borrow 重置窗。
    let (due, line) = {
        let now_hud = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now())
            .unwrap_or(0.0);
        let st = state.borrow();
        if !st.hud.due() {
            return;
        }
        // canvas CSS 尺寸 × DPR cap = 物理尺寸。
        let css_w = st.canvas.client_width().max(0);
        let css_h = st.canvas.client_height().max(0);
        let dpr = st.window.device_pixel_ratio().clamp(1.0, DPR_CAP);
        let (phys_w, phys_h) = (st.config.width, st.config.height);
        let adapter_line = st
            .hud
            .adapter_line
            .clone()
            .unwrap_or_else(|| "<adapter unknown>".to_owned());
        // 本窗 CPU 均值 ms / 帧。
        let cpu_ms = if st.hud.frame_count > 0 {
            st.hud.cpu_time_sum / st.hud.frame_count as f64
        } else {
            0.0
        };
        let stage_diag = format!(
            "stage: scale={:.2} off=({:+.2},{:+.2}) msg={}/{} [{}]",
            st.bridge.scale,
            st.bridge.offset_x,
            st.bridge.offset_y,
            st.bridge.msg_applied,
            st.bridge.msg_recv,
            st.bridge.last_msg
        );
        // RM6 待机生命体征快照（呼吸正弦值 + 眨眼相位回路）由 idle 边界给出。
        let idle_diag = idle::diag_line(&st.idle, now_hud);
        (
            true,
            format!(
                "GPU: {adapter_line} | canvas {css_w}x{css_h}@{dpr}dpr(物理{phys_w}x{phys_h}) | FPS {fps:.1} | cpu {cpu:.1}ms | sim {sim}/frame | {stage_diag} | {idle_diag}",
                adapter_line = adapter_line,
                css_w = css_w,
                css_h = css_h,
                dpr = dpr,
                phys_w = phys_w,
                phys_h = phys_h,
                fps = st.hud.last_fps,
                cpu = cpu_ms,
                sim = st.hud.last_sim_steps,
                idle_diag = idle_diag,
            ),
        )
    };
    if due {
        status(&line);
        // 重置统计窗，准备下一轮 500ms 累计。
        state.borrow_mut().hud.flush();
    }
}
