//! 双槽 / 强度 morph / 用户可调幅度的回归（从 tests.rs 拆出）。

use super::motion_curve::HOLD_END;
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
    // 表情带攻击段（2026-10-15）：16ms 处 = 满值 × curve(progress)，仍然压在 idle 之上。
    let p = 16.0 / EXPRESSION_MS;
    let mouth = out(preset("smile").unwrap(), p, 1.0, "ParamMouthForm");
    let brow = out(preset("smile").unwrap(), p, 1.0, "ParamBrowLY");
    assert!(mouth > 0.0 && brow > 0.0, "前提：还在攻击段内");
    assert_eq!(sink.effective("ParamMouthForm"), Some(mouth));
    assert_eq!(sink.effective("ParamBrowLY"), Some(brow));
    assert!(!sink.overrides.contains_key("ParamMouthOpenY"));

    rt.apply_frame(1_016.0 + EXPRESSION_MS, &mut sink);
    assert!(rt.active().is_none());
    assert!(sink.overrides.is_empty(), "到点必须整批撤销");
    assert_eq!(sink.effective("ParamMouthForm"), Some(0.5));
}

#[test]
fn switching_keeps_stale_until_attack_end_and_revoke_releases() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    rt.handle(
        parse_command(Some("look_left"), None, None, None),
        0.0,
        &mut sink,
    );
    rt.apply_frame(100.0, &mut sink);
    assert!(sink.overrides.contains_key("ParamAngleX"));

    // 同槽换包（手势槽 look_left -> nod）：旧通道**先接上**，到攻击段结束才清
    //（2026-10-15 执行单 §1；旧行为是这一步立刻清）。
    rt.handle(
        parse_command(Some("nod"), None, None, None),
        200.0,
        &mut sink,
    );
    assert!(
        sink.overrides.contains_key("ParamAngleX"),
        "接力窗口内旧通道不得先归零"
    );
    rt.apply_frame(300.0, &mut sink);
    assert!(sink.overrides.contains_key("ParamAngleY"));
    assert!(sink.overrides.contains_key("ParamAngleX"), "窗口内仍接旧值");

    // nod 0.22 × 900ms = 198ms 后进入保持段：旧通道清掉、只剩新包。
    rt.apply_frame(500.0, &mut sink);
    assert!(!sink.overrides.contains_key("ParamAngleX"));
    assert!(sink.overrides.contains_key("ParamAngleY"));

    // Revoke（2026-10-15）：不从当帧清参数，从屏幕值收到 0，到点才清槽。
    assert!(rt.handle(
        parse_command(Some("none"), None, None, None),
        600.0,
        &mut sink
    ));
    assert!(
        sink.overrides.contains_key("ParamAngleY"),
        "Revoke 当帧不清"
    );
    assert!(rt.active_gesture().is_some(), "放回途中槽还占着");
    rt.apply_frame(600.0, &mut sink);
    assert!(sink.overrides.contains_key("ParamAngleY"), "q = 0 值不动");
    // 放回时长 = nod 900ms × (1 - HOLD_END) = 342ms。
    rt.apply_frame(600.0 + MOTION_MS * (1.0 - HOLD_END), &mut sink);
    assert!(sink.overrides.is_empty(), "到点才清 override");
    assert!(rt.active().is_none(), "到点才清槽");
}

#[test]
fn motion_ramps_then_expiry_releases_before_clearing() {
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
    // 到期帧：与 Revoke 同一条放回——槽仍占着，通道仍写（p = 1 处 from ≈ 0）。
    rt.apply_frame(MOTION_MS, &mut sink);
    assert!(
        rt.active_gesture().is_some(),
        "手势到期应开一段放回，不是立即清槽"
    );
    assert!(
        rt.active_gesture().unwrap().release.is_some(),
        "到期帧应开 Release"
    );
    assert!(
        sink.overrides.contains_key("ParamAngleY"),
        "到期帧仍要写手势通道（q = 0，值 = from ≈ 0）"
    );
    // 走满 ttl_ms × (1 - HOLD_END) 才清槽 + 清 override。
    rt.apply_frame(MOTION_MS + MOTION_MS * (1.0 - HOLD_END), &mut sink);
    assert!(rt.active_gesture().is_none(), "放回走满才清槽");
    assert!(sink.overrides.is_empty(), "放回走满才清 override");
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
    rt.apply_frame(MOTION_MS / 2.0, &mut sink);
    assert!(
        sink.overrides.contains_key("ParamMouthForm"),
        "手势不吞表情"
    );
    assert!(sink.overrides.contains_key("ParamAngleY"), "表情不吞手势");

    // Revoke：两个槽各自从屏幕值放回，按**自己的** ttl 到点（2026-10-15）。
    rt.handle(
        parse_command(Some("none"), None, None, None),
        1.0,
        &mut sink,
    );
    assert!(
        rt.active_face().is_some() && rt.active_gesture().is_some(),
        "Revoke 后两个槽都在放回"
    );
    // 手势 900 × 0.38 = 342ms 先清；表情 2600 × 0.38 = 988ms 后清。
    rt.apply_frame(1.0 + MOTION_MS * (1.0 - HOLD_END), &mut sink);
    assert!(rt.active_gesture().is_none(), "手势槽到点清");
    assert!(rt.active_face().is_some(), "表情槽还在放回");
    rt.apply_frame(1.0 + EXPRESSION_MS * (1.0 - HOLD_END), &mut sink);
    assert!(rt.active_face().is_none(), "表情槽到点清");
    assert!(sink.overrides.is_empty(), "两个槽的 override 都已清");
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

    // 表情槽换包：smile -> surprised（只动表情槽：旧通道走接力窗口，
    // 到攻击段结束才清——2026-10-15 执行单 §1）。
    rt.handle(
        parse_command(Some("surprised"), None, None, None),
        20.0,
        &mut sink,
    );
    assert!(
        sink.overrides.contains_key("ParamEyeLSmile"),
        "接力窗口内旧表情通道先接上，不得立刻清"
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
    // surprised 在 20 + 0.22 × 2600 = 592ms 进入保持段：smile 独有的通道被清掉。
    rt.apply_frame(600.0, &mut sink);
    assert!(
        !sink.overrides.contains_key("ParamEyeLSmile"),
        "到 ATTACK_END 必须清掉上一条的残留"
    );
    assert!(sink.overrides.contains_key("ParamEyeLOpen"));
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
    let mouth = |i: f32| out(smile, 0.5, i, "ParamMouthForm");
    let head = |i: f32| out(smile, 0.5, i, "ParamAngleY");
    let body = |i: f32| out(smile, 0.5, i, "ParamBodyAngleY");
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
    let brow = |i: f32| out(unhappy, 0.5, i, "ParamBrowLY");
    let eye = |i: f32| out(unhappy, 0.5, i, "ParamEyeLOpen");
    let head_y = |i: f32| out(unhappy, 0.5, i, "ParamAngleY");

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
            out(unhappy, 0.5, i, "ParamBrowLY"),
            out(unhappy, 0.5, i, "ParamBrowRY")
        );
        assert_eq!(
            out(unhappy, 0.5, i, "ParamEyeLOpen"),
            out(unhappy, 0.5, i, "ParamEyeROpen")
        );
    }
}

/// intensity < low（=1）时从中性线性升到难过相；=0 全零（等价撤销）。
#[test]
fn unhappy_below_the_low_anchor_scales_from_neutral() {
    let unhappy = preset("unhappy").unwrap();
    for (_, v) in frame_params_scaled_by(unhappy, 0.5, 0.0, PresetScales::default()) {
        assert_eq!(v, 0.0, "intensity=0 应全零");
    }
    let half = out(unhappy, 0.5, 0.5, "ParamBrowLY");
    let full = out(unhappy, 0.5, 1.0, "ParamBrowLY");
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
