//! benchmark 共用 workload helper（P1-2-P0-1 修复）。
//!
//! 两条 runner（headless / surface）每帧除「取帧 + 提交/present」之外的
//! 部分（推进表演曲线、稀疏写入 input 层、口型 override、姿态栈推进）
//! **全部走这里**——保证两路径与生产 [`crate::app::frame::draw_and_present_model`]
//! 顺序一致（`driver.tick` → `adapter.apply_frame` →
//! `adapter.apply_mouth_level` → `core.update` → `sim_time += SIM_DT`）。
//!
//! 真实签名见 [`step_frame_workload`]。返回值映射到冒烟统计与
//! benchmark 自身的 `ActionCounts` / `SkipBuckets` 由两条 runner
//! 在调用方按本帧成功/失败路径自行落地（workload helper 只负责
//! 「按生产顺序跑一次模拟步」）。

use l2d::renderer::{ModelRendererCore, RenderError};
use live2d_ai_core::ParameterFrame;

use crate::adapter::BaiParamAdapter;
use crate::model_smoke::{SIM_DT, SmokeDriver, synthesized_mouth_level};

/// 推进一个 60 Hz 模拟步（生产路径同口径）。
///
/// 顺序（与 [`crate::app::frame::draw_and_present_model`] 1:1 对齐）：
/// 1. `driver.tick(&mut frame_buf)`：六动作固定循环，输出当前表演帧；
/// 2. `adapter.apply_frame(core, &frame_buf)`：稀疏写入 input 层
///    （P0-5 所有权；未持有通道绝不定值）；
/// 3. `mouth = synthesized_mouth_level(*sim_time)`：低频合成电平；
/// 4. `adapter.apply_mouth_level(core, mouth)`：final_override 最高优先级写入；
/// 5. `core.update(SIM_DT)`：重算 idle + 物理步进 + 合成最终姿态；
/// 6. `*sim_time += SIM_DT`：维持节拍。
///
/// 错误来源仅 `core.update`（`RenderError`：非法 dt / 栈异常），
/// 其余调用都不会失败。冒烟/benchmark 关心渲染耗时，**不**关心
/// `apply_frame` / `apply_mouth_level` 的写入计数（缺失降级由
/// `BaiParamAdapter::missing_params` 观测）。
pub(crate) fn step_frame_workload(
    driver: &mut SmokeDriver,
    adapter: &mut BaiParamAdapter,
    core: &mut ModelRendererCore,
    frame_buf: &mut ParameterFrame,
    sim_time: &mut f32,
) -> Result<(), RenderError> {
    driver.tick(frame_buf);
    adapter.apply_frame(core, frame_buf);
    let mouth = synthesized_mouth_level(*sim_time);
    adapter.apply_mouth_level(core, mouth);
    core.update(SIM_DT)?;
    *sim_time += SIM_DT;
    Ok(())
}
