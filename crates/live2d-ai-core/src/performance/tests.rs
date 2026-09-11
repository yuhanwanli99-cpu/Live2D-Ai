//! 表演参数曲线的契约守护单测：覆盖 t=0/端点中性、强度单调性、确定性、
//! look_around 双向扫视、interrupt 立即回中、抢占不残留、非法 dt 安全、
//! 无状态采样截断 + 时长元数据。

use super::{
    ActionId, BODY_ANGLE_LIMIT, EYE_OPEN_MAX, EYE_OPEN_MIN, HEAD_ANGLE_LIMIT, NORMALIZED_LIMIT,
    ParameterFrame, PerformancePlayer, SampleStatus, Strength, duration, sample_at,
};
use crate::action::SemanticAction;

/// 固定测试步长（60 Hz）。
const DT: f32 = 1.0 / 60.0;
/// 步数预算：覆盖最长动作（look_around 2.4s ≈ 144 帧）并留余量。
const MAX_STEPS: usize = 200;

fn act(id: ActionId, s: Strength) -> SemanticAction {
    SemanticAction::new(id, s, crate::ActionSource::LlmTool)
}

/// 从全新播放器采一段帧序列（含结束后的空闲帧）。
fn sample_frames(id: ActionId, s: Strength, steps: usize) -> Vec<ParameterFrame> {
    let mut player = PerformancePlayer::new();
    player.play(act(id, s));
    let mut f = ParameterFrame::neutral();
    let mut frames = Vec::with_capacity(steps);
    for _ in 0..steps {
        player.update(DT, &mut f);
        frames.push(f);
    }
    frames
}

/// 各通道相对中性的偏离幅度（顺序固定）。
fn abs_channels(f: &ParameterFrame) -> [f32; 11] {
    [
        f.head_angle_x.abs(),
        f.head_angle_y.abs(),
        f.head_angle_z.abs(),
        f.eye_x.abs(),
        f.eye_y.abs(),
        (f.eye_open_scale - 1.0).abs(),
        f.brow_y.abs(),
        f.mouth_form.abs(),
        f.body_angle_x.abs(),
        f.body_angle_y.abs(),
        f.body_angle_z.abs(),
    ]
}

fn assert_in_bounds(f: &ParameterFrame) {
    for v in [f.head_angle_x, f.head_angle_y, f.head_angle_z] {
        assert!(
            v.is_finite() && v.abs() <= HEAD_ANGLE_LIMIT,
            "head 越界: {v}"
        );
    }
    for v in [f.body_angle_x, f.body_angle_y, f.body_angle_z] {
        assert!(
            v.is_finite() && v.abs() <= BODY_ANGLE_LIMIT,
            "body 越界: {v}"
        );
    }
    for v in [f.eye_x, f.eye_y, f.brow_y, f.mouth_form] {
        assert!(
            v.is_finite() && v.abs() <= NORMALIZED_LIMIT,
            "归一化通道越界: {v}"
        );
    }
    assert!(
        f.eye_open_scale.is_finite() && (EYE_OPEN_MIN..=EYE_OPEN_MAX).contains(&f.eye_open_scale),
        "eye_open_scale 越界: {}",
        f.eye_open_scale
    );
}

/// 要求：t=0 与 t=end 都是精确中性帧；结束只报一次 Finished，之后恒 Idle+中性。
#[test]
fn t0_and_end_are_exactly_neutral() {
    for id in ActionId::ALL {
        for s in Strength::ALL {
            let mut player = PerformancePlayer::new();
            player.play(act(id, s));
            let mut f = ParameterFrame::neutral();

            let status = player.update(0.0, &mut f);
            assert_eq!(status, SampleStatus::Playing { progress: 0.0 });
            assert!(f.is_neutral(), "{id:?} t=0 必须中性");

            let mut finished = false;
            for _ in 0..MAX_STEPS {
                match player.update(DT, &mut f) {
                    SampleStatus::Finished => {
                        finished = true;
                        assert!(f.is_neutral(), "{id:?} 结束帧必须精确中性");
                        break;
                    }
                    SampleStatus::Playing { .. } => {}
                    SampleStatus::Idle => unreachable!("表演中不应回到 Idle"),
                }
            }
            assert!(finished, "{id:?} 应在步数预算内结束");
            assert_eq!(player.active(), None);

            for _ in 0..3 {
                assert_eq!(player.update(DT, &mut f), SampleStatus::Idle);
                assert!(f.is_neutral(), "{id:?} 结束后必须保持中性");
            }
        }
    }
}

/// 要求：六动作 × 三强度的每一采样帧都有限且落在 Bai 安全范围内。
#[test]
fn every_sampled_frame_is_within_safe_bounds() {
    for id in ActionId::ALL {
        for s in Strength::ALL {
            for f in sample_frames(id, s, MAX_STEPS) {
                assert_in_bounds(&f);
            }
        }
    }
}

/// 要求：Low/Medium/High 逐帧、逐通道幅度单调不减（曲线对增益线性）。
#[test]
fn strength_scales_every_channel_monotonically() {
    const STEPS: usize = 90;
    for id in ActionId::ALL {
        let low = sample_frames(id, Strength::Low, STEPS);
        let mid = sample_frames(id, Strength::Medium, STEPS);
        let high = sample_frames(id, Strength::High, STEPS);
        for ((l, m), h) in low.iter().zip(&mid).zip(&high) {
            let (lv, mv, hv) = (abs_channels(l), abs_channels(m), abs_channels(h));
            for i in 0..lv.len() {
                assert!(
                    hv[i] >= mv[i] && mv[i] >= lv[i],
                    "{id:?} 通道#{i} 非单调: low={} mid={} high={}",
                    lv[i],
                    mv[i],
                    hv[i]
                );
            }
        }
    }
}

/// 要求：相同输入序列 ⇒ 位级相同的帧序列（含同一播放器重放）。
#[test]
fn sampling_is_deterministic() {
    let a = sample_frames(ActionId::Surprise, Strength::High, 100);
    let b = sample_frames(ActionId::Surprise, Strength::High, 100);
    assert_eq!(a, b);

    let mut player = PerformancePlayer::new();
    player.play(act(ActionId::Surprise, Strength::High));
    let mut f = ParameterFrame::neutral();
    let mut replay = Vec::with_capacity(100);
    for _ in 0..100 {
        player.update(DT, &mut f);
        replay.push(f);
    }
    assert_eq!(replay, a);
}

/// 要求：look_around 双向扫视、视线穿过中心、头部小幅跟随，且结束精确回中。
#[test]
fn look_around_scans_both_sides_then_recenters() {
    let frames = sample_frames(ActionId::LookAround, Strength::Medium, MAX_STEPS);
    assert!(frames.iter().any(|f| f.eye_x > 0.4), "应看向一侧");
    assert!(frames.iter().any(|f| f.eye_x < -0.4), "应看向另一侧");
    // 正弦网格采样允许的小残差（理论峰值 ~0.03）。
    assert!(
        frames.iter().any(|f| f.eye_x.abs() < 0.05),
        "扫视应穿过中心"
    );
    assert!(
        frames.iter().any(|f| f.head_angle_x.abs() > 2.0),
        "头部应小幅跟随"
    );
    assert!(
        frames.last().is_some_and(|f| f.is_neutral()),
        "结束必须回中"
    );
}

/// 要求：interrupt 立即回到精确 neutral，空闲态 interrupt 幂等无害。
#[test]
fn interrupt_snaps_to_neutral_immediately() {
    let mut player = PerformancePlayer::new();
    player.play(act(ActionId::Nod, Strength::Medium));
    let mut f = ParameterFrame::neutral();
    for _ in 0..10 {
        player.update(DT, &mut f);
    }
    assert!(!f.is_neutral(), "前置：打断前应为非中性帧");

    player.interrupt();
    assert_eq!(player.active(), None);
    for _ in 0..5 {
        assert_eq!(player.update(DT, &mut f), SampleStatus::Idle);
        assert!(f.is_neutral());
    }

    player.interrupt(); // 空闲态再打断：无害。
    player.play(act(ActionId::Tilt, Strength::Low));
    assert_eq!(
        player.update(0.0, &mut f),
        SampleStatus::Playing { progress: 0.0 }
    );
    assert!(f.is_neutral());
}

/// 要求：抢占（对应 Transition）重置时间轴，旧动作完全不叠加。
#[test]
fn preempt_resets_timeline_without_blending() {
    let mut player = PerformancePlayer::new();
    player.play(act(ActionId::Nod, Strength::Medium));
    let mut f = ParameterFrame::neutral();
    for _ in 0..15 {
        player.update(DT, &mut f);
    }
    assert!(!f.is_neutral());

    // 抢占换演后与全新播放器的 shake_no 逐位一致。
    player.play(act(ActionId::ShakeNo, Strength::Low));
    let mut reference = PerformancePlayer::new();
    reference.play(act(ActionId::ShakeNo, Strength::Low));
    let mut rf = ParameterFrame::neutral();
    for _ in 0..40 {
        player.update(DT, &mut f);
        reference.update(DT, &mut rf);
        assert_eq!(f, rf, "抢占后不得残留旧动作分量");
    }
}

/// 要求：NaN / 负 dt / 零 dt 不推进时间轴；巨大有限 dt 安全终止；NaN 不入帧。
#[test]
fn nan_negative_and_huge_dt_are_safe() {
    let mut player = PerformancePlayer::new();
    player.play(act(ActionId::ShakeNo, Strength::Medium));
    let mut f = ParameterFrame::neutral();
    player.update(DT, &mut f);
    let baseline = f;

    for bad in [f32::NAN, -DT, 0.0, f32::NEG_INFINITY] {
        let status = player.update(bad, &mut f);
        assert!(matches!(status, SampleStatus::Playing { .. }));
        assert_eq!(f, baseline, "非法 dt({bad}) 不得改变输出帧");
    }

    assert_eq!(player.update(1.0e9, &mut f), SampleStatus::Finished);
    assert!(f.is_neutral());

    assert_eq!(player.update(f32::NAN, &mut f), SampleStatus::Idle);
    assert!(f.is_neutral());
}

/// 要求：无状态采样截断 progress；neutral 语义与时长元数据成立。
#[test]
fn stateless_sample_at_and_metadata() {
    let n = ParameterFrame::neutral();
    assert!(n.is_neutral());
    assert_eq!(n.eye_open_scale, 1.0);
    let mut m = n;
    m.head_angle_z = 5.0;
    assert!(!m.is_neutral());

    let a = act(ActionId::Tilt, Strength::High);
    let mut out = ParameterFrame::neutral();
    sample_at(a, 0.5, &mut out);
    assert!(out.head_angle_z > 0.0);
    for over in [1.0, 1.5, 42.0] {
        sample_at(a, over, &mut out);
        assert!(out.is_neutral());
    }
    sample_at(a, -3.0, &mut out); // 截断到 0 → 中性。
    assert!(out.is_neutral());

    for id in ActionId::ALL {
        let d = duration(id);
        assert!(d.is_finite() && d > 0.0, "{id:?} 时长必须为正");
    }
    assert_eq!(duration(ActionId::Nod), 0.9);
    assert_eq!(duration(ActionId::LookAround), 2.4);
}
