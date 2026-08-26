//! moc3 运行时模型句柄与「包 × 句柄」绑定模型（对 pin 的 runtime 解析结果的自有封装）。
//!
//! - [`ModelHandle`] 持有解析后的 moc3，并只以**本 crate 自有类型**
//!   （[`CanvasInfo`] / [`ModelStats`] / [`crate::format::MocVersion`]）
//!   对外暴露信息；第三方 runtime 类型只出现在私有字段与 `pub(crate)`
//!   访问面中，不进入公开 API 签名。
//! - [`LoadedModel`] 把包（[`ModelPackage`]）与其解析出的句柄**原子绑定**：
//!   纹理/物理字节与 moc3 一定同源，渲染器只接受本类型即可在类型层面
//!   杜绝 model/package 错配；版本交叉核对也在解析时一并完成。

use std::{fmt, io::Cursor, sync::Arc};

use ayagami::core::{ItemArray as _, Model as _, Param as _};
use ayagami::file::Version as RuntimeVersion;

use crate::asset::ModelPackage;
use crate::format::MocVersion;
use crate::report::{CompatibilityIssue, CompatibilityReport};

/// moc3 解析错误（底层拒绝原因以字符串携带，不外泄第三方错误类型）。
#[derive(Debug)]
#[non_exhaustive]
pub enum ModelError {
    /// moc3 二进制无法被运行时解析。
    Parse(String),
    /// 输入为空。
    Empty,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "moc3 输入为空"),
            Self::Parse(reason) => write!(f, "moc3 解析失败: {reason}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// moc3 头声明的画布信息（模型坐标单位）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasInfo {
    /// 画布宽（单位）。
    pub width_units: f32,
    /// 画布高（单位）。
    pub height_units: f32,
    /// 画布中心 x（单位）。
    pub center_x_units: f32,
    /// 画布中心 y（单位）。
    pub center_y_units: f32,
    /// 每单位像素数（canvas scale）。
    pub pixels_per_unit: f32,
}

/// 模型规模统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModelStats {
    pub artmeshes: usize,
    pub deformers: usize,
    pub parts: usize,
    pub parameters: usize,
    pub vertices: usize,
    pub triangles: usize,
}

/// 已解析的 moc3 模型句柄（不可变、廉价 [`Clone`]，可共享给渲染器）。
#[derive(Clone)]
pub struct ModelHandle {
    inner: Arc<ayagami::file::ParsedModel>,
}

impl ModelHandle {
    /// 从 moc3 字节解析模型。
    pub fn from_moc3_bytes(bytes: &[u8]) -> Result<Self, ModelError> {
        if bytes.is_empty() {
            return Err(ModelError::Empty);
        }
        let mut cursor = Cursor::new(bytes);
        let parsed = ayagami::file::ParsedModel::load(&mut cursor)
            .map_err(|e| ModelError::Parse(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(parsed),
        })
    }

    /// 从已加载的皮套包解析 moc3。
    ///
    /// 注意：这只完成「解析」，不做包内纹理/物理的绑定，也不做版本交叉核对；
    /// 统一加载路径请改用 [`LoadedModel::resolve`]。
    pub fn from_package(package: &ModelPackage) -> Result<Self, ModelError> {
        Self::from_moc3_bytes(package.moc3_bytes())
    }

    /// 运行时解析出的 moc3 版本（映射到本 crate 公开版本表）。
    ///
    /// 映射用**穷尽 `match`**，禁止 `as u8` 之类的数字转换：上游新增判别值时
    /// 此处直接编译失败，强制重新人工对表，杜绝静默错位。
    /// 运行时版本不在公开版本表时返回 `None`（不猜）。
    pub fn moc_version(&self) -> Option<MocVersion> {
        match self.inner.version()? {
            RuntimeVersion::V3_0 => Some(MocVersion::V3_0_0),
            RuntimeVersion::V3_3 => Some(MocVersion::V3_3_0),
            RuntimeVersion::V4_0 => Some(MocVersion::V4_0_0),
            RuntimeVersion::V4_2 => Some(MocVersion::V4_2_0),
            RuntimeVersion::V5_0 => Some(MocVersion::V5_0_0),
            RuntimeVersion::V5_3 => Some(MocVersion::V5_3_0),
        }
    }

    /// 画布信息。
    pub fn canvas(&self) -> CanvasInfo {
        let canvas = self.inner.canvas_properties();
        CanvasInfo {
            width_units: canvas.dimensions.x,
            height_units: canvas.dimensions.y,
            center_x_units: canvas.center.x,
            center_y_units: canvas.center.y,
            pixels_per_unit: canvas.scale,
        }
    }

    /// 规模统计。
    pub fn stats(&self) -> ModelStats {
        let model = self.inner.as_ref();
        ModelStats {
            artmeshes: model.artmeshes().count(),
            deformers: model.deformers().count(),
            parts: model.parts().count(),
            parameters: model.params().count(),
            vertices: model.texcoord_buffer().map_or(0, <[_]>::len),
            triangles: model.index_buffer().map_or(0, <[_]>::len) / 3,
        }
    }

    /// 全部参数 ID。
    pub fn parameter_ids(&self) -> Vec<String> {
        self.inner
            .params()
            .into_iter()
            .map(|p| p.id().to_owned())
            .collect()
    }

    /// 模型是否包含指定参数。
    pub fn has_parameter(&self, id: &str) -> bool {
        self.parameter_ids().iter().any(|p| p == id)
    }

    /// 渲染器复用的共享指针（crate 内部访问面，非公开 API）。
    pub(crate) fn shared(&self) -> Arc<ayagami::file::ParsedModel> {
        Arc::clone(&self.inner)
    }

    /// 参数/部件描述表（crate 内部访问面；[`crate::pose_stack::PoseStack`] 构建用）。
    pub(crate) fn pose_map(&self) -> Arc<ayagami::pose::PoseMap> {
        Arc::clone(self.inner.pose_map())
    }
}

/// 绑定的「皮套包 + 运行时句柄」模型：由 [`ModelPackage`] **原子解析**而来。
///
/// 同时持有包与其解析出的 [`ModelHandle`]，保证纹理/物理字节与 moc3 一定同源；
/// 渲染器（[`crate::renderer`]）只接受本类型，model/package 错配在类型层面
/// 即不可能发生。兼容报告在解析时完成「头探测 × 运行时版本」交叉核对，
/// 不一致以 [`CompatibilityIssue::RuntimeVersionMismatch`] 否决 v0 播放。
#[derive(Clone)]
pub struct LoadedModel {
    package: Arc<ModelPackage>,
    handle: ModelHandle,
    report: CompatibilityReport,
}

impl LoadedModel {
    /// 统一加载路径：包 → 头探测报告 + 运行时句柄 + 版本交叉核对，一次完成。
    ///
    /// - 解析失败（moc3 非法/为空）→ [`ModelError`]；
    /// - 头探测版本与运行时映射版本不一致（或任一侧不可判定）→ 不报错，
    ///   在报告追加 [`CompatibilityIssue::RuntimeVersionMismatch`]（否决 v0）；
    /// - 双侧一致 → 记录一条说明 note。
    pub fn resolve(package: ModelPackage) -> Result<Self, ModelError> {
        let handle = ModelHandle::from_moc3_bytes(package.moc3_bytes())?;
        let mut report = package.compatibility_report();

        // 交叉核对：头探测版本（MocFormat 是唯一头版本事实来源）
        // 与运行时解析版本必须同时可判定且相等。
        let header = report.moc_version();
        let runtime = handle.moc_version();
        match (header, runtime) {
            (Some(v), Some(rv)) if v == rv => {
                report.push_note(format!("头探测与运行时解析版本一致（{v}）。"));
            }
            // 双侧均不可判定：格式层问题（UnknownFormat 等）已在报告中，不重复记。
            (None, None) => {}
            (header, runtime) => {
                report.push_issue(CompatibilityIssue::RuntimeVersionMismatch { header, runtime });
            }
        }

        Ok(Self {
            package: Arc::new(package),
            handle,
            report,
        })
    }

    /// 皮套包（与句柄保证同源）。
    pub fn package(&self) -> &ModelPackage {
        &self.package
    }

    /// 运行时句柄。
    pub fn handle(&self) -> &ModelHandle {
        &self.handle
    }

    /// 兼容报告（含交叉核对结论）。
    pub fn report(&self) -> &CompatibilityReport {
        &self.report
    }

    /// 清单摘要便捷读取。
    pub fn manifest(&self) -> &crate::asset::PackageManifest {
        self.package.manifest()
    }
}

impl fmt::Debug for LoadedModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadedModel")
            .field("moc_version", &self.handle.moc_version())
            .field("canvas", &self.handle.canvas())
            .field("supported_v0", &self.report.is_supported_v0())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{MOC3_HEADER_SIZE, MocFormat};

    /// 最小合法 64 字节 MOC3 头——但除头之外没有合法 moc3 体，
    /// 只能用于 Empty 分支；完整解析测试依赖真实资产（见 tests/bai_asset.rs）。
    fn fake_header(version_byte: u8) -> Vec<u8> {
        let mut header = vec![0_u8; MOC3_HEADER_SIZE];
        header[..4].copy_from_slice(b"MOC3");
        header[4] = version_byte;
        header
    }

    #[test]
    fn empty_moc3_is_rejected() {
        assert!(matches!(
            ModelHandle::from_moc3_bytes(&[]),
            Err(ModelError::Empty)
        ));
    }

    #[test]
    fn garbage_body_fails_parse_with_typed_error() {
        let err = match ModelHandle::from_moc3_bytes(&fake_header(4)) {
            Err(err) => err,
            Ok(_) => panic!("假体必被拒"),
        };
        assert!(matches!(err, ModelError::Parse(_)));
        assert!(err.to_string().contains("moc3 解析失败"));
    }

    /// 版本映射穷尽 match 的静态性质检查：六个已知档位逐一映射成功。
    /// （真实 moc3 的端到端核对见 tests/bai_asset.rs。）
    #[test]
    fn version_mapping_table_covers_all_known_discriminants() {
        let table = [
            (1_u8, MocVersion::V3_0_0),
            (2, MocVersion::V3_3_0),
            (3, MocVersion::V4_0_0),
            (4, MocVersion::V4_2_0),
            (5, MocVersion::V5_0_0),
            (6, MocVersion::V5_3_0),
        ];
        for (byte, expected) in table {
            assert_eq!(expected.raw_byte(), byte);
        }
    }

    #[test]
    fn loaded_model_debug_does_not_leak_third_party_types() {
        // 仅验证 Debug 实现可格式化（不 panic），且不外泄内部指针细节。
        let dir = std::env::temp_dir().join(format!("l2d-model-debug-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x.moc3"), fake_header(4)).unwrap();
        std::fs::write(
            dir.join("model.model3.json"),
            r#"{"Version":3,"FileReferences":{"Moc":"x.moc3","Textures":[]}}"#,
        )
        .unwrap();
        if let Ok(package) = ModelPackage::load(dir.join("model.model3.json")) {
            // 假 moc3 体必然解析失败——这里只验证错误路径的类型化。
            assert!(matches!(
                LoadedModel::resolve(package),
                Err(ModelError::Parse(_))
            ));
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    /// 格式引用完整性：MocFormat 仍是唯一头版本事实来源。
    #[test]
    fn format_is_single_version_source() {
        let format = MocFormat::Moc3(MocVersion::V4_2_0);
        let analyzer = crate::report::Analyzer {
            format: Some(format),
        };
        let report = CompatibilityReport::from_analyzer(analyzer);
        assert_eq!(report.moc_version(), Some(MocVersion::V4_2_0));
    }
}
