//! 出厂资产（presets.json / preset_labels.json）的回归（从 tests.rs 拆出）。

use super::*;

// ─────────────────────────────────────────── 外置 JSON

#[test]
fn shipped_presets_json_parses_clean_and_matches_builtin_ids() {
    let json = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    ));
    let table = PresetTable::from_json(json).expect("出厂 presets.json 必须合法");
    assert!(
        table.warnings().is_empty(),
        "出厂表不该有告警：{:?}",
        table.warnings()
    );
    let json_ids: Vec<&str> = table.all().iter().map(|s| s.id).collect();
    for spec in PRESETS {
        assert!(
            json_ids.contains(&spec.id),
            "内建 id {} 必须仍在外置表里",
            spec.id
        );
    }
    assert_eq!(
        json_ids.len(),
        PRESETS.len(),
        "内建 fallback 与主 allowlist 同集合"
    );
    // morph 包必须真的解析出两极。
    let unhappy = table.get("unhappy").unwrap();
    assert!(unhappy.morph.is_some());
    match table.get("shake").unwrap().wave {
        MotionWave::Oscillate { cycles } => assert!((cycles - SHAKE_CYCLES).abs() < 1e-9),
        other => panic!("shake 必须是 oscillate：{other:?}"),
    }
    // 每一条手势包（9 条，0.3.7 T2 起含 bow）都必须带半身随动。
    for spec in table.all() {
        if spec.kind != PresetKind::Motion {
            continue;
        }
        assert!(
            spec.params
                .iter()
                .any(|(k, _)| k.starts_with("ParamBodyAngle")),
            "{} 必须带半身随动",
            spec.id
        );
    }
}

/// 内建 fallback 与出厂外置表**逐值相同**（0.3.7 T2 起 21 条）：kind / duration /
/// wave / params / morph 五项全等，顺序也一致。两份表是「产品表 + 加载失败兜底」，
/// 漂了就会出现「改了 JSON 忘改 Rust」的悬空能力。
#[test]
fn builtin_fallback_matches_the_external_table_for_every_pack() {
    let table = PresetTable::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .unwrap();
    assert_eq!(table.all().len(), PRESETS.len(), "21 条：两份表条数一致");
    let external_ids: Vec<&str> = table.all().iter().map(|s| s.id).collect();
    let builtin_ids: Vec<&str> = PRESETS.iter().map(|s| s.id).collect();
    assert_eq!(external_ids, builtin_ids, "两份表顺序也要一致（面板按序）");
    for builtin in PRESETS {
        let ext = table
            .get(builtin.id)
            .unwrap_or_else(|| panic!("外置表缺 {}", builtin.id));
        assert_eq!(ext.kind, builtin.kind, "{} kind 漂了", builtin.id);
        assert_eq!(
            ext.duration_ms, builtin.duration_ms,
            "{} duration_ms 漂了",
            builtin.id
        );
        assert_eq!(ext.wave, builtin.wave, "{} wave 漂了", builtin.id);
        assert_eq!(ext.params, builtin.params, "{} params 逐值漂了", builtin.id);
        match (ext.morph, builtin.morph) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!(a.low, b.low, "{} morph.low 漂了", builtin.id);
                assert_eq!(a.high, b.high, "{} morph.high 漂了", builtin.id);
                assert_eq!(
                    a.high_params, b.high_params,
                    "{} morph.high_params 逐值漂了",
                    builtin.id
                );
            }
            other => panic!(
                "{} 的 morph 存在性漂了：{:?}",
                builtin.id,
                other.0.is_some()
            ),
        }
    }
}

/// **D1=A（维护者 2026-10-18：手不做）的资产面证据**：
/// - 参数白名单仍是 **13 个标准参数**，没有手臂 / 手 / 肩 / 口型；
/// - wave / clap / point / cover_mouth / shrug 在内外两张表里都不存在，收到即未知 id；
/// - 21 条包（内建 + 外置）写的每个通道都在白名单内。
#[test]
fn d1_hand_is_out_whitelist_stays_thirteen_and_hand_packs_are_absent() {
    assert_eq!(
        ALLOWED_PARAMS.len(),
        13,
        "D1=A：渲染面参数白名单不得扩张（仍 13 个标准参数）"
    );
    for ch in ALLOWED_PARAMS {
        for forbidden in ["Arm", "Hand", "Shoulder", "MouthOpen"] {
            assert!(
                !ch.contains(forbidden),
                "白名单混入手 / 手臂 / 肩 / 口型通道：{ch}"
            );
        }
    }
    let table = PresetTable::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .unwrap();
    for id in ["wave", "clap", "point", "cover_mouth", "shrug"] {
        assert!(preset(id).is_none(), "{id} 不得出现在内建表（D1 手不做）");
        assert!(table.get(id).is_none(), "{id} 不得出现在外置表");
        assert_eq!(
            parse_command(Some(id), None, None, None),
            PresetCommand::Ignore,
            "{id} 收到必须是未知 id（静默 Ignore）"
        );
    }
    for spec in PRESETS.iter().chain(table.all().iter().copied()) {
        for (ch, _) in spec
            .params
            .iter()
            .chain(spec.morph.iter().flat_map(|m| m.high_params.iter()))
        {
            assert!(
                ALLOWED_PARAMS.contains(ch),
                "{} 写了白名单外通道 {ch}",
                spec.id
            );
        }
    }
}

/// 旧 id 兼容层已删除：资产顶层不得再有 `deprecated`，外置表也不得解析旧 id。
#[test]
fn shipped_presets_json_has_no_deprecated_and_removed_ids_do_not_resolve() {
    let json = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    ));
    let value: serde_json::Value =
        serde_json::from_str(json).expect("出厂 presets.json 必须合法 JSON");
    assert!(
        value.get("deprecated").is_none(),
        "旧 id 兼容层已删除，顶层不得再有 deprecated"
    );
    let table = PresetTable::from_json(json).unwrap();
    for old in super::removed_v2_ids() {
        assert!(table.get(old.as_str()).is_none(), "{old} 不得再被解析");
    }
}

#[test]
fn preset_labels_json_covers_every_shipped_preset_id() {
    let presets = PresetTable::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .unwrap();
    let labels: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/preset_labels.json"
    )))
    .expect("出厂 preset_labels.json 必须是合法 JSON");
    let map = labels
        .get("labels")
        .and_then(|v| v.as_object())
        .expect("preset_labels.json 缺少 labels 对象");
    assert_eq!(
        map.len(),
        presets.all().len() + 1,
        "标签表 = 21 条包 + none（0.3.7 T2 起）"
    );
    for spec in presets.all() {
        let entry = map
            .get(spec.id)
            .unwrap_or_else(|| panic!("标签表缺少 {}", spec.id));
        let zh = entry.get("zh").and_then(|v| v.as_str()).unwrap_or("");
        let en = entry.get("en").and_then(|v| v.as_str()).unwrap_or("");
        assert!(!zh.trim().is_empty(), "{} 缺中文展示名", spec.id);
        assert!(!en.trim().is_empty(), "{} 缺英文展示名", spec.id);
        // channel 必须与包表 kind 一致（渲染面槽位 / Flutter 面板按钮分组同源）。
        let want = match spec.kind {
            PresetKind::Expression => "expression",
            PresetKind::Motion => "motion",
        };
        assert_eq!(
            entry.get("channel").and_then(|v| v.as_str()).unwrap_or(""),
            want,
            "{} 的 channel 与 kind 漂了",
            spec.id
        );
    }
    assert!(map.contains_key("none"), "撤销哨兵 none 也要有展示名");
    // 标签表不得出现幽灵 id（旧 id 条目已随兼容层删除）。
    for (id, _) in map {
        assert!(
            id == "none" || presets.get(id).is_some(),
            "标签表出现幽灵 id（{id}）——旧 id 条目已删除"
        );
    }
}

#[test]
fn json_drops_unknown_channels_and_clamps_amplitudes() {
    let json = r#"[{"id":"x","kind":"motion","duration_ms":500,"params":[
        {"id":"ParamArmLA","value":5.0},
        {"id":"ParamAngleY","value":99.0},
        {"id":"ParamBodyAngleX","value":-99.0},
        {"id":"ParamMouthOpenY","value":1.0}]}]"#;
    let table = PresetTable::from_json(json).expect("至少一条有效");
    let spec = table.get("x").expect("x 应保留");
    assert_eq!(spec.params.len(), 2, "手臂 / 口型必须被丢");
    assert_eq!(param_of(spec, "ParamAngleY"), Some(HEAD_LIMIT));
    assert_eq!(param_of(spec, "ParamBodyAngleX"), Some(-BODY_LIMIT));
    assert!(table.warnings().len() >= 2, "丢弃与钳位都要有 warn");
}

/// 表情包的**小幅红线**在解析期执行（头 12 / 身 4 / 五官 4）。
#[test]
fn json_expression_amplitude_is_clamped_to_the_small_limits() {
    let json = r#"[{"id":"x","kind":"expression","params":[
        {"id":"ParamAngleY","value":99.0},
        {"id":"ParamBodyAngleX","value":-99.0},
        {"id":"ParamMouthForm","value":9.0},
        {"id":"ParamBrowLY","value":0.2}]}]"#;
    let table = PresetTable::from_json(json).unwrap();
    let spec = table.get("x").unwrap();
    assert_eq!(param_of(spec, "ParamAngleY"), Some(EXPRESSION_HEAD_LIMIT));
    assert_eq!(
        param_of(spec, "ParamBodyAngleX"),
        Some(-EXPRESSION_BODY_LIMIT)
    );
    assert_eq!(param_of(spec, "ParamMouthForm"), Some(EXPRESSION_LIMIT));
}

#[test]
fn json_invalid_falls_back_via_err() {
    assert!(PresetTable::from_json("{ not json").is_err());
    assert!(PresetTable::from_json("[]").is_err());
    assert!(PresetTable::from_json(r#"[{"id":"x","kind":"motion","params":[]}]"#).is_err());
}

#[test]
fn runtime_resolve_uses_the_loaded_table() {
    let json = r#"[{"id":"nod","kind":"motion","duration_ms":1234,"params":[
        {"id":"ParamAngleY","value":-1.0}]}]"#;
    let mut rt = PresetRuntime::default();
    match rt.resolve(Some("nod"), None, None, Some("debug")) {
        PresetCommand::Apply { spec, .. } => assert_eq!(spec.params[0].1, -12.0),
        other => panic!("应为 Apply：{other:?}"),
    }
    rt.set_table(PresetTable::from_json(json).unwrap());
    match rt.resolve(Some("nod"), None, None, Some("debug")) {
        PresetCommand::Apply { spec, ttl_ms, .. } => {
            assert_eq!(spec.params.len(), 1);
            assert_eq!(spec.params[0], ("ParamAngleY", -1.0));
            assert_eq!(ttl_ms, 1234.0);
        }
        other => panic!("应为 Apply：{other:?}"),
    }
    // 外置表里没有的 id -> Ignore（不报错）；已删除的旧 id 同样只是未知名。
    assert_eq!(
        rt.resolve(Some("smile"), None, None, None),
        PresetCommand::Ignore
    );
    for old in super::removed_v2_ids() {
        assert_eq!(
            rt.resolve(Some(old.as_str()), None, None, None),
            PresetCommand::Ignore,
            "{old} 不得再被解析"
        );
    }
}

/// 外置 JSON 的幅值 = 2026-09-24 标定后的那组（产品实际能力，逐值钉住）。
///
/// 内建 fallback 只是加载失败时的兜底，两表必须同形；这里把**外置表**的
/// 主轴 / 次轴 / 五官具体数字钉死，避免「改了 JSON 忘了内建」这类漂移。
#[test]
fn shipped_presets_json_carries_the_calibrated_amplitudes() {
    let table = PresetTable::from_json(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/actions/presets.json"
    )))
    .unwrap();
    let val = |id: &str, ch: &str| param_of(table.get(id).unwrap(), ch);
    let close = |got: Option<f32>, want: f32, what: &str| {
        let got = got.unwrap_or_else(|| panic!("{what} 缺失"));
        assert!((got - want).abs() < 1e-4, "{what} 应为 {want}，实得 {got}");
    };

    // 手势主轴：头 12 / 身 3.9（表内身/头 = 0.325）
    close(val("nod", "ParamAngleY"), -12.0, "nod ParamAngleY");
    close(val("nod", "ParamBodyAngleY"), -3.9, "nod ParamBodyAngleY");
    close(val("shake", "ParamAngleX"), 12.0, "shake ParamAngleX");
    close(
        val("shake", "ParamBodyAngleX"),
        3.9,
        "shake ParamBodyAngleX",
    );
    for (id, sign) in [("look_left", 1.0f32), ("look_right", -1.0f32)] {
        close(val(id, "ParamAngleX"), 12.0 * sign, id);
        close(val(id, "ParamBodyAngleX"), 3.9 * sign, id);
    }
    // T9：抬头 / 低头看——主轴 Y ±12 / 身 ±3.9，次轴 X ±2.7 / 身 ±0.8。
    for (id, sign) in [("look_up", 1.0f32), ("look_down", -1.0f32)] {
        close(val(id, "ParamAngleY"), 12.0 * sign, id);
        close(val(id, "ParamBodyAngleY"), 3.9 * sign, id);
        close(val(id, "ParamAngleX"), 2.7 * sign, id);
        close(val(id, "ParamBodyAngleX"), 0.8 * sign, id);
    }
    // T9：thinking 的五官五行（与 field_map.rs 的内建表情逐值一致）+ 小幅歪头。
    close(
        val("thinking", "ParamMouthForm"),
        0.0,
        "thinking ParamMouthForm",
    );
    close(
        val("thinking", "ParamEyeLOpen"),
        0.55,
        "thinking ParamEyeLOpen",
    );
    close(
        val("thinking", "ParamEyeROpen"),
        0.55,
        "thinking ParamEyeROpen",
    );
    close(
        val("thinking", "ParamBrowLY"),
        -0.45,
        "thinking ParamBrowLY",
    );
    close(
        val("thinking", "ParamBrowRY"),
        -0.45,
        "thinking ParamBrowRY",
    );
    close(val("thinking", "ParamAngleZ"), 6.0, "thinking ParamAngleZ");
    close(
        val("thinking", "ParamBodyAngleZ"),
        2.0,
        "thinking ParamBodyAngleZ",
    );
    for (id, sign) in [("tilt_left", 1.0f32), ("tilt_right", -1.0f32)] {
        close(val(id, "ParamAngleZ"), 12.0 * sign, id);
        close(val(id, "ParamBodyAngleZ"), 3.9 * sign, id);
    }

    // 次轴保持原比例（不是一律拉到 12 / 3.9）
    close(val("shake", "ParamAngleZ"), 2.2, "shake ParamAngleZ");
    close(
        val("shake", "ParamBodyAngleZ"),
        0.8,
        "shake ParamBodyAngleZ",
    );
    close(
        val("look_left", "ParamAngleZ"),
        2.7,
        "look_left ParamAngleZ",
    );
    close(
        val("look_left", "ParamBodyAngleZ"),
        0.8,
        "look_left ParamBodyAngleZ",
    );
    close(
        val("tilt_left", "ParamAngleX"),
        3.0,
        "tilt_left ParamAngleX",
    );

    // 五官：surprised 的眼 1.30 → 1.26（intensity 走满不再触 4 的上限）
    close(
        val("surprised", "ParamEyeLOpen"),
        1.26,
        "surprised ParamEyeLOpen",
    );
    close(
        val("surprised", "ParamEyeROpen"),
        1.26,
        "surprised ParamEyeROpen",
    );

    // 表情包的小幅头身不动（仍 ≤ 12 / 4）
    close(val("smile", "ParamAngleY"), 6.0, "smile ParamAngleY");
    close(
        val("smile", "ParamBodyAngleY"),
        2.0,
        "smile ParamBodyAngleY",
    );

    // 主轴身/头比（外置表单独复核）
    for id in [
        "nod",
        "shake",
        "look_left",
        "look_right",
        "look_up",
        "look_down",
        "tilt_left",
        "tilt_right",
    ] {
        let (head_id, body_id) =
            super::calibration::main_axis_pair(id).expect("手势包必须有主轴配对");
        let head = super::calibration::max_table_value(table.get(id).unwrap(), head_id);
        let body = super::calibration::max_table_value(table.get(id).unwrap(), body_id);
        let ratio = body / head;
        assert!(
            (0.30..=0.50).contains(&ratio),
            "{id} 外置表身/头 {ratio:.4} 应在 [0.30, 0.50]（tilt 原来 0.25）"
        );
    }
}
