//! 同槽换动作的**接力**回归（2026-10-15 执行单 §1）。
//!
//! 钉住「换下去那一帧 → 新动作」三类通道（共享 / 只旧有 / 只新有）在攻击段内的
//! 插值，以及窗口结束（`progress >= ATTACK_END`）之后与不接旧值**逐值相同**。
//! 空槽第一次施加、同一 id 再施加在这里；**手势** TTL 到期与 `Revoke` 同一条
//! `Release`（到期帧开放回、走满 `ttl_ms × (1 - HOLD_END)` 才清槽），`Revoke`
//! 的常态放回在 `release.rs`，表情到期仍立即清；而**快照一律是屏幕上这一帧的值**
//! （含接力 / 放回途中）。

use super::motion_curve::{ATTACK_END, HOLD_END};
use super::*;

/// 内建表的一条 Apply（缺省 intensity / ttl = 1.0 / 包时长）。
fn apply(id: &str) -> PresetCommand {
    parse_command(Some(id), None, None, None)
}

/// 保持段正中的进度（手势与表情都在满增益上）。
fn hold() -> f64 {
    (ATTACK_END + HOLD_END) / 2.0
}

/// 「不接旧值时的正常帧」某个通道的值（与运行时同一条算式）。
fn frame_value(spec: &'static PresetSpec, p: f64, scales: PresetScales, want: &str) -> f32 {
    frame_params_scaled_by(spec, p, 1.0, scales)
        .into_iter()
        .find(|(id, _)| *id == want)
        .unwrap_or_else(|| panic!("{} 缺 {want}", spec.id))
        .1
}

/// 当前 override 层里某个通道的值。
fn ov(sink: &FakeSink, id: &str) -> Option<f32> {
    sink.overrides.get(id).copied()
}

/// **手势保持段中途换手势**：新预设第一帧，共享通道接上旧值（不是 0），
/// 旧预设独有的通道仍等于旧值，新预设独有的通道从 0 起。
#[test]
fn gesture_swap_continues_from_the_previous_frame() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let left_old = preset("look_left").unwrap();
    let up_new = preset("look_up").unwrap();
    // look_left 与 look_up 共享 ParamAngleX / ParamBodyAngleX，各有独占通道。
    assert_ne!(left_old.id, up_new.id);

    // ① 旧手势走到保持段（头 X / 身 X 满幅）。
    rt.handle(apply("look_left"), 0.0, &mut sink);
    let t = left_old.duration_ms * hold();
    rt.apply_frame(t, &mut sink);
    let old_x = ov(&sink, "ParamAngleX").expect("保持段必须有值");
    let old_body_x = ov(&sink, "ParamBodyAngleX").expect("保持段必须有身随动");
    let old_z = ov(&sink, "ParamAngleZ").expect("旧预设独有通道也写上");
    assert!(
        old_x.abs() > 1.0 && old_z.abs() > 1.0,
        "前提：保持段幅值可辨"
    );

    // ② 同一槽换 look_up：第一帧（进度 0）不归零，逐通道接上。
    rt.handle(apply("look_up"), t, &mut sink);
    assert!(
        rt.active_gesture().unwrap().handoff.is_some(),
        "同槽不同 id 必须在活动预设上记下旧帧"
    );
    rt.apply_frame(t, &mut sink);
    assert_eq!(
        ov(&sink, "ParamAngleX"),
        Some(old_x),
        "共享头通道第一帧 = 旧值"
    );
    assert_eq!(
        ov(&sink, "ParamBodyAngleX"),
        Some(old_body_x),
        "共享身通道第一帧 = 旧值"
    );
    assert_ne!(ov(&sink, "ParamAngleX"), Some(0.0), "不是 0");
    assert_eq!(
        ov(&sink, "ParamAngleZ"),
        Some(old_z),
        "只有旧预设有的通道：起点仍等于旧值（之后才收到 0）"
    );
    assert_eq!(
        ov(&sink, "ParamAngleY"),
        Some(0.0),
        "只有新预设有的通道：从 0 起"
    );
    assert_eq!(ov(&sink, "ParamBodyAngleY"), Some(0.0));
}

/// **进度到 `ATTACK_END`**：共享通道等于不接旧值时的正常帧，旧预设独有通道
/// 已被清掉，快照丢掉；再往后与不接旧值逐值相同。
#[test]
fn handoff_ends_at_attack_end_with_the_plain_frame() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let up_new = preset("look_up").unwrap();
    let scales = rt.scales();

    rt.handle(apply("look_left"), 0.0, &mut sink);
    let t = preset("look_left").unwrap().duration_ms * hold();
    rt.apply_frame(t, &mut sink);

    rt.handle(apply("look_up"), t, &mut sink);
    rt.apply_frame(t + up_new.duration_ms * 0.1, &mut sink);
    assert!(
        rt.active_gesture().unwrap().handoff.is_some(),
        "攻击段内快照还在"
    );

    // 第一条 progress >= ATTACK_END 的帧：收尾。
    let elapsed = (ATTACK_END + 1e-3) * up_new.duration_ms;
    rt.apply_frame(t + elapsed, &mut sink);
    let p_end = progress_at(elapsed, up_new.duration_ms);
    assert_eq!(
        ov(&sink, "ParamAngleX"),
        Some(frame_value(up_new, p_end, scales, "ParamAngleX")),
        "共享通道 = 不接旧值时的正常帧"
    );
    assert!(
        !sink.overrides.contains_key("ParamAngleZ"),
        "旧预设独有通道已被清掉"
    );
    assert!(!sink.overrides.contains_key("ParamBodyAngleZ"));
    assert!(rt.active_gesture().unwrap().handoff.is_none(), "快照已丢");

    // 再往后（同一进度）：与刚收尾的那一帧逐值相同。
    rt.apply_frame(t + elapsed, &mut sink);
    assert_eq!(
        ov(&sink, "ParamAngleX"),
        Some(frame_value(up_new, p_end, scales, "ParamAngleX"))
    );
}

/// **表情换表情**：进度 0 = 旧表情值（满增益），进度 >= `ATTACK_END` = 新表情
/// 满增益；旧表情独有通道在窗口末被清掉。
#[test]
fn expression_swap_fades_from_the_old_face() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let smile = preset("smile").unwrap();
    let unhappy = preset("unhappy").unwrap();
    let scales = rt.scales();

    rt.handle(apply("smile"), 0.0, &mut sink);
    let t = smile.duration_ms * hold();
    rt.apply_frame(t, &mut sink);
    let old_mouth = ov(&sink, "ParamMouthForm").expect("旧表情必须有值");
    let old_smile_eye = ov(&sink, "ParamEyeLSmile").expect("旧表情独有通道");
    assert!(old_mouth.abs() > 0.5, "前提：旧表情已满增益");

    rt.handle(apply("unhappy"), t, &mut sink);
    rt.apply_frame(t, &mut sink);
    assert_eq!(
        ov(&sink, "ParamMouthForm"),
        Some(old_mouth),
        "进度 0 = 旧表情值"
    );
    assert_eq!(
        ov(&sink, "ParamEyeLSmile"),
        Some(old_smile_eye),
        "旧表情独有通道也先接上"
    );

    let elapsed = (ATTACK_END + 1e-3) * unhappy.duration_ms;
    let p_end = progress_at(elapsed, unhappy.duration_ms);
    rt.apply_frame(t + elapsed, &mut sink);
    assert_eq!(
        ov(&sink, "ParamMouthForm"),
        Some(frame_value(unhappy, p_end, scales, "ParamMouthForm")),
        "进度 >= ATTACK_END = 新表情在该进度的满增益帧"
    );
    assert!(
        !sink.overrides.contains_key("ParamEyeLSmile"),
        "旧表情独有通道在窗口末被清掉"
    );
}

/// **空槽第一次施加**：仍是「从 0 攻击」，不接力。
#[test]
fn empty_slot_still_attacks_from_zero() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    rt.handle(apply("look_left"), 500.0, &mut sink);
    assert!(
        rt.active_gesture().unwrap().handoff.is_none(),
        "空槽没有旧帧可接"
    );
    rt.apply_frame(500.0, &mut sink);
    assert!(!sink.overrides.is_empty(), "该写的通道还是要写");
    for (id, value) in &sink.overrides {
        assert_eq!(*value, 0.0, "{id} 在进度 0 必须恰好是 0");
    }
}

/// **同一 id 再施加**：仍是从 0 重开（不接力）——行为不变。
#[test]
fn reapplying_the_same_id_still_restarts() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let nod = preset("nod").unwrap();
    rt.handle(apply("nod"), 0.0, &mut sink);
    let t = nod.duration_ms * hold();
    rt.apply_frame(t, &mut sink);
    assert!(ov(&sink, "ParamAngleY").unwrap().abs() > 1.0);

    rt.handle(apply("nod"), t, &mut sink);
    assert!(
        rt.active_gesture().unwrap().handoff.is_none(),
        "同一 id 不接力"
    );
    rt.apply_frame(t, &mut sink);
    assert_eq!(ov(&sink, "ParamAngleY"), Some(0.0), "同一 id 照旧从 0 重开");
}

/// **手势 TTL 到期**：与 `Revoke` 同一条 `Release`——到期那一帧从屏幕值开一段
/// 放回，槽仍占着；走满 `ttl_ms × (1 - HOLD_END)` 才清 override + 清槽。
///
/// 手势曲线在 `p = 1` 恰好为 0，所以 `from` 通常 ≈ 0；但到期帧的通道**仍要写**
/// （q = 0 那一帧值 = from），不是当帧整批清掉。表情到期仍是立即清（见 `motion.rs`）。
#[test]
fn gesture_expiry_releases_then_clears() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let left = preset("look_left").unwrap();
    let ttl = left.duration_ms;
    let release = ttl * (1.0 - HOLD_END);

    rt.handle(apply("look_left"), 0.0, &mut sink);
    rt.apply_frame(ttl, &mut sink);
    assert!(
        rt.active_gesture().is_some(),
        "手势到期应开一段放回，不是立即清槽"
    );
    assert!(
        rt.active_gesture().unwrap().release.is_some(),
        "到期帧应开 Release（与 Revoke 同一条放回）"
    );
    assert!(
        sink.overrides.contains_key("ParamAngleX"),
        "到期帧仍要写手势通道（q = 0，值为 from）"
    );
    assert!(
        sink.overrides["ParamAngleX"].abs() < 1e-6,
        "手势曲线 p = 1 恰好为 0，from 应为 0（实得 {}）",
        sink.overrides["ParamAngleX"]
    );

    rt.apply_frame(ttl + release / 2.0, &mut sink);
    assert!(rt.active_gesture().is_some(), "放回途中槽还占着");

    rt.apply_frame(ttl + release, &mut sink);
    assert!(rt.active_gesture().is_none(), "到点才清槽");
    assert!(sink.overrides.is_empty(), "到点 override 全清");
}

/// **接力窗口内再换第三次**：第一帧等于第二次混合到一半的屏幕值，不是旧曲线、
/// 也不是新预设未混合的曲线值（2026-10-15 执行单 §3）。
#[test]
fn a_third_swap_takes_the_mixed_screen_value() {
    let mut rt = PresetRuntime::default();
    let mut sink = FakeSink::default();
    let left = preset("look_left").unwrap();
    let up = preset("look_up").unwrap();

    rt.handle(apply("look_left"), 0.0, &mut sink);
    let t = left.duration_ms * hold();
    rt.apply_frame(t, &mut sink);
    let left_x = ov(&sink, "ParamAngleX").unwrap();

    rt.handle(apply("look_up"), t, &mut sink);
    let mid = t + up.duration_ms * 0.1; // 攻击段内
    rt.apply_frame(mid, &mut sink);
    let mixed = ov(&sink, "ParamAngleX").unwrap();
    let plain = frame_value(
        up,
        progress_at(mid - t, up.duration_ms),
        rt.scales(),
        "ParamAngleX",
    );
    assert!(
        mixed.abs() > 0.0 && mixed.abs() < left_x.abs(),
        "前提：第二次已在混合（实得 {mixed}）"
    );
    assert_ne!(mixed, plain, "混合值不是新预设未混合的曲线值");

    rt.handle(apply("nod"), mid, &mut sink);
    assert!(
        rt.active_gesture().unwrap().handoff.is_some(),
        "第三次也接力"
    );
    rt.apply_frame(mid, &mut sink);
    assert_eq!(
        ov(&sink, "ParamAngleX"),
        Some(mixed),
        "第一帧 = 第二次混合到一半的屏幕值"
    );
}
