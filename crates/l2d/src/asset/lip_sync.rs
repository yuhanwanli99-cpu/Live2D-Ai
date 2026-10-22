//! model3 `Groups` 里的口型绑定解析（0.4.1 标准参数绑定）。
//!
//! 两条纯函数：
//! - [`lip_sync_ids`]：只从 `Groups` 收 `Target == "Parameter"` 且
//!   `Name == "LipSync"` 的 `Ids`（两个比较都精确、区分大小写、按出现顺序拼接）；
//! - [`mouth_write_ids`]：装载时把「解析结果 + moc3 是否真有 `ParamMouthOpenY`」
//!   合成本版真正要写的 id 名单——组非空就原样用，组空才回落标准名。
//!
//! 本模块**不做**大小写折叠、不建 `Param*` → `PARAM_*` 别名表、不读 cdi3；
//! 名单里模型上不存在的 id 由 `set_parameter` 当空操作跳过。

use serde_json::Value;

/// 从 model3.json 的 `Groups` 里取出 `LipSync` 组的 `Ids`。
///
/// 规则：
/// - 只看 `Groups` 数组；缺这个键或不是数组 → 空；
/// - 只收 `Target == "Parameter"` 且 `Name == "LipSync"` 的组（精确、区分大小写；
///   `lipsync` / `LIPSYNC` / `EyeBlink` 一律忽略）；
/// - `Ids` 里的字符串按出现顺序保留，非字符串跳过；`Ids` 缺失或空则这一组不贡献；
/// - 有多条符合的组时按出现顺序接成一条；
/// - 这里**不补** `ParamMouthOpenY`（回落只发生在 [`mouth_write_ids`]）。
pub(crate) fn lip_sync_ids(json: &Value) -> Vec<String> {
    let Some(groups) = json.get("Groups").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for group in groups {
        let is_lip_sync = group.get("Target").and_then(Value::as_str) == Some("Parameter")
            && group.get("Name").and_then(Value::as_str) == Some("LipSync");
        if !is_lip_sync {
            continue;
        }
        let Some(group_ids) = group.get("Ids").and_then(Value::as_array) else {
            continue;
        };
        for id in group_ids {
            if let Some(id) = id.as_str() {
                ids.push(id.to_owned());
            }
        }
    }
    ids
}

/// 合成装载时真正要写口型的 id 名单。
///
/// - `lip_sync` 非空：原样返回这一份，**不再追加** `ParamMouthOpenY`；组里某个 id
///   在模型上不存在也留着（`set_parameter` 对缺 id 是空操作）；
/// - `lip_sync` 空且 `has_param_mouth_open_y` 为真：`["ParamMouthOpenY"]`
///   （仓库内 bai 与官方 Mark 属于这一类）；
/// - 两者都没有：空（不发明别的嘴 id，Rice 属于这一类）。
pub(crate) fn mouth_write_ids(lip_sync: &[String], has_param_mouth_open_y: bool) -> Vec<String> {
    if !lip_sync.is_empty() {
        return lip_sync.to_vec();
    }
    if has_param_mouth_open_y {
        return vec!["ParamMouthOpenY".to_owned()];
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids_of(json: &str) -> Vec<String> {
        let value: Value = serde_json::from_str(json).expect("内嵌 JSON 必须合法");
        lip_sync_ids(&value)
    }

    #[test]
    fn lip_sync_ids_keeps_standard_mouth_id_without_appending() {
        let ids = ids_of(
            r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":["ParamMouthOpenY"]}]}"#,
        );
        assert_eq!(ids, vec!["ParamMouthOpenY"]);
        // 组非空 ⇒ 原样用，标志为真也不再追加。
        assert_eq!(mouth_write_ids(&ids, true), vec!["ParamMouthOpenY"]);
    }

    #[test]
    fn mouth_write_ids_keeps_group_ids_even_with_fallback_flag() {
        // Mao 情形：LipSync 是 ParamA；即便 moc 有 ParamMouthOpenY 也只写组里的 id。
        let ids =
            ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":["ParamA"]}]}"#);
        assert_eq!(ids, vec!["ParamA"]);
        assert_eq!(mouth_write_ids(&ids, true), vec!["ParamA"]);
    }

    #[test]
    fn mouth_write_ids_falls_back_to_standard_name() {
        // bai / Mark：LipSync 的 Ids 是空数组 ⇒ 回落 ParamMouthOpenY。
        let ids = ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":[]}]}"#);
        assert!(ids.is_empty());
        assert_eq!(mouth_write_ids(&ids, true), vec!["ParamMouthOpenY"]);
    }

    #[test]
    fn mouth_write_ids_empty_without_fallback() {
        // Rice：没有嘴参数、LipSync 也空 ⇒ 一笔都不写。
        let ids = ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":[]}]}"#);
        assert!(mouth_write_ids(&ids, false).is_empty());
    }

    #[test]
    fn lip_sync_ids_without_groups_is_empty() {
        assert!(ids_of(r#"{"Version":3,"FileReferences":{}}"#).is_empty());
        // 有 Groups 但不是数组：同样返回空。
        assert!(ids_of(r#"{"Groups":{"Target":"Parameter"}}"#).is_empty());
    }

    #[test]
    fn lip_sync_ids_skips_non_string_ids() {
        let ids = ids_of(
            r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":["ParamA",42,null,{"x":1},"ParamB"]}]}"#,
        );
        assert_eq!(ids, vec!["ParamA", "ParamB"]);
    }

    #[test]
    fn lip_sync_ids_ignores_other_targets_and_eye_blink() {
        let ids = ids_of(
            r#"{"Groups":[
                {"Target":"Part","Name":"LipSync","Ids":["ParamWrongTarget"]},
                {"Target":"Parameter","Name":"EyeBlink","Ids":["ParamEyeLOpen","ParamEyeROpen"]},
                {"Target":"Parameter","Name":"LipSync","Ids":["ParamMouthOpenY"]}
            ]}"#,
        );
        // 只收 Target=Parameter 且 Name=LipSync 的那一组；EyeBlink 不进嘴名单。
        assert_eq!(ids, vec!["ParamMouthOpenY"]);
    }

    #[test]
    fn lip_sync_ids_ignores_lowercase_name() {
        assert!(
            ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"lipsync","Ids":["ParamA"]}]}"#)
                .is_empty()
        );
        assert!(
            ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"LIPSYNC","Ids":["ParamA"]}]}"#)
                .is_empty()
        );
    }

    #[test]
    fn lip_sync_ids_preserves_uppercase_ids() {
        // epsilon 旧样本：大写 PARAM_MOUTH_OPEN_Y 原样留下，不折叠成 ParamMouthOpenY。
        let ids = ids_of(
            r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":["PARAM_MOUTH_OPEN_Y"]}]}"#,
        );
        assert_eq!(ids, vec!["PARAM_MOUTH_OPEN_Y"]);
        assert_eq!(mouth_write_ids(&ids, true), vec!["PARAM_MOUTH_OPEN_Y"]);
    }

    #[test]
    fn lip_sync_ids_concatenates_matching_groups_in_order() {
        let ids = ids_of(
            r#"{"Groups":[
                {"Target":"Parameter","Name":"LipSync","Ids":["ParamA"]},
                {"Target":"Parameter","Name":"EyeBlink","Ids":["ParamEyeLOpen"]},
                {"Target":"Parameter","Name":"LipSync","Ids":["ParamB","ParamC"]}
            ]}"#,
        );
        assert_eq!(ids, vec!["ParamA", "ParamB", "ParamC"]);
    }

    #[test]
    fn bai_model3_manifest_has_empty_lip_sync_ids() {
        // 仓库内 bai：LipSync 的 Ids 是空数组 ⇒ 回落发生在 load_model（这里解析结果必须为空）。
        // 路径从 CARGO_MANIFEST_DIR 拼到仓库根；读不到就失败，不许 skip。
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("assets/models/bai/runtime/bai.model3.json");
        let package = crate::asset::ModelPackage::load(&path)
            .unwrap_or_else(|e| panic!("读不到 bai 皮套包 {}: {e}", path.display()));
        assert!(
            package.manifest().lip_sync_ids.is_empty(),
            "bai 的 LipSync 组是空数组，解析结果必须是空（回落由 load_model 负责）"
        );
    }
}
