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
