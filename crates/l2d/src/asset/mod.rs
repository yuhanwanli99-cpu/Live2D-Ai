//! model3 皮套包加载（通用模型路径 + 内存资源映射，不绑定具体皮套）。
//!
//! 一个「包」指一组文件：`*.model3.json` 清单 + 其 `FileReferences` 指向的
//! moc3 / 纹理 / physics3.json / cdi3.json。两种构造入口**共用同一套
//! 「解析清单 → 校验引用 → 严格取字节」组装逻辑**（[`ModelPackage::assemble`]）：
//! - [`ModelPackage::load`]：从磁盘加载，相对引用按 model3.json 所在目录拼接；
//! - [`ModelPackage::from_memory`] / [`from_memory_map`](Self::from_memory_map)：
//!   输入 model3 JSON 字节 + 以规范化相对路径为键的资源映射闭包/映射表，
//!   面向无文件系统环境（浏览器 fetch、嵌入资源等）。
//!
//! 两条入口都只做**读取与清单解析**（moc3 二进制的真实解析在 [`crate::model`]，
//! GPU 资源上传在 [`crate::renderer`]），因此本模块可在无 GPU 环境下独立测试。
//!
//! 路径安全：清单里的每个相对引用都先经 [`normalize_resource_ref`] 规范化
//! （折叠 `.` 与连续 `/`；拒绝绝对路径、`\` 分隔符与任何 `..` 上跳段，
//! 错误携带清单里的原始资源名）。磁盘与内存映射查找都使用规范化后的键。
//!
//! 兼容性判定沿用 crate 既有骨架：moc3 头经 [`crate::format::guess_format`]
//! 白名单探测后生成 [`crate::report::CompatibilityReport`]；v0 唯一锚定档位为
//! Bai 实测的 MOC raw 版本字节 4（Cubism 4.2）。
//!
//! # 严格性约定
//!
//! 清单声明了的资源（moc3、每张纹理、physics3.json、cdi3.json）一律严格读取：
//! 磁盘缺失 → [`LoadError::Io`]；内存映射缺失 → [`LoadError::MissingResource`]；
//! 不做任何推断或降级。

mod manifest;
mod report;

use std::{collections::BTreeMap, fmt, fs, io, path};

use serde_json::Value;

use crate::report::CompatibilityReport;

use manifest::ManifestFile;

/// 包加载错误。
#[derive(Debug)]
#[non_exhaustive]
pub enum LoadError {
    /// 打开/读取某个包内文件失败。
    Io {
        /// 失败的文件路径。
        path: path::PathBuf,
        /// 底层 IO 错误。
        source: io::Error,
    },
    /// JSON（model3.json 或 physics3.json）解析失败。
    Json(serde_json::Error),
    /// 清单里的资源相对路径非法（空路径、绝对路径、含 `\` 或 `..` 上跳段）。
    ///
    /// `name` 是清单里的原始引用字符串；规范化规则见 [`normalize_resource_ref`]。
    InvalidResourcePath {
        /// 非法的资源引用（清单原文）。
        name: String,
    },
    /// 内存资源映射中缺少清单引用的资源（[`ModelPackage::from_memory`] 路径）。
    MissingResource {
        /// 缺失的资源键（规范化后的相对路径）。
        name: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, .. } => write!(f, "读取皮套包文件失败: {}", path.display()),
            Self::Json(e) => write!(f, "皮套包 JSON 解析失败: {e}"),
            Self::InvalidResourcePath { name } => write!(
                f,
                "皮套资源相对路径非法（禁止绝对路径、`\\` 与 `..` 上跳）: {name}"
            ),
            Self::MissingResource { name } => {
                write!(f, "皮套资源映射缺少清单引用的资源: {name}")
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json(e) => Some(e),
            Self::InvalidResourcePath { .. } | Self::MissingResource { .. } => None,
        }
    }
}

/// model3.json 的 `Layout` 块（显示用构图框，归一化坐标）。
///
/// `meta` 层未建模该块，这里按 bakeoff 验证过的口径从原始 JSON 补齐；
/// 缺省值为「整画布」（center 0/0、宽高 1/1）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutBox {
    pub center_x: f32,
    pub center_y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for LayoutBox {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }
}

/// model3.json 解析后的清单摘要（自有类型，不含第三方结构）。
#[derive(Debug, Clone, PartialEq)]
pub struct PackageManifest {
    /// model3.json 的 `Version` 字段。
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
    pub layout: LayoutBox,
}

/// 已加载的 model3 皮套包：清单 + 各文件字节。
///
/// 字节在本类型内持有，供 [`crate::model::ModelHandle`]（moc3 解析）与
/// [`crate::renderer::OffscreenRenderer`]（纹理上传、物理构建）消费。
#[derive(Debug, Clone)]
pub struct ModelPackage {
    manifest: PackageManifest,
    base_dir: path::PathBuf,
    moc3_bytes: Vec<u8>,
    texture_bytes: Vec<Vec<u8>>,
    physics_json: Option<Vec<u8>>,
    display_info_json: Option<Vec<u8>>,
}

impl ModelPackage {
    /// 从 `*.model3.json` 路径加载整个皮套包。
    ///
    /// 与 [`Self::from_memory`] 共用同一组装逻辑：解析清单后，严格读取全部被
    /// 引用文件（moc3、纹理、physics3.json、cdi3.json）；缺失引用即报错，
    /// 不做任何推断或降级。相对引用先经路径规范化（拒绝 `..`/绝对路径，
    /// 见 [`normalize_resource_ref`]），再按 model3.json 所在目录拼接。
    pub fn load(model3_path: impl AsRef<path::Path>) -> Result<Self, LoadError> {
        let model3_path = model3_path.as_ref();
        let raw = read_file(model3_path)?;
        let base_dir = model3_path
            .parent()
            .map(path::Path::to_path_buf)
            .unwrap_or_default();
        Self::assemble(&raw, base_dir.clone(), |name| {
            read_file(base_dir.join(name))
        })
    }

    /// 从内存构造皮套包：model3 JSON 字节 + 资源映射闭包。
    ///
    /// - 输入：`model3_json` 为 model3.json 的原始字节；`resources` 以
    ///   **规范化后的相对路径**为键提供资源字节，返回 `None` 视为缺失；
    /// - 组装逻辑与 [`Self::load`] 完全一致：解析 manifest 后严格取
    ///   moc3 / 纹理 / physics3.json / cdi3.json——任何一项缺失即
    ///   [`LoadError::MissingResource`]（错误携带资源名），不做降级；
    /// - 相对路径先经 [`normalize_resource_ref`] 校验规范化（防 `..` 上跳与
    ///   绝对路径），再查映射；常规清单引用规范化后不变；
    /// - 包的 `base_dir` 为空（无磁盘目录语义）。
    pub fn from_memory<F>(model3_json: &[u8], mut resources: F) -> Result<Self, LoadError>
    where
        F: FnMut(&str) -> Option<Vec<u8>>,
    {
        Self::assemble(model3_json, path::PathBuf::new(), |name| {
            resources(name).ok_or_else(|| LoadError::MissingResource {
                name: name.to_owned(),
            })
        })
    }

    /// [`Self::from_memory`] 的映射表便利重载：直接以
    /// `BTreeMap<String, Vec<u8>>` 为资源映射（键为规范化相对路径）。
    pub fn from_memory_map(
        model3_json: &[u8],
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> Result<Self, LoadError> {
        Self::from_memory(model3_json, |name| resources.get(name).cloned())
    }

    /// 统一组装逻辑（私有）：解析清单 → 逐个校验/规范化引用 → 经 `read` 取字节。
    ///
    /// 磁盘入口把 `read` 实现为「base_dir 拼接 + fs 读」，内存入口实现为
    /// 「映射查找」；两条入口在此汇合，行为必然一致。
    fn assemble<R>(
        raw_model3: &[u8],
        base_dir: path::PathBuf,
        mut read: R,
    ) -> Result<Self, LoadError>
    where
        R: FnMut(&str) -> Result<Vec<u8>, LoadError>,
    {
        let json: Value = serde_json::from_slice(raw_model3).map_err(LoadError::Json)?;
        let info: ManifestFile = serde_json::from_value(json.clone()).map_err(LoadError::Json)?;

        let refs = &info.file_references;
        let moc3_bytes = read(normalize_resource_ref(&refs.moc)?.as_str())?;

        let mut texture_bytes = Vec::with_capacity(refs.textures.len());
        for name in &refs.textures {
            texture_bytes.push(read(normalize_resource_ref(name)?.as_str())?);
        }

        let physics_json = match &refs.physics {
            Some(name) => Some(read(normalize_resource_ref(name)?.as_str())?),
            None => None,
        };
        let display_info_json = match &refs.display_info {
            Some(name) => Some(read(normalize_resource_ref(name)?.as_str())?),
            None => None,
        };

        Ok(Self {
            manifest: PackageManifest {
                version: info.version,
                moc3_file: refs.moc.clone(),
                texture_files: refs.textures.clone(),
                physics_file: refs.physics.clone(),
                display_info_file: refs.display_info.clone(),
                layout: LayoutBox::from_json(json.get("Layout")),
            },
            base_dir,
            moc3_bytes,
            texture_bytes,
            physics_json,
            display_info_json,
        })
    }

    /// 清单摘要。
    pub fn manifest(&self) -> &PackageManifest {
        &self.manifest
    }

    /// 包根目录（model3.json 所在目录；内存构造的包为空路径）。
    pub fn base_dir(&self) -> &path::Path {
        &self.base_dir
    }

    /// moc3 文件原始字节。
    pub fn moc3_bytes(&self) -> &[u8] {
        &self.moc3_bytes
    }

    /// 纹理文件原始字节（顺序与清单一致）。
    pub fn texture_bytes(&self) -> &[Vec<u8>] {
        &self.texture_bytes
    }

    /// physics3.json 原始字节（未启用物理的模型为 `None`）。
    pub fn physics_json(&self) -> Option<&[u8]> {
        self.physics_json.as_deref()
    }

    /// cdi3.json 原始字节（清单未声明显示信息文件时为 `None`）。
    pub fn cdi3_json(&self) -> Option<&[u8]> {
        self.display_info_json.as_deref()
    }

    /// 对包内 moc3 头做白名单探测并生成兼容报告。
    ///
    /// v0 判定语义与 crate 既有骨架一致：仅 Bai 实测档位（raw 字节 4 → Cubism 4.2）
    /// 无问题可通过；其余档位生成类型化问题并由
    /// [`CompatibilityReport::is_supported_v0`] 否决。注意这是**格式层**报告；
    /// 统一加载路径的「头探测 × 运行时版本」交叉核对在
    /// [`crate::model::LoadedModel::resolve`] 中完成。
    pub fn compatibility_report(&self) -> CompatibilityReport {
        report::build_compatibility_report(&self.manifest, &self.moc3_bytes)
    }
}

fn read_file(path: impl AsRef<path::Path>) -> Result<Vec<u8>, LoadError> {
    let path = path.as_ref();
    fs::read(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// 规范化清单里的资源相对路径（磁盘与内存入口共用）。
///
/// 规则（面向安全的最小规范化，不做平台化解释）：
/// - 空路径、以 `/` 开头的绝对路径、含 `\` 的路径 → [`LoadError::InvalidResourcePath`]；
/// - 任何 `..` 段一律拒绝（防目录上跳/zip-slip 类逃逸），即使最终不越界；
/// - `.` 段与连续 `/` 折叠；规范化结果为空 → 非法。
///
/// 错误携带清单里的原始引用字符串（`name`）。
fn normalize_resource_ref(name: &str) -> Result<String, LoadError> {
    let invalid = || LoadError::InvalidResourcePath {
        name: name.to_owned(),
    };
    if name.is_empty() || name.starts_with('/') || name.contains('\\') {
        return Err(invalid());
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in name.split('/') {
        match segment {
            "" | "." => {}
            ".." => return Err(invalid()),
            _ => parts.push(segment),
        }
    }
    if parts.is_empty() {
        return Err(invalid());
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::MOC3_HEADER_SIZE;

    /// 最小合法 64 字节 MOC3 头（版本字节 `version_byte`）。
    fn moc_header(version_byte: u8) -> Vec<u8> {
        let mut header = vec![0_u8; MOC3_HEADER_SIZE];
        header[..4].copy_from_slice(b"MOC3");
        header[4] = version_byte;
        header
    }

    /// 在临时目录组装一个最小包（假 moc3 头 + 假纹理字节），返回路径。
    fn write_fixture(dir: &path::Path, version_byte: u8) -> path::PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("dummy.moc3"), moc_header(version_byte)).unwrap();
        std::fs::write(dir.join("tex_00.png"), b"\x89PNG-fake-bytes").unwrap();
        std::fs::write(
            dir.join("physics3.json"),
            br#"{"Version":3,"Meta":{},"PhysicsSettings":[]}"#,
        )
        .unwrap();
        let model3 = r#"
                {
                    "Version": 3,
                    "FileReferences": {
                        "Moc": "dummy.moc3",
                        "Textures": ["tex_00.png"],
                        "Physics": "physics3.json"
                    },
                    "Layout": {"center_x": 0.25, "center_y": -0.5, "width": 0.75}
                }
        "#;
        let model3_path = dir.join("model.model3.json");
        std::fs::write(&model3_path, model3).unwrap();
        model3_path
    }

    #[test]
    fn loads_manifest_layout_and_bytes() {
        let dir =
            std::env::temp_dir().join(format!("l2d-asset-test-{}-{}", std::process::id(), line!()));
        let model3_path = write_fixture(&dir, 4);

        let package = ModelPackage::load(&model3_path).expect("load fixture");
        let manifest = package.manifest();
        assert_eq!(manifest.version, 3);
        assert_eq!(manifest.moc3_file, "dummy.moc3");
        assert_eq!(manifest.texture_files, vec!["tex_00.png"]);
        assert_eq!(manifest.physics_file.as_deref(), Some("physics3.json"));
        assert_eq!(manifest.display_info_file, None);
        assert_eq!(
            manifest.layout,
            LayoutBox {
                center_x: 0.25,
                center_y: -0.5,
                width: 0.75,
                height: 1.0,
            }
        );
        assert_eq!(package.moc3_bytes(), moc_header(4));
        assert_eq!(package.texture_bytes().len(), 1);
        assert!(package.physics_json().is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_referenced_file_is_io_error() {
        let dir =
            std::env::temp_dir().join(format!("l2d-asset-test-{}-{}", std::process::id(), line!()));
        std::fs::create_dir_all(&dir).unwrap();
        let model3 = r#"{
            "Version": 3,
            "FileReferences": {"Moc": "nope.moc3", "Textures": []}
        }"#;
        let model3_path = dir.join("broken.model3.json");
        std::fs::write(&model3_path, model3).unwrap();

        match ModelPackage::load(&model3_path) {
            Err(LoadError::Io { path, .. }) => {
                assert!(path.ends_with("nope.moc3"));
            }
            other => panic!("期望 Io 错误，实际 {other:?}"),
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    // ---- 内存构造（from_memory / from_memory_map）与路径加固 ----

    /// 最小内存包：model3 JSON 字节 + 全量资源映射（moc/纹理/physics/cdi）。
    fn mem_fixture(version_byte: u8) -> (Vec<u8>, BTreeMap<String, Vec<u8>>) {
        let model3 = br#"{
            "Version": 3,
            "FileReferences": {
                "Moc": "dummy.moc3",
                "Textures": ["tex_00.png"],
                "Physics": "physics3.json",
                "DisplayInfo": "cdi3.json"
            },
            "Layout": {"center_x": 0.5}
        }"#
        .to_vec();
        let mut resources = BTreeMap::new();
        resources.insert("dummy.moc3".to_owned(), moc_header(version_byte));
        resources.insert("tex_00.png".to_owned(), b"\x89PNG-fake-bytes".to_vec());
        resources.insert(
            "physics3.json".to_owned(),
            br#"{"Version":3,"Meta":{},"PhysicsSettings":[]}"#.to_vec(),
        );
        resources.insert("cdi3.json".to_owned(), br#"{"Parameters":[]}"#.to_vec());
        (model3, resources)
    }

    #[test]
    fn from_memory_builds_package_strictly_including_cdi() {
        let (model3, resources) = mem_fixture(4);

        let package = ModelPackage::from_memory_map(&model3, &resources).expect("内存包应可组装");
        assert_eq!(package.manifest().version, 3);
        assert_eq!(package.manifest().moc3_file, "dummy.moc3");
        assert_eq!(package.manifest().texture_files, vec!["tex_00.png"]);
        assert_eq!(package.moc3_bytes(), moc_header(4));
        assert_eq!(package.texture_bytes().len(), 1);
        assert!(package.physics_json().is_some());
        // cdi3.json 现在也被严格读入。
        assert_eq!(
            package.cdi3_json(),
            Some(br#"{"Parameters":[]}"#.as_slice())
        );
        // 内存包没有磁盘目录语义。
        assert_eq!(package.base_dir(), path::Path::new(""));

        // 与磁盘入口同构：同一份内容经 load() 组装后逐字段一致。
        let dir = std::env::temp_dir().join(format!("l2d-asset-mem-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["dummy.moc3", "tex_00.png", "physics3.json", "cdi3.json"] {
            std::fs::write(dir.join(name), resources[name].as_slice()).unwrap();
        }
        let model3_path = dir.join("mem.model3.json");
        std::fs::write(&model3_path, &model3).unwrap();
        let from_disk = ModelPackage::load(&model3_path).expect("磁盘入口");
        assert_eq!(from_disk.manifest(), package.manifest());
        assert_eq!(from_disk.moc3_bytes(), package.moc3_bytes());
        assert_eq!(from_disk.texture_bytes(), package.texture_bytes());
        assert_eq!(from_disk.physics_json(), package.physics_json());
        assert_eq!(from_disk.cdi3_json(), package.cdi3_json());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn from_memory_missing_resource_error_carries_name() {
        let (model3, mut resources) = mem_fixture(4);
        // physics 声明了但映射缺失 → MissingResource 且带资源名。
        assert!(resources.remove("physics3.json").is_some());

        match ModelPackage::from_memory_map(&model3, &resources) {
            Err(LoadError::MissingResource { name }) => {
                assert_eq!(name, "physics3.json");
            }
            other => panic!("期望 MissingResource，实际 {other:?}"),
        }

        // cdi 缺失同样严格报错。
        let (_, mut no_cdi) = mem_fixture(4);
        assert!(no_cdi.remove("cdi3.json").is_some());
        match ModelPackage::from_memory_map(&model3, &no_cdi) {
            Err(LoadError::MissingResource { name }) => assert_eq!(name, "cdi3.json"),
            other => panic!("期望 cdi MissingResource，实际 {other:?}"),
        }
    }

    #[test]
    fn from_memory_rejects_parent_escape_and_absolute_paths() {
        // `..` 上跳：即使映射里恰好有对应键也必须先在路径层被拒绝。
        let evil_model3 = br#"{
            "Version": 3,
            "FileReferences": {"Moc": "../evil.moc3", "Textures": []}
        }"#;
        let mut resources = BTreeMap::new();
        resources.insert("../evil.moc3".to_owned(), moc_header(4));
        match ModelPackage::from_memory_map(evil_model3, &resources) {
            Err(LoadError::InvalidResourcePath { name }) => assert_eq!(name, "../evil.moc3"),
            other => panic!("期望 InvalidResourcePath，实际 {other:?}"),
        }
        assert!(
            ModelPackage::from_memory_map(evil_model3, &resources)
                .unwrap_err()
                .to_string()
                .contains("../evil.moc3"),
            "错误信息必须携带资源名"
        );

        // 绝对路径同理。
        let abs_model3 = br#"{
            "Version": 3,
            "FileReferences": {"Moc": "/etc/passwd", "Textures": []}
        }"#;
        match ModelPackage::from_memory_map(abs_model3, &BTreeMap::new()) {
            Err(LoadError::InvalidResourcePath { name }) => assert_eq!(name, "/etc/passwd"),
            other => panic!("期望绝对路径被拒，实际 {other:?}"),
        }
    }

    /// 磁盘入口与内存入口共用规范化加固：`..` 引用在 fs 路径下同样被拒。
    #[test]
    fn disk_load_shares_path_hardening() {
        let dir = std::env::temp_dir().join(format!("l2d-asset-disk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let model3 = br#"{
            "Version": 3,
            "FileReferences": {"Moc": "../outside.moc3", "Textures": []}
        }"#;
        let model3_path = dir.join("escape.model3.json");
        std::fs::write(&model3_path, model3).unwrap();

        match ModelPackage::load(&model3_path) {
            Err(LoadError::InvalidResourcePath { name }) => {
                assert_eq!(name, "../outside.moc3")
            }
            other => panic!("期望路径加固生效，实际 {other:?}"),
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn normalize_resource_ref_rules() {
        // 恒等：常规引用不变。
        assert_eq!(normalize_resource_ref("dummy.moc3").unwrap(), "dummy.moc3");
        assert_eq!(
            normalize_resource_ref("bai.16384/texture_00_4096.png").unwrap(),
            "bai.16384/texture_00_4096.png"
        );
        // 折叠 `.` 与连续 `/`。
        assert_eq!(normalize_resource_ref("./a//b.png").unwrap(), "a/b.png");
        assert_eq!(normalize_resource_ref("./x/./y").unwrap(), "x/y");

        // 非法：空、绝对、反斜杠、`..`、纯 `.`。
        for bad in ["", "/abs/moc.bin", r"a\b.png", "../up.moc3", "a/../b", "."] {
            let err = normalize_resource_ref(bad).unwrap_err();
            let LoadError::InvalidResourcePath { name } = err else {
                panic!("{bad}: 期望 InvalidResourcePath");
            };
            assert_eq!(name, bad, "错误必须携带原始资源名");
        }
    }
}
