//! 0.3.7 T2：21 条包的 apply / revoke / ttl 边界回归——从 `packs.rs` 拆出
//! （AGENTS 测试文件 ≤800 行）。
//!
//! 内建 fallback 与 `assets/actions/presets.json` 两份表**逐条**跑一遍：apply 占槽、
//! `ttl - 1` 帧仍活、到点表情立即清 / 手势放回、`none` 放回，以及每个阶段的边界值。

use super::calibration::all_tables;
use super::motion_curve::HOLD_END;
use super::*;

/// **每条包**（内建 + 外置两份 × 21 条）的 apply / revoke / ttl 边界（0.3.7 T2）。
///
/// 一条测试覆盖全部包，而不是只挑新旧代表：
/// - apply 立刻占住**对应槽**、另一槽不动；
/// - `ttl - 1` 帧仍活；到点表情槽立即清、手势槽开一段放回，放回走满才清；
/// - `none`（Revoke）从屏幕值放回，走满 `ttl × (1-HOLD_END)` 才清槽。
#[test]
fn every_preset_applies_revokes_and_expires_at_its_ttl_boundary() {
    let factory = PresetScales::PRODUCT_DEFAULT;
    for (tag, specs) in all_tables() {
        for spec in specs {
            let slot = slot_of(spec.kind);
            let other = match slot {
                PresetSlot::Face => PresetSlot::Gesture,
                PresetSlot::Gesture => PresetSlot::Face,
            };
            let ttl = spec.duration_ms;

            // ── apply ──
            let mut rt = PresetRuntime::default();
            rt.set_scales(factory);
            let mut sink = FakeSink::default();
            assert!(
                rt.handle(
                    parse_command(Some(spec.id), Some(2.0), None, Some("debug")),
                    0.0,
                    &mut sink,
                ),
                "{tag}:{} apply 必须回执",
                spec.id
            );
            let active = rt
                .active_in(slot)
                .unwrap_or_else(|| panic!("{tag}:{} 没占住槽", spec.id));
            assert_eq!(active.spec.id, spec.id);
            assert_eq!(active.ttl_ms, ttl, "{tag}:{} ttl 应取表内时长", spec.id);
            assert_eq!(active.intensity, 2.0);
            assert!(
                rt.active_in(other).is_none(),
                "{tag}:{} 不得占另一槽",
                spec.id
            );
            // 保持段（p = 0.5）写通道。
            rt.apply_frame(ttl * 0.5, &mut sink);
            assert!(
                !sink.overrides.is_empty(),
                "{tag}:{} 保持段必须写 override",
                spec.id
            );
            // ttl 前一帧仍活。
            rt.apply_frame(ttl - 1.0, &mut sink);
            assert!(
                rt.active_in(slot).is_some(),
                "{tag}:{} ttl-1 帧仍应活着",
                spec.id
            );
            // 到点：表情立即清；手势开一段放回（槽仍占着）。
            rt.apply_frame(ttl, &mut sink);
            match slot {
                PresetSlot::Face => assert!(
                    rt.active_in(slot).is_none(),
                    "{tag}:{} 表情到点必须立即清槽",
                    spec.id
                ),
                PresetSlot::Gesture => {
                    let active = rt
                        .active_in(slot)
                        .unwrap_or_else(|| panic!("{tag}:{} 手势到点应先放回", spec.id));
                    assert!(active.release.is_some(), "{tag}:{} 没开 Release", spec.id);
                }
            }
            rt.apply_frame(ttl + ttl * (1.0 - HOLD_END) + 1.0, &mut sink);
            assert!(
                rt.active_in(slot).is_none(),
                "{tag}:{} 放回走满才清槽",
                spec.id
            );
            assert!(
                sink.overrides.is_empty(),
                "{tag}:{} 到期后 override 必须清空",
                spec.id
            );

            // ── revoke（none）──
            let mut rt = PresetRuntime::default();
            rt.set_scales(factory);
            let mut sink = FakeSink::default();
            assert!(rt.handle(
                parse_command(Some(spec.id), None, None, None),
                0.0,
                &mut sink
            ));
            rt.apply_frame(ttl * 0.5, &mut sink);
            assert!(
                rt.handle(
                    parse_command(Some(REVOKE_ID), None, None, None),
                    ttl * 0.5,
                    &mut sink
                ),
                "{tag}:{} revoke 必须回执",
                spec.id
            );
            assert!(
                rt.active_in(slot).is_some(),
                "{tag}:{} revoke 当帧不得清槽",
                spec.id
            );
            rt.apply_frame(ttl * 0.5 + ttl * (1.0 - HOLD_END) + 1.0, &mut sink);
            assert!(
                rt.active_in(slot).is_none(),
                "{tag}:{} revoke 放回走满才清槽",
                spec.id
            );
            assert!(
                sink.overrides.is_empty(),
                "{tag}:{} revoke 走满后 override 必须清空",
                spec.id
            );
        }
    }
}
