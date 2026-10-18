//! 字段表**重建 / 来源报告 / 模型代号门控**的原生回归（0.3.6 T2）。
//!
//! 锁四类东西：
//! 1. 任何取不到外置表的路径都回**新 model id** 的内建表（绝不沿用旧表）；
//! 2. `field_map` 载荷的三态（external / builtin / degraded）与现实口径一致：
//!    404 文件缺失不是降级；读取失败 / 非 UTF-8 / JSON 非法才是；
//! 3. legacy 的 `preset` 帧（不带 model_epoch）按旧行为接受，旧 epoch 一律 Ignore；
//! 4. `AckEvent` 的 `model_epoch` 只增字段，既有 `epoch`（导演 / 句 epoch）语义不变。

use super::*;
use serde_json::Value;

const OLD_OVERRIDE: &str = r#"{
    "version": 1,
    "models": {
        "bai": { "head": { "x": [{ "param": "ParamAngleX", "amount": 26.0 }] } }
    }
}"#;

/// 旧皮套的外置覆盖表（用于证明「重建后不再沿用旧表」）。
fn old_map() -> FieldMap {
    let map = FieldMap::with_override_json("bai", OLD_OVERRIDE).unwrap();
    assert!(map.has_override(), "测试前置：旧表必须有自己的覆盖");
    map
}

// ─────────────────────────────────────────── 重建 / 来源

/// 404（文件本来就没有）→ builtin + degraded=false（O9：不是错误）。
#[test]
fn missing_override_file_is_builtin_not_degraded() {
    let load = rebuild_for_model("epsilon_free", OverrideFetch::Missing("HTTP 404".into()));
    assert_eq!(load.map.model(), "epsilon_free", "必须绑定**新** model id");
    assert_eq!(load.map, FieldMap::for_model("epsilon_free"));
    assert_eq!(load.report.source, FieldMapSource::Builtin);
    assert!(!load.report.degraded, "404 不是降级");
    assert!(load.report.reason.contains("HTTP 404"));
    assert_eq!(load.report.to_json()["source"], "builtin");
    assert_eq!(load.report.to_json()["degraded"], false);
}

/// 外置表读取失败（非 404）→ degraded + 新 id 内建表，绝不沿用旧表。
#[test]
fn fetch_failure_degrades_to_new_builtin_and_never_reuses_old_table() {
    let old = old_map();
    let load = rebuild_for_model("epsilon_free", OverrideFetch::Failed("网络中断".into()));
    assert_eq!(load.map.model(), "epsilon_free");
    assert_eq!(
        load.map,
        FieldMap::for_model("epsilon_free"),
        "降级必须回新 id 的内建表"
    );
    assert_ne!(load.map, old, "旧皮套的表不得被沿用");
    assert_eq!(load.report.source, FieldMapSource::Degraded);
    assert!(load.report.degraded);
    assert!(load.report.reason.contains("网络中断"), "原因可读");
    assert_eq!(load.report.to_json()["source"], "degraded");
    assert_eq!(load.report.to_json()["degraded"], true);
}

/// 非 UTF-8 / JSON 非法同样是 degraded（外置文件存在但不可用）。
#[test]
fn unreadable_override_degrades() {
    let load = rebuild_for_model("bai", OverrideFetch::Fetched(vec![0xff, 0xfe, 0xfd]));
    assert_eq!(load.report.source, FieldMapSource::Degraded);
    assert!(load.report.degraded);
    assert_eq!(load.map.model(), "bai");

    let load = rebuild_for_model("bai", OverrideFetch::Fetched(b"{ not json".to_vec()));
    assert_eq!(load.report.source, FieldMapSource::Degraded);
    assert!(load.report.degraded);
    assert!(load.report.reason.contains("校验失败"));
    assert_eq!(load.map.model(), "bai");
}

/// 外置 JSON 合法但没有本模型 / default 节 → builtin + degraded=false。
#[test]
fn valid_json_without_model_section_keeps_builtin() {
    let json =
        br#"{"version":1,"models":{"other":{"head":{"x":[{"param":"ParamAngleX","amount":12.0}]}}}}"#;
    let load = rebuild_for_model("bai", OverrideFetch::Fetched(json.to_vec()));
    assert_eq!(load.report.source, FieldMapSource::Builtin);
    assert!(!load.report.degraded);
    assert!(!load.map.has_override());
    assert_eq!(load.map.model(), "bai");
    // 内容 = 内建表（warnings 里会带上「没有 bai 节」这句）。
    assert_eq!(
        load.map.axis_targets(Field::Head, 0),
        FieldMap::builtin().axis_targets(Field::Head, 0)
    );
    assert!(!load.report.reason.is_empty(), "reason 必须可读");
}

/// 外置覆盖真的生效 → external + degraded=false；值逐值落到新 id 的表上。
#[test]
fn applied_override_is_external() {
    let json =
        br#"{"version":1,"models":{"bai":{"head":{"x":[{"param":"ParamAngleX","amount":21.0}]}}}}"#;
    let load = rebuild_for_model("bai", OverrideFetch::Fetched(json.to_vec()));
    assert_eq!(load.report.source, FieldMapSource::External);
    assert!(!load.report.degraded);
    assert!(load.map.has_override());
    let targets = load.map.axis_targets(Field::Head, 0);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].param, "ParamAngleX");
    assert!((targets[0].amount - 21.0).abs() < 1e-6);
    assert_eq!(load.report.to_json()["source"], "external");
}

/// 部分目标被校验丢掉 → external + degraded=true，原因带告警。
#[test]
fn partial_override_with_warnings_is_degraded_external() {
    let json = br#"{"version":1,"models":{"bai":{"head":{"x":[
        {"param":"ParamAngleX","amount":21.0},
        {"param":"ParamMouthOpenY","amount":1.0}
    ]}}}}"#;
    let load = rebuild_for_model("bai", OverrideFetch::Fetched(json.to_vec()));
    assert_eq!(load.report.source, FieldMapSource::External);
    assert!(load.report.degraded, "有告警 = 部分降级");
    assert!(
        load.report.reason.contains("告警"),
        "原因带告警：{}",
        load.report.reason
    );
}

/// fetch 结果分类：只有 **404** 算「文件缺失」，其它都是失败。
#[test]
fn fetch_result_classification() {
    assert!(matches!(
        OverrideFetch::from_fetch(Ok(b"{}".to_vec())),
        OverrideFetch::Fetched(_)
    ));
    assert!(matches!(
        OverrideFetch::from_fetch(Err("HTTP 404".into())),
        OverrideFetch::Missing(_)
    ));
    assert!(matches!(
        OverrideFetch::from_fetch(Err("HTTP 500".into())),
        OverrideFetch::Failed(_)
    ));
    assert!(matches!(
        OverrideFetch::from_fetch(Err("TypeError: Failed to fetch".into())),
        OverrideFetch::Failed(_)
    ));
}

// ─────────────────────────────────────────── preset 帧的模型代号门控

/// legacy（不带 model_epoch）照旧接受；显式 null 视同未给。
#[test]
fn preset_frame_without_model_epoch_is_legacy_accepted() {
    let v: Value = serde_json::from_str(r#"{"field":"head","x":0.3}"#).unwrap();
    assert!(preset_frame_matches_model(3, &v));
    let v: Value = serde_json::from_str(r#"{"field":"head","x":0.3,"model_epoch":null}"#).unwrap();
    assert!(preset_frame_matches_model(3, &v));
}

/// 当前 epoch 接受；旧 epoch 与坏值一律 Ignore（不写参数、不回执）。
#[test]
fn preset_frame_with_stale_model_epoch_is_ignored() {
    let v: Value = serde_json::from_str(r#"{"id":"smile","model_epoch":3}"#).unwrap();
    assert!(preset_frame_matches_model(3, &v), "当前 epoch 必须接受");
    let v: Value = serde_json::from_str(r#"{"id":"smile","model_epoch":2}"#).unwrap();
    assert!(!preset_frame_matches_model(3, &v), "旧 epoch 必须 Ignore");
    let v: Value = serde_json::from_str(r#"{"id":"smile","model_epoch":4}"#).unwrap();
    assert!(!preset_frame_matches_model(3, &v), "未来 epoch 同样 Ignore");
    let v: Value = serde_json::from_str(r#"{"id":"smile","model_epoch":"3"}"#).unwrap();
    assert!(!preset_frame_matches_model(3, &v), "非 u64 坏值必须 Ignore");
}

// ─────────────────────────────────────────── ack 的 model_epoch

/// ack 加上 model_epoch 只**增字段**，既有 epoch（导演 / 句 epoch）语义不变。
#[test]
fn ack_carries_model_epoch_without_touching_sentence_epoch() {
    let e = AckEvent::segment_ended(4, 12.5, 2).with_model_epoch(7);
    let json = e.to_json();
    assert_eq!(json["model_epoch"], 7);
    assert_eq!(json["epoch"], 4, "既有 epoch 语义不得被 model_epoch 顶替");
    assert_eq!(json["seg"], 2);

    let applied = AckEvent::segment_ended(4, 12.5, 2)
        .with_model_epoch(9)
        .to_json();
    assert_eq!(applied["model_epoch"], 9);

    // 未盖章（构造点还不知道当前模型）时不得凭空发明字段。
    let plain = AckEvent::segment_ended(4, 12.5, 2).to_json();
    assert!(plain.get("model_epoch").is_none());
}
