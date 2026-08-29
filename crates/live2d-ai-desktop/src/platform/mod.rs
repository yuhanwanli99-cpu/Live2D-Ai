//! Linux X11/Wayland 会话线索与能力簿记（纯逻辑、零第三方依赖）。
//!
//! 本模块刻意不读取进程环境、不调用系统库：
//! 所有判定函数都接收显式输入，保证可单测、可在无显示 CI 中运行。
//!
//! 四层语义（重要，勿混淆）：
//! 1. [`capabilities::LinuxSessionHint`] —— 从环境变量**猜测**的会话类型，只是线索（hint），
//!    不代表真实窗口系统连接已建立；
//! 2. [`capabilities::DeclaredCapabilities`] —— 由 hint 推导的**声明性期望表**，明确不是实际能力 gate；
//!    真实能力以 [`capabilities::RuntimeCapabilities`] 为准；
//! 3. [`window::WindowBackendKind`] —— 由窗口的 **RawWindowHandle 实测分类**得到的真实后端
//!    （Xlib/Xcb → X11；Wayland → Wayland），与 hint 无关；
//! 4. [`capabilities::RuntimeCapabilities`] —— 记录**实际**窗口/surface/桌宠 API 初始化结果的簿记结构。
//!    tray / click_through / global_position / always_on_top / drag_move 只能在
//!    对应 API **真实调用成功（或按实测后端判定协议性不支持）**后改变状态，
//!    **禁止仅凭"当前是 X11 会话"就置为 Available**。
//!
//! 公开 API 路径全部经本模块 `pub use` 透传，调用方继续按 `crate::platform::*`
//! 引用——零外部破坏（backend.rs、app/*、benchmark 等引用点零改动）。

mod capabilities;
mod window;

// —— 能力数据：会话线索 / 三态能力状态 / 声明性 + 实际能力表 / alpha 选择 ——
pub use capabilities::{
    CapabilityState, DeclaredCapabilities, LinuxSessionHint, ObservedAlphaMode,
    RuntimeCapabilities, choose_alpha_mode,
};

// —— 窗口后端：实测后端分类 + 后端能力口 + 窗口定位算法 ——
pub use window::{WindowBackendKind, bottom_right_target};
