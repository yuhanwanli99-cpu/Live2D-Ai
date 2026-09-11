//! model smoke（Bai 实时渲染冒烟）的纯逻辑层：类型化错误 / 动作序列 /
//! 低频合成口型电平 / 运行统计。本模块不触碰 GPU、窗口与事件循环，
//! 全部可无显示单测；接线在 [`crate::app`]。
//!
//! 冒烟语义（RFC §4 批次 5 推进，D10）：
//! - 自动按固定顺序循环播放六动作（每动作 Medium 强度；上一动作结束
//!   才开始下一动作），同时以低频合成口型电平验证 `ParamMouthOpenY`
//!   通道——**不访问 LLM/TTS/声卡**；
//! - 失败类型化：模型代码错误/资产问题 → 退出码 1；GPU 环境不满足 → 退出码 3。
//!
//! 子模块划分：
//! - [`driver`]：动作序列 + [`SmokeDriver`] + [`FrameClock`]（纯逻辑时钟）；
//! - [`report`]：运行统计 [`ModelSmokeStats`]（并入 RunReport）；
//! - 本模块：类型化错误 [`ModelSmokeError`]、三阶段 [`RenderError`] 归类
//!   函数、口型合成 [`synthesized_mouth_level`] 与相关常量。

mod driver;
mod report;

pub use driver::{ActionSequence, FrameClock, SmokeDriver, TickOutcome};
pub use report::ModelSmokeStats;

use std::fmt;

use l2d::renderer::RenderError;
use live2d_ai_core::{ActionId, ActionSource, SemanticAction, Strength};

#[allow(unused_imports)]
pub use driver::{MAX_ACCUMULATOR_STEPS, MAX_FRAME_DT_SECS, MAX_TICKS_PER_FRAME, SIM_DT};

// ---------------------------------------------------------------- 口型合成

/// 合成口型的「说话/停顿」周期（秒）：低频门控，模拟一段一段的话语节奏。
pub const MOUTH_CYCLE_SECS: f32 = 3.0;
/// 周期内处于「说话」状态的时间占比（前段说话、后段静默）。
pub const MOUTH_DUTY: f32 = 0.6;
/// 音节级开合频率（Hz）：说话段内嘴一张一合的节奏。
pub const SYLLABLE_HZ: f32 = 2.5;

/// 低频合成口型电平 ∈ [0,1]（纯函数、确定性与模拟时钟绑定）。
///
/// 波形 = 「说话门控」（周期 [`MOUTH_CYCLE_SECS`]、占空比 [`MOUTH_DUTY`]）
/// × 音节正弦平方（频率 [`SYLLABLE_HZ`]）。用于验证 `ParamMouthOpenY`
/// 参数通道端到端可达，**不访问 LLM/TTS/声卡**。
///
/// 非法输入（非有限/负时间）一律输出 0（闭嘴）。
pub fn synthesized_mouth_level(sim_time: f32) -> f32 {
    if !sim_time.is_finite() || sim_time < 0.0 || MOUTH_CYCLE_SECS <= 0.0 {
        return 0.0;
    }
    let phase = sim_time % MOUTH_CYCLE_SECS;
    if phase >= MOUTH_CYCLE_SECS * MOUTH_DUTY {
        return 0.0; // 静默段：明确闭嘴。
    }
    let syllable = (std::f32::consts::TAU * SYLLABLE_HZ * phase).sin() * 0.5 + 0.5;
    let level = syllable * syllable; // 平滑、峰值 1、谷值 0。
    if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

// ---------------------------------------------------------------- 错误分类

/// model smoke 的类型化失败。类别决定退出码：
/// [`ModelSmokeError::Asset`] / [`ModelSmokeError::ModelCode`] → 1（代码/资产问题），
/// [`ModelSmokeError::GpuEnvironment`] → 3（运行环境不满足）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModelSmokeError {
    /// 资产问题：model3.json/moc3/纹理/physics 文件缺失或清单非法、
    /// 兼容报告否决 v0 播放。退出码 1。
    Asset(String),
    /// 模型代码错误：moc3 解析失败、渲染核心加载模型失败（模型内容无法构建）。
    /// 退出码 1。
    ModelCode(String),
    /// GPU 环境不满足：adapter/device/pipeline 创建失败、渲染中 poll 失败等。
    /// 与代码缺陷区分，退出码 3。
    GpuEnvironment(String),
}

impl ModelSmokeError {
    /// 类别名（日志/报告用）。
    ///
    /// 退出码映射（在 [`crate::app`] 落地）：Asset / ModelCode →
    /// `BackendError::Failed` → 进程退出码 **1**；GpuEnvironment →
    /// `BackendError::Environment` → 进程退出码 **3**。
    pub fn category(&self) -> &'static str {
        match self {
            Self::Asset(_) => "asset",
            Self::ModelCode(_) => "model-code",
            Self::GpuEnvironment(_) => "gpu-environment",
        }
    }
}

impl fmt::Display for ModelSmokeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asset(msg) => write!(f, "皮套资产问题: {msg}"),
            Self::ModelCode(msg) => write!(f, "模型代码错误: {msg}"),
            Self::GpuEnvironment(msg) => write!(f, "GPU 环境不满足: {msg}"),
        }
    }
}

impl std::error::Error for ModelSmokeError {}

impl From<l2d::asset::LoadError> for ModelSmokeError {
    fn from(e: l2d::asset::LoadError) -> Self {
        Self::Asset(e.to_string())
    }
}

impl From<l2d::model::ModelError> for ModelSmokeError {
    fn from(e: l2d::model::ModelError) -> Self {
        Self::ModelCode(e.to_string())
    }
}

/// 渲染核心创建失败（`ModelRendererCore::new`：在既有 device 上建管线）。
///
/// 管线创建失败通常是设备/驱动能力不足 → GPU 环境（退出码 3）。
pub fn classify_core_init_error(e: RenderError) -> ModelSmokeError {
    match e {
        RenderError::Renderer(_) => ModelSmokeError::GpuEnvironment(format!("渲染核心初始化: {e}")),
        _ => ModelSmokeError::ModelCode(format!("渲染核心初始化: {e}")),
    }
}

/// 模型加载进渲染核心失败（`ModelRendererCore::load_model`）。
///
/// physics3.json 属资产文件（解析失败 → 资产问题）；其余是模型内容无法
/// 构建为可绘制对象 → 模型代码错误。均退出码 1。
pub fn classify_load_error(e: RenderError) -> ModelSmokeError {
    match e {
        RenderError::PhysicsJson(msg) => ModelSmokeError::Asset(format!("physics3.json: {msg}")),
        other => ModelSmokeError::ModelCode(format!("load_model: {other}")),
    }
}

/// 逐帧渲染阶段失败（`render_to_view`）。
///
/// poll 失败/回调通道关闭属设备异常 → GPU 环境；dt 非法等参数错误属代码缺陷。
pub fn classify_frame_error(e: RenderError) -> ModelSmokeError {
    match e {
        RenderError::Poll(_) | RenderError::ReadbackClosed => {
            ModelSmokeError::GpuEnvironment(format!("渲染帧提交/等待: {e}"))
        }
        other => ModelSmokeError::ModelCode(format!("渲染帧: {other}")),
    }
}

// ---------------------------------------------------------------- 动作序列（脚本入口）

/// 构造脚本动作：固定 Medium 强度、来源标记为规则 fallback（确定性脚本，
/// 不是 LLM tool 输出——诚实标注来源便于观测区分）。
pub fn scripted_action(action: ActionId) -> SemanticAction {
    SemanticAction::new(action, Strength::Medium, ActionSource::RuleFallback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2d::renderer::RenderError;
    use live2d_ai_core::ActionId;

    #[test]
    fn error_categories_map_to_exit_code_contract() {
        assert_eq!(ModelSmokeError::Asset("x".into()).category(), "asset");
        assert_eq!(
            ModelSmokeError::GpuEnvironment("x".into()).category(),
            "gpu-environment"
        );
        // Display 文案可区分三类问题。
        assert!(
            ModelSmokeError::Asset("缺文件".into())
                .to_string()
                .contains("资产")
        );
        assert!(
            ModelSmokeError::ModelCode("解析失败".into())
                .to_string()
                .contains("模型代码")
        );
        assert!(
            ModelSmokeError::GpuEnvironment("无 adapter".into())
                .to_string()
                .contains("GPU 环境")
        );
    }

    #[test]
    fn render_error_classification_matches_stage_semantics() {
        // load 阶段：physics JSON 是资产问题，其余是模型代码错误。
        assert_eq!(
            classify_load_error(RenderError::PhysicsJson("bad json".into())),
            ModelSmokeError::Asset("physics3.json: bad json".into())
        );
        assert!(matches!(
            classify_load_error(RenderError::Renderer("build fail".into())),
            ModelSmokeError::ModelCode(_)
        ));
        // 核心初始化阶段：管线建不起来是 GPU 环境问题。
        assert!(matches!(
            classify_core_init_error(RenderError::Renderer("pipeline".into())),
            ModelSmokeError::GpuEnvironment(_)
        ));
        // 帧阶段：poll/回调通道异常是 GPU 环境；dt 非法是代码缺陷。
        assert!(matches!(
            classify_frame_error(RenderError::ReadbackClosed),
            ModelSmokeError::GpuEnvironment(_)
        ));
        assert!(matches!(
            classify_frame_error(RenderError::Poll("lost".into())),
            ModelSmokeError::GpuEnvironment(_)
        ));
        assert!(matches!(
            classify_frame_error(RenderError::InvalidDt(-1.0)),
            ModelSmokeError::ModelCode(_)
        ));
    }

    #[test]
    fn scripted_actions_are_always_medium_rule_fallback() {
        for id in ActionId::ALL {
            let a = scripted_action(id);
            assert_eq!(a.strength, Strength::Medium);
            assert_eq!(a.source, ActionSource::RuleFallback);
            assert_eq!(a.action, id);
        }
    }

    #[test]
    fn synthesized_mouth_level_is_bounded_gated_and_deterministic() {
        // 全周期采样：值域 [0,1]、静默段恒 0、峰值足够高。
        let steps = 600;
        let mut peak = 0.0_f32;
        let mut silent_seen = false;
        let mut speaking_seen = false;
        for i in 0..steps {
            let t = i as f32 * MOUTH_CYCLE_SECS / steps as f32;
            let level = synthesized_mouth_level(t);
            assert!(
                level.is_finite() && (0.0..=1.0).contains(&level),
                "t={t} level={level}"
            );
            peak = peak.max(level);
            if level == 0.0 {
                silent_seen = true;
            } else {
                speaking_seen = true;
            }
        }
        assert!(peak > 0.9, "合成电平应有接近满幅的峰值，实际 {peak}");
        assert!(silent_seen && speaking_seen, "必须有说话段与静默段");
        // 静默段（占空比之外）恒为 0。
        let rest_start = MOUTH_CYCLE_SECS * MOUTH_DUTY + 0.01;
        assert_eq!(synthesized_mouth_level(rest_start), 0.0);
        assert_eq!(synthesized_mouth_level(MOUTH_CYCLE_SECS * MOUTH_DUTY), 0.0);
        // 同周期内重复调用位级一致；跨周期相位相同者允许模运算的浮点微差。
        assert_eq!(synthesized_mouth_level(0.37), synthesized_mouth_level(0.37));
        let a = synthesized_mouth_level(0.37);
        let b = synthesized_mouth_level(0.37 + MOUTH_CYCLE_SECS * 7.0);
        assert!(
            (a - b).abs() < 1e-3,
            "跨周期同相位电平应近似一致: {a} vs {b}"
        );
        for bad in [f32::NAN, f32::INFINITY, -1.0] {
            assert_eq!(synthesized_mouth_level(bad), 0.0);
        }
    }
}
