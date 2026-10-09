//! 双槽 / 强度 morph / 用户可调幅度的回归（从 tests.rs 拆出）。

use super::*;

// ─────────────────────────────────────────── 层级 / 撤销 / 双槽

#[test]
fn idle_written_after_preset_never_wins_then_revoke_restores() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    assert!(rt.handle(
        parse_command(Some("smile"), None, None, Some("debug")),
        1_000.0,
        &mut sink,
    ));
    rt.apply_frame(1_016.0, &mut sink);
    sink.idle_input("ParamMouthForm", 0.5);
    sink.idle_input("ParamBrowLY", -0.05);
    assert_eq!(sink.effective("ParamMouthForm"), Some(1.0));
    assert_eq!(sink.effective("ParamBrowLY"), Some(0.5));
    assert!(!sink.overrides.contains_key("ParamMouthOpenY"));

    rt.apply_frame(1_016.0 + EXPRESSION_MS, &mut sink);
    assert!(rt.active().is_none());
    assert!(sink.overrides.is_empty(), "到点必须整批撤销");
    assert_eq!(sink.effective("ParamMouthForm"), Some(0.5));
}

#[test]
fn none_revokes_immediately_and_switching_clears_stale_params() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    rt.handle(
        parse_command(Some("look_left"), None, None, None),
        0.0,
        &mut sink,
    );
    rt.apply_frame(100.0, &mut sink);
    assert!(sink.overrides.contains_key("ParamAngleX"));

    // 同槽换包（手势槽 look_left -> nod）：清掉上一条的 ParamAngleX。
    rt.handle(
        parse_command(Some("nod"), None, None, None),
        200.0,
        &mut sink,
    );
    assert!(!sink.overrides.contains_key("ParamAngleX"));
    rt.apply_frame(300.0, &mut sink);
    assert!(sink.overrides.contains_key("ParamAngleY"));

    assert!(rt.handle(
        parse_command(Some("none"), None, None, None),
        400.0,
        &mut sink
    ));
    assert!(sink.overrides.is_empty());
    assert!(rt.active().is_none());
}

#[test]
fn motion_ramps_then_clears_at_ttl() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    rt.handle(parse_command(Some("nod"), None, None, None), 0.0, &mut sink);
    rt.apply_frame(0.0, &mut sink);
    assert_eq!(
        sink.overrides.get("ParamAngleY").copied().unwrap_or(999.0),
        0.0
    );
    rt.apply_frame(MOTION_MS / 2.0, &mut sink);
    assert!(
        sink.overrides
            .get("ParamAngleY")
            .copied()
            .unwrap_or(0.0)
            .abs()
            > 1.0
    );
    rt.apply_frame(MOTION_MS, &mut sink);
    assert!(rt.active().is_none(), "到点撤销");
    assert!(sink.overrides.is_empty());
}

#[test]
fn remaining_ms_is_observable() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    rt.handle(
        parse_command(Some("nod"), None, None, None),
        1_000.0,
        &mut sink,
    );
    assert_eq!(rt.remaining_ms(1_000.0), Some(MOTION_MS));
    assert_eq!(
        rt.remaining_ms(1_000.0 + MOTION_MS / 2.0),
        Some(MOTION_MS / 2.0)
    );
    assert_eq!(rt.remaining_ms(99_999.0), Some(0.0));
}

/// **双槽同轮**：微笑（表情槽）+ 点头（手势槽）同时活，互不撤销；
/// none 两个槽一起清。
#[test]
fn face_and_gesture_coexist_in_one_turn() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    rt.handle(
        parse_command(Some("smile"), None, None, None),
        0.0,
        &mut sink,
    );
    rt.handle(parse_command(Some("nod"), None, None, None), 0.0, &mut sink);
    assert_eq!(rt.active_face().unwrap().spec.id, "smile");
    assert_eq!(rt.active_gesture().unwrap().spec.id, "nod");
    rt.apply_frame(EXPRESSION_MS / 2.0, &mut sink);
    assert!(
        sink.overrides.contains_key("ParamMouthForm"),
        "手势不吞表情"
    );
    assert!(sink.overrides.contains_key("ParamAngleY"), "表情不吞手势");

    rt.handle(
        parse_command(Some("none"), None, None, None),
        1.0,
        &mut sink,
    );
    assert!(rt.active_face().is_none());
    assert!(rt.active_gesture().is_none());
    assert!(sink.overrides.is_empty());
}

/// **分槽撤销**：同槽换包只清该槽上一条的参数，另一槽原样。
#[test]
fn same_slot_switch_only_clears_that_slot() {
    let mut rt = PresetRuntime::default();
    rt.set_scales(PresetScales::default());
    let mut sink = FakeSink::default();
    rt.handle(
        parse_command(Some("smile"), None, None, None),
        0.0,
        &mut sink,
    );
    rt.handle(parse_command(Some("nod"), None, None, None), 0.0, &mut sink);
    rt.apply_frame(10.0, &mut sink);
    assert!(sink.overrides.contains_key("ParamEyeLSmile"));

    // 表情槽换包：smile -> surprised（只清 smile 的通道）。
    rt.handle(
        parse_command(Some("surprised"), None, None, None),
        20.0,
        &mut sink,
    );
    assert!(
        !sink.overrides.contains_key("ParamEyeLSmile"),
        "同槽换包必须清掉上一条的残留"
    );
    assert_eq!(
        rt.active_gesture().unwrap().spec.id,
        "nod",
        "另一槽（手势）的运行态不得被动到"
    );
    rt.apply_frame(450.0, &mut sink);
    assert!(sink.overrides.contains_key("ParamEyeLOpen"), "新表情已写入");
    let y = sink.overrides["ParamAngleY"];
    assert!(
        (y - (-12.0)).abs() < 1e-3,
        "共享头角仍由手势包写（实得 {y}）"
    );
}

/// 共享头 / 身通道：同帧 Face 先写、Gesture 后写，手势赢。
#[test]
fn gesture_wins_the_shared_head_channel() {
    let mut rt = PresetRuntime::default();
    rt.set_scales(PresetScales::default());
    let mut sink = FakeSink::default();
    rt.handle(
        parse_command(Some("smile"), None, None, None),
        0.0,
        &mut sink,
    );
    rt.handle(parse_command(Some("nod"), None, None, None), 0.0, &mut sink);
    rt.apply_frame(MOTION_MS / 2.0, &mut sink);
    let y = sink.overrides.get("ParamAngleY").copied().unwrap();
    assert!(
        (y - (-12.0)).abs() < 1e-3,
        "共享 ParamAngleY 应由手势包（nod 中途 -20）赢，实得 {y}"
    );
}

// ─────────────────────────────────────────── intensity 一等公民

/// 普通包：intensity 线性缩放五官 + 头身，且单调可辨。
#[test]
fn intensity_scales_face_and_head_body_monotonically() {
    let smile = preset("smile").unwrap();
    let mouth = |i: f32| out(smile, 1.0, i, "ParamMouthForm");
    let head = |i: f32| out(smile, 1.0, i, "ParamAngleY");
    let body = |i: f32| out(smile, 1.0, i, "ParamBodyAngleY");
    for (a, b) in [(1.0, 2.0), (2.0, 3.0)] {
        assert!(mouth(a) < mouth(b), "嘴形应随 intensity 单调：{a}->{b}");
        assert!(head(a) < head(b), "头角应随 intensity 单调：{a}->{b}");
        assert!(body(a) < body(b), "半身应随 intensity 单调：{a}->{b}");
    }
    assert!(
        (mouth(3.0) - 3.0 * mouth(1.0)).abs() < 1e-6,
        "普通包是线性缩放"
    );
}

/// **unhappy@1 vs @3 在眉 / 眼 / 头角上异号或异档**——这是合并 sad+angry 的机器证据。
#[test]
fn unhappy_morphs_sad_low_to_angry_high() {
    let unhappy = preset("unhappy").unwrap();
    assert!(unhappy.morph.is_some(), "unhappy 必须是 morph 包");
    let brow = |i: f32| out(unhappy, 1.0, i, "ParamBrowLY");
    let eye = |i: f32| out(unhappy, 1.0, i, "ParamEyeLOpen");
    let head_y = |i: f32| out(unhappy, 1.0, i, "ParamAngleY");

    // 低 = 难过相：眉上挑（正）、垂眼、低头。
    assert!(brow(1.0) > 0.2, "unhappy@1 眉应上挑，实得 {}", brow(1.0));
    assert!(eye(1.0) < 0.9, "unhappy@1 该是垂眼，实得 {}", eye(1.0));
    assert!(head_y(1.0) < -5.0, "unhappy@1 该低头，实得 {}", head_y(1.0));
    // 高 = 生气相：眉下压（负）、瞪眼、略抬。
    assert!(brow(3.0) < -0.5, "unhappy@3 眉应下压，实得 {}", brow(3.0));
    assert!(eye(3.0) > 1.0, "unhappy@3 该瞪眼，实得 {}", eye(3.0));
    assert!(
        head_y(3.0) > -1.0,
        "unhappy@3 不该低头，实得 {}",
        head_y(3.0)
    );
    // 异号 / 异档。
    assert!(brow(1.0).signum() != brow(3.0).signum(), "眉形必须异号");
    assert!(eye(3.0) - eye(1.0) > 0.3, "眼开合必须异档");
    // 三档单调（1 -> 2 -> 3 连续过渡，不是跳变）。
    assert!(brow(1.0) > brow(2.0) && brow(2.0) > brow(3.0));
    assert!(eye(1.0) < eye(2.0) && eye(2.0) < eye(3.0));
    assert!(head_y(1.0) < head_y(2.0) && head_y(2.0) < head_y(3.0));
    // 对称（左右眉/眼同值）。
    for i in [1.0, 2.0, 3.0] {
        assert_eq!(
            out(unhappy, 1.0, i, "ParamBrowLY"),
            out(unhappy, 1.0, i, "ParamBrowRY")
        );
        assert_eq!(
            out(unhappy, 1.0, i, "ParamEyeLOpen"),
            out(unhappy, 1.0, i, "ParamEyeROpen")
        );
    }
}

/// intensity < low（=1）时从中性线性升到难过相；=0 全零（等价撤销）。
#[test]
fn unhappy_below_the_low_anchor_scales_from_neutral() {
    let unhappy = preset("unhappy").unwrap();
    for (_, v) in frame_params_scaled_by(unhappy, 1.0, 0.0, PresetScales::default()) {
        assert_eq!(v, 0.0, "intensity=0 应全零");
    }
    let half = out(unhappy, 1.0, 0.5, "ParamBrowLY");
    let full = out(unhappy, 1.0, 1.0, "ParamBrowLY");
    assert!((half - full / 2.0).abs() < 1e-6, "半强度 = 难过相的一半");
}

/// morph 包的通道集合两极一致（否则插值会「凭空冒出一条通道」）。
#[test]
fn shipped_unhappy_morph_poles_cover_the_same_channels() {
    let table = PresetTable::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .unwrap();
    let spec = table.get("unhappy").unwrap();
    let m = spec.morph.expect("shipped unhappy 必须是 morph 包");
    let low: Vec<&str> = spec.params.iter().map(|(k, _)| *k).collect();
    let high: Vec<&str> = m.high_params.iter().map(|(k, _)| *k).collect();
    assert_eq!(low.len(), high.len(), "两极通道数应一致");
    for id in &low {
        assert!(high.contains(id), "high 极缺少 low 极通道 {id}");
    }
    assert_eq!(m.low, 1.0);
    assert_eq!(m.high, 3.0);
}

// ─────────────────────────────────────────── 用户可调幅度

#[test]
fn scale_class_classifies_head_body_and_expression() {
    assert_eq!(scale_class("ParamAngleX"), ScaleClass::Head);
    assert_eq!(scale_class("ParamBodyAngleX"), ScaleClass::Body);
    assert_eq!(scale_class("ParamMouthForm"), ScaleClass::Expression);
    assert_eq!(scale_class("ParamBrowLY"), ScaleClass::Expression);
}

#[test]
fn scales_multiply_their_own_channel_only() {
    let nod = preset("nod").unwrap();
    let p = 0.5;
    let base: HashMap<&'static str, f32> = frame_params(nod, p).into_iter().collect();
    let head_only: HashMap<&'static str, f32> = frame_params_scaled_by(
        nod,
        p,
        1.0,
        PresetScales {
            head: 1.25,
            body: 1.0,
            expression: 1.0,
        },
    )
    .into_iter()
    .collect();
    assert!((head_only["ParamAngleY"] - base["ParamAngleY"] * 1.25).abs() < 1e-6);
    assert!((head_only["ParamBodyAngleY"] - base["ParamBodyAngleY"]).abs() < 1e-6);
}

#[test]
fn scaled_values_are_clamped_to_channel_redline() {
    let nod = preset("nod").unwrap();
    let vals = frame_params_scaled_by(
        nod,
        0.5,
        3.0,
        PresetScales {
            head: 2.5,
            body: 2.5,
            expression: 2.5,
        },
    );
    for (id, v) in vals {
        let limit = amplitude_limit(id);
        assert!(v.abs() <= limit + 1e-6, "{id} 未钳到 {limit}：{v}");
    }
}

#[test]
fn scales_are_clamped_and_nan_safe() {
    assert_eq!(clamp_scale(f32::NAN), 1.0);
    assert_eq!(clamp_scale(99.0), MAX_SCALE);
    assert_eq!(clamp_scale(0.0), MIN_SCALE);
    assert_eq!(
        PresetScales::from_parts(Some(f64::NAN), None, Some(9.0)),
        PresetScales {
            head: 1.0,
            body: 1.0,
            expression: MAX_SCALE,
        }
    );
}

#[test]
fn runtime_applies_scales_to_frames() {
    let mut rt = PresetRuntime::default();
    rt.set_scales(PresetScales::default());
    let mut sink = FakeSink::default();
    rt.handle(parse_command(Some("nod"), None, None, None), 0.0, &mut sink);
    rt.apply_frame(MOTION_MS / 2.0, &mut sink);
    let base = sink.overrides["ParamAngleY"];
    assert!((base - (-12.0)).abs() < 1e-6);
}

#[test]
fn fresh_runtime_uses_product_default_scales() {
    let rt = PresetRuntime::default();
    assert_eq!(rt.scales(), PresetScales::PRODUCT_DEFAULT);
}

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

/// 峰值系数：包络 × 波形在 progress 上的数值上确界（真的采样算出来）。
///
/// - 表情：静态保持，恒 1.0；
/// - Single：sin(pi*p) 的上确界 = 1.0（p = 0.5）；
/// - Oscillate：逐点采样取最大（shake cycles=2 → 0.9285，**不许写 1.0**）。
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
fn all_tables() -> Vec<(&'static str, Vec<&'static PresetSpec>)> {
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
        "nod" => Some(("ParamAngleY", "ParamBodyAngleY")),
        // T9：look_up / look_down 的主轴是 Y（与 nod 同轴、方向相反）。
        "look_up" | "look_down" => Some(("ParamAngleY", "ParamBodyAngleY")),
        "shake" | "look_left" | "look_right" => Some(("ParamAngleX", "ParamBodyAngleX")),
        "tilt_left" | "tilt_right" => Some(("ParamAngleZ", "ParamBodyAngleZ")),
        _ => None,
    }
}

/// shake 的峰值系数**真的算出来**（不是 1.0）：两个旋钮的红线账都依赖它。
#[test]
fn shake_oscillate_peak_is_computed_not_assumed() {
    let shake = preset("shake").unwrap();
    let peak = peak_gain(shake);
    println!("shake peak_gain = {peak:.6}（写死 1.0 会低估约 7.7% 的行程）");
    assert!(
        (peak - 0.9285).abs() < 1e-3,
        "shake(cycles=2) 峰值应为 0.9285，实得 {peak}"
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
        checked, 16,
        "两表 × 8 条手势包（T9 起含 look_up / look_down）都该被覆盖"
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
