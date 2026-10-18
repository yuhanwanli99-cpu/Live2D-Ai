//! 预设类型 / 内建 fallback 表（自 `preset/mod.rs` 机械拆出）。

/// 预设通道类型（= 槽位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetKind {
    /// 表情：走起落曲线（进 / 保持 / 出），到点整批撤销。走 `PresetSlot::Face`。
    Expression,
    /// 短动作：按起落曲线 × 波形起落（单峰或多周期），之后整批撤销。走 `PresetSlot::Gesture`。
    Motion,
}

/// 双槽：表情（五官 + 小幅头身）与手势（头身动作）可同轮并存。
///
/// 同帧写入顺序 = Face → Gesture，因此共享通道（`ParamAngle*` / `ParamBodyAngle*`）
/// 由手势包赢；两者互不撤销对方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetSlot {
    /// 表情槽（`kind="expression"`）。
    Face,
    /// 手势槽（`kind="motion"`）。
    Gesture,
}

/// kind → 槽位。
pub const fn slot_of(kind: PresetKind) -> PresetSlot {
    match kind {
        PresetKind::Expression => PresetSlot::Face,
        PresetKind::Motion => PresetSlot::Gesture,
    }
}

/// 短动作的**波形**（叠在起落曲线之上，见 `motion_curve`）。
///
/// 表情忽略波形（增益同走起落曲线）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MotionWave {
    /// 一次起落：起落曲线本身（攻击 → 保持 → 放回）。点头 / 看左右这种「去、回」用它。
    Single,
    /// 左右多周期振荡：`sin(2*pi*cycles*p)`。
    ///
    /// `cycles` = 完整来回次数（`2.0` = 左右各两次）；非有限 / <=0 → 回落 2.0。
    /// **必须叠起落曲线**，否则首帧不是 0（会跳变）。
    Oscillate { cycles: f64 },
}

/// **强度 morph 包**的高强度一极（v3）。
///
/// 低强度一极就是 `PresetSpec::params`（`PresetSpec.params` 对 morph 包是
/// low 极）。intensity 在 `[low, high]` 之间线性插值；`< low` 时从 0（中性）
/// 线性升到 low 极，`> high` 时取 high 极。两个极的参数**通道集合应一致**
/// （缺失通道按 0 处理），shipped 表由回归钉住。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresetMorph {
    /// 低强度锚点（intensity；缺省 1）。
    pub low: f32,
    /// 高强度锚点（intensity；缺省 3）。
    pub high: f32,
    /// 高强度一极的参数。
    pub high_params: &'static [(&'static str, f32)],
}

/// 一条预设的参数写入。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresetSpec {
    pub id: &'static str,
    pub kind: PresetKind,
    /// 总时长（毫秒）。表情保持得久一点，短动作一次就回来。
    pub duration_ms: f64,
    /// 目标参数（id, 幅值）。morph 包时这是 **low 极**。
    pub params: &'static [(&'static str, f32)],
    /// 短动作波形（表情忽略）。见 `MotionWave`。
    pub wave: MotionWave,
    /// 强度 morph 两极；`None` = 普通包（表值 × intensity）。
    pub morph: Option<&'static PresetMorph>,
}

/// 摇头的完整来回次数（>=2 才算「左右摆」而不是「歪一下」）。
pub const SHAKE_CYCLES: f64 = 2.0;

/// 表情保持时长（毫秒）。
pub const EXPRESSION_MS: f64 = 2_600.0;
/// 短动作时长（毫秒）。
pub const MOTION_MS: f64 = 900.0;
/// 抬头 / 低头看包时长（T9，2026-10-07；主轴是 Y 的整段位移，比 nod 稍长）。
pub const LOOK_MS: f64 = 1_100.0;
/// 深鞠躬时长（0.3.7 T2；比 nod 的 900ms 更深更慢，主轴仍是 Y）。
pub const BOW_MS: f64 = 1_600.0;

/// `id:"none"`：撤销（每个还有姿势的槽从屏幕值放回，见 `release`）。
pub const REVOKE_ID: &str = "none";
/// 强度上限（P0-1）。
pub const MAX_INTENSITY: f32 = 3.0;
/// `ttl_ms` 上限（P0-1，与导演 cue 的 ≤5000ms 同口径）。
pub const MAX_TTL_MS: f64 = 5_000.0;

/// 红线的**主 allowlist**：产品里允许直接点名的包 id（`none` 是撤销哨兵）。
///
/// 旧的 v2 id 已删除、不在任何地方解析：收到即未知 id → `PresetCommand::Ignore`。
pub const PRESET_IDS: &[&str] = &[
    REVOKE_ID,
    // 表情槽（12）：0.3.7 T2 起含 8 条新表情。
    "smile",
    "unhappy",
    "surprised",
    "thinking",
    "angry",
    "sad",
    "blush",
    "wink",
    "confused",
    "sleepy",
    "excited",
    "calm",
    // 手势槽（9）：bow = 0.3.7 T2（13 参数能诚实表达的深鞠躬）。
    "nod",
    "shake",
    "look_left",
    "look_right",
    "look_up",
    "look_down",
    "tilt_left",
    "tilt_right",
    "bow",
];

/// 内建表（fallback）：表数据在 `builtin.rs`（0.3.7 T2 拆出，AGENTS 行数纪律）。
pub use super::builtin::PRESETS;

/// 按 id 查**内建**预设；未知 / `none` → `None`。
// 内建表查找：wasm 侧改走 PresetRuntime::resolve，留着是给测试与 API 完整性。
#[allow(dead_code)]
pub fn preset(id: &str) -> Option<&'static PresetSpec> {
    PRESETS.iter().find(|p| p.id == id)
}
