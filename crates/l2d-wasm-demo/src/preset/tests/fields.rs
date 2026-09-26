//! 字段化通道 + 音频时钟 + 事件级 ack 的回归（阶段4c；协议 §4 / §5 / §6 / §7）。
//!
//! 覆盖 W4c 必须落地的九条语义：三字段映射 / expression 只写五官 / 同类 add 先加后钳 /
//! 跨批 replace（D26）/ hold 永不自动回 / 非 hold 到点发 `preset-expired` /
//! 缺参数发 `preset-dropped` / stage-clock 取代墙钟 / `segment-ended` 每段恰好一次。
use super::fields_common::*;
use super::*;
use serde_json::Value;
// ─────────────────────────────────────────── C1 三字段 → 文档通道

#[test]
fn body_head_expression_map_to_the_documented_channels() {
    let map = FieldMap::builtin();
    for axis in 0..3 {
        for t in map.axis_targets(Field::Body, axis) {
            assert!(
                t.param.starts_with("ParamBodyAngle"),
                "body 只能落 ParamBodyAngle*，实得 {}",
                t.param
            );
        }
        for t in map.axis_targets(Field::Head, axis) {
            assert!(
                t.param.starts_with("ParamAngle"),
                "head 只能落 ParamAngle*，实得 {}",
                t.param
            );
        }
    }
    for id in map.expression_ids() {
        for t in map.expression_targets(id).unwrap() {
            assert!(
                FACIAL_PARAMS.contains(&t.param.as_str()),
                "expression {id} 只能写五官，实得 {}",
                t.param
            );
        }
    }
    for p in map.declared_params() {
        assert!(ALLOWED_PARAMS.contains(&p), "{p} 越出 13 参白名单");
    }

    // 运行期：三字段各自只写 §5.5 声明的通道（逐帧写入不得越白名单 / 越字段）。
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            x: Some(0.5),
            y: Some(0.25),
            ..cue(Field::Body, 1, 1)
        },
        1_000.0,
    );
    rt.accept_cue(
        FieldCue {
            x: Some(0.4),
            ..cue(Field::Head, 1, 2)
        },
        1_000.0,
    );
    rt.accept_cue(
        FieldCue {
            id: Some("smile".to_string()),
            ..cue(Field::Expression, 1, 3)
        },
        1_000.0,
    );
    let out = rt.frame(1_000.0, &mut sink);
    assert!(!out.writes.is_empty());
    for w in &out.writes {
        assert!(
            field_allows(w.field, &w.param),
            "{:?} 写了不属于该字段的通道 {}",
            w.field,
            w.param
        );
    }
    for p in [
        "ParamBodyAngleX",
        "ParamBodyAngleY",
        "ParamAngleX",
        "ParamMouthForm",
        "ParamEyeLSmile",
    ] {
        assert!(sink.get(p).is_some(), "{p} 应当已落 final_override");
    }
}

/// 白名单保持现状（O11）：13 个标准参数、不含非标准参数、不含口型。
#[test]
fn whitelist_stays_the_documented_thirteen() {
    assert_eq!(ALLOWED_PARAMS.len(), 13, "白名单集合不得变（O11）");
    assert!(!ALLOWED_PARAMS.contains(&"ParamMouthOpenY"));
    assert!(!ALLOWED_PARAMS.contains(&"ParamBodyAngleX3"));
    assert!(!ALLOWED_PARAMS.contains(&"ParamEyeBallX"));
    for field in Field::ALL {
        assert!(
            !field_allows(field, "ParamMouthOpenY"),
            "{field:?} 不得写口型"
        );
    }
    assert!(!field_allows(Field::Expression, "ParamAngleY"));
    assert!(!field_allows(Field::Expression, "ParamBodyAngleX"));
    assert!(!field_allows(Field::Body, "ParamAngleY"));
    assert!(!field_allows(Field::Head, "ParamBodyAngleY"));
}

// ─────────────────────────────────────────── C2 expression 只写五官

#[test]
fn expressions_only_write_facial_channels() {
    // 恶意 / 误配的覆盖表：expression 里塞了头、身与口型——只有五官能活下来。
    let json = r#"{"models":{"bai":{"expression":{"smile":[
        {"param":"ParamAngleY","amount":5.0},
        {"param":"ParamBodyAngleX","amount":3.0},
        {"param":"ParamMouthOpenY","amount":1.0},
        {"param":"ParamMouthForm","amount":1.0}
    ]}}}}"#;
    let map = FieldMap::with_override_json("bai", json).unwrap();
    let targets = map.expression_targets("smile").unwrap();
    assert_eq!(
        targets.iter().map(|t| t.param.as_str()).collect::<Vec<_>>(),
        vec!["ParamMouthForm"],
        "expression 的覆盖表只准留下五官"
    );
    assert!(
        map.warnings().iter().any(|w| w.contains("通道红线")),
        "被丢的通道要留下 warn：{:?}",
        map.warnings()
    );

    let mut rt = FieldRuntime::new(map);
    rt.set_scales(PresetScales::default());
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            id: Some("smile".to_string()),
            ..cue(Field::Expression, 1, 1)
        },
        1_000.0,
    );
    let out = rt.frame(1_000.0, &mut sink);
    assert_eq!(out.writes.len(), 1);
    assert_eq!(out.writes[0].param, "ParamMouthForm");
    for forbidden in ["ParamAngleY", "ParamBodyAngleX", "ParamMouthOpenY"] {
        assert!(
            sink.get(forbidden).is_none(),
            "expression 抢写了 {forbidden}（V3 红线）"
        );
        assert!(!out.writes.iter().any(|w| w.param == forbidden));
    }
}

// ─────────────────────────────────────────── C3 同类 add，先加后钳

#[test]
fn same_field_cues_add_before_clamping() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    // 同一批（epoch=1）两条 head y：相加 = 0.6 + 0.6 = 1.2 个通道上限。
    rt.accept_cue(
        FieldCue {
            y: Some(0.6),
            ..cue(Field::Head, 1, 1)
        },
        1_000.0,
    );
    rt.accept_cue(
        FieldCue {
            y: Some(0.6),
            ..cue(Field::Head, 1, 2)
        },
        1_000.0,
    );
    let out = rt.frame(1_000.0, &mut sink);
    let w = write_of(&out, "ParamAngleY");
    // raw = 0.6 × 30 + 0.6 × 30 = 36（**加完之后**才钳）；value = 30。
    assert!(
        (w.raw - 36.0).abs() < 1e-4,
        "raw 应为两条相加的 36（先加后钳），实得 {}",
        w.raw
    );
    assert!((w.value - HEAD_LIMIT).abs() < 1e-4);
    assert!(w.clamped, "合计触顶 → clamped=true");
    assert_eq!(w.value, sink.get("ParamAngleY").unwrap());
    // 两条各自 emit applied（不是一条）。
    assert_eq!(kinds(&out.events), vec![AckKind::Applied, AckKind::Applied]);
    assert!(out.events.iter().all(|e| e.clamped), "两条都落进被钳的合计");
}

// ─────────────────────────────────────────── C9 / D26 批内 add · 跨批 replace

#[test]
fn new_batch_replaces_the_field_animation_value_adds() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    // 批 1：两条同字段相加。
    rt.accept_cue(
        FieldCue {
            y: Some(0.3),
            ..cue(Field::Body, 1, 1)
        },
        1_000.0,
    );
    rt.accept_cue(
        FieldCue {
            y: Some(0.2),
            ..cue(Field::Body, 1, 2)
        },
        1_000.0,
    );
    let o1 = rt.frame(1_000.0, &mut sink);
    let v1 = write_of(&o1, "ParamBodyAngleY").value;
    assert!((v1 - 5.0).abs() < 1e-4, "批内相加 = 0.5 × 10，实得 {v1}");

    // 批 2：同一字段新值 → 结束旧动画段（preset-replaced）+ 以新值为当前值。
    let ev = rt.accept_cue(
        FieldCue {
            y: Some(-0.1),
            ..cue(Field::Body, 2, 3)
        },
        1_000.0,
    );
    let replaced: Vec<u32> = ev
        .iter()
        .filter(|e| e.kind == AckKind::Replaced)
        .map(|e| e.seq)
        .collect();
    assert_eq!(replaced, vec![1, 2], "旧批的两条都要 preset-replaced");
    let o2 = rt.frame(1_000.0, &mut sink);
    assert_eq!(o2.writes.len(), 1, "新批只有一条 body cue");
    let v2 = write_of(&o2, "ParamBodyAngleY").value;
    assert!(
        (v2 + 1.0).abs() < 1e-4,
        "replace 后是新值 -0.1 × 10，实得 {v2}"
    );
}

// ─────────────────────────────────────────── C4 hold 永不自动回

#[test]
fn hold_cue_never_auto_returns_and_never_expires() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.5),
            hold: true,
            ..cue(Field::Body, 1, 1)
        },
        1_000.0,
    );
    let mut all = rt.frame(1_000.0, &mut sink).events;
    assert_eq!(all[0].kind, AckKind::Applied);
    // O6：段 A（墙钟）→ 交接点（首个音频）→ 段 B，hold 的段 A cue 继续保留。
    all.extend(rt.on_clock(1, 0.0, true, 1_000.0));
    all.extend(rt.frame(1_000.0, &mut sink).events);
    // 音频时钟推进远超 ttl（900ms），hold 不得到点。
    all.extend(rt.on_clock(1, 5_000.0, true, 1_000.0));
    all.extend(rt.frame(1_000.0, &mut sink).events);
    all.extend(rt.frame(9_999_999.0, &mut sink).events);
    assert!(
        !all.iter().any(|e| e.kind == AckKind::Expired),
        "hold 不得产生 preset-expired：{all:?}"
    );
    assert!(sink.get("ParamBodyAngleY").is_some(), "hold 必须一直保持");
}

// ─────────────────────────────────────────── C5 非 hold 到点回

#[test]
fn non_hold_cue_expires_at_ttl_and_emits_preset_expired() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.5),
            ttl_ms: 900.0,
            ..cue(Field::Head, 1, 1)
        },
        1_000.0,
    );
    let applied = rt.frame(1_000.0, &mut sink);
    assert_eq!(kinds(&applied.events), vec![AckKind::Applied]);
    assert!(sink.get("ParamAngleY").is_some());
    // 800ms：还没到点。
    let before = rt.frame(1_800.0, &mut sink);
    assert!(before.events.is_empty(), "未到点不该有事件");
    assert!(sink.get("ParamAngleY").is_some());
    // 901ms：到点 → preset-expired + 回基准。
    let after = rt.frame(1_901.0, &mut sink);
    assert_eq!(kinds(&after.events), vec![AckKind::Expired]);
    assert_eq!(after.events[0].field, Some(Field::Head));
    assert!(
        sink.get("ParamAngleY").is_none(),
        "到点必须回基准（清 override）"
    );
    assert!(sink.cleared.iter().any(|p| p == "ParamAngleY"));
}

// ─────────────────────────────────────────── C6 缺参数不得静默

#[test]
fn missing_param_emits_preset_dropped_instead_of_silent_noop() {
    // 整条 cue 的目标参数皮套都没有 → preset-dropped（不静默无反应）。
    let mut rt = runtime();
    let mut sink = FieldSink::missing(&["ParamAngleY"]);
    rt.accept_cue(
        FieldCue {
            y: Some(0.5),
            ..cue(Field::Head, 1, 1)
        },
        1_000.0,
    );
    let out = rt.frame(1_000.0, &mut sink);
    assert_eq!(kinds(&out.events), vec![AckKind::Dropped]);
    assert_eq!(out.events[0].field, Some(Field::Head));
    assert_eq!(out.events[0].reason.as_deref(), Some("ParamAngleY"));
    assert!(out.events[0].degraded);

    // 部分缺：写了的照常 applied，但 degraded=true 且 reason 记下缺的通道。
    let mut rt = runtime();
    let mut sink = FieldSink::missing(&["ParamAngleY"]);
    rt.accept_cue(
        FieldCue {
            x: Some(0.3),
            y: Some(0.5),
            ..cue(Field::Head, 1, 1)
        },
        1_000.0,
    );
    let out = rt.frame(1_000.0, &mut sink);
    assert_eq!(kinds(&out.events), vec![AckKind::Applied]);
    assert!(out.events[0].degraded);
    assert_eq!(out.events[0].reason.as_deref(), Some("ParamAngleY"));
    assert!(sink.get("ParamAngleX").is_some());
    assert!(sink.get("ParamAngleY").is_none());
}

// ─────────────────────────────────────────── C7 stage-clock 取代墙钟

#[test]
fn clock_message_replaces_wall_clock_in_frame_advance() {
    let mut rt = runtime();
    // 无 clock（段 A）→ 墙钟。
    assert_eq!(rt.effective_now_ms(50_000.0), 50_000.0);
    assert_eq!(rt.domain(), ClockDomain::Wall);
    // 首个音频 start = 交接点 → 音频时间轴。
    rt.on_clock(1, 250.0, true, 50_000.0);
    assert_eq!(rt.domain(), ClockDomain::Audio);
    assert_eq!(
        rt.effective_now_ms(999_999.0),
        250.0,
        "有 stage-clock 时 now_ms 必须是音频时钟（墙钟被替换）"
    );
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.5),
            ttl_ms: 900.0,
            ..cue(Field::Head, 1, 1)
        },
        999_999.0,
    );
    rt.frame(999_999.0, &mut sink);
    // 墙钟继续前进 500 秒，但 clock 采样不动 → ttl 不推进。
    let idle = rt.frame(1_500_000.0, &mut sink);
    assert!(
        idle.events.is_empty(),
        "墙钟前进不得推进音频时钟的 ttl：{:?}",
        idle.events
    );
    // 新的 stage-clock 把段内位置推到 1150 → 推进 900ms → 到点。
    rt.on_clock(1, 1_150.0, true, 1_500_000.0);
    let out = rt.frame(1_500_001.0, &mut sink);
    assert_eq!(kinds(&out.events), vec![AckKind::Expired]);
}

// ─────────────────────────────────────────── C8 segment-ended 每段一次

#[test]
fn segment_ended_is_emitted_once_per_segment() {
    let mut rt = runtime();
    let mut all: Vec<AckEvent> = Vec::new();
    all.extend(rt.on_clock(1, 0.0, true, 0.0));
    all.extend(rt.on_clock(1, 100.0, true, 100.0));
    all.extend(rt.on_clock(2, 0.0, true, 200.0)); // 段 1 结束
    all.extend(rt.on_clock(2, 50.0, true, 250.0));
    all.extend(rt.on_clock(2, 0.0, false, 300.0)); // 段 2 结束
    all.extend(rt.on_clock(2, 0.0, false, 400.0)); // 重复停止 → 不再发
    let ended: Vec<u32> = all
        .iter()
        .filter(|e| e.kind == AckKind::SegmentEnded)
        .map(|e| e.seg.unwrap())
        .collect();
    assert_eq!(ended, vec![1, 2], "每段恰好一次 segment-ended");
    for e in all.iter().filter(|e| e.kind == AckKind::SegmentEnded) {
        assert_eq!(e.kind.wire_type(), "segment-ended");
        let payload = e.to_json();
        assert_eq!(
            payload.get("seg").and_then(Value::as_u64),
            e.seg.map(u64::from)
        );
        assert!(payload.get("seq").is_none());
    }
}

// ─────────────────────────────────────────── ack wire 名 + §7.2 payload

#[test]
fn ack_wire_names_and_payload_match_the_frozen_contract() {
    // O13：wire 名冻结，不得改名。
    assert_eq!(AckKind::Applied.wire_type(), "preset-applied");
    assert_eq!(AckKind::Replaced.wire_type(), "preset-replaced");
    assert_eq!(AckKind::Expired.wire_type(), "preset-expired");
    assert_eq!(AckKind::Dropped.wire_type(), "preset-dropped");
    assert_eq!(AckKind::SegmentEnded.wire_type(), "segment-ended");

    // 组一段真实序列并打印原始 JSON（回报用）：applied → replaced → applied
    // → expired → dropped → segment-ended。
    let mut rt = runtime();
    let mut sink = FieldSink::missing(&["ParamAngleZ"]);
    let mut seq: Vec<AckEvent> = Vec::new();
    seq.extend(rt.accept_cue(
        FieldCue {
            y: Some(0.3),
            ..cue(Field::Head, 1, 1)
        },
        0.0,
    ));
    seq.extend(rt.frame(0.0, &mut sink).events);
    seq.extend(rt.accept_cue(
        FieldCue {
            y: Some(0.6),
            ..cue(Field::Head, 2, 2)
        },
        0.0,
    ));
    seq.extend(rt.frame(0.0, &mut sink).events);
    seq.extend(rt.accept_cue(
        FieldCue {
            z: Some(0.5),
            ..cue(Field::Head, 2, 3)
        },
        0.0,
    ));
    seq.extend(rt.frame(0.0, &mut sink).events);
    seq.extend(rt.on_clock(1, 900.0, true, 0.0));
    seq.extend(rt.on_clock(1, 1_900.0, true, 0.0)); // 推进 900ms → 两条非 hold 到点
    seq.extend(rt.frame(1_900.0, &mut sink).events);
    seq.extend(rt.on_clock(2, 0.0, true, 2_000.0));
    let lines: Vec<String> = seq.iter().map(|e| e.to_json().to_string()).collect();
    println!("ACK-SEQUENCE-BEGIN");
    for line in &lines {
        println!("{line}");
    }
    println!("ACK-SEQUENCE-END");

    let applied = seq.iter().find(|e| e.kind == AckKind::Applied).unwrap();
    let payload = applied.to_json();
    for key in [
        "type",
        "epoch",
        "ts_ms",
        "seq",
        "field",
        "intensity",
        "clamped",
        "degraded",
    ] {
        assert!(payload.get(key).is_some(), "payload 缺 {key}: {payload}");
    }
    let dropped = seq.iter().find(|e| e.kind == AckKind::Dropped).unwrap();
    assert_eq!(dropped.reason.as_deref(), Some("ParamAngleZ"));
    assert!(dropped.degraded);
}
