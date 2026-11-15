//! `[action]` 段：动作幅度倍率（2026-09-16 用户可调；每模型覆盖在阶段5 D40 加入）。
//!
//! 从 `settings.rs` 拆出：那里贴着 code-stats 的 `>1000 行` 棘轮（上限 0），
//! 而这一段的注释（量程重标定的数值账）比代码长得多。

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

/// 动作幅度倍率的钳位下限（2026-09-16）。
pub const MIN_ACTION_SCALE: f32 = 0.2;
/// 动作幅度倍率的钳位上限（2026-09-16；**2026-09-24 重标定 2.5 → 2.2**）。
///
/// 旧上限下 body 滑条对 `nod` / `shake` / `look_*` 在 1.43 以上完全无效
///（出厂 1.4 已吃掉 96–98% 行程，见 RESEARCH §2.1）。收到 2.2 后，
/// 渲染面表值（手势主轴头 12 / 身 3.9）使**每个旋钮单独走满都不触上限**
///（头 12 × 0.9285 × 2.2 ≤ 30 × 0.95；身 3.9 × 2.2 ≤ 10 × 0.95）。
/// 数值账与「两个旋钮同时拉满会钳位的组合」见 `l2d-wasm-demo/src/preset/scales.rs`。
pub const MAX_ACTION_SCALE: f32 = 2.2;

/// 出厂默认头摆倍率（相对渲染面预设表内的幅值）。
pub const DEFAULT_HEAD_SCALE: f32 = 0.75;
/// 出厂默认身摆倍率（相对表内幅值）。
///
/// **2026-09-24 重标定 1.4 → 0.80**：旧值把身摆幅顶到 ±9.8（上限 10），
/// 出厂身/头比从表内 0.325 被放大到 0.65（躯干比头还显眼）；0.80 后
/// 出厂身/头比 ≈ 0.347，回到设计口径 [0.30, 0.50]。
pub const DEFAULT_BODY_SCALE: f32 = 0.80;
/// 出厂默认表情倍率。
pub const DEFAULT_EXPRESSION_SCALE: f32 = 1.0;

/// 把任意 f32 归一化进 `[MIN_ACTION_SCALE, MAX_ACTION_SCALE]`。
///
/// NaN / 无穷 → 回落 1.0 再钳（配置出错不该把动作整体关掉）。
#[must_use]
pub fn clamp_action_scale(value: f32) -> f32 {
    if !value.is_finite() {
        return 1.0;
    }
    value.clamp(MIN_ACTION_SCALE, MAX_ACTION_SCALE)
}

fn default_head_scale() -> f32 {
    DEFAULT_HEAD_SCALE
}
fn default_body_scale() -> f32 {
    DEFAULT_BODY_SCALE
}
fn default_expression_scale() -> f32 {
    DEFAULT_EXPRESSION_SCALE
}

/// `[action]` 段：用户可调的三项动作幅度倍率（2026-09-16）。
///
/// **真源在本文件（`live2d-ai.toml`）**，经 `GET/PATCH /api/v1/settings` 暴露，
/// 由 Flutter 设置面板的滑条编辑；渲染面在写 preset 帧时按参数归属乘对应倍率。
/// 前端/调试面板的临时覆盖**不落盘**，只影响当前会话。
///
/// 基准（文档写清，避免「改完不知道为什么变成这样」）：
/// - 渲染面预设表内的幅值是**基准**（头 `ParamAngle*` ≤30、身 `ParamBodyAngle*` ≤10）；
/// - 实际写入 = 表值 × 峰值系数 × intensity × 对应倍率，再按通道红线钳位
///   （头 ≤30、身 ≤10、五官 ≤4）。**两个乘法旋钮**各自走满都不触上限，
///   但**同时**拉满会钳位——会钳位的组合表见
///   `crates/l2d-wasm-demo/src/preset/scales.rs` 顶部注释。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionSettings {
    /// 头角度（`ParamAngle*`）倍率。
    #[serde(default = "default_head_scale")]
    pub head_scale: f32,
    /// 身角度（`ParamBodyAngle*`）倍率。
    #[serde(default = "default_body_scale")]
    pub body_scale: f32,
    /// 表情（口 / 眉 / 眼）倍率。
    #[serde(default = "default_expression_scale")]
    pub expression_scale: f32,
    /// **单模型覆盖表**（`[action.models.<model_id>]`，阶段5 D40，2026-09-26）。
    ///
    /// 键 = 模型 id（`active_model_id` 的原样字符串），值 = 三键**各自可选**的
    /// 覆盖；未覆盖的键**逐键回落**全局（见 [`ActionSettings::effective_for`]）。
    /// 空表整体省略（`skip_serializing_if`），因此没配覆盖的 toml 与改动前逐字一致。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub models: BTreeMap<String, ActionModelOverride>,
}

/// `[action.models.<model_id>]` 的单模型覆盖（阶段5 D40，2026-09-26）。
///
/// 三键都是 `Option<f32>`：`None` = **该键不覆盖**（回落全局），
/// `Some(v)` = 显式覆盖（写回前经 [`clamp_action_scale`] 钳进 `[0.2, 2.2]`）。
/// `deny_unknown_fields` 与 `[action]` 同口径——拼错的键启动即报错。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ActionModelOverride {
    /// 头角度覆盖；`None` = 跟随全局。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_scale: Option<f32>,
    /// 身角度覆盖；`None` = 跟随全局。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_scale: Option<f32>,
    /// 表情覆盖；`None` = 跟随全局。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression_scale: Option<f32>,
}

/// 某模型 id 的**最终生效**三项倍率（全局与覆盖逐键合并后；已在 `[0.2, 2.2]` 内）。
///
/// 这是前端 `ActionScalesSyncer` 下发的唯一数值来源；**渲染面零改动**。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct EffectiveActionScales {
    /// 生效头摆倍率。
    pub head_scale: f32,
    /// 生效身摆倍率。
    pub body_scale: f32,
    /// 生效表情倍率。
    pub expression_scale: f32,
}

/// 模型 id 的最大长度（字节；id 只允许 ASCII，故字节 = 字符）。
pub const MAX_MODEL_ID_LEN: usize = 64;

/// `[action.models.<id>]` 的 id 合法性（**纯函数**，patch 层据此拒绝非法 id）。
///
/// 规则：非空、长度 ≤ [`MAX_MODEL_ID_LEN`]、仅 ASCII 字母/数字与 `.` `_` `-`；
/// 另外拒绝含 `..` 的 id（`".."` / `"a..b"`）——它们看着像路径，既不该成为
/// TOML 键，也不该被当成模型目录名。
#[must_use]
pub fn is_valid_model_id(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_MODEL_ID_LEN {
        return false;
    }
    if id.contains("..") {
        return false;
    }
    id.bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

impl Default for ActionSettings {
    fn default() -> Self {
        Self {
            head_scale: DEFAULT_HEAD_SCALE,
            body_scale: DEFAULT_BODY_SCALE,
            expression_scale: DEFAULT_EXPRESSION_SCALE,
            models: BTreeMap::new(),
        }
    }
}

impl ActionSettings {
    /// 归一化后的三项倍率 + 每模型覆盖（全部钳进 `[0.2, 2.2]`）。
    ///
    /// 全局三键的语义**不变**；同时对 `models` 里每个 override 的 `Some` 值
    /// 逐键 `clamp_action_scale`（NaN / 无穷 → 1.0 再钳，与全局同一口径）。
    /// `None` 保持 `None`（= 不覆盖，继续回落全局）。
    #[must_use]
    pub fn normalized(&self) -> Self {
        Self {
            head_scale: clamp_action_scale(self.head_scale),
            body_scale: clamp_action_scale(self.body_scale),
            expression_scale: clamp_action_scale(self.expression_scale),
            models: self
                .models
                .iter()
                .map(|(id, ov)| {
                    (
                        id.clone(),
                        ActionModelOverride {
                            head_scale: ov.head_scale.map(clamp_action_scale),
                            body_scale: ov.body_scale.map(clamp_action_scale),
                            expression_scale: ov.expression_scale.map(clamp_action_scale),
                        },
                    )
                })
                .collect(),
        }
    }

    /// 某模型 id 的**最终生效**三项倍率：**逐键** `override.or(global)`。
    ///
    /// 内部先走 [`Self::normalized`]，所以返回的三个值一定在 `[0.2, 2.2]` 内；
    /// `model_id` 不在 `models` 里（或某键为 `None`）→ 该键跟随全局。
    #[must_use]
    pub fn effective_for(&self, model_id: &str) -> EffectiveActionScales {
        let g = self.normalized();
        let ov = g.models.get(model_id);
        EffectiveActionScales {
            head_scale: ov.and_then(|o| o.head_scale).unwrap_or(g.head_scale),
            body_scale: ov.and_then(|o| o.body_scale).unwrap_or(g.body_scale),
            expression_scale: ov
                .and_then(|o| o.expression_scale)
                .unwrap_or(g.expression_scale),
        }
    }
}
