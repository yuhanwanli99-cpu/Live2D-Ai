//! 字段**协议解析 / 映射覆盖表 / 撤销 / 段锚点 / HUD 串**的回归（阶段4c，
//! 从 fields.rs 拆出：AGENTS 测试文件 ≤800 行；核心九条留在 fields.rs）。

use super::fields_common::*;
use super::*;
use serde_json::Value;

// ─────────────────────────────────────────── 解析 / 覆盖表 / 撤销

#[test]
fn field_cue_parses_the_frozen_payload_shape() {
    let v: Value = serde_json::from_str(
        r#"{"field":"head","x":0.0,"y":-0.4,"z":0.2,"intensity":2,"at":"seg:2",
            "hold":false,"seq":3,"epoch":7,"ttl_ms":1200}"#,
    )
    .unwrap();
    let c = FieldCue::from_json(&v).unwrap();
    assert_eq!(c.field, Field::Head);
    assert_eq!(c.y, Some(-0.4));
    assert_eq!(c.z, Some(0.2));
    assert!((c.intensity - 2.0).abs() < 1e-6);
    assert_eq!(c.at, Anchor::Seg(2));
    assert_eq!(c.seq, 3);
    assert!(c.has_epoch);
    assert_eq!(c.epoch, 7);
    assert!((c.ttl_ms - 1_200.0).abs() < 1e-6);

    // body 给 z → 丢键（§2.4 #11）。
    let v: Value = serde_json::from_str(r#"{"field":"body","z":0.9}"#).unwrap();
    assert_eq!(FieldCue::from_json(&v).unwrap().z, None);
    // 缺 at → now；缺 ttl → 该字段默认（O3）。
    let v: Value = serde_json::from_str(r#"{"field":"expression","id":"smile"}"#).unwrap();
    let c = FieldCue::from_json(&v).unwrap();
    assert_eq!(c.at, Anchor::Now);
    assert!((c.ttl_ms - DEFAULT_TTL_EXPRESSION_MS).abs() < 1e-6);
    // sentence_seq 与 at:"seg:N" 同义（V11）。
    let v: Value = serde_json::from_str(r#"{"field":"head","sentence_seq":4}"#).unwrap();
    assert_eq!(FieldCue::from_json(&v).unwrap().at, Anchor::Seg(4));
    // 越界钳位。
    let v: Value = serde_json::from_str(r#"{"field":"head","x":1.7,"intensity":9}"#).unwrap();
    let c = FieldCue::from_json(&v).unwrap();
    assert_eq!(c.x, Some(1.0));
    assert!((c.intensity - 3.0).abs() < 1e-6);
    // 非法：词表外 field / seg:0 → Err（调用方转 preset-dropped）。
    let v: Value = serde_json::from_str(r#"{"field":"tail"}"#).unwrap();
    assert!(FieldCue::from_json(&v).is_err());
    let v: Value = serde_json::from_str(r#"{"field":"head","at":"seg:0"}"#).unwrap();
    assert!(FieldCue::from_json(&v).is_err());
    let v: Value = serde_json::from_str(r#"{"field":"head","at":"in:800"}"#).unwrap();
    assert!(FieldCue::from_json(&v).is_err());
}

#[test]
fn unknown_expression_id_and_none_revoke_are_explicit() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    // 未知 id → dropped（不得静默）。
    let ev = rt.accept_cue(
        FieldCue {
            id: Some("no_such".to_string()),
            ..cue(Field::Expression, 1, 1)
        },
        1_000.0,
    );
    assert_eq!(kinds(&ev), vec![AckKind::Dropped]);
    assert_eq!(ev[0].reason.as_deref(), Some("expression_unknown_id"));
    // id:"none" → 撤销表达式累加器 + preset-replaced。
    rt.accept_cue(
        FieldCue {
            id: Some("smile".to_string()),
            ..cue(Field::Expression, 1, 2)
        },
        1_000.0,
    );
    rt.frame(1_000.0, &mut sink);
    assert!(sink.get("ParamMouthForm").is_some());
    let ev = rt.accept_cue(
        FieldCue {
            id: Some("none".to_string()),
            ..cue(Field::Expression, 1, 3)
        },
        1_000.0,
    );
    assert_eq!(kinds(&ev), vec![AckKind::Replaced]);
    let out = rt.frame(1_000.0, &mut sink);
    assert!(sink.get("ParamMouthForm").is_none(), "none 必须清空累加器");
    assert!(out.writes.is_empty());
}

#[test]
fn field_map_override_json_is_per_model_and_validated() {
    let json = r#"{"models":{
        "bai": {"head":{"y":[{"param":"ParamAngleY","amount":18.0}]}},
        "default": {"body":{"x":[{"param":"ParamBodyAngleX","amount":7.0}]}}
    }}"#;
    let bai = FieldMap::with_override_json("bai", json).unwrap();
    assert_eq!(bai.model(), "bai");
    assert_eq!(bai.axis_targets(Field::Head, 1)[0].amount, 18.0);
    // bai 节没提 body → 保持内建（不是 default 节的 7.0）。
    assert_eq!(bai.axis_targets(Field::Body, 0)[0].amount, BODY_LIMIT);
    // 别的 model id → 退 default 节。
    let other = FieldMap::with_override_json("aya", json).unwrap();
    assert_eq!(other.axis_targets(Field::Body, 0)[0].amount, 7.0);
    assert_eq!(other.axis_targets(Field::Head, 1)[0].amount, HEAD_LIMIT);

    // 越白名单 / 越通道上限 → 丢或钳 + warn。
    let bad = r#"{"models":{"bai":{"head":{"y":[
        {"param":"ParamWing","amount":1.0},
        {"param":"ParamAngleY","amount":999.0}
    ]}}}}"#;
    let map = FieldMap::with_override_json("bai", bad).unwrap();
    assert_eq!(map.axis_targets(Field::Head, 1).len(), 1);
    assert_eq!(map.axis_targets(Field::Head, 1)[0].amount, HEAD_LIMIT);
    assert!(map.warnings().iter().any(|w| w.contains("白名单")));
    assert!(map.warnings().iter().any(|w| w.contains("钳位")));

    // 不是合法 JSON → Err（调用方回退内建）。
    assert!(FieldMap::with_override_json("bai", "not json").is_err());
}

#[test]
fn model_id_is_taken_from_the_manifest_url() {
    assert_eq!(
        model_id_from_url("/models/bai/runtime/bai.model3.json"),
        "bai"
    );
    assert_eq!(model_id_from_url("https://x/y/aya.model3.json?m=1"), "aya");
    assert_eq!(model_id_from_url(""), "");
    assert_eq!(model_id_from_url("/a/b"), "b");
}

#[test]
fn after_prev_waits_for_the_previous_cue_and_degrades_to_now_on_hold() {
    // 上一条非 hold → 事件式等待：它 preset-expired 时第二条才生效。
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.2),
            ttl_ms: 500.0,
            ..cue(Field::Head, 1, 1)
        },
        0.0,
    );
    rt.accept_cue(
        FieldCue {
            x: Some(0.4),
            at: Anchor::AfterPrev,
            ..cue(Field::Head, 1, 2)
        },
        0.0,
    );
    let out = rt.frame(0.0, &mut sink);
    assert!(
        !out.writes.iter().any(|w| w.param == "ParamAngleX"),
        "after_prev 在上一 cue 做完前不得生效"
    );
    let out = rt.frame(500.0, &mut sink);
    assert!(out.events.iter().any(|e| e.kind == AckKind::Expired));
    let out = rt.frame(500.0, &mut sink);
    assert!(
        out.writes.iter().any(|w| w.param == "ParamAngleX"),
        "上一 cue 到点后 after_prev 必须生效"
    );

    // 上一条 hold（O4）→ 在同锚点上立即生效，不得变成「永不生效」。
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.2),
            hold: true,
            ..cue(Field::Head, 1, 1)
        },
        0.0,
    );
    rt.accept_cue(
        FieldCue {
            x: Some(0.4),
            at: Anchor::AfterPrev,
            ..cue(Field::Head, 1, 2)
        },
        0.0,
    );
    let out = rt.frame(0.0, &mut sink);
    assert!(
        out.writes.iter().any(|w| w.param == "ParamAngleX"),
        "O4：after_prev 遇 hold 必须退化为 now"
    );
}

#[test]
fn segment_anchored_cue_fires_when_that_segment_starts() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.5),
            at: Anchor::Seg(2),
            ..cue(Field::Head, 1, 1)
        },
        0.0,
    );
    rt.on_clock(1, 0.0, true, 0.0);
    let out = rt.frame(0.0, &mut sink);
    assert!(out.writes.is_empty(), "seg:2 在段 1 不得生效");
    rt.on_clock(2, 0.0, true, 1_000.0);
    let out = rt.frame(1_000.0, &mut sink);
    assert_eq!(kinds(&out.events), vec![AckKind::Applied]);
    assert!(sink.get("ParamAngleY").is_some());
}

#[test]
fn revoke_all_clears_accumulators_and_overrides() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.5),
            ..cue(Field::Head, 1, 1)
        },
        0.0,
    );
    rt.accept_cue(
        FieldCue {
            id: Some("smile".to_string()),
            ..cue(Field::Expression, 1, 2)
        },
        0.0,
    );
    rt.frame(0.0, &mut sink);
    assert!(sink.get("ParamAngleY").is_some());
    rt.revoke_all(&mut sink);
    assert!(sink.get("ParamAngleY").is_none());
    assert!(sink.get("ParamMouthForm").is_none());
    assert_eq!(rt.diag().matches("-").count(), 3);
}

/// HUD `preset:` 行里 `fields:` 段的原生代理读数（无浏览器时的原始证据）。
#[test]
fn hud_fields_diag_is_readable() {
    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            y: Some(0.3),
            hold: true,
            ..cue(Field::Body, 1, 1)
        },
        0.0,
    );
    rt.accept_cue(
        FieldCue {
            id: Some("smile".to_string()),
            ..cue(Field::Expression, 1, 2)
        },
        0.0,
    );
    rt.frame(0.0, &mut sink);
    let line = format!("fields: {} | clock: {:?}", rt.diag(), rt.domain());
    println!("HUD-FIELDS-SEGMENT: {line}");
    assert!(line.contains("body[1,hold1]"), "{line}");
    assert!(line.contains("head: -"), "{line}");
    assert!(line.contains("expr:smile[1]"), "{line}");
    assert!(line.contains("clock: Wall"), "{line}");
}

// ─────────────────────────────────────────── T9 问句补丁的表情（字段通道）

/// **T9（2026-10-07）**：问句补丁用的 thinking 在**字段通道**只写五官五行——
/// 头角（AngleZ / BodyAngleZ）留在预设包里走 preset_id 通道，口型只归 TTS。
#[test]
fn thinking_expression_writes_facial_channels_only() {
    let map = FieldMap::builtin();
    let targets = map
        .expression_targets("thinking")
        .expect("T9：内建字段表必须有 thinking");
    let params: Vec<&str> = targets.iter().map(|t| t.param.as_str()).collect();
    assert_eq!(
        params,
        vec![
            "ParamMouthForm",
            "ParamEyeLOpen",
            "ParamEyeROpen",
            "ParamBrowLY",
            "ParamBrowRY",
        ],
        "thinking 的字段表只抄五官五行（头角在包里，字段通道的歪头由 head.z 负责）"
    );
    assert!(!params.contains(&"ParamMouthOpenY"), "口型只归 TTS");

    let mut rt = runtime();
    let mut sink = FieldSink::default();
    rt.accept_cue(
        FieldCue {
            id: Some("thinking".to_string()),
            ..cue(Field::Expression, 1, 1)
        },
        1_000.0,
    );
    let out = rt.frame(1_000.0, &mut sink);
    assert_eq!(out.writes.len(), 5, "五行五官各写一条：{:?}", out.writes);
    for w in &out.writes {
        assert!(
            FACIAL_PARAMS.contains(&w.param.as_str()),
            "thinking 不得写五官之外的通道：{}",
            w.param
        );
        assert!(w.param != "ParamMouthOpenY", "导演 cue 不得写口型");
    }
    for forbidden in ["ParamMouthOpenY", "ParamAngleZ", "ParamBodyAngleZ"] {
        assert!(
            sink.get(forbidden).is_none(),
            "thinking 抢写了 {forbidden}（字段通道红线）"
        );
    }
}
