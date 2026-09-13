//! 模型 registry（持久化 + 路径安全 + 原子写回 + DTO 类型）。
//!
//! 设计：
//! - **单一文件 JSON**：`model_registry.json`，住在**模型根内部**
//!   （`assets/models/model_registry.json`；路径由
//!   [`crate::web_api::model_root::registry_path`] 给出）。
//! - **单一模型根**（rc.2 2026-09-12）：registry 里的相对路径一律相对
//!   **仓库 `assets/models/`**——与静态 `/models/*` 完全同一个根。
//! - **原子写回**（与 W1 settings 写盘口径一致）：`atomic_write_json`
//!   内部 = 写 `<file>.tmp.<pid>` → `fdatasync` → rename。
//! - **路径安全**：[`normalize_relative_id`] 拒绝 `..`、绝对路径特征、
//!   连续 `/` / `.` 折叠；handler 端再走 [`crate::web_api::models_routes::handlers`]
//!   路径层校验。
//! - **路径存相对值**（P1-4）：`ModelEntry.relative_dir` / `model3_rel_path`
//!   只存相对 `assets_root` 的字符串；绝对路径在每次 handler 调用时由
//!   `assets_root.join(...)` 派生——避免 WSL 迁移 / `HOME` 改变 / `data_dir`
//!   改变后旧 registry 失效。
//! - **StageConfig 与 D1 §8.1/§8.2 对齐**：snake_case 字段名、有限枚举集合。
//!
//! 不引入额外依赖（serde + serde_json + 标准库）。

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

use serde::{Deserialize, Serialize};

// =====================================================================
// 路径安全
// =====================================================================

/// 规范化 model id（拒绝 `..`、绝对路径、连续斜杠/反斜杠）。
///
/// 规则：
/// - 空 / 含 `/` / 含 `\` / 含 `..` 段 / 以 `.` 开头段 / 含 NUL → `Err`；
/// - 允许字符：`[a-zA-Z0-9_-]`（snake_case / kebab-case / camelCase 友好）；
/// - 不修改原大小写（id 保留原样用于目录名；registry 内部用 BTreeMap 字典序）。
pub fn normalize_relative_id(id: &str) -> Result<String, String> {
    if id.is_empty() {
        return Err("id 为空".to_string());
    }
    if id.contains('/') || id.contains('\\') {
        return Err("id 不允许路径分隔符".to_string());
    }
    if id.contains("..") {
        return Err("id 不允许 `..` 段".to_string());
    }
    if id.starts_with('.') {
        return Err("id 不允许以 `.` 开头".to_string());
    }
    if id.contains('\0') {
        return Err("id 包含 NUL".to_string());
    }
    for c in id.chars() {
        let ok = c.is_ascii_alphanumeric() || c == '_' || c == '-';
        if !ok {
            return Err(format!("id 包含非法字符 {c:?}"));
        }
    }
    Ok(id.to_string())
}

// =====================================================================
// 原子写回（与 W1 settings 写盘口径一致）
// =====================================================================

/// 原子写回 JSON 文件（写 `<file>.tmp.<pid>` → `fdatasync` → rename）。
///
/// 失败返 `Err(String)`，由 caller 映射到 500 persist_failed。
pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    use std::io::Write;
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|e| format!("registry 序列化失败: {e}"))?;
    if bytes.is_empty() {
        return Err("registry 序列化结果为空（拒绝空写盘）".to_string());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建父目录失败（{}）: {e}", parent.display()))?;
    }
    let pid = process::id();
    let tmp = append_tmp_suffix(path, pid);
    let write_result = (|| -> io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_data()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "原子写回失败（{}，tmp={}）: {e}",
            path.display(),
            tmp.display()
        ));
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "rename 失败（{} → {}）: {e}",
            tmp.display(),
            path.display()
        ));
    }
    Ok(())
}

fn append_tmp_suffix(path: &Path, pid: u32) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".tmp.{pid}"));
    PathBuf::from(s)
}

// =====================================================================
// DTO / 模型
// =====================================================================

/// 模型来源（v1 仅 `LocalImport`；ZIP 上传标 D3.2 后置）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelSource {
    /// 受控本地导入（body 指定 assets/models/<id>/ 相对路径）。
    LocalImport,
    /// 其它来源（占位；本批不产出）。
    #[serde(other)]
    Unknown,
}

/// 展示适配模式（D1 §8.1 `fit_mode`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    #[default]
    Contain,
    Cover,
    Stretch,
}

/// 背景类型（D1 §8.2 `background_type`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundType {
    #[default]
    Transparent,
    Color,
    Image,
}

/// 模型展示配置（D1 §8.1）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelDisplay {
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub rotation: f32,
    pub fit_mode: FitMode,
}

impl Default for ModelDisplay {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            rotation: 0.0,
            fit_mode: FitMode::default(),
        }
    }
}

/// 舞台展示配置（D1 §8.2）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageConfig {
    pub width: u32,
    pub height: u32,
    pub background_type: BackgroundType,
    pub background_color: String,
    pub background_image: String,
    pub background_opacity: f32,
}

impl Default for StageConfig {
    fn default() -> Self {
        Self {
            width: 360,
            height: 540,
            background_type: BackgroundType::Transparent,
            background_color: "#00000000".to_string(),
            background_image: String::new(),
            background_opacity: 1.0,
        }
    }
}

pub fn default_stage() -> StageConfig {
    StageConfig::default()
}

/// 持久化用的 manifest 字段（从 `l2d::asset::PackageManifest` 拷贝所需字段；
/// `PackageManifest` 未实现 `Serialize/Deserialize`，本结构是其在 registry
/// 中的等价序列化形态——所有字段与 D1 §6.3 字段对齐表一致）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredManifest {
    /// model3.json `Version` 字段。
    pub version: u32,
    /// moc3 文件相对路径（相对 model3.json 所在目录）。
    pub moc3_file: String,
    /// 纹理相对路径列表。
    pub texture_files: Vec<String>,
    /// physics3.json 相对路径（可无）。
    pub physics_file: Option<String>,
    /// cdi3.json 相对路径（可无）。
    pub display_info_file: Option<String>,
    /// `Layout` 构图框（缺省整画布）。
    pub layout: StoredLayout,
}

impl From<&l2d::asset::PackageManifest> for StoredManifest {
    fn from(m: &l2d::asset::PackageManifest) -> Self {
        Self {
            version: m.version,
            moc3_file: m.moc3_file.clone(),
            texture_files: m.texture_files.clone(),
            physics_file: m.physics_file.clone(),
            display_info_file: m.display_info_file.clone(),
            layout: StoredLayout::from(&m.layout),
        }
    }
}

impl From<l2d::asset::PackageManifest> for StoredManifest {
    fn from(m: l2d::asset::PackageManifest) -> Self {
        Self::from(&m)
    }
}

/// 持久化用的 layout（拷贝自 `l2d::asset::LayoutBox`）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredLayout {
    pub center_x: f32,
    pub center_y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<&l2d::asset::LayoutBox> for StoredLayout {
    fn from(l: &l2d::asset::LayoutBox) -> Self {
        Self {
            center_x: l.center_x,
            center_y: l.center_y,
            width: l.width,
            height: l.height,
        }
    }
}

/// registry 中单条模型记录。
///
/// 路径字段（**相对 assets_root**；绝对路径由 handler 调用时按
/// `assets_root.join(relative_*)` 派生，避免 WSL 迁移 / `HOME` 改变 /
/// `data_dir` 改变时旧 registry 失效）：
/// - [`relative_dir`](Self::relative_dir)：相对 `assets_root` 的目录
///   （如 `"bai"`）。
/// - [`model3_rel_path`](Self::model3_rel_path)：相对 `assets_root` 的
///   `*.model3.json` 路径（如 `"bai/runtime/bai.model3.json"`）。
///
/// **兼容策略**：v1 旧 registry 文件存的字段名为 `absolute_dir` /
/// `model3_path`（绝对路径）。新字段名无 `default`，反序列化失败走
/// P1-5 的损坏降级路径——`model_registry.json.corrupt-<ts>` 备份后用
/// 空 registry 重新持久化。registry 是运行时数据，可重建（用户重
/// 跑 import 即可）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    pub display_name: String,
    pub source: ModelSource,
    pub relative_dir: String,
    pub model3_rel_path: String,
    pub manifest: StoredManifest,
    pub display: ModelDisplay,
    pub stage: StageConfig,
    pub imported_at: String,
}

/// 模型 registry（单一 JSON 文件）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelRegistry {
    pub schema_version: u32,
    pub active_id: Option<String>,
    pub entries: BTreeMap<String, ModelEntry>,
}

impl ModelRegistry {
    /// 从磁盘加载（缺失 = `Ok(Self::default())`；损坏 = `Err`）。
    pub fn load_from_path(path: &Path) -> Result<Self, io::Error> {
        let raw = fs::read(path)?;
        serde_json::from_slice(&raw).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("registry JSON 损坏: {e}"),
            )
        })
    }
}
