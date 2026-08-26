//! core [`ParameterFrame`] → Bai 标准参数 ID 的渲染适配层（RFC §4 批次 5）。
//!
//! 职责与边界：
//! - **纯映射 + 写入**：把 `live2d-ai-core` 表演曲线层的自有字段（角度制/归一化）
//!   翻译成 Bai 皮套的 Live2D 标准参数 ID，并经参数写入端（[`ParamSink`]）落到
//!   l2d 姿态栈对应层。本模块不做时间推进、不做曲线采样（那归 core）；
//! - **口型所有权**：嘴开合（`ParamMouthOpenY`）只来自口型电平通道
//!   （生产环境为 TTS RMS；model smoke 为低频合成电平），经 **final_override 层**
//!   （最高优先级）写入——表演曲线层从不写嘴开合（core 契约），物理/idle/input
//!   都压不过它，因此 surprise 等动作曲线不可能覆盖口型；
//! - **能力降级**：模型缺少某参数时写入返回 `false`——静默降级（不报错、不中止），
//!   但每个参数 ID 只记录一次 warn（避免逐帧刷屏）；缺失清单可经
//!   [`BaiParamAdapter::missing_params`] 查询供冒烟报告输出；
//! - **眨眼基线**：[`ParameterFrame::eye_open_scale`] 是睁眼倍率，按
//!   「Bai 默认睁眼值 = 1」乘法映射到 `ParamEyeLOpen` / `ParamEyeROpen`
//!   （1.0 = 不干预）。Bai 的 EyeBlink 组合在 model3.json 中声明了这组参数，
//!   默认值即 1（见 `bai.model3.json` Groups 与 cdi3 显示信息）。
//!
//! 本模块纯逻辑（trait 抽象掉 GPU 渲染核心），可无 GPU 单测。

use tracing::warn;

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

/// 一帧表演参数 → `(参数 ID, 值)` 有序表（纯函数，零分配）。
///
/// 固定 13 项：11 个字段中 `brow_y` 双写左右眉、`eye_open_scale` 双写左右眼。
/// 顺序稳定（头→眼→眉→嘴形→身体），便于测试与日志对照。
pub fn map_frame(frame: &ParameterFrame) -> [(&'static str, f32); 13] {
    // 曲线层已限幅（clamp_to_safe）；这里再防御一次有限性，NaN 绝不入姿态栈。
    let f = |v: f32| if v.is_finite() { v } else { 0.0 };
    let eye_open = {
        let scale = frame.eye_open_scale;
        let scale = if scale.is_finite() { scale } else { 1.0 };
        BAI_EYE_OPEN_DEFAULT * scale.clamp(performance::EYE_OPEN_MIN, performance::EYE_OPEN_MAX)
    };
    [
        (PARAM_ANGLE_X, f(frame.head_angle_x)),
        (PARAM_ANGLE_Y, f(frame.head_angle_y)),
        (PARAM_ANGLE_Z, f(frame.head_angle_z)),
        (PARAM_EYE_BALL_X, f(frame.eye_x)),
        (PARAM_EYE_BALL_Y, f(frame.eye_y)),
        (PARAM_EYE_L_OPEN, eye_open),
        (PARAM_EYE_R_OPEN, eye_open),
        (PARAM_BROW_L_Y, f(frame.brow_y)),
        (PARAM_BROW_R_Y, f(frame.brow_y)),
        (PARAM_MOUTH_FORM, f(frame.mouth_form)),
        (PARAM_BODY_ANGLE_X, f(frame.body_angle_x)),
        (PARAM_BODY_ANGLE_Y, f(frame.body_angle_y)),
        (PARAM_BODY_ANGLE_Z, f(frame.body_angle_z)),
    ]
}

// ---------------------------------------------------------------- 写入端抽象

/// 参数写入端：抽象掉 l2d 渲染核心的层写入接口，便于无 GPU 单测。
///
/// 生产实现是 [`ModelRendererCore`](l2d::renderer::ModelRendererCore)：
/// input 层优先级 idle < input < physics < final_override。
pub trait ParamSink {
    /// 向 input 层写参数；模型缺该参数时返回 `false` 且状态不变。
    fn set_input(&mut self, id: &str, value: f32) -> bool;

    /// 向 final_override 层写最高优先级覆盖；缺参返回 `false`。
    fn set_override(&mut self, id: &str, value: f32) -> bool;
}

impl ParamSink for l2d::renderer::ModelRendererCore {
    fn set_input(&mut self, id: &str, value: f32) -> bool {
        l2d::renderer::ModelRendererCore::set_parameter(self, id, value)
    }

    fn set_override(&mut self, id: &str, value: f32) -> bool {
        self.override_parameter(id, value)
    }
}

// ---------------------------------------------------------------- 适配器

/// 单帧写入统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApplyOutcome {
    /// 成功写入的参数个数。
    pub written: usize,
    /// 本次新记录为「模型缺失」的参数个数（历史已记录的不重复计）。
    pub newly_missing: usize,
}

/// Bai 参数适配器：持有一份「已记录过缺失」的清单，保证每个缺失参数只告警一次。
#[derive(Debug, Clone, Default)]
pub struct BaiParamAdapter {
    missing_logged: Vec<&'static str>,
}

impl BaiParamAdapter {
    /// 新建适配器（无缺失记录）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 把一帧表演参数写入 input 层。中性帧也照写（显式回中），
    /// 保证动作结束后不残留上一动作的偏移。
    pub fn apply_frame(
        &mut self,
        sink: &mut impl ParamSink,
        frame: &ParameterFrame,
    ) -> ApplyOutcome {
        let mut outcome = ApplyOutcome::default();
        for (id, value) in map_frame(frame) {
            if sink.set_input(id, value) {
                outcome.written += 1;
            } else {
                outcome.newly_missing += usize::from(self.record_missing(id));
            }
        }
        outcome
    }

    /// 口型电平 → `ParamMouthOpenY`（final_override 层，最高优先级）。
    ///
    /// 口型通道独立于表演曲线：即使 surprise 动作正在播放，本通道也不被覆盖
    /// （曲线层根本不写嘴开合；且 override 层优先级高于一切）。
    /// `level` 被夹到 [0, 1]；非有限值按 0（闭嘴）处理。
    ///
    /// 返回写入是否被模型接受（缺参时 `false` 并只告警一次）。
    pub fn apply_mouth_level(&mut self, sink: &mut impl ParamSink, level: f32) -> bool {
        let value = if level.is_finite() {
            level.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if sink.set_override(PARAM_MOUTH_OPEN_Y, value) {
            true
        } else {
            self.record_missing(PARAM_MOUTH_OPEN_Y);
            false
        }
    }

    /// 已记录缺失的参数 ID（顺序 = 首次遇到顺序；供冒烟报告输出）。
    pub fn missing_params(&self) -> &[&'static str] {
        &self.missing_logged
    }

    /// 记录一次缺失；返回是否为新记录（每 ID 至多一次）。
    fn record_missing(&mut self, id: &'static str) -> bool {
        if self.missing_logged.contains(&id) {
            return false;
        }
        self.missing_logged.push(id);
        // 静默降级：不报错、不中止，但留一条一次性观测记录。
        warn!(param = id, "模型缺少该参数，能力降级（仅记录一次）");
        true
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use live2d_ai_core::action::ActionId;
    use live2d_ai_core::{ActionSource, SemanticAction, Strength, performance};

    use super::*;

    /// 记录型写入端：模拟一个「部分参数缺失」的模型，供无 GPU 断言。
    #[derive(Default)]
    struct RecordingSink {
        inputs: BTreeMap<String, f32>,
        overrides: BTreeMap<String, f32>,
        missing: Vec<&'static str>,
    }

    impl RecordingSink {
        fn with_missing(missing: &[&'static str]) -> Self {
            Self {
                missing: missing.to_vec(),
                ..Self::default()
            }
        }

        fn input(&self, id: &str) -> f32 {
            *self.inputs.get(id).unwrap_or_else(|| panic!("未写入 {id}"))
        }

        fn override_of(&self, id: &str) -> f32 {
            *self
                .overrides
                .get(id)
                .unwrap_or_else(|| panic!("未覆盖写入 {id}"))
        }
    }

    impl ParamSink for RecordingSink {
        fn set_input(&mut self, id: &str, value: f32) -> bool {
            if self.missing.contains(&id) {
                return false;
            }
            self.inputs.insert(id.to_owned(), value);
            true
        }

        fn set_override(&mut self, id: &str, value: f32) -> bool {
            if self.missing.contains(&id) {
                return false;
            }
            self.overrides.insert(id.to_owned(), value);
            true
        }
    }

    /// 全字段非中性的样例帧。
    fn sample_frame() -> ParameterFrame {
        ParameterFrame {
            head_angle_x: 12.0,
            head_angle_y: -8.0,
            head_angle_z: 6.0,
            eye_x: -0.4,
            eye_y: 0.3,
            eye_open_scale: 1.2,
            brow_y: 0.5,
            mouth_form: -0.25,
            body_angle_x: 3.0,
            body_angle_y: -2.0,
            body_angle_z: 1.5,
        }
    }

    #[test]
    fn map_frame_covers_every_field_with_bai_standard_ids() {
        let frame = sample_frame();
        let mapped = map_frame(&frame);
        assert_eq!(mapped.len(), 13);

        let get = |id: &str| {
            mapped
                .iter()
                .find(|(k, _)| *k == id)
                .map(|(_, v)| *v)
                .unwrap_or_else(|| panic!("缺少映射 {id}"))
        };
        // 头/身角度：字段值原样映射到 Bai 标准参数。
        assert_eq!(get(PARAM_ANGLE_X), frame.head_angle_x);
        assert_eq!(get(PARAM_ANGLE_Y), frame.head_angle_y);
        assert_eq!(get(PARAM_ANGLE_Z), frame.head_angle_z);
        assert_eq!(get(PARAM_BODY_ANGLE_X), frame.body_angle_x);
        assert_eq!(get(PARAM_BODY_ANGLE_Y), frame.body_angle_y);
        assert_eq!(get(PARAM_BODY_ANGLE_Z), frame.body_angle_z);
        // 眼球。
        assert_eq!(get(PARAM_EYE_BALL_X), frame.eye_x);
        assert_eq!(get(PARAM_EYE_BALL_Y), frame.eye_y);
        // 眉毛双写左右。
        assert_eq!(get(PARAM_BROW_L_Y), frame.brow_y);
        assert_eq!(get(PARAM_BROW_R_Y), frame.brow_y);
        // 嘴形（不是嘴开合）。
        assert_eq!(get(PARAM_MOUTH_FORM), frame.mouth_form);
        // 映射表里没有嘴开合——口型通道独立（见 mouth 测试）。
        assert!(!mapped.iter().any(|(k, _)| *k == PARAM_MOUTH_OPEN_Y));
    }

    #[test]
    fn eye_open_scale_maps_to_both_eyes_around_bai_default_one() {
        // 中性倍率 1.0 × 默认 1 → 双眼全开。
        let mapped = map_frame(&ParameterFrame::neutral());
        for (id, v) in &mapped {
            if *id == PARAM_EYE_L_OPEN || *id == PARAM_EYE_R_OPEN {
                assert_eq!(*v, 1.0, "{id}");
            }
        }
        // 放大倍率双写一致。
        let mut wide = ParameterFrame::neutral();
        wide.eye_open_scale = 1.2;
        let mapped = map_frame(&wide);
        let l = mapped
            .iter()
            .find(|(k, _)| *k == PARAM_EYE_L_OPEN)
            .unwrap()
            .1;
        let r = mapped
            .iter()
            .find(|(k, _)| *k == PARAM_EYE_R_OPEN)
            .unwrap()
            .1;
        assert_eq!(l, 1.2);
        assert_eq!(r, 1.2);
        // 越界倍率被夹回安全范围 [EYE_OPEN_MIN, EYE_OPEN_MAX]。
        let mut extreme = ParameterFrame::neutral();
        extreme.eye_open_scale = 9.9;
        let mapped = map_frame(&extreme);
        let l = mapped
            .iter()
            .find(|(k, _)| *k == PARAM_EYE_L_OPEN)
            .unwrap()
            .1;
        assert_eq!(l, performance::EYE_OPEN_MAX);
    }

    #[test]
    fn apply_frame_writes_input_layer_and_reports_counts() {
        let mut adapter = BaiParamAdapter::new();
        let mut sink = RecordingSink::default();
        let outcome = adapter.apply_frame(&mut sink, &sample_frame());
        assert_eq!(outcome.written, 13);
        assert_eq!(outcome.newly_missing, 0);
        assert_eq!(sink.inputs.len(), 13);
        assert!(adapter.missing_params().is_empty());
        // 中性帧也显式写入（动作结束后不残留上一动作偏移）。
        let outcome = adapter.apply_frame(&mut sink, &ParameterFrame::neutral());
        assert_eq!(outcome.written, 13);
        assert_eq!(sink.input(PARAM_ANGLE_X), 0.0);
    }

    #[test]
    fn missing_parameters_degrade_silently_but_are_recorded_once() {
        const GONE: &[&str] = &[
            PARAM_BROW_L_Y,
            PARAM_BROW_R_Y,
            PARAM_EYE_BALL_Y,
            PARAM_MOUTH_OPEN_Y,
        ];
        let mut adapter = BaiParamAdapter::new();
        let mut sink = RecordingSink::with_missing(GONE);
        let frame = sample_frame();

        // 多帧重复写：缺参持续失败，但记录只新增一次。
        for i in 0..5 {
            let outcome = adapter.apply_frame(&mut sink, &frame);
            assert_eq!(outcome.written, 13 - 3); // BrowLY/BrowRY/EyeBallY 缺失
            if i == 0 {
                assert_eq!(outcome.newly_missing, 3);
            } else {
                assert_eq!(outcome.newly_missing, 0);
            }
            // 口型通道同样缺参：每帧返回 false，但只记一次。
            assert!(!adapter.apply_mouth_level(&mut sink, 0.5));
        }
        let mut missing = adapter.missing_params().to_vec();
        missing.sort_unstable();
        assert_eq!(missing, GONE.to_vec());

        // 其余参数照常写入（能力降级 ≠ 全部失效）。
        assert_eq!(sink.input(PARAM_ANGLE_Z), frame.head_angle_z);
        assert!(!sink.inputs.contains_key(PARAM_BROW_L_Y));
        assert!(!sink.overrides.contains_key(PARAM_MOUTH_OPEN_Y));
    }

    #[test]
    fn mouth_channel_is_highest_priority_and_never_covered_by_surprise_curve() {
        let mut adapter = BaiParamAdapter::new();
        let mut sink = RecordingSink::default();

        // 口型电平走 final_override 层。
        assert!(adapter.apply_mouth_level(&mut sink, 0.7));
        assert_eq!(sink.override_of(PARAM_MOUTH_OPEN_Y), 0.7);

        // surprise 动作曲线采样（含 mouth_form 微调）写入 input 层后，
        // 口型 override 不受影响；且曲线从不产生 MouthOpenY 的 input 写入。
        let surprise =
            SemanticAction::new(ActionId::Surprise, Strength::Medium, ActionSource::LlmTool);
        let mut frame = ParameterFrame::neutral();
        performance::sample_at(surprise, 0.35, &mut frame);
        adapter.apply_frame(&mut sink, &frame);
        assert!(frame.mouth_form != 0.0, "前置：surprise 应驱动嘴形");
        assert!(!sink.inputs.contains_key(PARAM_MOUTH_OPEN_Y));
        assert_eq!(sink.override_of(PARAM_MOUTH_OPEN_Y), 0.7);

        // 电平更新覆盖旧值（通道活着），并夹紧到 [0,1]。
        assert!(adapter.apply_mouth_level(&mut sink, 2.0));
        assert_eq!(sink.override_of(PARAM_MOUTH_OPEN_Y), 1.0);
        assert!(adapter.apply_mouth_level(&mut sink, -3.0));
        assert_eq!(sink.override_of(PARAM_MOUTH_OPEN_Y), 0.0);
        assert!(adapter.apply_mouth_level(&mut sink, f32::NAN));
        assert_eq!(sink.override_of(PARAM_MOUTH_OPEN_Y), 0.0);
    }
}
