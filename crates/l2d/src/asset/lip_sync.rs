//! model3 `Groups` 的参数绑定名单（0.4.1 口型 / 0.5.0 rc.2 眨眼）。
//!
//! 三个纯函数：
//! - [`lip_sync_ids`]：只从 `Groups` 收 `Target == "Parameter"` 且
//!   `Name == "LipSync"` 的 `Ids`（解析本体与 EyeBlink 共用
//!   [`super::manifest::group_ids`]，两个比较都精确、区分大小写、按出现顺序拼接）；
//! - [`mouth_write_ids`]：装载时合成真正要写口型的 id 名单——组非空原样用，
//!   组空才用 [`crate::asset::ParamBinder`] 解析出的模型实际 id
//!   （0.5.0 rc.2 优先级：`Groups[LipSync].Ids` > binder canonical > 不写）；
//! - [`eye_blink_write_ids`]：同理合成眨眼名单（`Groups[EyeBlink].Ids` >
//!   binder canonical > 不写；组空回落的语义名是 `ParamEyeLOpen` / `ParamEyeROpen`）。
//!
//! 本模块**不做**大小写折叠、不建 `Param*` → `PARAM_*` 别名表、不读 cdi3；
//! 名单里模型上不存在的 id 由 `set_parameter` 当空操作跳过。

use serde_json::Value;

/// 标准口型语义名（`Groups[LipSync]` 缺失时交给 binder 解析的键）。
pub(crate) const MOUTH_OPEN_PARAM: &str = "ParamMouthOpenY";
/// 标准左眼开合语义名（`Groups[EyeBlink]` 缺失时交给 binder 解析的键）。
pub(crate) const EYE_L_OPEN_PARAM: &str = "ParamEyeLOpen";
/// 标准右眼开合语义名（`Groups[EyeBlink]` 缺失时交给 binder 解析的键）。
pub(crate) const EYE_R_OPEN_PARAM: &str = "ParamEyeROpen";

/// 从 model3.json 的 `Groups` 里取出 `LipSync` 组的 `Ids`。
///
/// 解析规则见 [`super::manifest::group_ids`]；这里**不补** `ParamMouthOpenY`
/// （回落只发生在 [`mouth_write_ids`]）。
pub(crate) fn lip_sync_ids(json: &Value) -> Vec<String> {
    super::manifest::group_ids(json, "LipSync")
}

/// 合成装载时真正要写口型的 id 名单（0.5.0 rc.2 优先级）。
///
/// - `lip_sync` 非空：原样返回这一份，**不再追加**回落 id；组里某个 id 在模型上
///   不存在也留着（`set_parameter` 对缺 id 是空操作）；
/// - `lip_sync` 空且 `fallback` 有值（= [`crate::asset::ParamBinder`] 把标准名
///   `ParamMouthOpenY` 解析到的该模型实际 id；bai / Mark 属于这一类）：
///   `[fallback]`；
/// - 两者都没有：空（不发明别的嘴 id，Rice 属于这一类）。
pub(crate) fn mouth_write_ids(lip_sync: &[String], fallback: Option<&str>) -> Vec<String> {
    if !lip_sync.is_empty() {
        return lip_sync.to_vec();
    }
    fallback.map(str::to_owned).into_iter().collect()
}

/// 合成装载时真正要写眨眼的 id 名单（0.5.0 rc.2 优先级）。
///
/// - `eye_blink` 非空：原样用组名单（bai 的组就是两个标准名，与回落路径逐值等价）；
/// - 组空：用 binder 解析出的 `ParamEyeLOpen` / `ParamEyeROpen` 实际 id
///   （大写 `PARAM_EYE_L_OPEN` / `PARAM_EYE_R_OPEN` 的旧样本由此恢复眨眼）；
/// - 都解析不到：空（模型没有眼参数，一笔不写）。
///
/// 名单里的 id 全部写**同一个**开合值（Cubism 的 EyeBlink 组左右同值），因此
/// 不依赖组内左右顺序；左右解析成同一个 id 时去重。
pub(crate) fn eye_blink_write_ids(
    eye_blink: &[String],
    left: Option<&str>,
    right: Option<&str>,
) -> Vec<String> {
    if !eye_blink.is_empty() {
        return eye_blink.to_vec();
    }
    let mut ids: Vec<String> = Vec::new();
    for id in [left, right].into_iter().flatten() {
        if !ids.iter().any(|existing| existing == id) {
            ids.push(id.to_owned());
        }
    }
    ids
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
        // 组非空 ⇒ 原样用，binder 回落有值也不再追加。
        assert_eq!(
            mouth_write_ids(&ids, Some("ParamMouthOpenY")),
            vec!["ParamMouthOpenY"]
        );
    }

    #[test]
    fn mouth_write_ids_keeps_group_ids_even_with_fallback() {
        // Mao 情形：LipSync 是 ParamA；即便 binder 能解析出 ParamMouthOpenY 也只写组里的 id。
        let ids =
            ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":["ParamA"]}]}"#);
        assert_eq!(ids, vec!["ParamA"]);
        assert_eq!(
            mouth_write_ids(&ids, Some("ParamMouthOpenY")),
            vec!["ParamA"]
        );
    }

    #[test]
    fn mouth_write_ids_falls_back_to_binder_id() {
        // bai / Mark：LipSync 的 Ids 是空数组 ⇒ 回落 binder 解析结果（标准名 → 实际 id）。
        let ids = ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":[]}]}"#);
        assert!(ids.is_empty());
        assert_eq!(
            mouth_write_ids(&ids, Some("ParamMouthOpenY")),
            vec!["ParamMouthOpenY"]
        );
        // 大写 snake 模型：同一回路落实到 `PARAM_MOUTH_OPEN_Y`。
        assert_eq!(
            mouth_write_ids(&ids, Some("PARAM_MOUTH_OPEN_Y")),
            vec!["PARAM_MOUTH_OPEN_Y"]
        );
    }

    #[test]
    fn mouth_write_ids_empty_without_fallback() {
        // Rice：没有嘴参数、LipSync 也空 ⇒ 一笔都不写。
        let ids = ids_of(r#"{"Groups":[{"Target":"Parameter","Name":"LipSync","Ids":[]}]}"#);
        assert!(mouth_write_ids(&ids, None).is_empty());
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
        assert_eq!(
            mouth_write_ids(&ids, Some("PARAM_MOUTH_OPEN_Y")),
            vec!["PARAM_MOUTH_OPEN_Y"]
        );
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
    fn eye_blink_write_ids_prefers_group_ids_over_binder_fallback() {
        let group = vec!["ParamEyeLOpen".to_owned(), "ParamEyeROpen".to_owned()];
        assert_eq!(
            eye_blink_write_ids(&group, Some("ParamEyeLClose"), Some("ParamEyeRClose")),
            group,
            "组非空 ⇒ 原样用组名单，不追加 binder 回落"
        );
    }

    #[test]
    fn eye_blink_write_ids_falls_back_to_binder_ids() {
        // 组空：大写 snake 模型回落到 `PARAM_EYE_L_OPEN` / `PARAM_EYE_R_OPEN`。
        assert_eq!(
            eye_blink_write_ids(&[], Some("PARAM_EYE_L_OPEN"), Some("PARAM_EYE_R_OPEN")),
            vec!["PARAM_EYE_L_OPEN", "PARAM_EYE_R_OPEN"]
        );
        // 只有一侧可解析：只写这一侧（另一侧如实不写）。
        assert_eq!(
            eye_blink_write_ids(&[], Some("PARAM_EYE_L_OPEN"), None),
            vec!["PARAM_EYE_L_OPEN"]
        );
        // 都解析不到 → 空（模型没有眼参数）。
        assert!(eye_blink_write_ids(&[], None, None).is_empty());
        // 左右解析成同一个 id → 去重，避免同值写两遍。
        assert_eq!(
            eye_blink_write_ids(&[], Some("ParamEyeOpen"), Some("ParamEyeOpen")),
            vec!["ParamEyeOpen"]
        );
    }

    #[test]
    fn epsilon_fixture_writes_uppercase_snake_param_ids() {
        // epsilon 旧样本（fixture）：Groups 是大写 snake；写名单必须是模型实际 id，
        // 无论走「组优先」还是「组缺失 → binder 回落」都不能写回标准名。
        let value: Value = serde_json::from_str(
            r#"{"Version":3,"Groups":[
                {"Target":"Parameter","Name":"LipSync","Ids":["PARAM_MOUTH_OPEN_Y"]},
                {"Target":"Parameter","Name":"EyeBlink","Ids":["PARAM_EYE_L_OPEN","PARAM_EYE_R_OPEN"]}
            ]}"#,
        )
        .expect("内嵌 JSON 必须合法");
        let binder = crate::asset::ParamBinder::from_ids([
            "PARAM_MOUTH_OPEN_Y",
            "PARAM_EYE_L_OPEN",
            "PARAM_EYE_R_OPEN",
        ]);
        let lip = lip_sync_ids(&value);
        assert_eq!(
            mouth_write_ids(&lip, binder.resolve(MOUTH_OPEN_PARAM)),
            vec!["PARAM_MOUTH_OPEN_Y"]
        );
        let eye = crate::asset::manifest::group_ids(&value, "EyeBlink");
        assert_eq!(
            eye_blink_write_ids(
                &eye,
                binder.resolve(EYE_L_OPEN_PARAM),
                binder.resolve(EYE_R_OPEN_PARAM)
            ),
            vec!["PARAM_EYE_L_OPEN", "PARAM_EYE_R_OPEN"]
        );
        // 组缺失时 binder 仍能回落（同一批实际 id）：
        assert_eq!(
            mouth_write_ids(&[], binder.resolve(MOUTH_OPEN_PARAM)),
            vec!["PARAM_MOUTH_OPEN_Y"]
        );
        assert_eq!(
            eye_blink_write_ids(
                &[],
                binder.resolve(EYE_L_OPEN_PARAM),
                binder.resolve(EYE_R_OPEN_PARAM)
            ),
            vec!["PARAM_EYE_L_OPEN", "PARAM_EYE_R_OPEN"]
        );
    }

    #[test]
    fn bai_manifest_binds_mouth_and_eyes_to_identical_standard_ids() {
        // 仓库内 bai：LipSync 空数组、EyeBlink 两个标准名。
        // 断言 0.5.0 的两份写名单与 0.4.1 逐值相同 → bai 行为不变。
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("assets/models/bai/runtime/bai.model3.json");
        let package = crate::asset::ModelPackage::load(&path)
            .unwrap_or_else(|e| panic!("读不到 bai 皮套包 {}: {e}", path.display()));
        let handle = crate::model::ModelHandle::from_package(&package)
            .unwrap_or_else(|e| panic!("bai moc3 解析失败: {e}"));
        let binder = crate::asset::ParamBinder::from_ids(handle.parameter_ids());

        assert!(
            package.manifest().lip_sync_ids.is_empty(),
            "bai 的 LipSync 组是空数组"
        );
        // 口型：组空 + binder 解析 ParamMouthOpenY → 与 0.4.1 的回落名单相同。
        assert_eq!(
            mouth_write_ids(
                &package.manifest().lip_sync_ids,
                binder.resolve(MOUTH_OPEN_PARAM)
            ),
            vec!["ParamMouthOpenY"]
        );

        // 眨眼：组名单 = [ParamEyeLOpen, ParamEyeROpen]，与组空回落路径逐值等价。
        let group = package.eye_blink_ids().to_vec();
        assert_eq!(group, vec!["ParamEyeLOpen", "ParamEyeROpen"]);
        let from_group = eye_blink_write_ids(
            &group,
            binder.resolve(EYE_L_OPEN_PARAM),
            binder.resolve(EYE_R_OPEN_PARAM),
        );
        let from_fallback = eye_blink_write_ids(
            &[],
            binder.resolve(EYE_L_OPEN_PARAM),
            binder.resolve(EYE_R_OPEN_PARAM),
        );
        assert_eq!(from_group, vec!["ParamEyeLOpen", "ParamEyeROpen"]);
        assert_eq!(
            from_group, from_fallback,
            "bai EyeBlink 是标准名 ⇒ 组优先与回落路径逐值等价（行为不变）"
        );
    }
}
