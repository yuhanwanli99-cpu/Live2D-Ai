//! 舞台**动作包**表 + 运行状态机（2026-09-15，原生可测；2026-09-23 v3 包重构）。
//!
//! **行数豁免（≤1000 头注）**：本文件当前 997 行，超过 AGENTS「源码 ≤500」的常规线。
//! 豁免理由是**单一真源**：内建包表 / 协议解析 / 包络 / 强度 morph / 双槽状态机必须
//! 能被并排读到——已按行为边界把表（table）、标定（scales）、字段（field_*）、
//! 接力（handoff）、放回（release）拆出，本文件只留旧 `preset_id` 主线且 ≤1000 行。
//!
//! # 为什么要一个原生可测的模块
//!
//! 本仓库踩过两次同款坑（`mouth.rs`、`stage_bg.rs`）：纯逻辑写在 `wasm32` 才编译的
//! 文件里，`cargo test` 根本编译不到 → **没有回归**。所以预设的「id → 参数」
//! 映射、包络、时长、协议解析、强度 morph 与到点撤销全部放在这里，`wasm` 侧只负责
//! 按帧调用。
//!
//! # v3：包 = 五官 + 小幅头身；intensity 是一等公民
//!
//! - **表情包**（`smile` / `unhappy` / `surprised`，`kind="expression"`）：五官通道
//!   **加上**小幅 Angle/Body 位移（头为主、身约头的 1/3，头 ≤12、身 ≤4）。表情不再
//!   是「只有嘴和眉」——但也**不是**手势包那种大姿态（红线见 `EXPRESSION_HEAD_LIMIT`）。
//! - **手势包**（`nod` / `shake` / `look_*` / `tilt_*`，`kind="motion"`）：头 + 半身，
//!   头角可用到 ±30、身 ±10；左右方向与 intensity 正交，故左右各自保留一条。
//! - **intensity ∈ [0,3]**：普通包 = 表值 × intensity（五官 + 头身一起缩放）；
//!   **morph 包**（`PresetMorph`，目前只有 `unhappy`）= 在 low/high 两极之间按
//!   intensity 插值（1=难过相、3=生气相），intensity 不再二次相乘。
//! - **双槽**：`PresetSlot::Face` 与 `PresetSlot::Gesture` 可**同轮并存**
//!   （微笑 + 点头）。同槽换包**从换下去那一帧接上**（`handoff`），`id:"none"`
//!   按槽**放回**（`release`）到点才清；同帧写入顺序 Face → Gesture，共享头/身由手势赢。
//!
//! # P1-1：表外置（2026-09-16）
//!
//! 内建 PRESETS 仍是 **fallback**；运行时优先加载 assets/actions/presets.json
//! （经桌面端 /actions/* 静态路由取到），由 PresetTable::from_json 解析 + 校验
//! （通道白名单 / 幅值分档 / 未知通道丢通道 + warn）。改 JSON 的幅值 / 周期 / 时长
//! **不需要重编 Rust**，重开页面即生效；加载失败回退内建表并在状态栏说清。
//!
//! # 旧 id 已删除：未知 → Ignore（2026-09-23）
//!
//! v2 的旧 id（含资产 `deprecated`、内建别名表、导演旧 id 表）已**整体删除**。
//! 收到旧 id 与收到任意未知名等价：`PresetCommand::Ignore`——**不动任何参数**、不回执。
//! 产品里只认 `PRESET_IDS`（主 allowlist）与 `assets/actions/presets.json`。
//!
//! # 文件划分（AGENTS.md 行数纪律：源码 ≤500 行）
//!
//! - `mod.rs`（本文件）：类型 / 内建 fallback 表 / 协议解析 / 包络 / 强度 morph /
//!   双槽到点撤销状态机；
//! - `table.rs`：外置 JSON → 表 + 校验；
//! - `handoff.rs`：同槽换动作时的接力插值（2026-10-15）；
//! - `release.rs`：`Revoke` 的放回（2026-10-15）；
//! - `scales.rs`：通道红线、用户可调倍率、最终下发；
//! - `tests/`：协议（`mod.rs` 内联）+ 双槽 / morph（`packs.rs`）+ 资产（`assets.rs`）。
//!
//! # 通道（严格按任务书）
//!
//! 只做标准皮套的两条常见通道：
//! - **表情**（expression）：进 / 保持 / 出与手势同一条起落曲线；
//! - **短动作**（motion）：带包络的短动作（`nod` / `shake` / `look_*` / `tilt_*`）。
//!
//! **不做**手臂 / 手指 / 特效 / 第二路 LLM / motion3 / exp3。缺参数的皮套由
//! 渲染层静默降级（未知参数 `override_parameter` 返回 `false`，不报错）。
//!
//! # 半身随动 + 起落曲线（2026-09-16 L1 / 2026-10-14 裁决）
//!
//! 手势包一律头 + 身同向，主轴**表内身/头 = 0.325**（设计口径 [0.30, 0.50]；乘出厂
//! 倍率 0.80/0.75 后 ≈ 0.347）；摇头用 `MotionWave::Oscillate`（`sin(2*pi*cycles*p)`，
//! >=2 个完整来回）叠加同一个起落曲线，**首末仍为 0**。起落不再用单峰 `sin(pi*p)`：
//! 改成 `motion_curve` 的**攻击（0 → 0.22）→ 保持（→ 0.62）→ 放回（→ 1）**三段
//! smootherstep，保持段恰好 1、两端值与斜率都是 0——**表情与手势同一条曲线**；
//! `Single` 的幅值乘子就是它（不再等于 `envelope`）。**身体滞后**：`ParamBodyAngle*`
//! 走 `body_p = (p - 0.12) / 0.88`，身比头晚起步、同一帧归零。幅值 / 通道钳位不动，
//! `envelope()` 留着（「两端恰好 0」的旧回归继续测它）。
//!
//! # 幅值标定（2026-09-24，W2）
//!
//! 手势主轴表值头 12.0 / 身 3.9（峰值系数：2026-10-14 起起落曲线带保持段，
//! Single 与 shake 的 Oscillate `cycles=2` **都是 1.0**；旧的 `sin(pi*p)` 时代
//! shake 是 0.9285），使「出厂组合」「scale 走满」「intensity 走满」三条各自
//! 都留在上限 ×0.95 以内；数值账与「两个旋钮同时拉满会钳位的组合」见
//! `scales.rs` 顶部注释（那里是标定真源，本表是它的逐值副本）。
//!
//! # P0-2：预设写 `final_override` 层，不再写 input
//!
//! 旧实现把预设写进 `input` 层，而待机微表情（`idle.rs`）在**同一帧更晚**写同一个
//! input 层的 `ParamMouthForm` / `ParamBrowLY` / `ParamBrowRY` → 表情被 idle 盖掉。
//! 现在预设经 `PresetSink` 写 `final_override`（最高优先级），到点用
//! `clear_override_parameter` **整批撤销**。口型 `ParamMouthOpenY` 仍走 input。
//!
//! # P0-1：`preset` 协议
//!
//! 旧形状 `{"id": "smile"}` 继续可用（缺省 = 该预设时长、强度 1、来源
//! `unknown`）。新增：
//! - `id:"none"` = 撤销：每个还有姿势的槽**从屏幕值放回** 0（见 `release`）；
//! - 可选 `intensity`（钳 `[0, 3]`）、`ttl_ms`（钳 `(0, 5000]`）、
//!   `source`（`debug` / `director` / 其它 → `unknown`）。
//!
//! 未知 id **仍静默忽略**（不报错、不改任何参数）。解析逻辑是纯函数
//! `parse_command`，原生 `cargo test` 直接回归。

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

/// 预设来源（P0-1；仅用于 HUD / 调试面板显示「谁在演」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetSource {
    /// 开发工具「动作调试」直接下发。
    Debug,
    /// 导演 Mod 经 Flutter 转发。
    Director,
    /// 缺省 / 不认识的来源。
    Unknown,
}

impl PresetSource {
    /// 稳定字符串（HUD / 状态行回显）。
    pub const fn as_str(self) -> &'static str {
        match self {
            PresetSource::Debug => "debug",
            PresetSource::Director => "director",
            PresetSource::Unknown => "unknown",
        }
    }

    /// 解析来源字符串；未知一律 `PresetSource::Unknown`（宽容，不失败）。
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "debug" | "ui" => PresetSource::Debug,
            "director" => PresetSource::Director,
            _ => PresetSource::Unknown,
        }
    }
}

/// 一条解析后的 `preset` 指令（P0-1）。
#[derive(Debug, Clone, PartialEq)]
pub enum PresetCommand {
    /// 应用一条预设。
    Apply {
        spec: &'static PresetSpec,
        intensity: f32,
        ttl_ms: f64,
        source: PresetSource,
    },
    /// `id:"none"`：撤销（按槽从屏幕值放回，见 `release`）。
    Revoke,
    /// 未知 / 缺失 id：静默忽略（不动任何参数，不回执）。
    Ignore,
}

/// 从原始字段解析一条 `preset` 指令（**纯函数**，原生可测；内建表）。
///
/// 缺省与钳位（P0-1，兼容旧 `{id}`）：
/// - `intensity` 缺省 / 非有限 → `1.0`，钳 `[0, MAX_INTENSITY]`；
/// - `ttl_ms` 缺省 / 非正 → 该预设的 `duration_ms`，钳 `(0, MAX_TTL_MS]`；
/// - `source` 缺省 / 未知 → `PresetSource::Unknown`；
/// - `id:"none"` → `PresetCommand::Revoke`；未知（含已删除的旧 id）→ `PresetCommand::Ignore`。
// 内建表版的协议解析：wasm 侧走 PresetRuntime::resolve（外置表）；测试仍用这个。
#[allow(dead_code)]
pub fn parse_command(
    id: Option<&str>,
    intensity: Option<f64>,
    ttl_ms: Option<f64>,
    source: Option<&str>,
) -> PresetCommand {
    parse_command_with(
        |id| preset(id).map(|s| (s, None)),
        id,
        intensity,
        ttl_ms,
        source,
    )
}

/// 同 parse_command，但用**给定的查找函数**解析 id。
///
/// PresetRuntime::resolve 用它接外置表（&'static PresetSpec 是同一类型，
/// 只是来源不同）。查找返回的 `Option<f32>` 保留给「查表默认强度」这一扩展点；
/// 当前所有实现都返回 `None`（旧 id 的别名默认强度已删除）。
fn parse_command_with(
    lookup: impl Fn(&str) -> Option<(&'static PresetSpec, Option<f32>)>,
    id: Option<&str>,
    intensity: Option<f64>,
    ttl_ms: Option<f64>,
    source: Option<&str>,
) -> PresetCommand {
    let Some(raw_id) = id else {
        return PresetCommand::Ignore;
    };
    let id = raw_id.trim();
    if id == REVOKE_ID {
        return PresetCommand::Revoke;
    }
    let Some((spec, lookup_default)) = lookup(id) else {
        return PresetCommand::Ignore;
    };
    // 显式给了合法 intensity 就用它；缺省 / 非有限 → 查表默认（当前恒 1.0）。
    let intensity = match intensity {
        Some(v) if v.is_finite() => v as f32,
        _ => lookup_default.unwrap_or(1.0),
    }
    .clamp(0.0, MAX_INTENSITY);
    let ttl_ms = ttl_ms
        .filter(|v| v.is_finite() && *v > 0.0)
        .map_or(spec.duration_ms, |v| v.min(MAX_TTL_MS));
    let source = source.map_or(PresetSource::Unknown, PresetSource::parse);
    PresetCommand::Apply {
        spec,
        intensity,
        ttl_ms,
        source,
    }
}

/// 正弦包络：`progress ∈ [0,1]` → 幅值乘子，两端为 0、中途为 1。
///
/// 用 `sin(πp)` 而不是线性三角波：起落都是**平滑**的，首末帧幅值为 0，于是撤销参数
/// 时不会「跳一下」（跳变正是皮套看起来像抽帧的原因）。
///
/// **2026-10-14 起手势不再走它**（改走 `motion_curve`）；留着是因为它「两端**恰好** 0」
/// 仍是要测的口径（旧回归继续测这条函数），且是 `motion_curve` 之前的起落定义。
#[allow(dead_code)]
pub fn envelope(progress: f64) -> f32 {
    let p = progress.clamp(0.0, 1.0);
    // 两端**恰好** 0：sin(pi) 在 f64 里是 1.2e-16 而不是 0。
    if p <= 0.0 || p >= 1.0 {
        return 0.0;
    }
    ((std::f64::consts::PI * p).sin()).max(0.0) as f32
}

/// 手势**头 / 其它通道**的幅值乘子 = 起落曲线 × 波形（表情忽略波形，增益仍走曲线）。
///
/// 曲线与身体滞后都住在 `motion_curve`：本函数是「用本帧 `p`」的那一支；
/// 身体通道（`ParamBodyAngle*`）的增益请走 `pack_gain`，它按通道选进度。
pub fn motion_gain(wave: MotionWave, progress: f64) -> f32 {
    motion_curve::gain(wave, progress)
}

/// 本帧的**基准参数**（不含 intensity / 倍率，但含起落 / 波形增益）。
///
/// - 普通包 = `params`；
/// - morph 包 = low 极（= `params`，等价于 `morph_mix(spec, low)`）。
///
/// 增益**逐通道**算：手势的身体通道走滞后进度（见 `pack_gain`）。
pub fn frame_params(spec: &PresetSpec, progress: f64) -> Vec<(&'static str, f32)> {
    base_params(spec)
        .into_iter()
        .map(|(id, v)| (id, v * pack_gain(spec.kind, spec.wave, progress, id)))
        .collect()
}

/// 普通包 = `params × intensity`；morph 包 = 按 intensity 在两极间插值。
///
/// 返回**未乘通道倍率、未钳位**的基准值（见 `frame_params_scaled_by`）。
pub fn base_params_for_intensity(spec: &PresetSpec, intensity: f32) -> Vec<(&'static str, f32)> {
    match spec.morph {
        Some(_) => morph_mix(spec, intensity),
        None => spec
            .params
            .iter()
            .map(|(id, v)| (*id, v * intensity))
            .collect(),
    }
}

/// morph 包的基准值：在 low / high 两极之间按 `intensity` 插值。
///
/// - `intensity <= low`：从 0（中性）线性升到 low 极（`t = i/low`）；
/// - `low < intensity < high`：low 极 ↔ high 极线性插值；
/// - `intensity >= high`：high 极。
///
/// 非 morph 包原样返回 `params`（调用方不应走这里）。
pub fn morph_mix(spec: &PresetSpec, intensity: f32) -> Vec<(&'static str, f32)> {
    let Some(m) = spec.morph else {
        return spec.params.to_vec();
    };
    let i = if intensity.is_finite() {
        intensity.max(0.0)
    } else {
        m.low
    };
    let low = if m.low.is_finite() && m.low > 0.0 {
        m.low
    } else {
        1.0
    };
    let high = if m.high.is_finite() && m.high > low {
        m.high
    } else {
        MAX_INTENSITY.max(low)
    };
    let high_of = |id: &str| -> f32 {
        m.high_params
            .iter()
            .find(|(k, _)| *k == id)
            .map_or(0.0, |(_, v)| *v)
    };
    let mut out: Vec<(&'static str, f32)> = Vec::with_capacity(spec.params.len());
    for (id, low_v) in spec.params {
        let high_v = high_of(id);
        let v = if i <= low {
            low_v * (i / low)
        } else if i >= high {
            high_v
        } else {
            let t = (i - low) / (high - low);
            low_v + (high_v - low_v) * t
        };
        out.push((*id, v));
    }
    // 只出现在 high 极的通道（shipped 表不允许；宽容补上，从 0 插值）。
    for (id, high_v) in m.high_params {
        if out.iter().any(|(k, _)| k == id) {
            continue;
        }
        let v = if i <= low {
            0.0
        } else if i >= high {
            *high_v
        } else {
            let t = (i - low) / (high - low);
            *high_v * t
        };
        out.push((*id, v));
    }
    out
}

/// 基准参数：morph 包 = low 极（`params`），普通包 = `params`。
fn base_params(spec: &PresetSpec) -> Vec<(&'static str, f32)> {
    spec.params.to_vec()
}

/// 起落 / 波形乘子：**表情与手势同走 `motion_curve::curve`**（进度 0 / 1 恰好 0，
/// 保持段恰好 1）；手势**逐通道**算，身体（`ParamBodyAngle*`）走滞后进度。
fn pack_gain(kind: PresetKind, wave: MotionWave, progress: f64, param_id: &str) -> f32 {
    match kind {
        PresetKind::Expression => motion_curve::curve(progress) as f32,
        PresetKind::Motion => {
            if param_id.starts_with(motion_curve::BODY_CHANNEL_PREFIX) {
                motion_gain(wave, motion_curve::body_progress(progress))
            } else {
                motion_gain(wave, progress)
            }
        }
    }
}

/// 同 `frame_params`，再乘 `intensity`（P0-1；缺省 1.0 时逐值等价）。
// 只带强度的旧 API：运行时改走 frame_params_scaled_by；留着给旧回归逐值对照。
#[allow(dead_code)]
pub fn frame_params_scaled(
    spec: &PresetSpec,
    progress: f64,
    intensity: f32,
) -> Vec<(&'static str, f32)> {
    base_params_for_intensity(spec, intensity)
        .into_iter()
        .map(|(id, v)| (id, v * pack_gain(spec.kind, spec.wave, progress, id)))
        .collect()
}

/// 归一化进度（`elapsed_ms` 相对 `duration_ms`，两端钳到 `[0,1]`）。
pub fn progress_at(elapsed_ms: f64, duration_ms: f64) -> f64 {
    if duration_ms <= 0.0 {
        return 1.0;
    }
    (elapsed_ms / duration_ms).clamp(0.0, 1.0)
}

mod handoff;
mod motion_curve;
mod release;
mod scales;
pub use handoff::Handoff;
pub use release::Release;
// 这些名字大多给 table.rs / 测试用（cfg(test) 下才被「用到」），故放宽 unused。
#[allow(unused_imports)]
pub use scales::{
    BODY_LIMIT, EXPRESSION_BODY_LIMIT, EXPRESSION_HEAD_LIMIT, EXPRESSION_LIMIT, HEAD_LIMIT,
    MAX_SCALE, MIN_SCALE, PresetScales, ScaleClass, clamp_scale, clamp_to_channel,
    frame_params_scaled_by, scale_class,
};

// 阶段4c：三字段（body / head / expression）的映射表 + 运行状态机 + 音频时钟 + ack。
// 与旧的 `preset_id` 双槽（`PresetRuntime`）**并存**：旧路径一字不删（V11）。
mod field_clock;
mod field_map;
mod field_runtime;
#[allow(unused_imports)]
pub use field_clock::{ClockDomain, ClockState, StageClock};
#[allow(unused_imports)]
pub use field_map::{FACIAL_PARAMS, Field, FieldMap, field_allows, model_id_from_url};
#[allow(unused_imports)]
pub use field_runtime::{
    AckEvent, AckKind, Anchor, DEFAULT_TTL_EXPRESSION_MS, FieldCue, FieldRuntime, FrameOutcome,
    FrameWrite, default_ttl_ms, dropped_for_bad_cue,
};

/// 一条预设用到的**全部通道**（低 / 高两极并集；去重、稳定顺序）。
pub fn spec_channels(spec: &PresetSpec) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for (id, _) in spec.params {
        if !out.contains(id) {
            out.push(id);
        }
    }
    if let Some(m) = spec.morph {
        for (id, _) in m.high_params {
            if !out.contains(id) {
                out.push(id);
            }
        }
    }
    out
}

mod table;
#[allow(unused_imports)]
pub use table::{ALLOWED_PARAMS, PresetTable, amplitude_limit, pack_limit};

/// 预设的**唯一写入目标**：只暴露 `final_override` 层的写 / 清（P0-2）。
pub trait PresetSink {
    /// 写 `final_override`；模型缺该参数返回 `false`（静默降级）。
    fn override_param(&mut self, id: &str, value: f32) -> bool;
    /// 清 `final_override`。
    fn clear_override_param(&mut self, id: &str) -> bool;
}

/// 清掉**一条**预设用到的 override 参数（到点按槽撤销；换包改走 `handoff` 接力）。
pub fn clear_spec_overrides(spec: &PresetSpec, sink: &mut impl PresetSink) {
    for id in spec_channels(spec) {
        sink.clear_override_param(id);
    }
}

/// 当前活动的预设（跨帧状态）。
#[derive(Debug, Clone, PartialEq)]
pub struct ActivePreset {
    pub spec: &'static PresetSpec,
    /// 起始时刻（`performance.now()` 毫秒，与 rAF 的 dt 同源）。
    pub started_ms: f64,
    /// 生效时长（毫秒，已钳位）。
    pub ttl_ms: f64,
    /// 强度（已钳位）。
    pub intensity: f32,
    pub source: PresetSource,
    /// 同槽换包时的「换下去那一帧」快照（见 `handoff`）；无接力 = `None`。
    pub handoff: Option<Handoff>,
    /// `Revoke` 的放回状态（见 `release`）；非放回 = `None`。
    pub release: Option<Release>,
}

/// 预设运行状态机（**纯逻辑**，原生可测）：表情槽 + 手势槽。
#[derive(Debug, Clone)]
pub struct PresetRuntime {
    /// 预设表（内建或外置 JSON；见 PresetTable）。
    table: PresetTable,
    /// 表情槽（kind=expression）。
    face: Option<ActivePreset>,
    /// 手势槽（kind=motion）。
    gesture: Option<ActivePreset>,
    /// 用户可调的幅度倍率（初始 = 出厂默认 0.75/1.4/1.0）。
    scales: PresetScales,
}

impl Default for PresetRuntime {
    fn default() -> Self {
        Self {
            table: PresetTable::builtin(),
            face: None,
            gesture: None,
            scales: PresetScales::PRODUCT_DEFAULT,
        }
    }
}

impl PresetRuntime {
    /// 最近触发的活动预设（兼容旧 API）：优先表情槽，其次手势槽。
    ///
    /// HUD / 调试面板要看**两个槽**时用 `Self::active_face` / `Self::active_gesture`。
    pub fn active(&self) -> Option<&ActivePreset> {
        self.face.as_ref().or(self.gesture.as_ref())
    }

    /// 表情槽的当前预设。
    pub fn active_face(&self) -> Option<&ActivePreset> {
        self.face.as_ref()
    }

    /// 手势槽的当前预设。
    pub fn active_gesture(&self) -> Option<&ActivePreset> {
        self.gesture.as_ref()
    }

    /// 手势槽里是否**有活动预设**（表情槽不算）——待机摆头让开的判据。
    ///
    /// 到点撤销在 `apply_frame` 里先行，所以每帧 `apply_frame` 之后取这个布尔，
    /// 拿到的是「本帧还活着的手势」。表情不占手势槽（脸不该压住摆头）。
    pub fn gesture_occupied(&self) -> bool {
        self.gesture.is_some()
    }

    /// 某个槽的当前预设。
    pub fn active_in(&self, slot: PresetSlot) -> Option<&ActivePreset> {
        self.slot_ref(slot)
    }

    /// 当前生效的幅度倍率。
    pub fn scales(&self) -> PresetScales {
        self.scales
    }

    /// 设置幅度倍率（消息入口调用；内部再钳一次，防非法值）。
    pub fn set_scales(&mut self, scales: PresetScales) {
        self.scales = scales.clamped();
    }

    /// 替换预设表（启动时加载外置 JSON 后调用；失败回退内建则不调用）。
    pub fn set_table(&mut self, table: PresetTable) {
        self.table = table;
    }

    /// 当前表（诊断 / 启动日志用）。
    #[allow(dead_code)]
    pub fn table(&self) -> &PresetTable {
        &self.table
    }

    fn slot_ref(&self, slot: PresetSlot) -> Option<&ActivePreset> {
        match slot {
            PresetSlot::Face => self.face.as_ref(),
            PresetSlot::Gesture => self.gesture.as_ref(),
        }
    }

    fn slot_mut(&mut self, slot: PresetSlot) -> &mut Option<ActivePreset> {
        match slot {
            PresetSlot::Face => &mut self.face,
            PresetSlot::Gesture => &mut self.gesture,
        }
    }

    /// 用**本运行时的表**解析一条 preset 指令（未知 id → `Ignore`）。
    pub fn resolve(
        &self,
        id: Option<&str>,
        intensity: Option<f64>,
        ttl_ms: Option<f64>,
        source: Option<&str>,
    ) -> PresetCommand {
        parse_command_with(
            |id| self.table.get(id).map(|spec| (spec, None)),
            id,
            intensity,
            ttl_ms,
            source,
        )
    }

    /// 剩余毫秒（两个槽里**最晚结束**的那个；都没有 → `None`）。
    pub fn remaining_ms(&self, now_ms: f64) -> Option<f64> {
        [self.face.as_ref(), self.gesture.as_ref()]
            .into_iter()
            .flatten()
            .map(|a| (a.ttl_ms - (now_ms - a.started_ms)).max(0.0))
            .reduce(f64::max)
    }

    /// 处理一条指令；返回值 = **是否值得回执**（`applied`）。
    ///
    /// - `PresetCommand::Revoke`：每个还有姿势的槽从**屏幕值**放回 0（见 `release`）；
    /// - `PresetCommand::Apply`：空槽 / 同一 id 照旧；同一槽换**不同 id** 时不先
    ///   归零，把「换下去那一帧」记在新预设上，由下一帧 `Self::apply_frame` 接力；
    /// - `PresetCommand::Ignore`：什么都不做、不回执（未知 id 静默降级）。
    pub fn handle(&mut self, cmd: PresetCommand, now_ms: f64, _sink: &mut impl PresetSink) -> bool {
        match cmd {
            PresetCommand::Ignore => false,
            PresetCommand::Revoke => {
                // **不从当帧清参数**：每个还有姿势的槽从屏幕上这一帧的值收到 0，到点清槽。
                let scales = self.scales;
                for slot in [PresetSlot::Face, PresetSlot::Gesture] {
                    if let Some(active) = self.slot_mut(slot) {
                        let from = handoff::on_screen(active, now_ms, scales);
                        active.release = Some(Release::new(from, active.ttl_ms, now_ms));
                    }
                }
                true
            }
            PresetCommand::Apply {
                spec,
                intensity,
                ttl_ms,
                source,
            } => {
                let slot = slot_of(spec.kind);
                // 同一槽换下一个动作：不先归零。把「换下去那一帧」记在新预设上，
                // 由 `apply_frame` 在攻击段内插值接上（空槽 / 同一 id 不接力）。
                let scales = self.scales;
                let handoff = self
                    .slot_ref(slot)
                    .filter(|old| old.spec.id != spec.id)
                    .map(|old| Handoff::capture(old, now_ms, scales));
                *self.slot_mut(slot) = Some(ActivePreset {
                    spec,
                    started_ms: now_ms,
                    ttl_ms,
                    intensity,
                    source,
                    handoff,
                    release: None,
                });
                true
            }
        }
    }

    /// 每帧把两个槽写进 `final_override`；到点 / 放回到点按槽撤销并清空该槽。
    ///
    /// 同帧写入顺序 Face → Gesture：共享的头 / 身通道由手势包赢（否则点头会被
    /// 表情包的小幅抬头盖掉）。
    pub fn apply_frame(&mut self, now_ms: f64, sink: &mut impl PresetSink) {
        for slot in [PresetSlot::Face, PresetSlot::Gesture] {
            let expired = self.slot_ref(slot).is_some_and(|a| {
                a.release.is_none() && (now_ms - a.started_ms).max(0.0) >= a.ttl_ms
            });
            if expired {
                if let Some(old) = self.slot_ref(slot) {
                    clear_spec_overrides(old.spec, sink);
                }
                *self.slot_mut(slot) = None;
            }
        }
        let scales = self.scales;
        for slot in [PresetSlot::Face, PresetSlot::Gesture] {
            let Some(active) = self.slot_ref(slot).cloned() else {
                continue;
            };
            if let Some(release) = &active.release {
                // 放回：每帧从这一帧屏幕上的值按 smootherstep 收到 0，到点清槽。
                if release.write(now_ms, sink) {
                    *self.slot_mut(slot) = None;
                }
                continue;
            }
            let elapsed = (now_ms - active.started_ms).max(0.0);
            let progress = progress_at(elapsed, active.ttl_ms);
            let finished = match active.handoff.as_ref() {
                Some(handoff) => {
                    handoff.write(active.spec, progress, active.intensity, scales, sink)
                }
                None => {
                    for (id, value) in
                        frame_params_scaled_by(active.spec, progress, active.intensity, scales)
                    {
                        sink.override_param(id, value);
                    }
                    true
                }
            };
            // 攻击段结束：丢掉旧帧快照，此后帧值与不接旧值时逐值相同。
            if finished && let Some(cur) = self.slot_mut(slot) {
                cur.handoff = None;
            }
        }
    }
}
#[cfg(test)]
mod tests;
