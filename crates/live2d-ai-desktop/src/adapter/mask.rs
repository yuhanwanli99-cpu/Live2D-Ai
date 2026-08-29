//! Bai 标准参数 ID 常量 + `core` → Bai 的固定映射表。
//!
//! 行为契约：仅做「按所有权位挑选条目 + 把字段值翻译到 Bai 参数」，
//! 写入端（[`super::ParamSink`]）与所有权位（[`live2d_ai_core::ParameterMask`]）
//! 解耦不动；嘴开合（`ParamMouthOpenY`）不在此列——口型通道由
//! [`super::BaiParamAdapter::apply_mouth_level`] 经 final_override 写。

use live2d_ai_core::{ParameterFrame, performance};

// ---------------------------------------------------------------- 参数 ID

/// 头部水平转（度 ±30）。
pub const PARAM_ANGLE_X: &str = "ParamAngleX";
/// 头部俯仰（度 ±30）。
pub const PARAM_ANGLE_Y: &str = "ParamAngleY";
/// 头部侧倾（度 ±30）。
pub const PARAM_ANGLE_Z: &str = "ParamAngleZ";
/// 眼球水平注视（归一化 ±1）。
pub const PARAM_EYE_BALL_X: &str = "ParamEyeBallX";
/// 眼球垂直注视（归一化 ±1）。
pub const PARAM_EYE_BALL_Y: &str = "ParamEyeBallY";
/// 左眼开合（默认 1）。
pub const PARAM_EYE_L_OPEN: &str = "ParamEyeLOpen";
/// 右眼开合（默认 1）。
pub const PARAM_EYE_R_OPEN: &str = "ParamEyeROpen";
/// 左眉上下（归一化 ±1）。
pub const PARAM_BROW_L_Y: &str = "ParamBrowLY";
/// 右眉上下（归一化 ±1）。
pub const PARAM_BROW_R_Y: &str = "ParamBrowRY";
/// 嘴形（归一化 ±1）。
pub const PARAM_MOUTH_FORM: &str = "ParamMouthForm";
/// 嘴开合（归一化 [0,1]）——口型通道专属，final_override 最高优先级写入。
pub const PARAM_MOUTH_OPEN_Y: &str = "ParamMouthOpenY";
/// 身体水平转（度 ±10）。
pub const PARAM_BODY_ANGLE_X: &str = "ParamBodyAngleX";
/// 身体俯仰（度 ±10）。
pub const PARAM_BODY_ANGLE_Y: &str = "ParamBodyAngleY";
/// 身体侧倾（度 ±10）。
pub const PARAM_BODY_ANGLE_Z: &str = "ParamBodyAngleZ";

/// Bai 皮套「睁眼默认值」：eye_open_scale 以此为基线做乘法映射。
///
/// v0 只锚定 Bai（RFC D5），其 `ParamEyeLOpen`/`ParamEyeROpen` 默认值为 1；
/// 若未来接入默认值非 1 的皮套，应改为从模型显示信息读取真实默认值。
pub const BAI_EYE_OPEN_DEFAULT: f32 = 1.0;

// ---------------------------------------------------------------- 映射

/// 带所有权位的完整通道表：`(该通道对应字段的所有权位, 参数 ID, 值)`。
///
/// 固定 13 项：11 个字段中 `brow_y` 双写左右眉、`eye_open_scale` 双写左右眼；
/// 顺序稳定（头→眼→眉→嘴形→身体），便于测试与日志对照。
/// 是否允许写某一条由 [`ParameterFrame::owned`] 所有权位图决定
/// （见 [`super::BaiParamAdapter::apply_frame`] 的稀疏语义）。
pub fn frame_entries(
    frame: &ParameterFrame,
) -> [(live2d_ai_core::ParameterMask, &'static str, f32); 13] {
    use live2d_ai_core::ParameterMask as M;
    // 曲线层已限幅（clamp_to_safe）；这里再防御一次有限性，NaN 绝不入姿态栈。
    let f = |v: f32| if v.is_finite() { v } else { 0.0 };
    let eye_open = {
        let scale = frame.eye_open_scale;
        let scale = if scale.is_finite() { scale } else { 1.0 };
        BAI_EYE_OPEN_DEFAULT * scale.clamp(performance::EYE_OPEN_MIN, performance::EYE_OPEN_MAX)
    };
    [
        (M::HEAD_ANGLE_X, PARAM_ANGLE_X, f(frame.head_angle_x)),
        (M::HEAD_ANGLE_Y, PARAM_ANGLE_Y, f(frame.head_angle_y)),
        (M::HEAD_ANGLE_Z, PARAM_ANGLE_Z, f(frame.head_angle_z)),
        (M::EYE_X, PARAM_EYE_BALL_X, f(frame.eye_x)),
        (M::EYE_Y, PARAM_EYE_BALL_Y, f(frame.eye_y)),
        (M::EYE_OPEN_SCALE, PARAM_EYE_L_OPEN, eye_open),
        (M::EYE_OPEN_SCALE, PARAM_EYE_R_OPEN, eye_open),
        (M::BROW_Y, PARAM_BROW_L_Y, f(frame.brow_y)),
        (M::BROW_Y, PARAM_BROW_R_Y, f(frame.brow_y)),
        (M::MOUTH_FORM, PARAM_MOUTH_FORM, f(frame.mouth_form)),
        (M::BODY_ANGLE_X, PARAM_BODY_ANGLE_X, f(frame.body_angle_x)),
        (M::BODY_ANGLE_Y, PARAM_BODY_ANGLE_Y, f(frame.body_angle_y)),
        (M::BODY_ANGLE_Z, PARAM_BODY_ANGLE_Z, f(frame.body_angle_z)),
    ]
}
