//! 真实 Linux 原生窗口/GPU：winit 0.30 `ApplicationHandler`（UserEvent）+ wgpu 29 surface。
//!
//! 本文件是 [`app`] 模块的薄入口：保留 `crate::app::ShellApp` /
//! `crate::app::run_shell` / `crate::app::ChatBridge` / `crate::app::panic_message`
//! 的对外路径不变。实现按职责拆分到子模块：
//!
//! - [`types`]：核心数据结构（`ShellApp` / `ModelState` / `SurfaceState` /
//!   `SHUTDOWN_WATCHDOG`）。
//! - [`capability`]：桌宠窗口能力（后端探测 / 初始定位回读 / 置顶 / 穿透 /
//!   显隐 / 拖动） + `RawWindowHandle`/`RawDisplayHandle` 分类与
//!   `wgpu` 纹理格式/alpha 协商纯函数。
//! - [`bootstrap`]：GPU/窗口 bootstrap（窗口 + wgpu surface/adapter/device/queue +
//!   surface 配置 + 桌宠能力探测 + 模型加载）。
//! - [`surface`]：模型视口同步 / 重新 configure / Lost 重建。
//! - [`frame`]：每帧取帧与渲染（`draw_and_present_clear` /
//!   `draw_and_present_model` / `render_frame` / `frame_target_reached`）。
//! - [`interaction`]：托盘/用户事件落地（`handle_tray_event`）、
//!   对话 UI 事件落地（`apply_conversation_ui`）。
//!   （原 `apply_render_command` 渲染命令落地已于 2026-09-11 随动作系统删除。）
//! - [`handler`]：`ApplicationHandler<AppEvent>` impl（`resumed` / `user_event` /
//!   `window_event` / `about_to_wait` / `new_events` / `exiting`）。
//! - [`shutdown`]：关机 handshake（`request_shutdown`） + 兜底 watchdog。
//! - [`tests`]：纯函数与分类的单元测试。
//!
//! 生命周期（[`ShellApp`]）：
//! - [`ApplicationHandler::resumed`]：创建**透明、无边框、可调整大小**窗口 →
//!   建 wgpu surface/adapter/device/queue → 按 surface 实际能力协商
//!   format / alpha mode / present mode 并完成首次 configure；
//!   协商结果如实记入 [`RuntimeCapabilities`]（不凭会话线索推断）；
//! - **桌宠窗口能力**：RawWindowHandle 实测分类后端
//!   （Xlib/Xcb→X11，Wayland→Wayland）；底部右侧初始定位 +
//!   回读验证；`set_window_level` 置顶（仅 X11 生效）；
//!   左键按下交互态 `drag_window` 拖动；`set_cursor_hittest` 动态切换点击穿透；
//!   托盘菜单事件经 [`PetUserEvent`] 在事件循环线程落地——托盘线程绝不触窗；
//! - **Live2D 实时渲染**（`RunOptions::model_smoke` 为 `Some` 时）：用**同一**
//!   device/queue 构造 [`GpuContext::from_parts_with_label`]（绝不创建第二个
//!   GPU）→ [`ModelPackage::load`] / [`LoadedModel::resolve`]（兼容报告逐条记录，
//!   否决即类型化失败）→ [`ModelRendererCore::new`/`load_model`]；每帧按真实
//!   dt（截断到合理范围，60 Hz 固定步累加器）推进模拟，resize 同步
//!   `set_viewport`，**调 `render_to_view_submit` 只提交不等待**（C1 闭环）
//!   渲染到 **surface view** 后 present，帧耗时即纯 CPU 提交成本；
//! - 纯透明壳模式（无模型配置）：连续重绘透明清屏（原 `--window-smoke` 行为）；
//! - 每帧 `device.poll(PollType::Poll)` 泵送设备队列，**失败升级为 fatal**
//!   （纯透明壳 → 环境错误退出 3；模型路径 → GPU 环境错误退出 3，C2 闭环）；
//! - 异步 GPU 错误经 `Device::on_uncaptured_error` 写入 `SurfaceState`
//!   `gpu_fault_latch`，由 `render_frame` 每帧开始消费并按 C2 表分类
//!   （Validation → 退出 1；OOM/Internal → 退出 3）。
//! - resize / ScaleFactorChanged / 取帧结果（Success/Suboptimal/Timeout/Occluded/
//!   Outdated/Lost/Validation）逐类处理，Lost 时重建 surface；
//! - 冒烟停止条件：帧数目标 / 超时兜底 / 窗口关闭 / 托盘退出。
//!
//! 本模块是环境边界：允许读取进程环境变量做观测日志；
//! 纯判定逻辑一律下沉到 [`crate::platform`] / [`crate::model_smoke`] /
//! [`crate::user_event`]（保持零依赖可单测）。

mod bootstrap;
mod capability;
mod frame;
mod handler;
mod interaction;
mod settings_ui;
#[cfg(test)]
mod settings_ui_tests;
mod shutdown;
mod surface;
mod types;

#[cfg(test)]
mod tests;

// 公开 API 透传：保持 `crate::app::ShellApp` / `run_shell` / `ChatBridge` /
// `panic_message` 路径稳定，外部引用点（backend.rs / main.rs）零改动。
pub(crate) use types::ChatBridge;
pub(crate) use types::ShellApp;
pub(crate) use types::panic_message;
pub use types::run_shell;

// P1-2-P0-2 修复：把 surface 状态机三件套（`AcquireOutcome` / `SurfaceAction` /
// `decide_after_acquire`）以 `pub(crate)` 透传给 `crate::benchmark::runners_surface`，
// 让 benchmark 复用生产状态机（不再自造简化版）。surface.rs 内部类型本身仍
// 是 `pub(crate)`，本 re-export 只是把可见性抬到 crate 根。
pub(crate) use surface::{AcquireOutcome, SurfaceAction, decide_after_acquire};
