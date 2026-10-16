//! 手势起落曲线 / 波形 / 半身随动的原生回归（2026-10-14 从 `tests/mod.rs` 拆出）。
//!
//! 拆出来的直接原因：那边的判据加强后文件会顶破 `crates/*/src` > 500 行的棘轮
//! （`xtask code_stats --check --only lines`，上限 44）。这里独立一行数余量。
//!
//! 内容 = 起落曲线（攻击 → 保持 → 放回）的两端与保持段、`envelope` 的旧两端口径、
//! 以及**身体滞后**（`ParamBodyAngle*` 走 `body_p = (p - 0.12) / 0.88`）后的幅度关系。

use super::motion_curve::{ATTACK_END, BODY_LAG, HOLD_END};
use super::*;

/// frame_params 里某一通道的值（不存在 → panic）。
fn at(spec: &'static PresetSpec, p: f64, want: &str) -> f32 {
    frame_params(spec, p)
        .into_iter()
        .find(|(id, _)| *id == want)
        .unwrap_or_else(|| panic!("{} 缺 {want}", spec.id))
        .1
}

// ─────────────────────────────────────────── 包络 / 波形

#[test]
fn envelope_is_smooth_and_zero_at_both_ends() {
    assert_eq!(envelope(0.0), 0.0);
    assert_eq!(envelope(1.0), 0.0);
    assert!((envelope(0.5) - 1.0).abs() < 1e-6);
    assert_eq!(envelope(-5.0), 0.0);
    assert_eq!(envelope(5.0), 0.0);
}

/// **手势**保持段幅值恰好 1、两端恰好 0（2026-10-14 起落曲线；2026-10-15 表情也走它）。
#[test]
fn motion_holds_at_peak_and_zero_at_both_ends() {
    let nod = preset("nod").unwrap();
    assert_eq!(nod.kind, PresetKind::Motion);
    assert_eq!(at(nod, 0.0, "ParamAngleY"), 0.0);
    assert_eq!(at(nod, 1.0, "ParamAngleY"), 0.0, "末尾归零，撤销不跳变");
    // 保持段（[0.22, 0.62]）的幅值乘子恰好 1，于是 |ParamAngleY| = 表内峰值。
    let peak = param_of(nod, "ParamAngleY").unwrap().abs();
    for p in [ATTACK_END, 0.3, 0.5, HOLD_END] {
        assert_eq!(
            at(nod, p, "ParamAngleY").abs(),
            peak,
            "保持段内 |ParamAngleY| 应为峰值（p={p}）"
        );
    }
    // 段外（攻击 / 放回）必须**小于**峰值——否则「保持」段没有意义。
    for p in [0.1, 0.9] {
        assert!(
            at(nod, p, "ParamAngleY").abs() < peak,
            "段外应小于峰值（p={p}）"
        );
    }
}

/// **表情与手势同一条曲线（2026-10-15 执行单 §1）**：进度 0 / 1 恰好 0，保持段恰好是
/// 不乘曲线的满值；不再是「整段恒 1」。
#[test]
fn expression_follows_the_same_curve_as_motion() {
    let smile = preset("smile").unwrap();
    let mouth = param_of(smile, "ParamMouthForm").unwrap();
    assert_eq!(at(smile, 0.0, "ParamMouthForm"), 0.0, "进度 0 恰好 0");
    assert_eq!(at(smile, 1.0, "ParamMouthForm"), 0.0, "进度 1 恰好 0");
    for p in [ATTACK_END, 0.3, 0.5, HOLD_END] {
        assert_eq!(
            at(smile, p, "ParamMouthForm"),
            mouth,
            "保持段 = 满值（p={p}）"
        );
    }
    for p in [0.1, 0.9] {
        assert!(
            at(smile, p, "ParamMouthForm").abs() < mouth.abs(),
            "进 / 出段应小于满值（p={p}）"
        );
    }
    // 表情的头 / 身也走同一条曲线（不是「五官动、头身恒满」）。
    assert_eq!(at(smile, 0.0, "ParamAngleY"), 0.0);
    assert_eq!(at(smile, 1.0, "ParamAngleY"), 0.0);
    assert_eq!(
        at(smile, 0.5, "ParamAngleY"),
        param_of(smile, "ParamAngleY").unwrap()
    );
    assert_ne!(
        frame_params(smile, 0.1),
        frame_params(smile, 0.9),
        "进 / 出段不再同值（旧行为是整段恒 1）"
    );
}

/// 运行期：表情槽在进度 0 写出的参数**全是 0**，保持段等于满值，到点那一帧槽已清。
#[test]
fn expression_slot_starts_at_zero_holds_then_clears_at_ttl() {
    let mut rt = PresetRuntime::default();
    rt.set_scales(PresetScales::default()); // identity：逐值对照 frame_params
    let mut sink = FakeSink::default();
    let smile = preset("smile").unwrap();
    rt.handle(
        parse_command(Some("smile"), None, None, None),
        0.0,
        &mut sink,
    );
    rt.apply_frame(0.0, &mut sink);
    assert!(!sink.overrides.is_empty());
    for (id, value) in &sink.overrides {
        assert_eq!(*value, 0.0, "{id} 在进度 0 必须恰好 0");
    }
    rt.apply_frame(smile.duration_ms * 0.5, &mut sink);
    for (id, value) in frame_params(smile, 0.5) {
        assert_eq!(sink.overrides[id], value, "{id} 保持段 = 不乘曲线的满值");
    }
    rt.apply_frame(smile.duration_ms, &mut sink);
    assert!(rt.active_face().is_none(), "到点清槽");
    assert!(sink.overrides.is_empty(), "到点 override 全清");
}

#[test]
fn progress_is_clamped_to_one() {
    let nod = preset("nod").unwrap();
    assert_eq!(progress_at(0.0, nod.duration_ms), 0.0);
    assert!((progress_at(nod.duration_ms / 2.0, nod.duration_ms) - 0.5).abs() < 1e-9);
    assert_eq!(progress_at(nod.duration_ms * 10.0, nod.duration_ms), 1.0);
    assert_eq!(progress_at(0.0, 0.0), 1.0);
}

#[test]
fn motion_gain_falls_back_and_stays_bounded() {
    let bad = motion_gain(MotionWave::Oscillate { cycles: f64::NAN }, 0.125);
    let good = motion_gain(
        MotionWave::Oscillate {
            cycles: SHAKE_CYCLES,
        },
        0.125,
    );
    assert!((bad - good).abs() < 1e-6, "NaN cycles 应回落缺省");
    for k in 0..=100 {
        let p = k as f64 / 100.0;
        assert!(motion_gain(MotionWave::Oscillate { cycles: 3.0 }, p).abs() <= 1.0 + 1e-6);
        assert!(motion_gain(MotionWave::Single, p) >= 0.0);
    }
    // 2026-10-14：Single = 起落曲线（**不再**逐点等于 envelope）；两端恰好 0。
    assert_eq!(motion_gain(MotionWave::Single, 0.0), 0.0);
    assert_eq!(motion_gain(MotionWave::Single, 1.0), 0.0);
    for k in 0..=40 {
        let p = ATTACK_END + (HOLD_END - ATTACK_END) * (k as f64 / 40.0);
        assert_eq!(
            motion_gain(MotionWave::Single, p),
            1.0,
            "保持段（{ATTACK_END}..={HOLD_END}）该通道应为峰值 1（p={p}）"
        );
    }
    let mut prev = 0.0f32;
    for k in 1..20 {
        let p = ATTACK_END * (k as f64 / 20.0);
        let g = motion_gain(MotionWave::Single, p);
        assert!(g > prev && g < 1.0, "攻击段应单调升且在 (0,1)（p={p}）");
        prev = g;
    }
    // 与旧曲线的差别是**判据**（不是「差不多」）：同一点上两者不相等。
    assert_ne!(motion_gain(MotionWave::Single, 0.11), envelope(0.11));
}

/// **手势幅度 / 半身随动（2026-10-14 起带身体滞后）**。
///
/// 身体**不再**与头逐采样同相：身体进度 `body_p = (p - 0.12) / 0.88` 对 shake
/// （`cycles=2`）最多把相位推开约 86°，所以「头到峰时身还在对面」是**设计内**的
/// ——执行单也只要求「靠近头的过零点允许身体短暂反号」。这里钉住的是：
/// 两端归零（不剪尾巴）、头左右变号 >= 2、峰值仍肉眼明显、身体**真的**晚起步，
/// 以及**幅度峰值**上身/头仍是表内 0.325（设计窗口 [0.25, 0.55]）。
#[test]
fn shake_oscillates_both_ways_with_body_in_phase() {
    let shake = preset("shake").unwrap();
    assert_eq!(shake.kind, PresetKind::Motion);
    assert!(matches!(shake.wave, MotionWave::Oscillate { .. }));
    for id in ["ParamAngleX", "ParamBodyAngleX"] {
        assert_eq!(at(shake, 0.0, id), 0.0, "{id} 起点必须为 0");
        assert_eq!(
            at(shake, 1.0, id),
            0.0,
            "{id} 终点必须为 0（槽到期不剪尾巴）"
        );
    }
    let mut last_sign = 0i32;
    let mut changes = 0usize;
    let mut peak = 0.0f32;
    let mut body_peak = 0.0f32;
    let mut opposite = 0usize;
    let mut body_zero_while_head_moved = false;
    for k in 0..400 {
        let p = (k as f64 + 0.5) / 400.0;
        let h = at(shake, p, "ParamAngleX");
        let b = at(shake, p, "ParamBodyAngleX");
        peak = peak.max(h.abs());
        body_peak = body_peak.max(b.abs());
        if h.abs() > 1e-3 && b.abs() > 1e-3 && h.signum() != b.signum() {
            opposite += 1;
        }
        // 头已经离开默认、身体还没起步 = 滞后（p <= BODY_LAG 时身体恰好 0）。
        if p <= BODY_LAG && h.abs() > 1e-3 {
            assert_eq!(b, 0.0, "身体滞后：p <= {BODY_LAG} 时身必须还是 0（p={p}）");
            body_zero_while_head_moved = true;
        }
        let s = if h > 1e-3 {
            1
        } else if h < -1e-3 {
            -1
        } else {
            last_sign
        };
        if last_sign != 0 && s != last_sign {
            changes += 1;
        }
        last_sign = s;
    }
    assert!(changes >= 2, "shake 必须左右变号 >=2 次（实得 {changes}）");
    // 2026-09-24 重标定：主轴表值 22 → 12；2026-10-14 起峰值系数 = 1.0（保持段满幅）。
    assert!(peak >= 10.0, "摇头峰值要肉眼明显（实得 {peak}）");
    assert!(body_zero_while_head_moved, "身体滞后必须真的晚起步");
    assert!(
        opposite > 0,
        "身体滞后必须真的让头身相位分开（至少一个采样异号）"
    );
    let ratio = body_peak / peak;
    assert!(
        (0.25..=0.55).contains(&ratio),
        "身/头幅度峰比 {ratio} 应在 0.25~0.55（表内 0.325）"
    );
}

#[test]
fn nod_and_look_carry_body_follow() {
    let hy = param_of(preset("nod").unwrap(), "ParamAngleY").unwrap();
    let by = param_of(preset("nod").unwrap(), "ParamBodyAngleY").unwrap();
    assert!(
        hy.signum() == by.signum(),
        "nod 身必须与头同向：{hy} / {by}"
    );
    let ratio = (by / hy).abs();
    assert!(
        (0.3..=0.5).contains(&ratio),
        "nod 身幅比 {ratio} 应在 0.3~0.5"
    );

    for id in ["look_left", "look_right"] {
        let s = preset(id).unwrap();
        let hx = param_of(s, "ParamAngleX").unwrap();
        let hz = param_of(s, "ParamAngleZ").unwrap();
        let bx = param_of(s, "ParamBodyAngleX").unwrap();
        let bz = param_of(s, "ParamBodyAngleZ").unwrap();
        assert!(hx.signum() == bx.signum(), "{id} BodyAngleX 必须与头同向");
        assert!(hz.signum() == bz.signum(), "{id} BodyAngleZ 必须与头同向");
        assert!(bx.abs() < hx.abs() && bz.abs() < hz.abs());
    }
    assert!(param_of(preset("shake").unwrap(), "ParamBodyAngleX").is_some());
}
