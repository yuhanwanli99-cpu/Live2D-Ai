//! 模型路由 DTO（HTTP 响应 / 请求类型）。
//!
//! 字段与 D1 §1.3 / §6.3 / §8.1 / §8.2 字段对齐表一致；snake_case。
//! `deny_unknown_fields` 锁住未知键——前端传错键一目了然。

use serde::{Deserialize, Serialize};

use super::registry::{BackgroundType, FitMode, ModelDisplay, StageConfig};

/// `GET /api/v1/models` 列表项。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModelListItem {
    pub id: String,
    pub display_name: String,
    pub version: u32,
    pub layout: LayoutDto,
    pub moc3_file: String,
    pub texture_files: Vec<String>,
    pub has_physics: bool,
    pub has_display_info: bool,
    pub active: bool,
    pub imported_at: String,
    pub size_bytes: u64,
}

/// `Layout` 段 DTO（与 `l2d::asset::LayoutBox` 字段一一对应）。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LayoutDto {
    pub center_x: f32,
    pub center_y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<&l2d::asset::LayoutBox> for LayoutDto {
    fn from(b: &l2d::asset::LayoutBox) -> Self {
        Self {
            center_x: b.center_x,
            center_y: b.center_y,
            width: b.width,
            height: b.height,
        }
    }
}

/// `StoredLayout` → `LayoutDto`（registry 反序列化后用）。
impl From<&super::registry::StoredLayout> for LayoutDto {
    fn from(b: &super::registry::StoredLayout) -> Self {
        Self {
            center_x: b.center_x,
            center_y: b.center_y,
            width: b.width,
            height: b.height,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelListResponse {
    pub models: Vec<ModelListItem>,
}

/// `POST /api/v1/models/import` 请求 body。
#[derive(Debug, Clone, Deserialize)]
pub struct ImportRequest {
    pub id: String,
}

/// `POST /api/v1/models/import` 响应。
#[derive(Debug, Clone, Serialize)]
pub struct ImportResponse {
    pub id: String,
    /// 解析后的 model3.json 相对路径（相对 data_dir）。
    pub model3_json_path: String,
    /// 是否替换了既有 id（v1 = 恒为 false；overwrite 标 D3.2 后置）。
    pub applied: bool,
}

/// `POST /api/v1/models/{id}/activate` 响应。
#[derive(Debug, Clone, Serialize)]
pub struct ActivateResponse {
    pub active_id: String,
    pub prev_active_id: Option<String>,
    /// v1 = false（supervisor 重新加载延后到 settings_p0c 闭环或重启）。
    pub requires_restart: bool,
    /// 模型 runtime URL（`/models/<model3_rel_path>`），供前端直接加载。
    pub model_url: String,
}

/// `PATCH /api/v1/models/{id}/display` 请求 body。
///
/// 段级（`model` / `stage`）缺省 = 不参与；`null` = 显式回退默认（**本批
/// 不区分** `null` vs 缺省，handler 视为"未提供"——避免误清空）。
/// D3.2 后置可加 `clear_*` 段级标志。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayPatchBody {
    #[serde(default)]
    pub model: Option<DisplayModelPatch>,
    #[serde(default)]
    pub stage: Option<DisplayStagePatch>,
}

impl DisplayPatchBody {
    /// 跨段校验：依次调用 model/stage 的 `validate`，首个失败短路返回。
    pub fn validate(&self) -> Result<(), String> {
        if let Some(m) = &self.model {
            m.validate()?;
        }
        if let Some(s) = &self.stage {
            s.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayModelPatch {
    #[serde(default)]
    pub scale: Option<f32>,
    #[serde(default)]
    pub offset_x: Option<f32>,
    #[serde(default)]
    pub offset_y: Option<f32>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub fit_mode: Option<FitMode>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayStagePatch {
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub background_type: Option<BackgroundType>,
    #[serde(default)]
    pub background_color: Option<String>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub background_opacity: Option<f32>,
}

impl DisplayModelPatch {
    /// 段级 apply（None 字段保留原值；避免误清空）。
    pub fn apply_into(self, target: &mut ModelDisplay) {
        if let Some(v) = self.scale {
            target.scale = v;
        }
        if let Some(v) = self.offset_x {
            target.offset_x = v;
        }
        if let Some(v) = self.offset_y {
            target.offset_y = v;
        }
        if let Some(v) = self.rotation {
            target.rotation = v;
        }
        if let Some(v) = self.fit_mode {
            target.fit_mode = v;
        }
    }

    /// 产品级最小校验（仅检查 None 之外的字段）。
    ///
    /// 规则：
    /// - `scale` 必须 > 0 且 finite；
    /// - `offset_x` / `offset_y` / `rotation` 必须 finite。
    ///
    /// 返回首个失败原因（行内 400 invalid_params 用）。
    pub fn validate(&self) -> Result<(), String> {
        if let Some(v) = self.scale {
            if !v.is_finite() {
                return Err("model.scale 必须有限（不允许 NaN/Inf）".into());
            }
            if v <= 0.0 {
                return Err(format!("model.scale 必须 > 0（收到 {v}）"));
            }
        }
        if let Some(v) = self.offset_x
            && !v.is_finite()
        {
            return Err("model.offset_x 必须有限".into());
        }
        if let Some(v) = self.offset_y
            && !v.is_finite()
        {
            return Err("model.offset_y 必须有限".into());
        }
        if let Some(v) = self.rotation
            && !v.is_finite()
        {
            return Err("model.rotation 必须有限".into());
        }
        Ok(())
    }
}

impl DisplayStagePatch {
    /// 段级 apply（None 字段保留原值；避免误清空）。
    pub fn apply_into(self, target: &mut StageConfig) {
        if let Some(v) = self.width {
            target.width = v;
        }
        if let Some(v) = self.height {
            target.height = v;
        }
        if let Some(v) = self.background_type {
            target.background_type = v;
        }
        if let Some(v) = self.background_color {
            target.background_color = v;
        }
        if let Some(v) = self.background_image {
            target.background_image = v;
        }
        if let Some(v) = self.background_opacity {
            target.background_opacity = v;
        }
    }

    /// 产品级最小校验（仅检查 None 之外的字段）。
    ///
    /// 规则：
    /// - `width` / `height` 必须 > 0（拒绝 0，避免渲染 0×0 stage）；
    /// - `background_opacity` 必须 ∈ [0, 1] 且 finite；
    /// - `background_color` 必须匹配 `#RRGGBB` 或 `#RRGGBBAA`。
    pub fn validate(&self) -> Result<(), String> {
        if let Some(w) = self.width
            && w == 0
        {
            return Err("stage.width 必须 > 0（收到 0）".into());
        }
        if let Some(h) = self.height
            && h == 0
        {
            return Err("stage.height 必须 > 0（收到 0）".into());
        }
        if let Some(op) = self.background_opacity {
            if !op.is_finite() {
                return Err("stage.background_opacity 必须有限".into());
            }
            if !(0.0..=1.0).contains(&op) {
                return Err(format!(
                    "stage.background_opacity 必须在 [0, 1]（收到 {op}）"
                ));
            }
        }
        if let Some(color) = self.background_color.as_deref()
            && !is_hex_color(color)
        {
            return Err(format!(
                "stage.background_color 必须为 #RRGGBB 或 #RRGGBBAA（收到 {color:?}）"
            ));
        }
        Ok(())
    }
}

/// 手写 hex 颜色校验：`#` + 6 或 8 位 `[0-9a-fA-F]`（不接受 `#RGB`/`rgba()`/`named`）。
fn is_hex_color(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 7 && bytes.len() != 9 {
        return false;
    }
    if bytes[0] != b'#' {
        return false;
    }
    bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
}
