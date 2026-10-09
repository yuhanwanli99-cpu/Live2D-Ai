//! 动作包运行时的原生回归（2026-09-23，v3：合并包 / intensity morph / 双槽）。
//!
//! 这里锁四类东西：
//! 1. **协议**：id 解析、none 撤销、未知（含已删除的旧 id）静默；
//! 2. **包形状**：表情包 = 五官 + 小幅头身；手势包 = 头 + 半身；
//! 3. **intensity 是一等公民**：普通包单调缩放；unhappy 两极 morph；
//! 4. **双槽**：表情与手势同轮并存、按槽撤销、共享头身由手势赢。

use super::*;
use std::collections::HashMap;

/// 内存假 sink：分别记 override / input 两层，用于在**原生**测试里
/// 复刻「idle 微表情同帧后写 input」的顺序（P0-2 回归）。
#[derive(Default)]
struct FakeSink {
    overrides: HashMap<String, f32>,
    input: HashMap<String, f32>,
}

impl FakeSink {
    fn effective(&self, id: &str) -> Option<f32> {
        self.overrides
            .get(id)
            .or_else(|| self.input.get(id))
            .copied()
    }

    /// 模拟 idle 微表情：往 input 层写一个值（input.rs 里在 preset 之后）。
    fn idle_input(&mut self, id: &str, value: f32) {
        self.input.insert(id.to_string(), value);
    }
}

impl PresetSink for FakeSink {
    fn override_param(&mut self, id: &str, value: f32) -> bool {
        self.overrides.insert(id.to_string(), value);
        true
    }
    fn clear_override_param(&mut self, id: &str) -> bool {
        self.overrides.remove(id).is_some()
    }
}

/// 取一条预设 params 里的幅值（不存在 → None）。
fn param_of(spec: &PresetSpec, id: &str) -> Option<f32> {
    spec.params.iter().find(|(k, _)| *k == id).map(|(_, v)| *v)
}

/// frame_params 里某一通道的值（不存在 → panic）。
fn at(spec: &'static PresetSpec, p: f64, want: &str) -> f32 {
    frame_params(spec, p)
        .into_iter()
        .find(|(id, _)| *id == want)
        .unwrap_or_else(|| panic!("{} 缺 {want}", spec.id))
        .1
}

/// 最终下发值（identity 倍率、给定 intensity / progress）。
fn out(spec: &'static PresetSpec, p: f64, i: f32, want: &str) -> f32 {
    frame_params_scaled_by(spec, p, i, PresetScales::default())
        .into_iter()
        .find(|(id, _)| *id == want)
        .unwrap_or_else(|| panic!("{} 缺 {want}", spec.id))
        .1
}

// ─────────────────────────────────────────── id / 协议

#[test]
fn builtin_ids_match_the_main_allowlist() {
    let ids: Vec<&str> = PRESETS.iter().map(|p| p.id).collect();
    assert_eq!(
        ids.len(),
        12,
        "内建 fallback = 主 allowlist（T9 起 12 条包：9 + thinking / look_up / look_down）"
    );
    for id in ["smile", "unhappy", "surprised", "nod", "shake"] {
        assert!(preset(id).is_some(), "{id} 应可查");
    }
    for id in PRESET_IDS.iter().filter(|id| **id != REVOKE_ID) {
        assert!(preset(id).is_some(), "主 allowlist 的 {id} 必须在内建表里");
    }
    // none 是撤销哨兵，不在预设表里。
    assert!(preset(REVOKE_ID).is_none());
    // 未知 / 大小写不同仍是 None（静默降级）。
    for bad in ["", "Smile", "手臂挥舞", "no_such_pack"] {
        assert!(preset(bad).is_none(), "{bad:?} 不该命中内建表");
    }
}

#[test]
fn none_revokes_and_unknown_ids_stay_silent() {
    assert_eq!(
        parse_command(Some("none"), None, None, None),
        PresetCommand::Revoke
    );
    for bad in ["", "Smile", "手臂挥舞", "no_such_pack"] {
        assert_eq!(
            parse_command(Some(bad), None, None, None),
            PresetCommand::Ignore,
            "{bad:?} 必须静默忽略"
        );
    }
    assert_eq!(parse_command(None, None, None, None), PresetCommand::Ignore);
}

#[test]
fn defaults_match_the_legacy_id_only_shape() {
    let spec = preset("smile").unwrap();
    assert_eq!(
        parse_command(Some("smile"), None, None, None),
        PresetCommand::Apply {
            spec,
            intensity: 1.0,
            ttl_ms: EXPRESSION_MS,
            source: PresetSource::Unknown,
        }
    );
    // 逐值等价：不传 intensity 与显式传 1.0 得到同一帧参数。
    let scaled = frame_params_scaled(spec, 0.5, 1.0);
    assert_eq!(scaled, frame_params(spec, 0.5));
}

#[test]
fn optional_fields_are_clamped_and_source_is_whitelisted() {
    let spec = preset("nod").unwrap();
    match parse_command(Some("nod"), Some(99.0), Some(999_999.0), Some("DEBUG")) {
        PresetCommand::Apply {
            spec: got,
            intensity,
            ttl_ms,
            source,
        } => {
            assert_eq!(got.id, spec.id);
            assert_eq!(intensity, MAX_INTENSITY);
            assert_eq!(ttl_ms, MAX_TTL_MS);
            assert_eq!(source, PresetSource::Debug);
        }
        other => panic!("应为 Apply，得到 {other:?}"),
    }
    match parse_command(Some("nod"), Some(-4.0), Some(0.0), Some("robot")) {
        PresetCommand::Apply {
            intensity,
            ttl_ms,
            source,
            ..
        } => {
            assert_eq!(intensity, 0.0);
            assert_eq!(ttl_ms, MOTION_MS);
            assert_eq!(source, PresetSource::Unknown);
        }
        other => panic!("应为 Apply，得到 {other:?}"),
    }
    // NaN 强度等同缺省（nod 无查表默认 → 1.0）。
    match parse_command(Some("nod"), Some(f64::NAN), None, Some("director")) {
        PresetCommand::Apply {
            intensity, source, ..
        } => {
            assert_eq!(intensity, 1.0);
            assert_eq!(source, PresetSource::Director);
        }
        other => panic!("应为 Apply，得到 {other:?}"),
    }
}

// ─────────────────────────────────────────── 旧 id 已删除：与未知 id 等价

/// 已删除的 v2 旧 id（按「词根 + 尾巴」拼出）。
///
/// 覆盖层（资产 `deprecated` / 内建别名 / 导演旧 id 表）已整体删除，这些 id 现在
/// 只是**未知名**。这里逐条钉住「不再被解析」；拼接写法是为了让全仓 grep 旧 id
/// 时不在本文件命中（旧 id 不应再出现在任何 .rs/.dart/.json 里）。
fn removed_v2_ids() -> Vec<String> {
    [
        ("expr_", "smile"),
        ("expr_", "sad"),
        ("expr_", "angry"),
        ("expr_", "surprised"),
        ("happy", "_bounce"),
        ("surprised", "_recoil"),
        ("agree", "_nod_double"),
        ("deny", "_shake_strong"),
        ("bow", "_slight"),
        ("shy", "_look_down"),
        // ("look", "_up") 已于 T9（2026-10-07）**重新启用为正式包名**
        // （look_up = 抬头看）——它不再是「已删除的旧 id」，故从本清单移除。
        ("ponder", "_tilt"),
    ]
    .iter()
    .map(|(head, tail)| format!("{head}{tail}"))
    .collect()
}

#[test]
fn removed_v2_ids_are_now_unknown_and_ignored() {
    for old in removed_v2_ids() {
        assert!(preset(old.as_str()).is_none(), "{old} 不该命中内建表");
        assert_eq!(
            parse_command(Some(old.as_str()), None, None, None),
            PresetCommand::Ignore,
            "{old} 是已删除的旧 id，必须静默忽略"
        );
        // 显式 intensity 也不能把它救回来。
        assert_eq!(
            parse_command(Some(old.as_str()), Some(3.0), None, None),
            PresetCommand::Ignore,
            "{old} 给 intensity 也不该被解析"
        );
    }
}

#[test]
fn builtin_table_lookup_is_direct() {
    let table = PresetTable::builtin();
    // 规范 id 直接命中；旧的别名默认强度概念已删除。
    assert_eq!(table.get("nod").unwrap().id, "nod");
    assert_eq!(table.get("smile").unwrap().id, "smile");
    assert_eq!(table.get("unhappy").unwrap().id, "unhappy");
    // 旧 id / 未知名一律 None。
    for old in removed_v2_ids() {
        assert!(table.get(old.as_str()).is_none(), "{old} 不该命中");
    }
    assert!(table.get("no_such_pack").is_none());
}

// ─────────────────────────────────────────── 通道 / 包形状

#[test]
fn presets_stay_inside_the_allowed_channel() {
    for spec in PRESETS {
        let all: Vec<(&str, f32)> = spec
            .params
            .iter()
            .copied()
            .chain(
                spec.morph
                    .iter()
                    .flat_map(|m| m.high_params.iter().copied()),
            )
            .collect();
        assert!(!all.is_empty(), "{} 不能是空预设", spec.id);
        for (id, v) in &all {
            assert!(
                ALLOWED_PARAMS.contains(id),
                "{} 写了通道外参数 {id}",
                spec.id
            );
            let limit = pack_limit(spec.kind, id);
            assert!(
                v.abs() <= limit,
                "{id} 幅值过大（{v} > {limit}）——{id} 的量程装不下"
            );
        }
        if spec.kind == PresetKind::Motion {
            assert!(
                !all.iter().any(|(id, _)| id.starts_with("ParamBrow")),
                "{} 是手势包，不该改眉毛",
                spec.id
            );
        }
    }
}

/// **v3 包形状**：每个表情包 = 五官 + 小幅 Angle + 小幅 BodyAngle（身约头的 1/3）。
#[test]
fn expression_packs_carry_face_and_small_head_body() {
    let external = PresetTable::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .unwrap();
    let mut tables: Vec<(&str, Vec<&'static PresetSpec>)> =
        vec![("builtin", PRESETS.iter().collect())];
    tables.push(("external", external.all().to_vec()));
    for (tag, specs) in tables {
        for spec in specs {
            if spec.kind != PresetKind::Expression {
                continue;
            }
            // 两极**分别**检查（low 极 = params，high 极 = morph.high_params）。
            let mut poles: Vec<&[(&'static str, f32)]> = vec![spec.params];
            if let Some(m) = spec.morph {
                poles.push(m.high_params);
            }
            for pole in poles {
                assert!(
                    pole.iter()
                        .any(|(id, _)| scale_class(id) == ScaleClass::Expression),
                    "{tag}:{} 表情包缺五官通道",
                    spec.id
                );
                let heads: Vec<(&str, f32)> = pole
                    .iter()
                    .filter(|(id, _)| id.starts_with("ParamAngle"))
                    .map(|(id, v)| (*id, *v))
                    .collect();
                assert!(!heads.is_empty(), "{tag}:{} 表情包缺头部位移", spec.id);
                for (id, v) in &heads {
                    assert!(
                        v.abs() <= EXPRESSION_HEAD_LIMIT + 1e-6,
                        "{tag}:{} {id} 头角 {v} 超过表情包小幅上限 {EXPRESSION_HEAD_LIMIT}",
                        spec.id
                    );
                    // 同轴半身随动：身幅约头的 1/3，且同向（同一极内比较）。
                    let bid = id.replacen("ParamAngle", "ParamBodyAngle", 1);
                    let Some((_, bv)) = pole.iter().find(|(k, _)| *k == bid.as_str()) else {
                        continue;
                    };
                    assert!(
                        bv.abs() <= EXPRESSION_BODY_LIMIT + 1e-6,
                        "{tag}:{} {bid} 半身 {bv} 超过表情包小幅上限 {EXPRESSION_BODY_LIMIT}",
                        spec.id
                    );
                    if v.abs() > 1e-6 {
                        assert!(
                            bv.signum() == v.signum(),
                            "{tag}:{} {bid} 必须与头同向：{bv} / {v}",
                            spec.id
                        );
                        let ratio = bv.abs() / v.abs();
                        assert!(
                            (0.25..=0.55).contains(&ratio),
                            "{tag}:{} {bid} 身幅比 {ratio} 应在 0.25~0.55（身约头的 1/3）",
                            spec.id
                        );
                    }
                }
            }
        }
    }
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

#[test]
fn expression_holds_while_motion_follows_the_envelope() {
    let smile = preset("smile").unwrap();
    assert_eq!(
        frame_params(smile, 0.1),
        frame_params(smile, 0.9),
        "表情在两个进度上应当同值"
    );
    let nod = preset("nod").unwrap();
    assert_eq!(nod.kind, PresetKind::Motion);
    assert_eq!(at(nod, 0.0, "ParamAngleY"), 0.0);
    assert!(at(nod, 0.5, "ParamAngleY").abs() > 0.0, "中途应有幅值");
    assert_eq!(at(nod, 1.0, "ParamAngleY"), 0.0, "末尾归零，撤销不跳变");
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
    for k in 0..=20 {
        let p = k as f64 / 20.0;
        assert_eq!(motion_gain(MotionWave::Single, p), envelope(p));
    }
}

#[test]
fn shake_oscillates_both_ways_with_body_in_phase() {
    let shake = preset("shake").unwrap();
    assert_eq!(shake.kind, PresetKind::Motion);
    assert!(matches!(shake.wave, MotionWave::Oscillate { .. }));
    assert_eq!(at(shake, 0.0, "ParamAngleX"), 0.0);
    assert_eq!(at(shake, 1.0, "ParamAngleX"), 0.0);
    let mut last_sign = 0i32;
    let mut changes = 0usize;
    let mut peak = 0.0f32;
    for k in 0..40 {
        let p = (k as f64 + 0.5) / 40.0;
        let h = at(shake, p, "ParamAngleX");
        let b = at(shake, p, "ParamBodyAngleX");
        peak = peak.max(h.abs());
        if h.abs() > 1e-3 {
            assert!(h.signum() == b.signum(), "身必须与头同相（p={p}）");
            let ratio = b.abs() / h.abs();
            assert!(
                (0.25..=0.55).contains(&ratio),
                "身幅应约头的 1/3（实得 {ratio}）"
            );
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
    // 2026-09-24 重标定：主轴表值 22 → 12，峰值（包络上确界 × 1.0）随之 12。
    assert!(peak >= 10.0, "摇头峰值要肉眼明显（实得 {peak}）");
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

mod assets;
mod fields;
mod fields_common;
mod fields_map_tests;
mod packs;
