//! 内建动作包表（fallback，自 `data.rs` 拆出：0.3.7 T2 后 data.rs 超过
//! AGENTS「源码 ≤500 行」——表数据与类型 / 常量分两个文件，路径不变：
//! `data.rs` 仍 `pub use` 本文件，`preset::PRESETS` 依旧可用）。
//!
//! 本表只是**加载失败时的兜底**；产品实际能力 = `assets/actions/presets.json`
//! （两份表逐值一致，回归 `builtin_fallback_matches_the_external_table_for_every_pack`
//! 钉住）。语义与标定口径见 `data.rs` 顶部与 `scales.rs`。

use super::{
    BOW_MS, EXPRESSION_MS, LOOK_MS, MOTION_MS, MotionWave, PresetKind, PresetMorph, PresetSpec,
    SHAKE_CYCLES,
};

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
///
/// **0.3.7 T2（2026-10-18，D1 手不做）**：表情从 4 → 12 条（+8）、手势从 8 → 9 条
/// （+bow），共 **21 条**（不含 `none`）。8 条新表情五官表值 ≤1.0、头角 ≤6、
/// 身随动 ≤2.0；bow = 头 Y -12 / 身 Y -3.9 / 1600ms。参数白名单不变（仍 13 个
/// 标准参数，没有手臂 / 手 / 肩通道）；本表与 `assets/actions/presets.json`
/// 逐值一致（回归钉住）。
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
    // ---- 表情（0.3.7 T2，D1 手不做：只用 13 个标准参数）----
    PresetSpec {
        id: "angry",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 眉下压 + 嘴角下撇 + 瞪眼；头略低。五官与头身都远在红线内。
        params: &[
            ("ParamMouthForm", -0.8),
            ("ParamBrowLY", -0.9),
            ("ParamBrowRY", -0.9),
            ("ParamEyeLOpen", 0.8),
            ("ParamEyeROpen", 0.8),
            ("ParamAngleY", -4.0),
            ("ParamBodyAngleY", -1.3),
        ],
    },
    PresetSpec {
        id: "sad",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 内眉上挑 + 垂眼 + 嘴角下撇；低头 + 小幅歪头（身/头 = 0.325 / 0.333）。
        params: &[
            ("ParamMouthForm", -1.0),
            ("ParamBrowLY", 0.4),
            ("ParamBrowRY", 0.4),
            ("ParamEyeLOpen", 0.6),
            ("ParamEyeROpen", 0.6),
            ("ParamAngleY", -6.0),
            ("ParamAngleZ", 3.0),
            ("ParamBodyAngleY", -2.0),
            ("ParamBodyAngleZ", 1.0),
        ],
    },
    PresetSpec {
        id: "blush",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 害羞：13 参数没有脸颊通道，用眯眼笑 + 低头歪头表达（不假装脸红）。
        params: &[
            ("ParamMouthForm", 0.6),
            ("ParamEyeLSmile", 0.6),
            ("ParamEyeRSmile", 0.6),
            ("ParamBrowLY", 0.2),
            ("ParamBrowRY", 0.2),
            ("ParamAngleY", -2.0),
            ("ParamAngleZ", 4.0),
            ("ParamBodyAngleY", -0.65),
            ("ParamBodyAngleZ", 1.3),
        ],
    },
    PresetSpec {
        id: "wink",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 单眼闭（EyeLOpen 0 + EyeLSmile 0.9）+ 右眼睁 + 嘴角上扬 + 歪头。
        params: &[
            ("ParamMouthForm", 0.8),
            ("ParamEyeLSmile", 0.9),
            ("ParamEyeRSmile", 0.2),
            ("ParamEyeLOpen", 0.0),
            ("ParamEyeROpen", 1.0),
            ("ParamBrowLY", 0.2),
            ("ParamBrowRY", 0.35),
            ("ParamAngleY", 2.0),
            ("ParamAngleZ", 5.0),
            ("ParamBodyAngleY", 0.65),
            ("ParamBodyAngleZ", 1.6),
        ],
    },
    PresetSpec {
        id: "confused",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 左右眉 / 双眼不对称 + 歪头；不对称就是语义。
        params: &[
            ("ParamMouthForm", -0.3),
            ("ParamBrowLY", -0.5),
            ("ParamBrowRY", 0.3),
            ("ParamEyeLOpen", 0.9),
            ("ParamEyeROpen", 0.75),
            ("ParamAngleX", 2.0),
            ("ParamAngleZ", -6.0),
            ("ParamBodyAngleX", 0.65),
            ("ParamBodyAngleZ", -1.95),
        ],
    },
    PresetSpec {
        id: "sleepy",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 半垂眼 + 低头；没有张嘴参数（口型归 TTS），不表现哈欠。
        params: &[
            ("ParamMouthForm", 0.1),
            ("ParamBrowLY", 0.15),
            ("ParamBrowRY", 0.15),
            ("ParamEyeLOpen", 0.25),
            ("ParamEyeROpen", 0.25),
            ("ParamAngleY", -4.0),
            ("ParamAngleZ", 2.0),
            ("ParamBodyAngleY", -1.3),
            ("ParamBodyAngleZ", 0.65),
        ],
    },
    PresetSpec {
        id: "excited",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 满幅笑眼 + 睁大眼 + 抬眉 + 抬头；smile 的强档（不合并，保留可点名两档）。
        params: &[
            ("ParamMouthForm", 1.0),
            ("ParamEyeLSmile", 0.8),
            ("ParamEyeRSmile", 0.8),
            ("ParamEyeLOpen", 1.0),
            ("ParamEyeROpen", 1.0),
            ("ParamBrowLY", 0.6),
            ("ParamBrowRY", 0.6),
            ("ParamAngleY", 4.0),
            ("ParamAngleZ", 3.0),
            ("ParamBodyAngleY", 1.3),
            ("ParamBodyAngleZ", 1.0),
        ],
    },
    PresetSpec {
        id: "calm",
        kind: PresetKind::Expression,
        duration_ms: EXPRESSION_MS,
        wave: MotionWave::Single,
        morph: None,
        // 轻微闭眼 + 微抿嘴 + 极小幅偏头；比 none（归零）多一层放松常驻。
        params: &[
            ("ParamMouthForm", 0.2),
            ("ParamEyeLSmile", 0.15),
            ("ParamEyeRSmile", 0.15),
            ("ParamBrowLY", 0.1),
            ("ParamBrowRY", 0.1),
            ("ParamEyeLOpen", 0.85),
            ("ParamEyeROpen", 0.85),
            ("ParamAngleX", 2.0),
            ("ParamAngleZ", -2.0),
            ("ParamBodyAngleX", 0.65),
            ("ParamBodyAngleZ", -0.65),
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
    PresetSpec {
        id: "bow",
        kind: PresetKind::Motion,
        duration_ms: BOW_MS,
        // 0.3.7 T2：头 + 半身同向深鞠躬，主轴 Y 与 nod 同幅值（表内身/头 = 0.325），
        // 靠更久（1600ms）表达「深」；13 参数里没有肩 / 手臂，不躬身抱拳。
        params: &[("ParamAngleY", -12.0), ("ParamBodyAngleY", -3.9)],
        wave: MotionWave::Single,
        morph: None,
    },
];
