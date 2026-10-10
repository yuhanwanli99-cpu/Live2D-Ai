//! 姿态分层栈（纯逻辑、无 GPU）：base < idle < input < physics < final_override。
//!
//! 旧实现把 idle 驱动与外部输入全部合并进**一个持久的物理工作姿态**，
//! 每帧再被用户姿态覆盖——后果是：
//! 1. idle 值写进累积态后，会被任何曾经设置过同键的用户值永久压制
//!    （idle 被后续 update 覆盖）；
//! 2. `clear_parameter` 无法撤销已并入累积态的旧值（撤销不传播）。
//!
//! 本模块把各层拆开、每帧从独立层**重新合成最终姿态**，按固定优先级覆盖：
//!
//! | 层 | 写入方 | 优先级 |
//! |---|---|---|
//! | base | 模型清单默认值（空层，缺省即默认） | 0（最低） |
//! | idle | 内建待机驱动（呼吸正弦 / 摆头），每帧整体重算 | 1 |
//! | input | [`PoseStack::set_parameter`]，[`PoseStack::clear_parameter`] 可撤销 | 2 |
//! | physics | 物理引擎输出（消费 idle+input+final_override 作为输入信号） | 3 |
//! | final_override | [`PoseStack::override_parameter`]，最高优先级覆盖 | 4 |
//!
//! 物理引擎是唯一的**有状态层**：其工作姿态跨帧保留上一帧输出
//! （摆动/弹簧需要连续性），但每帧先被「idle+input」信号刷新输入侧，
//! 因此撤销/覆盖都能正确传播。其余层全部无状态合成。

use std::{fmt, sync::Arc};

use ayagami::{meta, physics, pose};

use crate::model::{LoadedModel, ModelHandle};

/// 呼吸参数 ID（上游 demo 同款 idle 驱动；模型没有该参数时自动跳过）。
pub const BREATH_PARAM: &str = "ParamBreath";
/// 头部摆动参数 ID（给物理一个可见输入；缺失时跳过）。
pub const ANGLE_Z_PARAM: &str = "ParamAngleZ";

/// 非法 dt：非有限（NaN / ±Inf）或 <= 0。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InvalidDt(pub f32);

impl fmt::Display for InvalidDt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "非法 dt `{}`（必须为有限且 > 0）", self.0)
    }
}

impl std::error::Error for InvalidDt {}

/// 姿态栈构建错误。
#[derive(Debug)]
#[non_exhaustive]
pub enum PoseStackError {
    /// physics3.json 解析失败。
    PhysicsJson(String),
}

impl fmt::Display for PoseStackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicsJson(e) => write!(f, "physics3.json 解析失败: {e}"),
        }
    }
}

impl std::error::Error for PoseStackError {}

/// 姿态分层栈：无 GPU 纯逻辑，可独立测试（合成参数表即可构建）。
pub struct PoseStack {
    map: Arc<pose::PoseMap>,
    /// input 层：外部显式输入（set/clear_parameter）。
    input: pose::Pose,
    /// physics 层工作姿态（有状态：跨帧保留物理输出连续性）。
    physics_work: pose::Pose,
    /// final_override 层：最高优先级覆盖。
    final_override: pose::Pose,
    /// 最近一次 update 重算出的 idle 层（finalize 用；初始为空 = 全默认）。
    latest_idle: pose::Pose,
    engine: Option<physics::PhysicsEngine>,
    breath_range: Option<(f32, f32)>,
    breath_key: pose::Key<'static>,
    angle_key: pose::Key<'static>,
    has_angle_z: bool,
    /// idle 开关（默认 true）：false 时 **idle 层整体停写**——不写呼吸正弦，
    /// 也不写 `ParamAngleZ` 的 ±3° 摆头。`IdleState` / `apply_idle_life` 共用同一开关。
    idle_enabled: bool,
    elapsed: f32,
}

impl PoseStack {
    /// 从既有参数描述表构建（无物理引擎；测试可注入合成表）。
    pub fn from_pose_map(map: Arc<pose::PoseMap>) -> Self {
        let empty = pose::Pose::with_map(Arc::clone(&map));
        let breath_range = map
            .get(&pose::Key::param(BREATH_PARAM))
            .map(|(_, d)| (d.min, d.max));
        let has_angle_z = map.has_key(&pose::Key::param(ANGLE_Z_PARAM));
        Self {
            map,
            input: empty.clone(),
            physics_work: empty.clone(),
            final_override: empty.clone(),
            latest_idle: empty,
            engine: None,
            breath_range,
            breath_key: pose::Key::param(BREATH_PARAM),
            angle_key: pose::Key::param(ANGLE_Z_PARAM),
            has_angle_z,
            idle_enabled: true,
            elapsed: 0.0,
        }
    }

    /// 从运行时句柄构建（无物理引擎）。
    pub fn new(handle: &ModelHandle) -> Self {
        Self::from_pose_map(handle.pose_map())
    }

    /// 从绑定的 [`LoadedModel`] 构建：有 physics3.json 时一并挂接物理引擎
    /// 并做确定性 settle（与 bakeoff 验证的初始化序列一致）。
    pub fn from_loaded_model(loaded: &LoadedModel) -> Result<Self, PoseStackError> {
        let mut stack = Self::new(loaded.handle());
        if let Some(bytes) = loaded.package().physics_json() {
            stack.attach_physics_json(bytes)?;
        }
        Ok(stack)
    }

    /// 从 physics3.json 字节挂接物理引擎并做确定性 settle
    /// （[`Self::from_loaded_model`] 内部同样走此路径；合成参数表亦可注入）。
    pub fn attach_physics_json(&mut self, json: &[u8]) -> Result<(), PoseStackError> {
        let setting: meta::Physics3 =
            serde_json::from_slice(json).map_err(|e| PoseStackError::PhysicsJson(e.to_string()))?;
        let mut engine =
            physics::PhysicsEngine::new(setting, physics::PhysicsOptions::compatible(None));
        engine.settle(&self.physics_work);
        self.engine = Some(engine);
        Ok(())
    }

    /// 是否已挂接物理引擎。
    pub fn has_physics(&self) -> bool {
        self.engine.is_some()
    }

    /// 已推进的模拟时间（秒）。
    pub fn elapsed(&self) -> f32 {
        self.elapsed
    }

    /// 待机开关（默认 `true`）。false 时下一帧起 idle 层不再写呼吸正弦与
    /// `ParamAngleZ` 的 ±3° 摆头（与前端「待机小动作」同开关）。
    pub fn set_idle_enabled(&mut self, enabled: bool) {
        self.idle_enabled = enabled;
    }

    /// 当前待机开关。
    pub fn idle_enabled(&self) -> bool {
        self.idle_enabled
    }

    /// 向 **input 层**写入参数（优先级高于 idle、低于 physics/final_override）。
    ///
    /// 模型没有该参数时返回 `false` 且状态不变。
    pub fn set_parameter(&mut self, id: &str, value: f32) -> bool {
        Self::write_param(&mut self.input, id, value)
    }

    /// 撤销 **input 层**参数（下一帧起不再生效；返回是否有值被移除）。
    pub fn clear_parameter(&mut self, id: &str) -> bool {
        let key = pose::Key::param(id);
        let had = self.input.has_value(&key);
        self.input.unset(&key);
        had
    }

    /// 向 **final_override 层**写入最高优先级覆盖（压过 idle/input/physics）。
    ///
    /// 模型没有该参数时返回 `false` 且状态不变。
    pub fn override_parameter(&mut self, id: &str, value: f32) -> bool {
        Self::write_param(&mut self.final_override, id, value)
    }

    /// 撤销 **final_override 层**参数（返回是否有值被移除）。
    pub fn clear_override_parameter(&mut self, id: &str) -> bool {
        let key = pose::Key::param(id);
        let had = self.final_override.has_value(&key);
        self.final_override.unset(&key);
        had
    }

    /// 推进一个固定步：重算 idle 层 → 物理步进 → 合成最新最终姿态。
    ///
    /// `dt` 必须为有限且 > 0，否则返回 [`InvalidDt`] 且**任何状态都不变**
    /// （包括模拟时钟）。推荐 [`crate::renderer::FIXED_DT_60HZ`]。
    pub fn update(&mut self, dt: f32) -> Result<(), InvalidDt> {
        if !(dt.is_finite() && dt > 0.0) {
            return Err(InvalidDt(dt));
        }
        let time = self.elapsed;
        self.elapsed += dt;

        // ---- idle 层：每帧从零重算（独立层，不再被其他层的历史值污染）----
        // `idle_enabled == false` 时整层停写：呼吸/摆头都回到 base 默认值。
        let mut idle = pose::Pose::with_map(Arc::clone(&self.map));
        if self.idle_enabled {
            if let Some((min, max)) = self.breath_range {
                let v =
                    ((time / 2.0 * std::f32::consts::PI).cos() / -2.0 + 0.5) * (max - min) + min;
                idle.set(&self.breath_key, v);
            }
            if self.has_angle_z {
                // P3.1 修复：i32 截断会让正弦在峰值（导数≈0）处连续多帧停在同
                // 一整数再跳变——表现为"摆到最左/最右卡一下"。改为连续 f32；
                // 幅度 15°→3°（轻微摇晃，py 版生命体征同级），周期放慢一倍。
                let wave = (time * std::f32::consts::PI / 2.0).sin() * 3.0;
                idle.set(&self.angle_key, wave);
            }
        }

        // ---- physics 层：工作姿态 ← 上帧输出 ⊕ 本帧信号（idle+input+final_override）----
        // final_override 也进物理输入（同一参数上它最后写、即赢），于是导演写在
        // `ParamAngleZ` 上的歪头会带动物理——头发/被物理带动的身体跟着这一下，
        // 而不是继续 idle 层的 ±3°。新 cue 晚一帧进物理（tick 顺序不变）。
        if let Some(engine) = self.engine.as_mut() {
            let mut signal = idle.clone();
            signal.update(&self.input);
            signal.update(&self.final_override);
            // 引擎在 work 上就地步进：输入参数读取 signal 刷新后的值，
            // 被驱动参数继承上帧输出保证连续性。
            self.physics_work.update(&signal);
            engine.update(&mut self.physics_work, dt);
        }

        self.latest_idle = idle;
        Ok(())
    }

    /// 合成当前最终姿态：base ← idle ← input ← physics ← final_override。
    pub fn finalize(&self) -> pose::Pose {
        let mut out = pose::Pose::with_map(Arc::clone(&self.map)); // base：全默认
        out.update(&self.latest_idle); // idle
        out.update(&self.input); // input
        if self.engine.is_some() {
            out.update(&self.physics_work); // physics 输出覆盖被驱动参数
        }
        out.update(&self.final_override); // 最终覆盖
        out
    }

    /// 观测：某参数当前合成值（flatten 到默认值口径；模型无该参数时 `None`）。
    pub fn get_parameter(&self, id: &str) -> Option<f32> {
        self.finalize().get_flattened(&pose::Key::param(id))
    }

    /// 观测：input 层是否持有该参数值。
    pub fn has_input_value(&self, id: &str) -> bool {
        self.input.has_value(&pose::Key::param(id))
    }

    /// 观测：final_override 层是否持有该参数值。
    pub fn has_override_value(&self, id: &str) -> bool {
        self.final_override.has_value(&pose::Key::param(id))
    }

    /// 向指定层写入参数；模型（层共享参数表）没有该参数时返回 `false`。
    fn write_param(layer: &mut pose::Pose, id: &str, value: f32) -> bool {
        let key = pose::Key::param(id);
        if !layer.has_key(&key) {
            return false;
        }
        layer.set(&key, value);
        true
    }
}

impl fmt::Debug for PoseStack {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoseStack")
            .field("has_physics", &self.engine.is_some())
            .field("elapsed", &self.elapsed)
            .field("breath", &self.breath_range.is_some())
            .field("angle_z", &self.has_angle_z)
            .field("idle_enabled", &self.idle_enabled)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::FIXED_DT_60HZ;
    use ayagami::pose::Descriptor;

    /// 合成参数表：呼吸 + 摆头 + 一个普通参数（默认 0，范围 [-10,10]）。
    fn synthetic_map(with_breath: bool, with_angle_z: bool) -> Arc<pose::PoseMap> {
        let mut map = pose::PoseMap::new();
        let mut uid = 0_u64;
        let mut add = |map: &mut pose::PoseMap, id: &str, min: f32, max: f32, default: f32| {
            uid += 1;
            map.add(Descriptor {
                key: pose::Key::from_param(id.to_owned()),
                uid,
                name: None,
                min,
                max,
                default,
            });
        };
        if with_breath {
            add(&mut map, BREATH_PARAM, 0.0, 1.0, 0.0);
        }
        if with_angle_z {
            add(&mut map, ANGLE_Z_PARAM, -30.0, 30.0, 0.0);
        }
        add(&mut map, "ParamPlain", -10.0, 10.0, 0.0);
        Arc::new(map)
    }

    #[test]
    fn dt_must_be_finite_and_positive_and_rejection_keeps_state() {
        let mut stack = PoseStack::from_pose_map(synthetic_map(false, false));
        for bad in [0.0_f32, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let err = stack.update(bad).expect_err("非法 dt 必须被拒");
            // NaN != NaN，用位模式比较。
            assert_eq!(err.0.to_bits(), bad.to_bits());
            assert!(err.to_string().contains("非法 dt"));
        }
        assert_eq!(stack.elapsed(), 0.0, "拒绝时不得推进时钟");
        assert!(stack.update(FIXED_DT_60HZ).is_ok());
        assert!(stack.elapsed() > 0.0);
    }

    #[test]
    fn missing_parameter_is_reported_not_written() {
        let mut stack = PoseStack::from_pose_map(synthetic_map(false, false));
        assert!(!stack.set_parameter("ParamNope", 1.0));
        assert!(!stack.has_input_value("ParamNope"));
        assert_eq!(stack.get_parameter("ParamNope"), None);
        assert!(stack.set_parameter("ParamPlain", 2.5));
        assert!(stack.has_input_value("ParamPlain"));
    }

    #[test]
    fn base_defaults_then_input_overrides() {
        let mut stack = PoseStack::from_pose_map(synthetic_map(false, false));
        stack.update(FIXED_DT_60HZ).unwrap();
        // base：未设置 → 默认值 0。
        assert_eq!(stack.get_parameter("ParamPlain"), Some(0.0));
        // input 层覆盖 base。
        stack.set_parameter("ParamPlain", 7.0);
        assert_eq!(stack.get_parameter("ParamPlain"), Some(7.0));
    }

    #[test]
    fn idle_drives_breath_and_is_not_stuck_after_clear() {
        // 回归：旧实现里 idle 被并入持久累积态后，一旦设置过同键用户值，
        // clear 也无法让 idle 恢复。新实现每帧重算 idle 层。
        let mut stack = PoseStack::from_pose_map(synthetic_map(true, true));

        // t=0 时呼吸正弦取最小值 0。
        stack.update(FIXED_DT_60HZ).unwrap();
        let t0 = stack.get_parameter(BREATH_PARAM).unwrap();
        assert!((0.0..=1.0).contains(&t0), "呼吸值应在 [min,max] 内：{t0}");

        // 用户覆盖呼吸 → 生效（input > idle）。
        stack.set_parameter(BREATH_PARAM, 0.75);
        assert_eq!(stack.get_parameter(BREATH_PARAM), Some(0.75));

        // 撤销后下一帧恢复 idle 驱动值（不再是卡死的 0.75）。
        assert!(stack.clear_parameter(BREATH_PARAM));
        assert!(!stack.has_input_value(BREATH_PARAM));
        stack.update(FIXED_DT_60HZ).unwrap();
        let resumed = stack.get_parameter(BREATH_PARAM).unwrap();
        assert!((0.0..=1.0).contains(&resumed));
    }

    #[test]
    fn override_layer_beats_everything_and_clear_restores() {
        let mut stack = PoseStack::from_pose_map(synthetic_map(true, false));
        stack.update(FIXED_DT_60HZ).unwrap();

        stack.set_parameter("ParamPlain", 3.0);
        assert!(stack.override_parameter("ParamPlain", -9.0));
        assert_eq!(stack.get_parameter("ParamPlain"), Some(-9.0));

        // 同键三层并存：override > input > idle/base。
        assert!(stack.has_override_value("ParamPlain"));
        assert!(stack.has_input_value("ParamPlain"));

        // 撤销 override → 回落到 input 层的 3.0。
        assert!(stack.clear_override_parameter("ParamPlain"));
        assert_eq!(stack.get_parameter("ParamPlain"), Some(3.0));
    }

    /// **P0-2 回归（同帧先后写）**：动作预设写 `final_override`，待机微表情写
    /// input —— 即使 idle 在同一帧**更晚**写，也不能盖掉表情的嘴/眉。
    ///
    /// 复刻 `l2d-wasm-demo/src/web/surface/input.rs` 的真实顺序：
    /// 先 `override_parameter`（预设）…… 后 `set_parameter`（`apply_idle_life`）。
    /// 旧实现把预设也写 input 层，于是后写的 idle 会赢（表情闪一下又变平）。
    #[test]
    fn idle_micro_expression_written_after_preset_cannot_override_it() {
        let mut map = pose::PoseMap::new();
        for (i, id) in ["ParamMouthForm", "ParamBrowLY", "ParamBrowRY"]
            .into_iter()
            .enumerate()
        {
            map.add(Descriptor {
                key: pose::Key::from_param(id.to_owned()),
                uid: i as u64 + 1,
                name: None,
                min: -10.0,
                max: 10.0,
                default: 0.0,
            });
        }
        let mut stack = PoseStack::from_pose_map(Arc::new(map));
        stack.update(FIXED_DT_60HZ).unwrap();

        // 同帧顺序：预设（final_override）在前，idle 微表情（input）在后。
        stack.override_parameter("ParamMouthForm", 1.0);
        stack.override_parameter("ParamBrowLY", 0.6);
        stack.set_parameter("ParamMouthForm", 0.5);
        stack.set_parameter("ParamBrowLY", -0.05);

        assert_eq!(
            stack.get_parameter("ParamMouthForm"),
            Some(1.0),
            "idle 后写不得盖掉表情嘴形"
        );
        assert_eq!(
            stack.get_parameter("ParamBrowLY"),
            Some(0.6),
            "idle 后写不得盖掉表情眉"
        );

        // 到点整批撤销 → 回到 idle/input 层的值（预设不再持有 override）。
        assert!(stack.clear_override_parameter("ParamMouthForm"));
        assert!(stack.clear_override_parameter("ParamBrowLY"));
        assert_eq!(stack.get_parameter("ParamMouthForm"), Some(0.5));
        assert_eq!(stack.get_parameter("ParamBrowLY"), Some(-0.05));
        assert!(!stack.has_override_value("ParamMouthForm"));
    }

    #[test]
    fn idle_recomputes_each_frame_so_values_advance() {
        let mut stack = PoseStack::from_pose_map(synthetic_map(true, true));
        stack.update(FIXED_DT_60HZ).unwrap();
        let a = stack.get_parameter(ANGLE_Z_PARAM).unwrap();
        stack.update(FIXED_DT_60HZ).unwrap();
        stack.update(FIXED_DT_60HZ).unwrap();
        let b = stack.get_parameter(ANGLE_Z_PARAM).unwrap();
        // 正弦摆头随时间推进（t≈0 与 t≈3dt 的 sin 值不同）。
        assert!(a != b, "idle 应逐帧重算而非冻结：{a} vs {b}");
        assert!(a.abs() <= 15.0 && b.abs() <= 15.0);
    }

    /// 最小物理描述：ParamAngleZ（Angle 输入）→ 单摆锤（顶点 0 为根，
    /// 顶点 1 为摆锤）→ ParamHairFront（输出，指向顶点 1）。
    const SYNTHETIC_PHYSICS_JSON: &str = r#"{
        "Version": 3,
        "Meta": {
            "PhysicsSettingCount": 1,
            "TotalInputCount": 1,
            "TotalOutputCount": 1,
            "VertexCount": 1,
            "Fps": 60,
            "EffectiveForces": {"Gravity": {"X": 0, "Y": -1}, "Wind": {"X": 0, "Y": 0}},
            "PhysicsDictionary": []
        },
        "PhysicsSettings": [{
            "Id": "PhysicsSetting1",
            "Input": [{"Source": {"Target": "Parameter", "Id": "ParamAngleZ"},
                        "Weight": 1, "Type": "Angle", "Reflect": false}],
            "Output": [{"Destination": {"Target": "Parameter", "Id": "ParamHairFront"},
                         "VertexIndex": 1, "Scale": 1, "Weight": 1, "Reflect": false}],
            "Vertices": [{"Mobility": 0, "Delay": 0, "Acceleration": 0, "Radius": 10},
                          {"Mobility": 1, "Delay": 0.5, "Acceleration": 1, "Radius": 10}],
            "Normalization": {
                "Position": {"Minimum": -10, "Default": 0, "Maximum": 10},
                "Angle": {"Minimum": -10, "Default": 0, "Maximum": 10}
            }
        }]
    }"#;

    #[test]
    fn physics_layer_beats_input_but_final_override_beats_physics() {
        // 参数表：摆头（物理输入）+ 头发前摆（物理输出）。
        let mut map = pose::PoseMap::new();
        for (i, (id, min, max)) in [
            (ANGLE_Z_PARAM, -30.0_f32, 30.0_f32),
            ("ParamHairFront", -10.0, 10.0),
        ]
        .into_iter()
        .enumerate()
        {
            map.add(Descriptor {
                key: pose::Key::from_param(id.to_owned()),
                uid: i as u64 + 1,
                name: None,
                min,
                max,
                default: 0.0,
            });
        }
        let mut stack = PoseStack::from_pose_map(Arc::new(map));
        assert!(!stack.has_physics());
        stack
            .attach_physics_json(SYNTHETIC_PHYSICS_JSON.as_bytes())
            .expect("合成 physics3.json 应可解析");
        assert!(stack.has_physics());

        // input 层写死头发参数，同时给物理一个非零输入并步进数帧。
        stack.set_parameter("ParamHairFront", 5.0);
        stack.set_parameter(ANGLE_Z_PARAM, 30.0);
        for _ in 0..12 {
            stack.update(FIXED_DT_60HZ).unwrap();
        }
        let driven = stack.get_parameter("ParamHairFront").unwrap();
        assert!(
            (driven - 5.0).abs() > 1e-3,
            "physics 层（优先级 3）应覆盖 input 层（优先级 2）：{driven}"
        );

        // final_override（优先级 4）仍压过物理输出。
        stack.override_parameter("ParamHairFront", -4.0);
        assert_eq!(stack.get_parameter("ParamHairFront"), Some(-4.0));
        assert!(stack.clear_override_parameter("ParamHairFront"));
    }

    /// **待机开关（回归）**：关掉 idle 后连续 `update`，呼吸与 `ParamAngleZ`
    /// 都回到 base 默认值（不再写呼吸正弦、不再摆 ±3°）。
    #[test]
    fn idle_disabled_freezes_breath_and_angle_z_at_default() {
        let mut stack = PoseStack::from_pose_map(synthetic_map(true, true));
        assert!(stack.idle_enabled(), "缺省应为开");
        stack.set_idle_enabled(false);
        assert!(!stack.idle_enabled());
        for _ in 0..30 {
            stack.update(FIXED_DT_60HZ).unwrap();
            assert_eq!(
                stack.get_parameter(ANGLE_Z_PARAM),
                Some(0.0),
                "关待机后 ParamAngleZ 必须保持默认"
            );
            assert_eq!(
                stack.get_parameter(BREATH_PARAM),
                Some(0.0),
                "关待机后 ParamBreath 必须保持默认"
            );
        }
    }

    /// **待机开关（回归）**：开着时会离开默认——否则上一条断言可能是在测一个
    /// 从来不动的参数（假绿灯）。
    #[test]
    fn idle_enabled_moves_angle_z_off_default() {
        let mut stack = PoseStack::from_pose_map(synthetic_map(true, true));
        let mut left_default = false;
        for _ in 0..60 {
            stack.update(FIXED_DT_60HZ).unwrap();
            if stack.get_parameter(ANGLE_Z_PARAM) != Some(0.0) {
                left_default = true;
                break;
            }
        }
        assert!(left_default, "开待机时 ParamAngleZ 应离开默认");
    }

    /// **物理跟导演（回归）**：`final_override` 上的 `ParamAngleZ` 进物理输入，
    /// 且同一参数上 `final_override` 赢过 idle 的 ±3°。
    ///
    /// 合成物理：`ParamAngleZ`（Angle 输入）→ 单摆锤 → `ParamHairFront`（输出）。
    #[test]
    fn final_override_angle_z_reaches_physics_and_outranks_idle() {
        fn phys_stack() -> PoseStack {
            let mut map = pose::PoseMap::new();
            for (i, (id, min, max)) in [
                (ANGLE_Z_PARAM, -30.0_f32, 30.0_f32),
                ("ParamHairFront", -10.0, 10.0),
            ]
            .into_iter()
            .enumerate()
            {
                map.add(Descriptor {
                    key: pose::Key::from_param(id.to_owned()),
                    uid: i as u64 + 1,
                    name: None,
                    min,
                    max,
                    default: 0.0,
                });
            }
            let mut stack = PoseStack::from_pose_map(Arc::new(map));
            stack
                .attach_physics_json(SYNTHETIC_PHYSICS_JSON.as_bytes())
                .expect("合成 physics3.json 应可解析");
            stack
        }

        // 对照：关待机、无 override → 物理输入恒为默认 0，头发不被动。
        let mut cold = phys_stack();
        cold.set_idle_enabled(false);
        for _ in 0..120 {
            cold.update(FIXED_DT_60HZ).unwrap();
        }
        assert_eq!(
            cold.get_parameter("ParamHairFront"),
            Some(0.0),
            "无输入时物理不该动（证明下一步的位移来自 final_override）"
        );

        // 关待机 + override `ParamAngleZ=30`：物理输入只能来自 final_override，
        // 头发被带着走（若 final_override 不进物理，输入恒为默认 0，头发不会动）。
        let mut off = phys_stack();
        off.set_idle_enabled(false);
        off.override_parameter(ANGLE_Z_PARAM, 30.0);
        for _ in 0..120 {
            off.update(FIXED_DT_60HZ).unwrap();
        }
        let driven = off.get_parameter("ParamHairFront").unwrap();
        assert!(
            (driven - 0.0).abs() > 1e-3,
            "final_override 上的 ParamAngleZ 必须进物理输入：{driven}"
        );

        // 开待机 + 同样的 override：override 赢过 ±3°，物理输出应与关待机逐帧一致。
        //
        // **判据必须避开物理角度归一化的钳位干扰**：归一化上限是 10，若两侧都用
        // 30，30±3 都会被钳成同一个值，即使 idle 顶掉 override 也看不出差别
        // （原实现的漏洞）。这里改用**落在 [-10,10] 内**的 5.0：idle 的 ±3 也在
        // 量程内、不会被钳。若 idle 赢，开待机的物理输入会是时变的 ±3 正弦，输出
        // 与关待机的常量 5.0 **不可能逐位相等**；只有 override 真赢（两侧输入都是
        // 常量 5.0）才相等。
        let mut off_small = phys_stack();
        off_small.set_idle_enabled(false);
        off_small.override_parameter(ANGLE_Z_PARAM, 5.0);
        for _ in 0..120 {
            off_small.update(FIXED_DT_60HZ).unwrap();
        }
        let mut on = phys_stack();
        on.override_parameter(ANGLE_Z_PARAM, 5.0);
        for _ in 0..120 {
            on.update(FIXED_DT_60HZ).unwrap();
        }
        assert_eq!(
            on.get_parameter("ParamHairFront").unwrap(),
            off_small.get_parameter("ParamHairFront").unwrap(),
            "final_override（5.0，未触钳位）应赢过 idle 的 ±3°：两条轨迹必须逐位相等"
        );
    }
}
