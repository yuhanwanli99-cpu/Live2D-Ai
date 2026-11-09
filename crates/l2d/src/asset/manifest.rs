//! model3.json 清单解析片段。
//!
//! 仅 `mod.rs` 内部可见。`LayoutBox::from_json` 也在此：它读 model3 的
//! `Layout` 块，与 `ManifestFile` / `FileReferences` 同属"清单反序列化"职责。

use serde_json::Value;

use super::LayoutBox;

/// model3.json 的最小清单形状（私有；仅提取包加载所需字段，其余忽略）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct ManifestFile {
    pub(super) version: u32,
    pub(super) file_references: FileReferences,
}

/// model3.json 的 `FileReferences` 块（私有）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct FileReferences {
    pub(super) moc: String,
    #[serde(default)]
    pub(super) textures: Vec<String>,
    #[serde(default)]
    pub(super) physics: Option<String>,
    #[serde(default)]
    pub(super) display_info: Option<String>,
    /// model3 `FileReferences.Pose`（pose3.json 文件名；0.5.0 rc.4）。
    #[serde(default)]
    pub(super) pose: Option<String>,
}

/// 从 model3.json 的 `Groups` 里取某个参数组的 `Ids`（0.4.1 LipSync / 0.5.0
/// EyeBlink 共用这一段解析）。
///
/// 规则（0.4.1 `lip_sync.rs` 的形状，行为逐条保持一致）：
/// - 只看 `Groups` 数组；缺这个键或不是数组 → 空；
/// - 只收 `Target == "Parameter"` 且 `Name == name` 的组（两个比较都精确、
///   区分大小写；`lipsync` / `LIPSYNC` / 其它组一律忽略）；
/// - `Ids` 里的字符串按出现顺序保留，非字符串跳过；`Ids` 缺失或空则这一组不贡献；
/// - 有多条符合的组时按出现顺序接成一条。
///
/// 这里**不**补任何标准名回落——回落由写参数名单
/// （[`super::lip_sync::mouth_write_ids`] / `eye_blink_write_ids`）结合模型
/// 实际参数决定。
pub(crate) fn group_ids(json: &Value, name: &str) -> Vec<String> {
    let Some(groups) = json.get("Groups").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    for group in groups {
        let is_match = group.get("Target").and_then(Value::as_str) == Some("Parameter")
            && group.get("Name").and_then(Value::as_str) == Some(name);
        if !is_match {
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

impl LayoutBox {
    pub(super) fn from_json(value: Option<&Value>) -> Self {
        let default = Self::default();
        let Some(value) = value else {
            return default;
        };
        // serde_json::Value 没有 as_f32，统一走 as_f64 再收窄。
        let f32_of = |key: &str| value.get(key).and_then(Value::as_f64).map(|x| x as f32);
        Self {
            center_x: f32_of("center_x").unwrap_or(default.center_x),
            center_y: f32_of("center_y").unwrap_or(default.center_y),
            width: f32_of("width").unwrap_or(default.width),
            height: f32_of("height").unwrap_or(default.height),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(raw: &str) -> Value {
        serde_json::from_str(raw).expect("内嵌 JSON 必须合法")
    }

    #[test]
    fn group_ids_takes_exact_name_and_parameter_target_only() {
        let value = json(
            r#"{"Groups":[
                {"Target":"Part","Name":"EyeBlink","Ids":["PartWrong"]},
                {"Target":"Parameter","Name":"Eyelink","Ids":["ParamEyeLOpen"]},
                {"Target":"Parameter","Name":"EyeBlink","Ids":["PARAM_EYE_L_OPEN",42,null,"PARAM_EYE_R_OPEN"]},
                {"Target":"Parameter","Name":"EyeBlink","Ids":["ParamExtra"]}
            ]}"#,
        );
        assert_eq!(
            group_ids(&value, "EyeBlink"),
            vec!["PARAM_EYE_L_OPEN", "PARAM_EYE_R_OPEN", "ParamExtra"],
            "只收 Target=Parameter + Name 精确匹配的组；非字符串跳过；多组按序拼接"
        );
        assert!(group_ids(&value, "LipSync").is_empty());
    }

    #[test]
    fn group_ids_handles_missing_or_malformed_groups() {
        assert!(group_ids(&json(r#"{"Version":3}"#), "EyeBlink").is_empty());
        assert!(group_ids(&json(r#"{"Groups":{"Name":"EyeBlink"}}"#), "EyeBlink").is_empty());
        assert!(
            group_ids(
                &json(r#"{"Groups":[{"Target":"Parameter","Name":"EyeBlink"}]}"#),
                "EyeBlink"
            )
            .is_empty(),
            "组缺 Ids → 不贡献"
        );
        assert!(
            group_ids(
                &json(r#"{"Groups":[{"Target":"Parameter","Name":"EYEBLINK","Ids":["ParamX"]}]}"#),
                "EyeBlink"
            )
            .is_empty(),
            "组名精确匹配（区分大小写）"
        );
    }

    #[test]
    fn uppercase_snake_groups_keep_ids_verbatim() {
        // epsilon 旧样本：大写 snake 的 Groups；解析结果原样保留，不折叠成标准名。
        let value = json(
            r#"{"Groups":[
                {"Target":"Parameter","Name":"LipSync","Ids":["PARAM_MOUTH_OPEN_Y"]},
                {"Target":"Parameter","Name":"EyeBlink","Ids":["PARAM_EYE_L_OPEN","PARAM_EYE_R_OPEN"]}
            ]}"#,
        );
        assert_eq!(group_ids(&value, "LipSync"), vec!["PARAM_MOUTH_OPEN_Y"]);
        assert_eq!(
            group_ids(&value, "EyeBlink"),
            vec!["PARAM_EYE_L_OPEN", "PARAM_EYE_R_OPEN"]
        );
    }

    #[test]
    fn bai_manifest_parses_eye_blink_and_empty_lip_sync() {
        // 仓库内 bai（白模型随仓库分发）：LipSync 空数组，EyeBlink 两个标准名。
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("assets/models/bai/runtime/bai.model3.json");
        let package = crate::asset::ModelPackage::load(&path)
            .unwrap_or_else(|e| panic!("读不到 bai 皮套包 {}: {e}", path.display()));
        assert_eq!(
            package.eye_blink_ids(),
            vec!["ParamEyeLOpen".to_owned(), "ParamEyeROpen".to_owned()].as_slice()
        );
        assert!(
            package.manifest().lip_sync_ids.is_empty(),
            "bai 的 LipSync 组是空数组（回落由写参数名单负责）"
        );
    }
}
