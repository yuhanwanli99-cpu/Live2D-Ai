//! `Revoke`（`preset_id == "none"`）**放回**的原生回归（2026-10-15 执行单 §1/§3）。
//!
//! 钉住三件事：不当帧清参数（当帧 = 撤销前屏幕上的值）、按
//! `ttl_ms × (1 - HOLD_END)` 收到 0 后**才**清槽、放回途中的切换快照是这一帧的屏幕值。

use super::motion_curve::{ATTACK_END, HOLD_END};
use super::*;

/// 内建表的一条 Apply（缺省 intensity / ttl = 1.0 / 包时长）。
fn apply(id: &str) -> PresetCommand {
    parse_command(Some(id), None, None, None)
}

/// `id:"none"` 撤销哨兵。
fn revoke() -> PresetCommand {
    parse_command(Some("none"), None, None, None)
}

/// 该槽的放回时长：`ttl_ms × (1 - HOLD_END)`。
fn release_ms(ttl_ms: f64) -> f64 {
    ttl_ms * (1.0 - HOLD_END)
}

/// 当前 override 层里某个通道的值。
fn ov(sink: &FakeSink, id: &str) -> Option<f32> {
    sink.overrides.get(id).copied()
}

/// **保持段中途 `Revoke`**：当帧参数不动（= 撤销前屏幕上的值），中途按 smootherstep
/// 收到 0，走满 `ttl_ms × (1 - HOLD_END)` 才清槽 + 清 override。
#[test]
fn revoke_releases_from_the_screen_value_then_clears_at_the_end() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let nod = preset("nod").unwrap();
    let t = nod.duration_ms * 0.5; // 保持段
    rt.handle(apply("nod"), 0.0, &mut sink);
    rt.apply_frame(t, &mut sink);
    let screen = sink.overrides.clone();
    assert!(
        ov(&sink, "ParamAngleY").unwrap().abs() > 1.0,
        "前提：保持段满幅"
    );

    rt.handle(revoke(), t, &mut sink);
    assert_eq!(sink.overrides, screen, "Revoke 当帧不清参数");
    assert!(
        rt.active_gesture().unwrap().release.is_some(),
        "槽上记下放回状态"
    );
    rt.apply_frame(t, &mut sink);
    assert_eq!(sink.overrides, screen, "q = 0 逐值等于撤销前屏幕上的值");

    // 中途：`smootherstep(0.5) = 0.5`，值收了一半但槽还占着。
    let half = release_ms(nod.duration_ms) / 2.0;
    rt.apply_frame(t + half, &mut sink);
    let y = ov(&sink, "ParamAngleY").unwrap();
    let full = screen["ParamAngleY"];
    assert!(
        (y.abs() - full.abs() * 0.5).abs() < 1e-4,
        "中途应为一半（实得 {y}，满值 {full}）"
    );

    // 到点：槽空、override 全清。
    rt.apply_frame(t + release_ms(nod.duration_ms), &mut sink);
    assert!(rt.active_gesture().is_none(), "到点才清槽");
    assert!(sink.overrides.is_empty(), "到点 override 全清");
}

/// **放回途中再 `Revoke`**：从这一帧的屏幕值重开一段，不回跳到撤销前的值。
#[test]
fn a_second_revoke_restarts_from_the_mid_release_value() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let left = preset("look_left").unwrap();
    let t = left.duration_ms * 0.5;
    rt.handle(apply("look_left"), 0.0, &mut sink);
    rt.apply_frame(t, &mut sink);
    rt.handle(revoke(), t, &mut sink);

    let half_at = t + release_ms(left.duration_ms) / 2.0;
    rt.apply_frame(half_at, &mut sink);
    let half = ov(&sink, "ParamAngleX").unwrap();
    assert!(half.abs() > 0.0, "前提：还在放回途中");

    rt.handle(revoke(), half_at, &mut sink);
    rt.apply_frame(half_at, &mut sink);
    assert_eq!(ov(&sink, "ParamAngleX"), Some(half), "重开第一帧 = 屏幕值");
    rt.apply_frame(half_at + release_ms(left.duration_ms), &mut sink);
    assert!(rt.active_gesture().is_none(), "第二段到点也清槽");
    assert!(sink.overrides.is_empty());
}

/// **放回到一半时换另一个 id**：新预设第一帧 = 这一半的屏幕值——不是 0，也不是旧
/// 预设未混合的曲线值；放回状态被新预设取代。
#[test]
fn swapping_mid_release_snapshots_the_half_released_value() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let left = preset("look_left").unwrap();
    let right = preset("look_right").unwrap();
    let t = left.duration_ms * 0.5;
    rt.handle(apply("look_left"), 0.0, &mut sink);
    rt.apply_frame(t, &mut sink);
    let full = ov(&sink, "ParamAngleX").unwrap();
    rt.handle(revoke(), t, &mut sink);

    let half_at = t + release_ms(left.duration_ms) / 2.0;
    rt.apply_frame(half_at, &mut sink);
    let half = ov(&sink, "ParamAngleX").unwrap();
    assert!(
        half.abs() > 0.0 && half.abs() < full.abs(),
        "前提：收到一半"
    );

    rt.handle(apply("look_right"), half_at, &mut sink);
    let active = rt.active_gesture().unwrap();
    assert!(active.release.is_none(), "换预设后放回状态消失");
    assert!(active.handoff.is_some(), "换 id 走接力");
    rt.apply_frame(half_at, &mut sink);
    assert_eq!(
        ov(&sink, "ParamAngleX"),
        Some(half),
        "第一帧 = 一半的屏幕值"
    );
    assert_ne!(ov(&sink, "ParamAngleX"), Some(0.0));
    assert_ne!(ov(&sink, "ParamAngleX"), Some(full));

    // 攻击段结束：与不接旧值逐值相同（快照丢掉）。
    rt.apply_frame(half_at + right.duration_ms * (ATTACK_END + 1e-3), &mut sink);
    assert!(
        rt.active_gesture().unwrap().handoff.is_none(),
        "攻击段结束丢掉快照"
    );
}

/// **手势短 TTL 到期**：与 `Revoke` 同一条 `Release`——到期帧槽仍 Some + 开 Release，
/// 走满 `300 × (1 - HOLD_END) = 114ms` 才清槽 / 清 override；中途仍占着。
#[test]
fn gesture_short_ttl_expiry_releases_then_clears() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let ttl = 300.0;
    let release = 300.0 * (1.0 - HOLD_END);

    rt.handle(
        parse_command(Some("nod"), None, Some(ttl), None),
        0.0,
        &mut sink,
    );
    rt.apply_frame(ttl, &mut sink);
    assert!(
        rt.active_gesture().is_some(),
        "手势短 ttl 到期应开一段放回，不是立即清槽"
    );
    assert!(
        rt.active_gesture().unwrap().release.is_some(),
        "到期帧应开 Release（与 Revoke 同一条放回）"
    );
    assert!(
        sink.overrides.contains_key("ParamAngleY"),
        "到期帧仍要写手势通道（q = 0，值为 from）"
    );

    // 中途（t = 350 < 300 + 114）：槽还占着，通道还在写。
    rt.apply_frame(350.0, &mut sink);
    assert!(rt.active_gesture().is_some(), "放回途中槽还占着");
    assert!(
        sink.overrides.contains_key("ParamAngleY"),
        "放回途中仍在写通道"
    );

    // 到点（300 + 114 = 414）才清。
    rt.apply_frame(ttl + release, &mut sink);
    assert!(rt.active_gesture().is_none(), "到点才清槽");
    assert!(sink.overrides.is_empty(), "到点 override 全清");
}

/// **表情短 TTL 对照**（与上一条紧邻）：同一 ttl 下表情槽仍**立即**清
/// （槽 None + override 清掉），不走手势那条放回。
#[test]
fn face_short_ttl_expiry_still_clears_immediately() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let ttl = 300.0;

    rt.handle(
        parse_command(Some("smile"), None, Some(ttl), None),
        0.0,
        &mut sink,
    );
    rt.apply_frame(ttl, &mut sink);
    assert!(rt.active_face().is_none(), "表情到期仍立即清槽");
    assert!(
        !sink.overrides.contains_key("ParamMouthForm"),
        "表情到期仍立即清掉自己的 override"
    );
    assert!(!sink.overrides.contains_key("ParamEyeLSmile"));
}
