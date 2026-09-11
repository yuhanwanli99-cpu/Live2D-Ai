//! core [`ParameterFrame`] → Bai 标准参数 ID 的渲染适配层（RFC §4 批次 5）。
//!
//! 职责与边界：
//! - **纯映射 + 稀疏写入**：把 `live2d-ai-core` 表演曲线层的自有字段（角度制/
//!   归一化）翻译成 Bai 皮套的 Live2D 标准参数 ID，并经参数写入端
//!   （[`ParamSink`]）落到 l2d 姿态栈对应层。**只写
//!   [`ParameterFrame::owned`] 置位的通道**（P0-5 稀疏所有权）；上一帧持有、
//!   本帧释放的通道执行 `clear_parameter` 交还 idle/物理——「中性」是
//!   释放所有权，不是持续写零。本模块不做时间推进、不做曲线采样（那归 core）；
//! - **口型所有权**：嘴开合（`ParamMouthOpenY`）只来自口型电平通道
//!   （生产环境为 TTS RMS；model smoke 为低频合成电平），经 **final_override 层**
//!   （最高优先级）写入——表演曲线层从不写嘴开合（core 契约，所有权位图里
//!   也不存在 MouthOpenY），物理/idle/input 都压不过它；
//! - **能力降级**：模型缺少某参数时写入返回 `false`——静默降级（不报错、不中止），
//!   但每个参数 ID 只记录一次 warn（避免逐帧刷屏）；缺失清单可经
//!   [`BaiParamAdapter::missing_params`] 查询供冒烟报告输出；
//! - **眨眼基线**：[`ParameterFrame::eye_open_scale`] 是睁眼倍率，按
//!   「Bai 默认睁眼值 = 1」乘法映射到 `ParamEyeLOpen` / `ParamEyeROpen`
//!   （1.0 = 不干预）。v0 诚实声明：项目当前没有自动眨眼驱动源，
//!   不承诺「模型自带眨眼会自动运行」；blink 应作为独立参数源接入后与本倍率合成。
//!
//! 本模块纯逻辑（trait 抽象掉 GPU 渲染核心），可无 GPU 单测。

pub mod mask;

pub use mask::{PARAM_MOUTH_OPEN_Y, frame_entries};

use tracing::warn;

use live2d_ai_core::ParameterFrame;

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

    /// 撤销某参数在 input 层的写入（P0-5：所有权释放路径）。
    /// 模型本就缺该参数时为无害空操作。
    fn clear_input(&mut self, id: &str);
}

impl ParamSink for l2d::renderer::ModelRendererCore {
    fn set_input(&mut self, id: &str, value: f32) -> bool {
        l2d::renderer::ModelRendererCore::set_parameter(self, id, value)
    }

    fn set_override(&mut self, id: &str, value: f32) -> bool {
        self.override_parameter(id, value)
    }

    fn clear_input(&mut self, id: &str) {
        // 返回值（模型是否有该参数）不关心：clear 永远是安全收尾。
        let _ = self.clear_parameter(id);
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

/// Bai 参数适配器：持有「已记录过缺失」清单（每 ID 至多告警一次）与
/// 「当前动作持有且已写入 input 层」的参数清单（P0-5 稀疏所有权的释放依据）。
#[derive(Debug, Clone, Default)]
pub struct BaiParamAdapter {
    missing_logged: Vec<&'static str>,
    /// 当前仍被动作持有、且本适配器已写入 input 层的参数 ID。
    /// 帧掩码释放这些通道时据此执行 `clear_input`（idle/物理立即接管）。
    applied_owned: Vec<&'static str>,
}

impl BaiParamAdapter {
    /// 新建适配器（无缺失记录、无持有序）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 把一帧表演参数**稀疏**写入 input 层（P0-5 裁决）。
    ///
    /// - 只写 [`ParameterFrame::owned`] 置位通道：从未持有的通道绝不写——
    ///   尤其绝不写「中性绝对值零」（那会永久压住 idle 摆动）；
    /// - 上一帧持有、本帧不再持有的通道 → 执行 [`ParamSink::clear_input`]
    ///   （所有权释放），动作结束/被打断的下一帧自动完成回中交接；
    /// - 完整清除入口见 [`Self::release_all`]（stop / 模型切换用）。
    pub fn apply_frame(
        &mut self,
        sink: &mut impl ParamSink,
        frame: &ParameterFrame,
    ) -> ApplyOutcome {
        let mut outcome = ApplyOutcome::default();
        let mut now_owned: Vec<&'static str> = Vec::with_capacity(self.applied_owned.len());
        for (owned_bit, id, value) in frame_entries(frame) {
            if !frame.owned.contains(owned_bit) {
                continue; // 稀疏语义：未持有的通道绝不定值
            }
            now_owned.push(id);
            if sink.set_input(id, value) {
                outcome.written += 1;
            } else {
                outcome.newly_missing += usize::from(self.record_missing(id));
            }
        }

        // 所有权差集释放：written 记录的是本帧成功写入数；
        // 释放（clear）不计入 written——它是收尾动作，不是新值。
        let previously_owned = std::mem::take(&mut self.applied_owned);
        for id in previously_owned {
            if !now_owned.contains(&id) {
                sink.clear_input(id);
            }
        }
        self.applied_owned = now_owned;
        outcome
    }

    /// 一次性释放全部动作所有权并清空对应 input 写入
    /// （stop / 模型切换 / 回交互空闲时由 shell 显式调用；P0-5 裁决）。
    ///
    /// 返回被清除的参数个数。口型通道（final_override）不在此列：
    /// 由 shell 以口型电平 0 驱动归零（职责分离，见模块文档）。
    /// 此后下一帧即使 owned 掩码非空也会按全新持有处理。
    ///
    /// 当前仅测试引用；supervisor（最终接线步骤 8 的 stop / 模型切换路径）
    /// 届时消费并移除 allow。
    #[allow(dead_code)]
    pub fn release_all(&mut self, sink: &mut impl ParamSink) -> usize {
        let previously_owned = std::mem::take(&mut self.applied_owned);
        let count = previously_owned.len();
        for id in previously_owned {
            sink.clear_input(id);
        }
        count
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

    use super::mask::{
        PARAM_ANGLE_X, PARAM_ANGLE_Y, PARAM_ANGLE_Z, PARAM_BODY_ANGLE_Z, PARAM_BROW_L_Y,
        PARAM_BROW_R_Y, PARAM_EYE_BALL_X, PARAM_EYE_BALL_Y, PARAM_EYE_L_OPEN, PARAM_EYE_R_OPEN,
        PARAM_MOUTH_FORM, PARAM_MOUTH_OPEN_Y, frame_entries,
    };
    use super::{BaiParamAdapter, ParamSink};

    /// 记录型写入端：模拟一个「部分参数缺失」的模型，供无 GPU 断言。
    #[derive(Default)]
    struct RecordingSink {
        inputs: BTreeMap<String, f32>,
        overrides: BTreeMap<String, f32>,
        cleared: Vec<String>,
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

        fn clear_input(&mut self, id: &str) {
            // 语义对齐 ModelRendererCore::clear_parameter：撤销 input 层写入。
            self.inputs.remove(id);
            self.cleared.push(id.to_owned());
        }
    }

    /// 全字段非中性的样例帧（所有权 = 全部 11 位）。
    fn sample_frame() -> live2d_ai_core::ParameterFrame {
        use live2d_ai_core::ParameterMask as M;
        let all = M::HEAD_ANGLE_X
            .union(M::HEAD_ANGLE_Y)
            .union(M::HEAD_ANGLE_Z)
            .union(M::EYE_X)
            .union(M::EYE_Y)
            .union(M::EYE_OPEN_SCALE)
            .union(M::BROW_Y)
            .union(M::MOUTH_FORM)
            .union(M::BODY_ANGLE_X)
            .union(M::BODY_ANGLE_Y)
            .union(M::BODY_ANGLE_Z);
        live2d_ai_core::ParameterFrame {
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
            owned: all,
        }
    }

    #[test]
    fn frame_entries_covers_every_field_with_bai_standard_ids() {
        let frame = sample_frame();
        let mapped = frame_entries(&frame);
        assert_eq!(mapped.len(), 13);

        let get = |id: &str| {
            mapped
                .iter()
                .find(|(_, k, _)| *k == id)
                .map(|(_, _, v)| *v)
                .unwrap_or_else(|| panic!("缺少映射 {id}"))
        };
        // 头/身角度：字段值原样映射到 Bai 标准参数。
        assert_eq!(get(PARAM_ANGLE_X), frame.head_angle_x);
        assert_eq!(get(PARAM_ANGLE_Y), frame.head_angle_y);
        assert_eq!(get(PARAM_ANGLE_Z), frame.head_angle_z);
        assert_eq!(get(super::mask::PARAM_BODY_ANGLE_X), frame.body_angle_x);
        assert_eq!(get(super::mask::PARAM_BODY_ANGLE_Y), frame.body_angle_y);
        assert_eq!(get(super::mask::PARAM_BODY_ANGLE_Z), frame.body_angle_z);
        // 眼球。
        assert_eq!(get(PARAM_EYE_BALL_X), frame.eye_x);
        assert_eq!(get(PARAM_EYE_BALL_Y), frame.eye_y);
        // 眉毛双写左右。
        assert_eq!(get(PARAM_BROW_L_Y), frame.brow_y);
        assert_eq!(get(PARAM_BROW_R_Y), frame.brow_y);
        // 嘴形（不是嘴开合）。
        assert_eq!(get(PARAM_MOUTH_FORM), frame.mouth_form);
        // 映射表里没有嘴开合——口型通道独立（见 mouth 测试）。
        assert!(!mapped.iter().any(|(_, k, _)| *k == PARAM_MOUTH_OPEN_Y));
    }

    #[test]
    fn eye_open_scale_maps_to_both_eyes_around_bai_default_one() {
        // 中性倍率 1.0 × 默认 1 → 双眼全开。
        let entries = frame_entries(&live2d_ai_core::ParameterFrame::neutral());
        for (_, id, v) in &entries {
            if *id == PARAM_EYE_L_OPEN || *id == PARAM_EYE_R_OPEN {
                assert_eq!(*v, 1.0, "{id}");
            }
        }
        // 放大倍率双写一致。
        let mut wide = live2d_ai_core::ParameterFrame::neutral();
        wide.eye_open_scale = 1.2;
        let entries = frame_entries(&wide);
        for (_, id, v) in &entries {
            if *id == PARAM_EYE_L_OPEN || *id == PARAM_EYE_R_OPEN {
                assert_eq!(*v, 1.2, "{id}");
            }
        }
        // 越界倍率被夹回安全范围 [EYE_OPEN_MIN, EYE_OPEN_MAX]。
        let mut extreme = live2d_ai_core::ParameterFrame::neutral();
        extreme.eye_open_scale = 9.9;
        let entries = frame_entries(&extreme);
        let l = entries
            .iter()
            .find(|(_, k, _)| *k == PARAM_EYE_L_OPEN)
            .unwrap()
            .2;
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

        // P0-5：空所有权帧（动作结束/空闲）**不写任何中性绝对值**，
        // 而是释放上一帧持有的全部通道（idle/物理接管）。
        let outcome = adapter.apply_frame(&mut sink, &live2d_ai_core::ParameterFrame::neutral());
        assert_eq!(outcome.written, 0, "未持有通道绝不写零");
        assert!(sink.inputs.is_empty(), "持有集应被 clear 全部撤销");
    }

    /// P0-5 核心：稀疏写入只落持有通道；帧掩码释放后 input 层清空、
    /// idle 可接管——「动作结束后 ParamAngleZ 恢复」的锚定测试。
    #[test]
    fn sparse_write_only_owned_channels_and_release_on_finish() {
        let mut adapter = BaiParamAdapter::new();
        let mut sink = RecordingSink::default();

        // tilt 播放中段：只允许写 tilt 声明的通道。
        let tilt =
            SemanticAction::new(ActionId::Tilt, Strength::Medium, ActionSource::RuleFallback);
        let mut frame = live2d_ai_core::ParameterFrame::neutral();
        performance::sample_at(tilt, 0.5, &mut frame);
        assert!(frame.head_angle_z != 0.0, "前置：tilt 应驱动 AngleZ");
        let outcome = adapter.apply_frame(&mut sink, &frame);
        // 持有字段：HEAD_ANGLE_Z、BODY_ANGLE_Z、EYE_X、BROW_Y（双写左右眉）
        // → 共 5 个参数条目。
        assert_eq!(outcome.written, 5);
        assert!(sink.inputs.contains_key(PARAM_ANGLE_Z));
        assert!(sink.inputs.contains_key(PARAM_BODY_ANGLE_Z));
        assert!(sink.inputs.contains_key(PARAM_EYE_BALL_X));
        assert!(sink.inputs.contains_key(PARAM_BROW_L_Y));
        // 未持有的通道绝不写入——哪怕值恰好是零（无「压住 idle」的可能）。
        assert!(
            !sink.inputs.contains_key(PARAM_ANGLE_X),
            "AngleX 不在 tilt 所有权内"
        );
        assert!(!sink.inputs.contains_key(PARAM_MOUTH_OPEN_Y));

        // 动作结束：player 进入 Idle/Finished → 帧 neutral + 空掩码
        // → 全部持有通道被 clear（渲染层从栈里移除，回到模型默认/idle）。
        let outcome = adapter.apply_frame(&mut sink, &live2d_ai_core::ParameterFrame::neutral());
        assert_eq!(outcome.written, 0);
        assert!(sink.inputs.is_empty(), "释放后不得残留任何 input 写入");
        for id in [PARAM_ANGLE_Z, PARAM_BODY_ANGLE_Z, PARAM_EYE_BALL_X] {
            assert!(sink.cleared.iter().any(|c| c == id), "{id} 应被显式 clear");
        }
    }

    /// P0-5 附：`release_all` 一次性清空当前持有序（stop/模型切换入口）；
    /// 之后同一帧重新按全新持有处理。
    #[test]
    fn release_all_clears_everything_tracked() {
        let mut adapter = BaiParamAdapter::new();
        let mut sink = RecordingSink::default();

        let nod = SemanticAction::new(ActionId::Nod, Strength::Low, ActionSource::LlmTool);
        let mut frame = live2d_ai_core::ParameterFrame::neutral();
        performance::sample_at(nod, 0.5, &mut frame);
        adapter.apply_frame(&mut sink, &frame);
        assert_eq!(sink.inputs.len(), 2); // AngleY + BodyY

        assert_eq!(adapter.release_all(&mut sink), 2);
        assert!(sink.inputs.is_empty());
        assert_eq!(sink.cleared.len(), 2);

        // release 后再 apply：按新帧正常写入（状态机可重入）。
        adapter.apply_frame(&mut sink, &frame);
        assert_eq!(sink.inputs.len(), 2);
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
        let mut frame = live2d_ai_core::ParameterFrame::neutral();
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
