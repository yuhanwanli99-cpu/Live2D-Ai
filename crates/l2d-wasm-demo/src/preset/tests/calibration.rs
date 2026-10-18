//! 幅值标定回归（2026-09-24 W2；0.3.7 T2 扩充到 21 条包）——从 `packs.rs` 拆出
//! （AGENTS 测试文件 ≤800 行）。
//!
//! 口径真源 = `preset/scales.rs` 顶部注释；四条硬指标**逐条**断言，且同时覆盖
//! **内建 fallback 表**与 `assets/actions/presets.json` 两份：
//!   ① 出厂组合 |表值 × 峰值 × 出厂倍率| ≤ 上限 × 0.60；
//!   ② scale 旋钮走满 |表值 × 峰值 × MAX_SCALE| ≤ 上限 × 0.95；
//!   ③ intensity 旋钮走满（方案 (i)）|表值 × 峰值 × 3 × 出厂倍率| ≤ 上限 × 0.95；
//!   ④ 手势主轴身/头比（表内 + 出厂后）∈ [0.30, 0.50]。
//! 另有死区起点逐通道打印（--nocapture 可见），并断言每个通道都长过滑条上限。

use super::*;

// ─────────────────────────────────────────── 幅值标定（2026-09-24，W2）
//
// 口径真源 = scales.rs 顶部注释。四条硬指标**逐条**断言，且同时覆盖
// **内建 fallback 表**与 assets/actions/presets.json 两份：
//   ① 出厂组合 |表值 × 峰值 × 出厂倍率| ≤ 上限 × 0.60；
//   ② scale 旋钮走满 |表值 × 峰值 × MAX_SCALE| ≤ 上限 × 0.95；
//   ③ intensity 旋钮走满（方案 (i)）|表值 × 峰值 × 3 × 出厂倍率| ≤ 上限 × 0.95；
//   ④ 手势主轴身/头比（表内 + 出厂后）∈ [0.30, 0.50]。
// 另有死区起点逐通道打印（--nocapture 可见），并断言每个通道都
// 长过滑条上限（即 MAX_SCALE 之前不会被钳死）。

/// 运行期上限（clamp_to_channel 口径）：头 30 / 身 10 / 五官 4。
///
/// 注意：amplitude_limit 的 30 是**解析期手势包**口径；表情包的 12 / 4 是
/// pack_limit（解析期）。两者都**不是**运行期上限，别混用。
fn runtime_limit(id: &str) -> f32 {
    match scale_class(id) {
        ScaleClass::Head => HEAD_LIMIT,
        ScaleClass::Body => BODY_LIMIT,
        ScaleClass::Expression => EXPRESSION_LIMIT,
    }
}

/// 峰值系数：起落曲线 × 波形在 progress 上的数值上确界（真的采样算出来）。
///
/// - 表情：保持段的上确界 = 1.0（进 / 出段与手势同一条曲线）；
/// - Single：起落曲线上确界 = 1.0（保持段）；
/// - Oscillate：逐点采样取最大（2026-10-14 起保持段可达满幅 → 1.0；
///   旧的 sin(pi*p) 时代是 0.9285）。
fn peak_gain(spec: &PresetSpec) -> f32 {
    match spec.kind {
        PresetKind::Expression => 1.0,
        PresetKind::Motion => {
            const N: u32 = 1_000_000;
            let mut peak = 0.0f32;
            for k in 0..=N {
                let p = k as f64 / N as f64;
                peak = peak.max(motion_gain(spec.wave, p).abs());
            }
            peak
        }
    }
}

/// 一条包在某个通道上的最大表值（morph 包取两极较大者）。
pub(super) fn max_table_value(spec: &PresetSpec, id: &str) -> f32 {
    let mut m = 0.0f32;
    for (k, v) in spec.params {
        if *k == id {
            m = m.max(v.abs());
        }
    }
    if let Some(mo) = spec.morph {
        for (k, v) in mo.high_params {
            if *k == id {
                m = m.max(v.abs());
            }
        }
    }
    m
}

/// 两份表：内建 fallback 与出厂 JSON（产品实际能力）。
pub(super) fn all_tables() -> Vec<(&'static str, Vec<&'static PresetSpec>)> {
    let external = PresetTable::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .expect("出厂 presets.json 必须合法");
    vec![
        ("builtin", PRESETS.iter().collect()),
        ("external", external.all().to_vec()),
    ]
}

/// 手势包的**主轴配对**（身/头比只按主轴算，次轴不参与）。
pub(super) fn main_axis_pair(id: &str) -> Option<(&'static str, &'static str)> {
    match id {
        // nod / look_up / look_down / bow 的主轴都是 Y（bow = 0.3.7 T2 深鞠躬）。
        "nod" | "look_up" | "look_down" | "bow" => Some(("ParamAngleY", "ParamBodyAngleY")),
        "shake" | "look_left" | "look_right" => Some(("ParamAngleX", "ParamBodyAngleX")),
        "tilt_left" | "tilt_right" => Some(("ParamAngleZ", "ParamBodyAngleZ")),
        _ => None,
    }
}

/// shake 的峰值系数**真的算出来**（不许写死）：两个旋钮的红线账都依赖它。
///
/// 2026-10-14 起手势起落曲线有**保持段**（值恰好 1），于是 Oscillate 的峰值从
/// `sin(pi*p)` 时代的 0.9285 变成 **1.0**（保持段里 `|sin(2*pi*cycles*p)|` 取得到 1）。
#[test]
fn shake_oscillate_peak_is_computed_not_assumed() {
    let shake = preset("shake").unwrap();
    let peak = peak_gain(shake);
    println!("shake peak_gain = {peak:.6}（2026-10-14 保持段起 = 1.0）");
    assert!(
        (peak - 1.0).abs() < 1e-3,
        "shake(cycles=2) 峰值应为 1.0（保持段满幅），实得 {peak}"
    );
    assert!(
        peak_gain(preset("nod").unwrap()) > 0.9999,
        "Single 的上确界 = 1.0"
    );
}

/// ① 出厂组合：|表值 × 峰值 × 出厂倍率| ≤ 上限 × 0.60。
#[test]
fn factory_scales_stay_below_sixty_percent_of_every_channel_limit() {
    let factory = PresetScales::PRODUCT_DEFAULT;
    for (tag, specs) in all_tables() {
        for spec in specs {
            let peak = peak_gain(spec);
            for id in spec_channels(spec) {
                let t = max_table_value(spec, id);
                let limit = runtime_limit(id);
                let final_v = t * peak * factory.for_param(id);
                assert!(
                    final_v <= limit * 0.60 + 1e-6,
                    "{tag}:{} {id} 出厂组合 {final_v:.3} > 上限 {limit} × 0.60 = {:.3}（表 {t} × 峰值 {peak:.4} × 倍率 {}）",
                    spec.id,
                    limit * 0.60,
                    factory.for_param(id),
                );
            }
        }
    }
}

/// ② scale 旋钮独立走满：|表值 × 峰值 × MAX_SCALE| ≤ 上限 × 0.95。
#[test]
fn max_scale_knob_alone_never_reaches_the_limit() {
    for (tag, specs) in all_tables() {
        for spec in specs {
            let peak = peak_gain(spec);
            for id in spec_channels(spec) {
                let t = max_table_value(spec, id);
                let limit = runtime_limit(id);
                let final_v = t * peak * MAX_SCALE;
                assert!(
                    final_v <= limit * 0.95 + 1e-6,
                    "{tag}:{} {id} scale 走满 {final_v:.3} > 上限 {limit} × 0.95 = {:.3}",
                    spec.id,
                    limit * 0.95,
                );
            }
        }
    }
}

/// ③ intensity 旋钮独立走满（方案 (i)：下调表值，MAX_INTENSITY 保持 3.0）：
/// |表值 × 峰值 × 3 × 出厂倍率| ≤ 上限 × 0.95。
#[test]
fn intensity_knob_alone_never_reaches_the_limit() {
    assert!(
        (MAX_INTENSITY - 3.0).abs() < f32::EPSILON,
        "方案 (i)：MAX_INTENSITY 保持 3.0，不收缩 intensity"
    );
    let factory = PresetScales::PRODUCT_DEFAULT;
    for (tag, specs) in all_tables() {
        for spec in specs {
            let peak = peak_gain(spec);
            for id in spec_channels(spec) {
                let t = max_table_value(spec, id);
                let limit = runtime_limit(id);
                let final_v = t * peak * MAX_INTENSITY * factory.for_param(id);
                assert!(
                    final_v <= limit * 0.95 + 1e-6,
                    "{tag}:{} {id} intensity 走满 {final_v:.3} > 上限 {limit} × 0.95 = {:.3}",
                    spec.id,
                    limit * 0.95,
                );
            }
        }
    }
}

/// ④ 手势包主轴身/头比：表内与出厂后都落在 [0.30, 0.50]；
/// 并复核裁决给的窗口 body ∈ [0.30 × head, min(上限_身 × 0.95 / (3 × 出厂body),
/// 0.50 × head × 出厂head / 出厂body)]（tilt 原来 0.25 不达标，本轮一起修）。
#[test]
fn gesture_main_axis_body_head_ratio_returns_to_the_design_window() {
    let factory = PresetScales::PRODUCT_DEFAULT;
    let mut checked = 0usize;
    for (tag, specs) in all_tables() {
        for spec in specs {
            let Some((head_id, body_id)) = main_axis_pair(spec.id) else {
                continue;
            };
            let head = max_table_value(spec, head_id);
            let body = max_table_value(spec, body_id);
            assert!(head > 0.0 && body > 0.0, "{tag}:{} 主轴通道缺失", spec.id);
            let table_ratio = body / head;
            let factory_ratio = body * factory.body / (head * factory.head);
            assert!(
                (0.30..=0.50).contains(&table_ratio),
                "{tag}:{} 表内身/头 {table_ratio:.4} 应在 [0.30, 0.50]",
                spec.id,
            );
            assert!(
                (0.30..=0.50).contains(&factory_ratio),
                "{tag}:{} 出厂身/头 {factory_ratio:.4} 应在 [0.30, 0.50]",
                spec.id,
            );
            let lo = 0.30 * head;
            let hi = (BODY_LIMIT * 0.95 / (MAX_INTENSITY * factory.body))
                .min(0.50 * head * factory.head / factory.body);
            assert!(
                (lo..=hi).contains(&body),
                "{tag}:{} body {body} 应在裁决窗口 [{lo:.3}, {hi:.3}]",
                spec.id,
            );
            checked += 1;
        }
    }
    assert_eq!(
        checked, 18,
        "两表 × 9 条手势包（0.3.7 T2 起含 look_up / look_down / bow）都该被覆盖"
    );
}

/// 逐通道打印 T1（表值 / 峰值 / 出厂值 / 上限 / 死区起点），并断言
/// 死区起点 ≥ MAX_SCALE——即「倍率滑条全行程有效」。
#[test]
fn dead_zone_starts_are_printed_for_every_channel() {
    let factory = PresetScales::PRODUCT_DEFAULT;
    println!(
        "T1 幅值标定（MAX_SCALE={MAX_SCALE} / MAX_INTENSITY={MAX_INTENSITY} / 出厂 h{}/b{}/e{}）",
        factory.head, factory.body, factory.expression
    );
    println!(
        "{:<18} {:<16} {:<10} {:>7} {:>8} {:>9} {:>6} {:>9}",
        "pack", "channel", "class", "table", "peak", "factory", "limit", "dead-zone"
    );
    for (tag, specs) in all_tables() {
        for spec in specs {
            let peak = peak_gain(spec);
            for id in spec_channels(spec) {
                let t = max_table_value(spec, id);
                let limit = runtime_limit(id);
                let dz = limit * 0.95 / (t * peak);
                println!(
                    "{tag}:{:<11} {:<16} {:<10} {:>7.3} {:>8.4} {:>9.3} {:>6.1} {:>9.3}",
                    spec.id,
                    id,
                    format!("{:?}", scale_class(id)),
                    t,
                    peak,
                    t * peak * factory.for_param(id),
                    limit,
                    dz,
                );
                assert!(
                    dz >= MAX_SCALE - 1e-4,
                    "{tag}:{} {id} 死区起点 {dz:.3} < MAX_SCALE {MAX_SCALE}——滑条上半段会失效",
                    spec.id,
                );
            }
        }
    }
}

/// 标定常量本身钉住（裁决参数集）：漂了就红，而不是靠人记。
#[test]
fn scale_calibration_constants_match_the_2026_09_24_decision() {
    assert!((MAX_SCALE - 2.2).abs() < f32::EPSILON, "MAX_SCALE = 2.2");
    assert!(
        (PresetScales::PRODUCT_DEFAULT.head - 0.75).abs() < f32::EPSILON
            && (PresetScales::PRODUCT_DEFAULT.body - 0.80).abs() < f32::EPSILON
            && (PresetScales::PRODUCT_DEFAULT.expression - 1.0).abs() < f32::EPSILON,
        "出厂倍率 = 0.75 / 0.80 / 1.0，实得 {:?}",
        PresetScales::PRODUCT_DEFAULT
    );
}

/// intensity=3 时该通道的**真实**基准值（morph 包取 high 极，不再乘 3）。
fn value_at_max_intensity(spec: &PresetSpec, id: &str) -> f32 {
    match spec.morph {
        Some(mo) => mo
            .high_params
            .iter()
            .find(|(k, _)| *k == id)
            .map_or(0.0, |(_, v)| v.abs()),
        None => max_table_value(spec, id) * MAX_INTENSITY,
    }
}

/// T3：两个旋钮同时拉满（intensity = 3 且 scale = 2.2）**只有**文档列出的组合会钳位。
///
/// 口径与 scales.rs 顶部 / presets.json _doc 的那张表逐条一致；morph 包是例外
/// （intensity=3 直接取 high 极、不二次相乘），所以 unhappy 一个通道都不钳。
#[test]
fn both_knobs_at_max_clamp_only_the_documented_combinations() {
    let expected: &[(&str, &str)] = &[
        ("nod", "ParamAngleY"),
        ("nod", "ParamBodyAngleY"),
        ("shake", "ParamAngleX"),
        ("shake", "ParamBodyAngleX"),
        ("look_left", "ParamAngleX"),
        ("look_left", "ParamBodyAngleX"),
        ("look_right", "ParamAngleX"),
        ("look_right", "ParamBodyAngleX"),
        ("tilt_left", "ParamAngleZ"),
        ("tilt_left", "ParamBodyAngleZ"),
        ("tilt_right", "ParamAngleZ"),
        ("tilt_right", "ParamBodyAngleZ"),
        // T9：thinking 的小幅歪头（预设包唯一新增的表情包，钳位只在头/身）
        ("thinking", "ParamAngleZ"),
        ("thinking", "ParamBodyAngleZ"),
        ("look_up", "ParamAngleY"),
        ("look_up", "ParamBodyAngleY"),
        ("look_down", "ParamAngleY"),
        ("look_down", "ParamBodyAngleY"),
        ("smile", "ParamMouthForm"),
        ("smile", "ParamEyeLSmile"),
        ("smile", "ParamEyeRSmile"),
        ("smile", "ParamAngleY"),
        ("smile", "ParamBodyAngleY"),
        ("surprised", "ParamEyeLOpen"),
        ("surprised", "ParamEyeROpen"),
        ("surprised", "ParamBrowLY"),
        ("surprised", "ParamBrowRY"),
        ("surprised", "ParamAngleY"),
        ("surprised", "ParamBodyAngleY"),
        // 0.3.7 T2（D1 手不做）：8 条新表情 + bow。blush / sleepy 在两旋钮拉满时
        // 一个通道都不钳（五官 ≤0.6 / 头身 ≤4）；其余按 value_at_max_intensity ×
        // MAX_SCALE > 上限逐通道列出（与 presets.json 的 _doc 表逐条一致）。
        ("angry", "ParamMouthForm"),
        ("angry", "ParamBrowLY"),
        ("angry", "ParamBrowRY"),
        ("angry", "ParamEyeLOpen"),
        ("angry", "ParamEyeROpen"),
        ("sad", "ParamMouthForm"),
        ("sad", "ParamAngleY"),
        ("sad", "ParamBodyAngleY"),
        ("wink", "ParamMouthForm"),
        ("wink", "ParamEyeLSmile"),
        ("wink", "ParamEyeROpen"),
        ("wink", "ParamAngleZ"),
        ("wink", "ParamBodyAngleZ"),
        ("confused", "ParamEyeLOpen"),
        ("confused", "ParamEyeROpen"),
        ("confused", "ParamAngleZ"),
        ("confused", "ParamBodyAngleZ"),
        ("excited", "ParamMouthForm"),
        ("excited", "ParamEyeLSmile"),
        ("excited", "ParamEyeRSmile"),
        ("excited", "ParamEyeLOpen"),
        ("excited", "ParamEyeROpen"),
        ("calm", "ParamEyeLOpen"),
        ("calm", "ParamEyeROpen"),
        ("bow", "ParamAngleY"),
        ("bow", "ParamBodyAngleY"),
    ];
    for (tag, specs) in all_tables() {
        let mut clamped: Vec<(&str, &str)> = Vec::new();
        for spec in specs {
            let peak = peak_gain(spec);
            for id in spec_channels(spec) {
                let final_v = value_at_max_intensity(spec, id) * peak * MAX_SCALE;
                let limit = runtime_limit(id);
                println!(
                    "T3 {tag}:{} {id} -> {final_v:.3} / 上限 {limit:.1}{}",
                    spec.id,
                    if final_v > limit { "  [钳位]" } else { "" }
                );
                if spec.id == "unhappy" {
                    assert!(
                        final_v <= limit,
                        "{tag}:unhappy {id} 是 morph 包（intensity 不乘 3），不该钳位"
                    );
                }
                if final_v > limit {
                    clamped.push((spec.id, id));
                }
            }
        }
        let mut got = clamped.clone();
        got.sort_unstable();
        let mut want = expected.to_vec();
        want.sort_unstable();
        assert_eq!(
            got, want,
            "{tag} 表：两旋钮拉满的钳位集合与文档不一致（实得 {clamped:?}）"
        );
    }
}
