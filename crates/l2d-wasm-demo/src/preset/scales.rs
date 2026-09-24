//! 用户可调幅度 + 通道红线 + 最终下发（从 preset/mod.rs 拆出，见 AGENTS 行数纪律）。
//!
//! 头 / 身 / 表情三条倍率、通道量程红线（含 v3 的**表情包小幅**红线常量）
//! 与最终下发（基准值 × 包络 × 倍率 → 钳位）都在这里。

use super::{PresetSpec, base_params_for_intensity, pack_gain};

// ─────────────────────────────────────────────────────────────────────
// 用户可调幅度（2026-09-16）
// ─────────────────────────────────────────────────────────────────────

/// 通道红线：头角度（`ParamAngle*`）的量程上限（手势包）。
pub const HEAD_LIMIT: f32 = 30.0;
/// 通道红线：半身角度（`ParamBodyAngle*`）的量程上限（手势包）。
pub const BODY_LIMIT: f32 = 10.0;
/// 表情通道的兜底上下限（只防失控值，不参与「标准量程」语义）。
pub const EXPRESSION_LIMIT: f32 = 4.0;
/// **表情包**的头部位移上限（v3 红线：表情不是大姿态，忌 ±24 抽帧感）。
pub const EXPRESSION_HEAD_LIMIT: f32 = 12.0;
/// **表情包**的半身随动上限（v3 红线；约头的 1/3）。
pub const EXPRESSION_BODY_LIMIT: f32 = 4.0;
/// 倍率下限（与 `live2d_ai_runtime::settings::MIN_ACTION_SCALE` 同口径）。
pub const MIN_SCALE: f32 = 0.2;
/// 倍率上限（与 `live2d_ai_runtime::settings::MAX_ACTION_SCALE` 同口径）。
///
/// 2026-09-24 重标定：2.5 → **2.2**。旧上限下 body 滑条对 `nod` / `shake` / `look_*`
/// 在 1.43 以上完全无效（出厂 1.4 已吃掉 96–98% 行程，见 RESEARCH §2.1）；
/// 收到 2.2 后**每个旋钮单独走满都不碰红线**（表值 × 峰值 × 2.2 ≤ 上限 × 0.95）。
pub const MAX_SCALE: f32 = 2.2;

// ─────────────────────────────────────────────────────────────────────
// 幅值标定（2026-09-24）：出厂组合 / 两个旋钮各自的红线账
// ─────────────────────────────────────────────────────────────────────
//
// 公式（不变）：`最终值 = clamp_to_channel(id, 表值 × 峰值系数 × intensity × 通道倍率)`。
// **有两个乘法旋钮**：intensity（调试面板 0.5–3.0；表演层 cue 合法区间 1..=3）与
// 通道倍率（滑条 0.2–MAX_SCALE）。上限是死的（头 30 / 身 10 / 五官 4）。
//
// 因此**不可能**要求「两个旋钮同时拉满也不触上限」：那要求
// `表值 ≤ 上限 / (3 × 2.2) ≈ 4`，默认摆幅会小到看不见。
// 本表的验收口径是**每个旋钮独立走满都不触上限**（留 5% 余量）：
//   ① 出厂组合（峰值系数：Single = 1.0；Oscillate 取 |包络 × 波形| 的数值上确界，
//      shake(cycles=2) = 0.9285，由回归实测）：
//      `|表值 × 峰值 × 出厂倍率| ≤ 上限 × 0.60`
//   ② scale 旋钮走满：`|表值 × 峰值 × MAX_SCALE| ≤ 上限 × 0.95`
//   ③ intensity 旋钮走满（方案 (i)：下调表值）：
//      `|表值 × 峰值 × 3 × 出厂倍率| ≤ 上限 × 0.95`
//      （morph 包 intensity=3 取 high 极、不乘 3；这里一律乘 3 是**保守**口径。）
//
// 峰值在手势主轴上给出的**死区起点**（= 上限 × 0.95 / (表值 × 峰值)，再拖就钳死）：
//   head 主轴 `ParamAngleX/Y/Z` = 12.0 → 2.375（Single：nod / look_* / tilt_*）/ 2.558（shake，含 0.9285）
//   body 主轴 `ParamBodyAngleX/Y/Z` = 3.9 → 2.436（Single）/ 2.623（shake）
//   五官 `ParamEyeLOpen/ROpen`（surprised）= 1.26 → 3.016
// 全部 > MAX_SCALE(2.2)：**滑条全行程有效**。
//
// ⚠ **两个旋钮同时拉满**（intensity = 3 且 scale = 2.2）：**普通包会钳位，morph 包不会**。
// 这是刻意接受的代价（口径是上面那条「独立走满」，不是「整个二维网格都不触上限」）。
// 口径：普通包 = 表值 × 峰值 × 3 × 2.2；**morph 包**（unhappy）intensity=3 直接取 high 极、
// **不再乘 3**，故 |high 极| × 2.2 全部 ≤ 上限——unhappy 在两旋钮拉满时也不钳位。
// 会钳位的组合（回归 `both_knobs_at_max_clamp_only_the_documented_combinations` 钉住）：
//   | 包 | 通道 | 拉满需要 | 上限 |
//   | --- | --- | --- | --- |
//   | shake（主轴 ×0.9285） | ParamAngleX | 73.5 | 30 |
//   | nod（主轴 ×1.0，Single） | ParamAngleY | 79.2 | 30 |
//   | look_left/right / tilt_left/right（主轴 ×1.0） | ParamAngleX / ParamAngleZ | 79.2 | 30 |
//   | 同上四条主轴的身侧 | ParamBodyAngleX / ParamBodyAngleZ | 23.9(shake) / 25.7 | 10 |
//   | smile | ParamMouthForm / ParamEyeLSmile / ParamEyeRSmile | 6.6 | 4 |
//   | smile | ParamAngleY / ParamBodyAngleY | 39.6 / 13.2 | 30 / 10 |
//   | surprised | ParamEyeLOpen / ParamEyeROpen / ParamBrowLY / ParamBrowRY | 8.316 / 5.28 | 4 |
//   | surprised | ParamAngleY / ParamBodyAngleY | 33.0 / 13.2 | 30 / 10 |
// **不钳位**（同一口径下的反向读数）：全部手势次轴（shake Z 13.5、look Z 17.8、tilt X 19.8、
// 身次轴 ≤5.3）；smile 的 Brow（3.3）与 AngleZ（13.2）；surprised 的 MouthForm（1.32）、
// AngleX（19.8）、BodyAngleX（6.6）；**unhappy 全部通道**（最大 6.6）。
// 同一张表也写在 `assets/actions/presets.json` 的 `_doc`；`docs/architecture/action-packs-v0.md`
// 的副本由 W1/W8 在动作文档里对齐（本文件是数值真源）。

/// 参数归属：决定它乘哪一条倍率。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleClass {
    /// 头角度（`ParamAngle*`）→ `head`。
    Head,
    /// 半身角度（`ParamBodyAngle*`）→ `body`。
    Body,
    /// 表情（口 / 眉 / 眼 / 其它面部）→ `expression`。
    Expression,
}

/// 按参数 id 判定归属。
///
/// **顺序要紧**：先判 Body（前缀 `ParamBodyAngle`），再判 Head（前缀 `ParamAngle`），
/// 其余归表情。
pub fn scale_class(param_id: &str) -> ScaleClass {
    if param_id.starts_with("ParamBodyAngle") {
        ScaleClass::Body
    } else if param_id.starts_with("ParamAngle") {
        ScaleClass::Head
    } else {
        ScaleClass::Expression
    }
}

/// 按通道红线钳位（**乘完倍率之后**再钳）。
///
/// 头 ≤30、身 ≤10；表情只做兜底钳位。表情包的**小幅度**红线在解析期
/// （`pack_limit`）执行，运行期 intensity 放大不受它限制（情绪化程度是用户输入）。
pub fn clamp_to_channel(param_id: &str, value: f32) -> f32 {
    if !value.is_finite() {
        return 0.0;
    }
    match scale_class(param_id) {
        ScaleClass::Head => value.clamp(-HEAD_LIMIT, HEAD_LIMIT),
        ScaleClass::Body => value.clamp(-BODY_LIMIT, BODY_LIMIT),
        ScaleClass::Expression => value.clamp(-EXPRESSION_LIMIT, EXPRESSION_LIMIT),
    }
}

/// 用户可调的三项幅度倍率（相对表内幅值的乘数）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresetScales {
    pub head: f32,
    pub body: f32,
    pub expression: f32,
}

impl Default for PresetScales {
    fn default() -> Self {
        Self {
            head: 1.0,
            body: 1.0,
            expression: 1.0,
        }
    }
}

impl PresetScales {
    /// 出厂默认倍率（相对表内幅值）：head 0.75 / body 0.80 / expression 1.0。
    ///
    /// 与 runtime 设置 `[action]` 的缺省**逐值一致**；这里是**渲染面的兜底**。
    ///
    /// 2026-09-24 重标定：body 1.4 → **0.80**。旧值把身摆幅顶到 ±9.8（上限 10），
    /// 出厂身/头比从表内 0.325 被放大到 0.65；收到 0.80 后出厂身/头比 ≈ 0.35
    /// （表内 × body/head = 0.325 × 0.80/0.75），回到设计口径 [0.30, 0.50]。
    pub const PRODUCT_DEFAULT: PresetScales = PresetScales {
        head: 0.75,
        body: 0.80,
        expression: 1.0,
    };

    /// 逐个钳进 `[MIN_SCALE, MAX_SCALE]`（NaN / 无穷 → 1.0）。
    pub fn clamped(self) -> Self {
        Self {
            head: clamp_scale(self.head),
            body: clamp_scale(self.body),
            expression: clamp_scale(self.expression),
        }
    }

    /// 从可选的 JSON 数值解析（缺省 / 非法 → 1.0，再钳）；顺序 head / body / expression。
    pub fn from_parts(head: Option<f64>, body: Option<f64>, expression: Option<f64>) -> Self {
        Self {
            head: clamp_scale(opt_to_f32(head)),
            body: clamp_scale(opt_to_f32(body)),
            expression: clamp_scale(opt_to_f32(expression)),
        }
    }

    /// 是否等于缺省（全 1.0）。
    // 测试用（wasm 侧不需要）。
    #[allow(dead_code)]
    pub fn is_identity(self) -> bool {
        self == Self::default()
    }

    /// 取本参数该乘的倍率。
    pub fn for_param(self, param_id: &str) -> f32 {
        match scale_class(param_id) {
            ScaleClass::Head => self.head,
            ScaleClass::Body => self.body,
            ScaleClass::Expression => self.expression,
        }
    }
}

fn opt_to_f32(v: Option<f64>) -> f32 {
    v.filter(|x| x.is_finite()).map_or(1.0, |x| x as f32)
}

/// 单值倍率钳位；非有限 → 1.0。
pub fn clamp_scale(value: f32) -> f32 {
    if !value.is_finite() {
        return 1.0;
    }
    value.clamp(MIN_SCALE, MAX_SCALE)
}

/// 最终下发：基准值（普通包 × intensity / morph 包插值）× 包络 × 通道倍率，再钳通道红线。
pub fn frame_params_scaled_by(
    spec: &PresetSpec,
    progress: f64,
    intensity: f32,
    scales: PresetScales,
) -> Vec<(&'static str, f32)> {
    let gain = pack_gain(spec.kind, spec.wave, progress);
    base_params_for_intensity(spec, intensity)
        .into_iter()
        .map(|(id, v)| (id, clamp_to_channel(id, v * gain * scales.for_param(id))))
        .collect()
}
