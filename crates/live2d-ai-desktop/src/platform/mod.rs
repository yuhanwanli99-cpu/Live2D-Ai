//! Linux X11/Wayland 会话线索与能力簿记（纯逻辑、零第三方依赖）。
//!
//! 本模块刻意不读取进程环境、不调用系统库：
//! 所有判定函数都接收显式输入，保证可单测、可在无显示 CI 中运行。
//!
//! 两层语义（重要，勿混淆）：
//! 1. [`capabilities::LinuxSessionHint`] —— 从环境变量**猜测**的会话类型，只是线索（hint），
//!    不代表真实窗口系统连接已建立；
//! 2. [`capabilities::DeclaredCapabilities`] —— 由 hint 推导的**声明性期望表**，明确不是实际能力 gate；
//!    真实能力以 [`capabilities::RuntimeCapabilities`] 为准（W2-B 后只剩 `not_probed`）。
//!
//! 2026-10-01（W2-B / D1 第二段）：原生壳移出构建 ⇒ `window.rs`（RawWindowHandle 实测
//! 分类 + 窗口定位）与 `ObservedAlphaMode` / `choose_alpha_mode` 一并删除；
//! 本模块只剩默认模式 Info 输出仍要用的三个类型。恢复条件见
//! `docs/architecture/ARCHIVED-native-shell.md`（tag `checkpoint/pre-d1-dormant`）。

mod capabilities;

// —— 能力数据：会话线索 / 三态能力状态 / 声明性 + 实际能力表 ——
pub use capabilities::{DeclaredCapabilities, LinuxSessionHint, RuntimeCapabilities};
