//! 表演字段 `expression.id` → 五官通道的**内建映射表**（0.3.7 T2 从
//! `field_map.rs` 拆出：AGENTS 行数纪律「源码 ≤500 行」）。
//!
//! 表值与 `assets/actions/presets.json` 逐值一致（回归
//! `field_map_carries_every_expression_preset_face` 钉住）；头身分量按 V3
//! 不进字段通道（手不做——这 12 条表情全部只用 13 个标准参数里的五官通道）。
//!
//! 口径（协议 §4.6 / §5.5）：
//! - 只准 `ParamMouthForm` / `ParamEyeLSmile` / `ParamEyeRSmile` /
//!   `ParamEyeLOpen` / `ParamEyeROpen` / `ParamBrowLY` / `ParamBrowRY`；
//! - 不得出现头角 / 半身 / 口型参数（`field_allows` 会再拦一次）。

use super::field_map::{ExpressionEntry, target};

/// 内建表情映射（顺序与 `assets/actions/presets.json` 的表情出现顺序一致）。
pub(super) fn builtin_expressions() -> Vec<ExpressionEntry> {
    vec![
        ExpressionEntry {
            id: "smile".to_string(),
            targets: vec![
                target("ParamMouthForm", 1.0),
                target("ParamEyeLSmile", 1.0),
                target("ParamEyeRSmile", 1.0),
                target("ParamBrowLY", 0.5),
                target("ParamBrowRY", 0.5),
            ],
        },
        ExpressionEntry {
            // unhappy 的 low 极（难过相）；morph 是包时代的概念，
            // v1 expression 字段取首版映射 = low 极。
            id: "unhappy".to_string(),
            targets: vec![
                target("ParamMouthForm", -1.0),
                target("ParamBrowLY", 0.4),
                target("ParamBrowRY", 0.4),
                target("ParamEyeLOpen", 0.65),
                target("ParamEyeROpen", 0.65),
            ],
        },
        ExpressionEntry {
            // T9（2026-10-07）：问句补丁的思考表情。**只抄五官五行**——
            // 预设包里的 AngleZ / BodyAngleZ 走 preset_id 通道；字段通道的
            // 歪头由补丁的 head.z 负责，避免两路叠加。
            id: "thinking".to_string(),
            targets: vec![
                target("ParamMouthForm", 0.0),
                target("ParamEyeLOpen", 0.55),
                target("ParamEyeROpen", 0.55),
                target("ParamBrowLY", -0.45),
                target("ParamBrowRY", -0.45),
            ],
        },
        ExpressionEntry {
            id: "surprised".to_string(),
            targets: vec![
                target("ParamEyeLOpen", 1.26),
                target("ParamEyeROpen", 1.26),
                target("ParamBrowLY", 0.8),
                target("ParamBrowRY", 0.8),
                target("ParamMouthForm", 0.2),
            ],
        },
        // 0.3.7 T2（2026-10-18，D1 手不做）：8 条新表情的**五官部分**，与
        // assets/actions/presets.json 逐值一致；头身分量不在这里（V3）。
        ExpressionEntry {
            id: "angry".to_string(),
            targets: vec![
                target("ParamMouthForm", -0.8),
                target("ParamBrowLY", -0.9),
                target("ParamBrowRY", -0.9),
                target("ParamEyeLOpen", 0.8),
                target("ParamEyeROpen", 0.8),
            ],
        },
        ExpressionEntry {
            id: "sad".to_string(),
            targets: vec![
                target("ParamMouthForm", -1.0),
                target("ParamBrowLY", 0.4),
                target("ParamBrowRY", 0.4),
                target("ParamEyeLOpen", 0.6),
                target("ParamEyeROpen", 0.6),
            ],
        },
        ExpressionEntry {
            id: "blush".to_string(),
            targets: vec![
                target("ParamMouthForm", 0.6),
                target("ParamEyeLSmile", 0.6),
                target("ParamEyeRSmile", 0.6),
                target("ParamBrowLY", 0.2),
                target("ParamBrowRY", 0.2),
            ],
        },
        ExpressionEntry {
            id: "wink".to_string(),
            targets: vec![
                target("ParamMouthForm", 0.8),
                target("ParamEyeLSmile", 0.9),
                target("ParamEyeRSmile", 0.2),
                target("ParamEyeLOpen", 0.0),
                target("ParamEyeROpen", 1.0),
                target("ParamBrowLY", 0.2),
                target("ParamBrowRY", 0.35),
            ],
        },
        ExpressionEntry {
            id: "confused".to_string(),
            targets: vec![
                target("ParamMouthForm", -0.3),
                target("ParamBrowLY", -0.5),
                target("ParamBrowRY", 0.3),
                target("ParamEyeLOpen", 0.9),
                target("ParamEyeROpen", 0.75),
            ],
        },
        ExpressionEntry {
            id: "sleepy".to_string(),
            targets: vec![
                target("ParamMouthForm", 0.1),
                target("ParamBrowLY", 0.15),
                target("ParamBrowRY", 0.15),
                target("ParamEyeLOpen", 0.25),
                target("ParamEyeROpen", 0.25),
            ],
        },
        ExpressionEntry {
            id: "excited".to_string(),
            targets: vec![
                target("ParamMouthForm", 1.0),
                target("ParamEyeLSmile", 0.8),
                target("ParamEyeRSmile", 0.8),
                target("ParamEyeLOpen", 1.0),
                target("ParamEyeROpen", 1.0),
                target("ParamBrowLY", 0.6),
                target("ParamBrowRY", 0.6),
            ],
        },
        ExpressionEntry {
            id: "calm".to_string(),
            targets: vec![
                target("ParamMouthForm", 0.2),
                target("ParamEyeLSmile", 0.15),
                target("ParamEyeRSmile", 0.15),
                target("ParamBrowLY", 0.1),
                target("ParamBrowRY", 0.1),
                target("ParamEyeLOpen", 0.85),
                target("ParamEyeROpen", 0.85),
            ],
        },
    ]
}
