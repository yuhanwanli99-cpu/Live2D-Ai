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
    "smile",
    "unhappy",
    "surprised",
    "thinking",
    "nod",
    "shake",
    "look_left",
    "look_right",
    "look_up",
    "look_down",
    "tilt_left",
    "tilt_right",
];

// ── `unhappy` 的两极（单一真源：资产；这里是 fallback 的逐值副本） ──

/// `unhappy` low 极（= 难过相）。与 `assets/actions/presets.json` 逐值一致。
const UNHAPPY_LOW: &[(&str, f32)] = &[
    ("ParamMouthForm", -1.0),
    ("ParamBrowLY", 0.4),
    ("ParamBrowRY", 0.4),
    ("ParamEyeLOpen", 0.65),
    ("ParamEyeROpen", 0.65),
    ("ParamAngleX", -2.0),
    ("ParamAngleY", -8.0),
    ("ParamAngleZ", 5.0),
    ("ParamBodyAngleX", -0.8),
    ("ParamBodyAngleY", -3.0),
];

/// `unhappy` high 极（= 生气相）。与资产逐值一致。
const UNHAPPY_HIGH: &[(&str, f32)] = &[
    ("ParamMouthForm", -0.9),
    ("ParamBrowLY", -0.95),
    ("ParamBrowRY", -0.95),
    ("ParamEyeLOpen", 1.1),
    ("ParamEyeROpen", 1.1),
    ("ParamAngleX", 3.0),
    ("ParamAngleY", 2.0),
    ("ParamAngleZ", 0.0),
    ("ParamBodyAngleX", 1.0),
    ("ParamBodyAngleY", 0.7),
];

/// `unhappy` 的 morph 描述（low 极 = `UNHAPPY_LOW`）。
const UNHAPPY_MORPH: PresetMorph = PresetMorph {
    low: 1.0,
    high: 3.0,
    high_params: UNHAPPY_HIGH,
};

/// 内置预设表（fallback）。
///
/// 参数名取 Cubism 标准命名。表情包 = 五官 + 小幅头身（头为主、身约头 1/3）；
/// 手势包主轴 = 头 ±12、身 ±3.9（表内身/头 = 0.325）——标定口径见 `scales.rs`
/// 顶部；出厂倍率 head 0.75 / body 0.80 / expression 1.0 之后，
/// look 头 9.0°/身 3.12°、nod 头 9.0°/身 3.12°、shake 头 8.36°/身 2.90°。
pub const PRESETS: &[PresetSpec] = &[
    PresetSpec {
        id: "smile",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        params: &[
            ("ParamMouthForm", 1.0),
            ("ParamEyeLSmile", 1.0),
            ("ParamEyeRSmile", 1.0),
            ("ParamBrowLY", 0.5),
            ("ParamBrowRY", 0.5),
            ("ParamAngleY", 6.0),
            ("ParamAngleZ", 2.0),
            ("ParamBodyAngleY", 2.0),
        ],
    },
    PresetSpec {
        id: "unhappy",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: Some(&UNHAPPY_MORPH),
        params: UNHAPPY_LOW,
    },
    PresetSpec {
        id: "surprised",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        params: &[
            // 2026-09-24：1.3 → 1.26。五官运行期上限 4，intensity 走满 3 时
            // 1.3 × 3 = 3.9 > 4 × 0.95 = 3.8（会钳位）；1.26 × 3 = 3.78 ✔。
            ("ParamEyeLOpen", 1.26),
            ("ParamEyeROpen", 1.26),
            ("ParamBrowLY", 0.8),
            ("ParamBrowRY", 0.8),
            ("ParamMouthForm", 0.2),
            ("ParamAngleY", 5.0),
            ("ParamAngleX", -3.0),
            ("ParamBodyAngleY", 2.0),
            ("ParamBodyAngleX", -1.0),
        ],
    },
    PresetSpec {
        id: "thinking",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // T9（2026-10-07）：五官（抿嘴 / 半垂眼 / 眉下压）+ 小幅歪头。
        // 字段通道只抄五官五行（field_map.rs），头角留给 preset_id 通道。
        params: &[
            ("ParamMouthForm", 0.0),
            ("ParamEyeLOpen", 0.55),
            ("ParamEyeROpen", 0.55),
            ("ParamBrowLY", -0.45),
            ("ParamBrowRY", -0.45),
            ("ParamAngleZ", 6.0),
            ("ParamBodyAngleZ", 2.0),
        ],
    },
    // ---- 短动作（motion）：头 + 半身随动，方向保守但肉眼明显 ----
    PresetSpec {
        id: "nod",
        kind: PresetKind::Motion,
        duration_ms: MOTION_MS,
        // 头 -12 / 身 -3.9：同向，表内身/头 = 0.325（W2 标定，见 scales.rs）。
        params: &[("ParamAngleY", -12.0), ("ParamBodyAngleY", -3.9)],
        wave: MotionWave::Single,
        morph: None,
    },
    PresetSpec {
        id: "shake",
        kind: PresetKind::Motion,
        duration_ms: MOTION_MS,
        // 头 X 12 / 身 X 3.9（主轴，表内身/头 = 0.325）；次轴 Z 保持原比例
        // （Z/X 由 4/22 降到 2.2/12，身 Z 0.8）。**同相**振荡，
        // 波形 = sin(2*pi*2*p)（2 个完整来回），峰值系数 1.0
        // （2026-10-14 起曲线有保持段；旧 sin(pi*p) 时代是 0.9285）。
        params: &[
            ("ParamAngleX", 12.0),
            ("ParamAngleZ", 2.2),
            ("ParamBodyAngleX", 3.9),
            ("ParamBodyAngleZ", 0.8),
        ],
        wave: MotionWave::Oscillate {
            cycles: SHAKE_CYCLES,
        },
        morph: None,
    },
    PresetSpec {
        id: "look_left",
        kind: PresetKind::Motion,
        duration_ms: MOTION_MS,
        params: &[
            ("ParamAngleX", 12.0),
            ("ParamAngleZ", 2.7),
            ("ParamBodyAngleX", 3.9),
            ("ParamBodyAngleZ", 0.8),
        ],
        wave: MotionWave::Single,
        morph: None,
    },
    PresetSpec {
        id: "look_right",
        kind: PresetKind::Motion,
        duration_ms: MOTION_MS,
        params: &[
            ("ParamAngleX", -12.0),
            ("ParamAngleZ", -2.7),
            ("ParamBodyAngleX", -3.9),
            ("ParamBodyAngleZ", -0.8),
        ],
        wave: MotionWave::Single,
        morph: None,
    },
    PresetSpec {
        id: "look_up",
        kind: PresetKind::Motion,
        duration_ms: LOOK_MS,
        // T9：主轴 Y（头 +12 / 身 +3.9，表内身/头 = 0.325），次轴 X 保持小比例。
        params: &[
            ("ParamAngleY", 12.0),
            ("ParamBodyAngleY", 3.9),
            ("ParamAngleX", 2.7),
            ("ParamBodyAngleX", 0.8),
        ],
        wave: MotionWave::Single,
        morph: None,
    },
    PresetSpec {
        id: "look_down",
        kind: PresetKind::Motion,
        duration_ms: LOOK_MS,
        // T9：look_up 的镜像（全轴取负）。
        params: &[
            ("ParamAngleY", -12.0),
            ("ParamBodyAngleY", -3.9),
            ("ParamAngleX", -2.7),
            ("ParamBodyAngleX", -0.8),
        ],
        wave: MotionWave::Single,
        morph: None,
    },
    PresetSpec {
        id: "tilt_left",
        kind: PresetKind::Motion,
        duration_ms: 1_000.0,
        params: &[
            ("ParamAngleZ", 12.0),
            ("ParamAngleX", 3.0),
            ("ParamBodyAngleZ", 3.9),
        ],
        wave: MotionWave::Single,
        morph: None,
    },
    PresetSpec {
        id: "tilt_right",
        kind: PresetKind::Motion,
        duration_ms: 1_000.0,
        params: &[
            ("ParamAngleZ", -12.0),
            ("ParamAngleX", -3.0),
            ("ParamBodyAngleZ", -3.9),
        ],
        wave: MotionWave::Single,
        morph: None,
    },
];

/// 按 id 查**内建**预设；未知 / `none` → `None`。
// 内建表查找：wasm 侧改走 PresetRuntime::resolve，留着是给测试与 API 完整性。
#[allow(dead_code)]
pub fn preset(id: &str) -> Option<&'static PresetSpec> {
    PRESETS.iter().find(|p| p.id == id)
}
