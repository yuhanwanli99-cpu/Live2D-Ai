//! 参数语义值 ↔ 模型值换算：**纯函数、无 web 依赖**，原生 `cargo test` 可回归。
//!
//! # 为什么需要这一层（2026-09-10 修复的真实缺陷）
//!
//! 动作编舞表（`web::surface::CHOREOGRAPHY`，照抄 py 版 `choreography.ts`）里的数值是
//! **语义值**：`-1..1`，`0` = 该参数的 profile 默认位。它们**必须**按各参数的量程
//! 换算后才能写进模型。
//!
//! 历史上这层换算在 Rust 移植时丢了——关键帧被**原样**写入。而 Bai 的
//! `ParamAngleX/Y/Z` 量程是 **±30**（见下发证据），于是 `nod` 的 `-0.55` 实际只让
//! 头转了 **0.55 度**（约 ±30 度目标幅度的 1.8%）——肉眼完全看不出动作。
//!
//! # 量程来源（可核对）
//!
//! `assets/models/bai/runtime/bai.vtube.json` → `ParameterSettings`：
//!
//! | Live2D 参数 | OutputRangeLower / Upper |
//! | --- | --- |
//! | `ParamAngleX` / `ParamAngleY` / `ParamAngleZ` | `-30.0` / `30.0` |
//! | `ParamBodyAngleX` / `ParamBodyAngleY` / `ParamBodyAngleZ` | `-10.0` / `10.0` |
//! | `ParamEyeBallX` / `ParamEyeBallY` | `-1.0` / `1.0`（见 cdi3 参数表） |
//!
//! 原生侧同口径交叉验证：`live2d-ai-core` 的 `HEAD_ANGLE_LIMIT = 30.0`，且
//! `PerformanceFrame::head_angle_y` 以**度**为单位直接写 `ParamAngleY`
//! （`live2d-ai-desktop/src/adapter/mask.rs`）——即原生路径本来就是模型域，
//! 只有 wasm 侧的关键帧是语义域，需要本模块换算。
//!
//! 注意：本表是**模型相关**的。换皮套若量程不同，需要重新标定；这里给出的是
//! Bai 的实测值，并且 `semantic_to_param` 对未知参数按 1.0 处理（保守不放大）。

/// `ParamAngleX/Y/Z`（头部三轴旋转）的量程：±30 度。
pub const HEAD_ANGLE_SCALE: f32 = 30.0;

/// `ParamBodyAngleX/Y/Z`（身体三轴旋转）的量程：±10 度。
pub const BODY_ANGLE_SCALE: f32 = 10.0;

/// 某参数的量程（语义 `1.0` 对应的模型值）。
///
/// 未列出的参数返回 `1.0`：它们的量程本身就是 ±1 或 0..1
/// （`ParamEyeBallX/Y`、`ParamEyeLOpen/ROpen`、`ParamBrowLY/RY`、`ParamMouthOpenY` 等），
/// 语义值即模型值。**保守**取 1.0 而不是猜一个放大倍数——猜错会让动作穿模。
pub const fn param_scale(id: &str) -> f32 {
    match id.as_bytes() {
        b"ParamAngleX" | b"ParamAngleY" | b"ParamAngleZ" => HEAD_ANGLE_SCALE,
        b"ParamBodyAngleX" | b"ParamBodyAngleY" | b"ParamBodyAngleZ" => BODY_ANGLE_SCALE,
        _ => 1.0,
    }
}

/// 语义值（`-1..1`，`0` = 默认位）→ 模型参数值。
///
/// 非有限输入返回 `0.0`（= 默认位），不 panic、不放大。
pub fn semantic_to_param(id: &str, semantic: f32) -> f32 {
    if !semantic.is_finite() {
        return 0.0;
    }
    semantic * param_scale(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **回归（2026-09-10）**：关键帧的头部角度必须换算到模型量程。
    ///
    /// 历史缺陷：`nod` 的首帧 `-0.55` 被原样写入量程为 ±30 的 `ParamAngleY`
    /// → 实际只转 0.55 度，动作肉眼不可见。
    #[test]
    fn head_angle_keyframes_reach_a_visible_amplitude() {
        // nod 首帧的语义幅度（见 CHOREOGRAPHY）。
        let nod_first = semantic_to_param("ParamAngleY", -0.55);
        assert!(
            (nod_first + 16.5).abs() < 1e-4,
            "-0.55 语义应换算为 -16.5 度，got {nod_first}"
        );
        // 可见性判据：真实幅度必须达到量程的 1/4 以上（约 7.5 度）。
        assert!(
            nod_first.abs() > HEAD_ANGLE_SCALE / 4.0,
            "头部动作幅度过小（{}度），肉眼不可见",
            nod_first.abs()
        );
        // 关键对照：旧行为（原样写入）只有 0.55 度。
        assert!(nod_first.abs() > 0.55 * 10.0, "必须显著大于未换算的旧行为");
    }

    #[test]
    fn body_angles_use_their_own_smaller_scale() {
        assert_eq!(semantic_to_param("ParamBodyAngleX", 0.5), 5.0);
        assert_eq!(semantic_to_param("ParamBodyAngleZ", -0.5), -5.0);
        // 身体量程必须小于头部（Bai 实测 ±10 vs ±30），否则会扭断。
        let _ = BODY_ANGLE_SCALE;
        const { assert!(BODY_ANGLE_SCALE < HEAD_ANGLE_SCALE) };
    }

    /// 量程本就是 ±1 / 0..1 的参数**不得**被放大——否则表情会穿模。
    #[test]
    fn unit_range_params_are_passed_through_unchanged() {
        for id in [
            "ParamEyeBallX",
            "ParamEyeBallY",
            "ParamEyeLOpen",
            "ParamEyeROpen",
            "ParamBrowLY",
            "ParamBrowRY",
            "ParamMouthOpenY",
            "ParamMouthForm",
            "ParamBreath",
        ] {
            assert_eq!(param_scale(id), 1.0, "{id} 不应被缩放");
            assert_eq!(semantic_to_param(id, 0.9), 0.9, "{id} 应原样透传");
        }
    }

    #[test]
    fn unknown_params_default_to_identity() {
        assert_eq!(param_scale("ParamNotAThing"), 1.0);
        assert_eq!(semantic_to_param("ParamNotAThing", 0.25), 0.25);
    }

    #[test]
    fn non_finite_semantic_values_collapse_to_default() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(semantic_to_param("ParamAngleY", bad), 0.0, "{bad}");
        }
    }

    #[test]
    fn conversion_is_linear_and_sign_preserving() {
        assert_eq!(semantic_to_param("ParamAngleX", 0.0), 0.0);
        assert_eq!(semantic_to_param("ParamAngleX", 1.0), HEAD_ANGLE_SCALE);
        assert_eq!(semantic_to_param("ParamAngleX", -1.0), -HEAD_ANGLE_SCALE);
        // 单调：语义越大，模型值越大。
        let mut prev = f32::NEG_INFINITY;
        for step in -10..=10 {
            let v = semantic_to_param("ParamAngleZ", step as f32 / 10.0);
            assert!(v > prev, "必须严格单调");
            prev = v;
        }
    }
}
