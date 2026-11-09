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
    /// **无 Layout** 的新模型用的产品默认取景（与出厂 bai 的 model3 同口径：
    /// center_y -0.07、0.56×0.56）。
    ///
    /// 2026-11-09 真机实测：VTS 导出的 model3 很多完全不带 `Layout`，旧行为退
    /// 1.0×1.0「整画布」→ 竖长画布被撑满裁头（《大肥鱼》568KB 裁切 → 改成
    /// 同口径取景后 319KB 全身）。只影响「Layout 缺失」的模型，不动 bai /
    /// epsilon 等本来就有 Layout 的皮套。
    pub(super) fn product_framing() -> Self {
        Self {
            center_x: 0.0,
            center_y: -0.07,
            width: 0.56,
            height: 0.56,
        }
    }

    pub(super) fn from_json(value: Option<&Value>) -> Self {
        let default = Self::default();
        let Some(value) = value else {
            return Self::product_framing();
        };
        let Some(object) = value.as_object() else {
            return Self::product_framing();
        };
        if object.is_empty() {
            return Self::product_framing();
        }
        // 两套键都认：官方 Cubism model3 用 PascalCase（`CenterX` / `Width`…），
        // 本仓库历史自带的 bai 用小写 snake_case（`center_x`…）。小写优先，
        // 因此既有 bai 的解析结果逐值不变。
        let f32_of = |snake: &str, pascal: &str| {
            value
                .get(snake)
                .or_else(|| value.get(pascal))
                .and_then(Value::as_f64)
                .map(|x| x as f32)
        };
        Self {
            center_x: f32_of("center_x", "CenterX").unwrap_or(default.center_x),
            center_y: f32_of("center_y", "CenterY").unwrap_or(default.center_y),
            width: f32_of("width", "Width").unwrap_or(default.width),
            height: f32_of("height", "Height").unwrap_or(default.height),
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

    /// 官方 Cubism model3 的 `Layout` 用 PascalCase（CenterX/CenterY/Width/Height）。
    /// 旧的只读小写实现会整块忽略 → 模型 1.0×1.0 撑满裁切（真机实测）。
    #[test]
    fn layout_accepts_official_pascal_case_keys() {
        let value =
            json(r#"{"Layout":{"CenterX":0.1,"CenterY":-0.07,"Width":0.56,"Height":0.56}}"#);
        assert_eq!(
            LayoutBox::from_json(Some(&value["Layout"])),
            LayoutBox {
                center_x: 0.1,
                center_y: -0.07,
                width: 0.56,
                height: 0.56,
            }
        );
    }

    /// 两套键同时出现时**小写优先**（既有 bai 行为不变）；大小写键等价。
    #[test]
    fn layout_lowercase_takes_precedence_when_both_key_styles_present() {
        let value = json(
            r#"{"Layout":{"center_x":0.2,"CenterX":9.0,"center_y":-0.07,"CenterY":9.0,
                 "width":0.56,"Width":9.0,"height":0.56,"Height":9.0}}"#,
        );
        assert_eq!(
            LayoutBox::from_json(Some(&value["Layout"])),
            LayoutBox {
                center_x: 0.2,
                center_y: -0.07,
                width: 0.56,
                height: 0.56,
            }
        );
    }

    /// 没有 Layout / 空对象 / 不是对象 → 产品默认取景（与 bai 同口径）。
    #[test]
    fn missing_or_empty_layout_uses_product_framing() {
        let framing = LayoutBox {
            center_x: 0.0,
            center_y: -0.07,
            width: 0.56,
            height: 0.56,
        };
        assert_eq!(LayoutBox::from_json(None), framing, "缺 Layout");
        let empty = json(r#"{"Layout":{}}"#);
        assert_eq!(
            LayoutBox::from_json(Some(&empty["Layout"])),
            framing,
            "空 Layout"
        );
        let not_object = json(r#"{"Layout":1.0}"#);
        assert_eq!(
            LayoutBox::from_json(Some(&not_object["Layout"])),
            framing,
            "非对象 Layout"
        );
    }

    /// Layout 存在但只写了一部分键：仍按「整画布」补其余字段（既有语义不变）。
    #[test]
    fn partial_layout_keeps_whole_canvas_field_fallback() {
        let value = json(r#"{"Layout":{"Width":0.56}}"#);
        assert_eq!(
            LayoutBox::from_json(Some(&value["Layout"])),
            LayoutBox {
                center_x: 0.0,
                center_y: 0.0,
                width: 0.56,
                height: 1.0,
            }
        );
    }

    /// 仓库内 bai（小写 `Layout`）解析结果逐值不变：0/‑0.07/0.56/0.56。
    #[test]
    fn bai_lowercase_layout_still_parses_to_the_same_framing() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("assets/models/bai/runtime/bai.model3.json");
        let package = crate::asset::ModelPackage::load(&path)
            .unwrap_or_else(|e| panic!("读不到 bai 皮套包 {}: {e}", path.display()));
        assert_eq!(
            package.manifest().layout,
            LayoutBox {
                center_x: 0.0,
                center_y: -0.07,
                width: 0.56,
                height: 0.56,
            },
            "bai 的既有取景不得变化"
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
