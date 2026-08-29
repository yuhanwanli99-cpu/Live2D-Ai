//! Surface 维护：模型视口同步 / 重新 configure / Lost 重建。
//!
//! - [`ShellApp::sync_model_viewport_to`]：把模型视口同步到**调用方提供
//!   的**原子尺寸快照（resize / 重配置共用，C4 复审精度改进——configure
//!   与 viewport 使用同一份快照）。
//! - [`ShellApp::reconfigure_current`]：用当前窗口尺寸重新 configure
//!   （Outdated/Suboptimal/Lost 恢复共用）。**必须在旧 `SurfaceTexture`
//!   drop/present 之后调用**——wgpu 在仍有存活 swapchain 纹理时 configure
//!   会 panic。
//! - [`ShellApp::recreate_surface`]：surface Lost 时丢弃旧 surface 并按
//!   当前窗口重建后重配置。
//! - [`ShellApp::apply_pending_resize`]：消费 `pending_surface_size`，
//!   把最新非零尺寸应用为新 config 并同步模型视口（C4 裁决 P0-3 的
//!   latest-size 合并落地处）。
//!
//! 状态机/合并规则也以纯函数 [`merge_pending_resize`] /
//! [`decide_after_acquire`] 暴露给 `tests.rs` 做不依赖真实 wgpu 的
//! 单元断言（resize 多事件合并、Suboptimal 先 present 再 configure、
//! Outdated 跳过本帧）。
//!
//! 同一事件批次的多个 `Resized` 经 [`merge_pending_resize`] 天然合并
//! 到最后非零值：不使用固定毫秒防抖（制造视觉延迟），仅保留最新一
//! 个非零值并在下一次 redraw/acquire 前应用一次（C4 第 77 行裁决）。

use tracing::{error, info, warn};
use winit::dpi::PhysicalSize;

use crate::app::types::ShellApp;

/// `Resized` 事件合并规则（C4 裁决 P0-3，纯函数，tests.rs 断言）。
///
/// - 已有 `pending = Some(prev)` + 新值非零 → 覆盖为新值（last-wins）；
/// - 已有 `pending = Some(prev)` + 新值零 → 保留 `prev`（零尺寸视为最小化
///   折叠占位，**不**覆盖已记录的非零尺寸）；
/// - 已有 `pending = None` + 新值非零 → 写入新值；
/// - 已有 `pending = None` + 新值零 → 保持 `None`（无有效尺寸可记）。
pub(crate) fn merge_pending_resize(
    current: Option<PhysicalSize<u32>>,
    new: PhysicalSize<u32>,
) -> Option<PhysicalSize<u32>> {
    if new.width == 0 || new.height == 0 {
        // 零尺寸 = 最小化/折叠：保持原 pending（若有），等下一帧。
        return current;
    }
    Some(new)
}

/// 取帧后状态机决策（纯函数，与 wgpu 解耦）。
///
/// 把 `wgpu::CurrentSurfaceTexture` 5 个变体映射到 [`SurfaceAction`]，
/// 单测可直接断言顺序与跳过逻辑；实际 wgpu 侧由
/// [`ShellApp::render_frame`] 读取并执行副作用（present/configure/重建）。
///
/// **C4 契约**：
/// - `Suboptimal` 必须先 `present` 再 `configure`（texture 仍在 drop 前
///   不允许 configure，会 panic）；
/// - `Outdated` 没有 texture，直接 `configure` 后跳过本帧；
/// - `Lost` 需要重建 surface + `configure` + 同步 viewport（`Recreate`
///   分支隐含 configure + viewport）；
/// - `Timeout` / `Occluded` / `Validation` 静默跳过本帧，不重配。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceAction {
    /// 正常出帧并 present（不再额外动作）。
    Present,
    /// 先 present 当前 texture，再按当前 config 重新 configure
    /// （C4 顺序：先 present 再 configure）。
    PresentThenReconfigure,
    /// 无 texture：直接按当前 config 重新 configure，跳过本帧。
    SkipAndReconfigure,
    /// surface Lost：重建 surface + configure + 同步 viewport 后跳过本帧。
    SkipAndRecreate,
    /// 单纯跳过本帧（瞬态错误/校验错误）。
    Skip(&'static str),
}

/// 取帧错误分类（与 `wgpu::CurrentSurfaceTexture` 5 个变体一一对应）。
///
/// 是 `CurrentSurfaceTexture` 的"纯化"——剥离 wgpu 句柄，让状态机
/// 不需要持有 GPU 对象也能决策。`Suboptimal` 携带 `present_now` 标记
/// 表示"先 render/present 当前 frame 再 configure"是否成立（C4 契约）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcquireOutcome {
    Success,
    Suboptimal,
    Outdated,
    Lost,
    Timeout,
    Occluded,
    Validation,
}

impl AcquireOutcome {
    /// 由 `wgpu::CurrentSurfaceTexture` 判别式映射（避免在测试里直接持有
    /// 真实 wgpu 类型），只读 `discriminant` 风格的纯函数。
    pub(crate) fn from_current(texture: &wgpu::CurrentSurfaceTexture) -> Self {
        match texture {
            wgpu::CurrentSurfaceTexture::Success(_) => Self::Success,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => Self::Suboptimal,
            wgpu::CurrentSurfaceTexture::Outdated => Self::Outdated,
            wgpu::CurrentSurfaceTexture::Lost => Self::Lost,
            wgpu::CurrentSurfaceTexture::Timeout => Self::Timeout,
            wgpu::CurrentSurfaceTexture::Occluded => Self::Occluded,
            wgpu::CurrentSurfaceTexture::Validation => Self::Validation,
        }
    }
}

/// 状态机主函数：把取帧分类映射为动作决策（C4 契约，纯函数）。
pub(crate) fn decide_after_acquire(outcome: AcquireOutcome) -> SurfaceAction {
    match outcome {
        AcquireOutcome::Success => SurfaceAction::Present,
        // C4 顺序：先 render/present 当前 texture，再 configure。
        AcquireOutcome::Suboptimal => SurfaceAction::PresentThenReconfigure,
        // 无 texture：直接 configure 跳过本帧。
        AcquireOutcome::Outdated => SurfaceAction::SkipAndReconfigure,
        // Lost：重建 + configure + viewport。
        AcquireOutcome::Lost => SurfaceAction::SkipAndRecreate,
        // 瞬态/校验：单纯跳过。
        AcquireOutcome::Timeout => SurfaceAction::Skip("timeout"),
        AcquireOutcome::Occluded => SurfaceAction::Skip("occluded"),
        AcquireOutcome::Validation => SurfaceAction::Skip("validation"),
    }
}

/// `surface_validation_streak` 单帧推进的纯函数（C2 裁决第 59 行）。
///
/// 语义（与 `frame.rs` 既有生产路径 1:1 对齐，**不允许在抽取时改变行为**）：
/// - `is_validation = true`：本帧是 `Validation` 跳过 → 计数 +1。
///   - 计数后等于 1 → [`StreakDecision::SkipFirst`]（首次记录、跳过本帧）；
///   - 计数后 ≥ 2 → [`StreakDecision::FatalCode`]（升级为代码错误 → 退出 1）。
/// - `is_validation = false`：本帧是其他瞬态（timeout/occluded）或已成功
///   出帧 → 计数清零 → [`StreakDecision::NotValidation`]。
///
/// `pub(crate)`：调用方 `frame.rs` 的 `render_frame` 在拿到
/// `SurfaceAction::Skip("validation")` 时调用；测试在 `tests.rs` 模拟状态机
/// 序列断言每次返回值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreakDecision {
    /// 首次记录 Validation（计数 = 1），跳过本帧、仅打 warn。
    SkipFirst,
    /// 连续 ≥ 2 次 Validation，升级为代码错误，调用方应写
    /// `model_failure` + `event_loop.exit()`。
    FatalCode,
    /// 非 Validation 取帧（timeout/occluded/已成功出帧）→ 计数清零，
    /// 跳过或出帧由调用方按 `SurfaceAction` 处理。
    NotValidation,
}

/// 推进 `surface_validation_streak` 并返回本帧应采取的决策（C2 第 59 行）。
///
/// 纯函数：只动 `*streak`；无日志/退出副作用（由调用方在 frame.rs 配合
/// `SurfaceAction` 落地）。`u8::saturating_add(1)` 与生产路径保持一致：
/// 阈值是 2 帧，saturate 永远不会自然触发，**仅作风格一致性**。
pub(crate) fn advance_validation_streak(streak: &mut u8, is_validation: bool) -> StreakDecision {
    if is_validation {
        *streak = streak.saturating_add(1);
        if *streak == 1 {
            StreakDecision::SkipFirst
        } else {
            // streak >= 2（u8 saturating_add 兜底：实际阈值 = 2）。
            StreakDecision::FatalCode
        }
    } else {
        // 其他瞬态/成功出帧：清零计数，恢复"单次 Validation 不致命"语义。
        *streak = 0;
        StreakDecision::NotValidation
    }
}

impl ShellApp {
    /// 把模型视口同步到指定物理尺寸 `width, height`（resize / 重配置共用）。
    ///
    /// 供 configure 路径传入与 configure 同一份尺寸快照，避免
    /// "configure 用 size A、视口用 size B" 的 race（C4 复审精度改进）。
    /// 尺寸 0 自动夹到 1，与 `apply_pending_resize` / `reconfigure_current`
    /// 的 `.max(1)` 行为保持一致。
    pub(crate) fn sync_model_viewport_to(&mut self, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if let Some(model) = &mut self.model
            && let Err(e) = model.core.set_viewport(w, h)
        {
            warn!("同步模型视口失败: {e}");
        }
    }

    /// 用窗口当前物理尺寸重新 configure（Outdated/Suboptimal/Lost 恢复共用）。
    ///
    /// 注意：必须在旧 `SurfaceTexture` drop/present **之后**调用——
    /// wgpu 在仍有存活 swapchain 纹理时 configure 会 panic。
    pub(crate) fn reconfigure_current(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };
        let Some(window) = &self.window else {
            return;
        };
        let PhysicalSize { width, height } = window.inner_size();
        state.config.width = width.max(1);
        state.config.height = height.max(1);
        state.surface.configure(&state.device, &state.config);
        // 模型视口与 surface 尺寸保持同步：使用 configure 同帧的尺寸快照，
        // 避免 configure 之后再读 `inner_size()` 期间被 Resized 改写。
        // 先取出尺寸再调用，避开 `state` 与 `self` 的借用冲突。
        let (w, h) = (state.config.width, state.config.height);
        self.sync_model_viewport_to(w, h);
    }

    /// 消费 `pending_surface_size`：若非空且非零，把 config 更新到
    /// pending 值、configure 一次并同步模型视口，然后清空 pending。
    ///
    /// 在每帧 `render_frame` 开始前调用（C4 裁决 P0-3）：让 `Resized`
    /// 事件只写 pending + `request_redraw()`，多事件在同一批次合并
    /// 到最后非零值；`request_redraw` 触发的下一次 redraw 之前由本函数
    /// 统一落地 configure。
    pub(crate) fn apply_pending_resize(&mut self) {
        let Some(pending) = self.pending_surface_size.take() else {
            return;
        };
        let Some(state) = &mut self.state else {
            return;
        };
        let width = pending.width.max(1);
        let height = pending.height.max(1);
        if state.config.width == width && state.config.height == height {
            // 已生效（多帧 race：上一帧已应用过），不再多余 configure。
            return;
        }
        state.config.width = width;
        state.config.height = height;
        state.surface.configure(&state.device, &state.config);
        // 模型视口与 surface 尺寸保持同步：使用同一份 pending 快照，
        // 避免 configure 之后再读 `inner_size()` 期间被 Resized 改写。
        self.sync_model_viewport_to(width, height);
    }

    /// surface Lost：丢弃旧 surface 并按当前窗口重建后重配置。
    pub(crate) fn recreate_surface(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        match self.instance.create_surface(window.clone()) {
            Ok(surface) => {
                if let Some(state) = &mut self.state {
                    state.surface = surface;
                }
                self.reconfigure_current();
                info!("surface 因 Lost 重建完成");
            }
            Err(e) => error!("surface 重建失败（保留状态等待下帧重试）: {e}"),
        }
    }
}
