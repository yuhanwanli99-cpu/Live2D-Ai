//! WASM 端 surface：渲染面按行为边界拆分后的模块根（C4 裁决落地处）。
//!
//! 与 `main.rs` 同样只在 `target_arch = "wasm32"` 下编译（由父模块的
//! `#[cfg(target_arch = "wasm32")]` 门控）。rc.3 N1（计划 §4.2.2）把原 1167 行单文件
//! 按行为边界拆开，本文件只做声明与再导出：
//! - [`gpu`]：WebGPU 优先、WebGL2 回退的设备/格式/尺寸协商（C3/C4）。
//! - [`render`]：rAF 单帧状态机与 surface 生命周期契约（C4）、resize 自愈、运行时 HUD。
//! - [`input`]：父页 bridge 消息与指针/滚轮写入的舞台输入状态，以及每帧的可见效果。
//! - [`idle`]：**待机生命体征**（呼吸/眨眼/微表情）——与动作系统是两套机制，必须保留。
//!
//! 实时路径只调用 **`render_to_view_submit`**（不得调用阻塞的 `render_to_view`
//! ——C1/C6/C8 硬门禁）；单帧契约见 [`render::tick`]。

mod gpu;
mod idle;
mod input;
mod render;

pub(crate) use gpu::{adapter_info, init_gpu};
pub(crate) use idle::IdleState;
pub(crate) use input::{BridgeState, normalize_stage_color};
pub(crate) use render::{
    FrameSlot, FrameState, HudState, SharedState, canvas_css_metrics, install_resize_handler, tick,
};
