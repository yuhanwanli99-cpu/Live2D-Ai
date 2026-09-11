//! `ModelPackage::compatibility_report` 的内部构造。
//!
//! 仅 `mod.rs` 内部可见。逻辑从 `ModelPackage::compatibility_report` 提取，
//! 以便 mod.rs 自身保持"清单/IO 路径"焦点。

use crate::format::{MOC3_HEADER_SIZE, guess_format};
use crate::report::{Analyzer, CompatibilityReport};

use super::PackageManifest;

/// 生成 `ModelPackage` 的兼容报告（moc3 头探测 + 清单/构图框注解）。
///
/// v0 判定语义与 crate 既有骨架一致：仅 Bai 实测档位（raw 字节 4 → Cubism 4.2）
/// 无问题可通过；其余档位生成类型化问题并由
/// [`CompatibilityReport::is_supported_v0`] 否决。注意这是**格式层**报告；
/// 统一加载路径的「头探测 × 运行时版本」交叉核对在
/// [`crate::model::LoadedModel::resolve`] 中完成。
pub(super) fn build_compatibility_report(
    manifest: &PackageManifest,
    moc3_bytes: &[u8],
) -> CompatibilityReport {
    let header_len = moc3_bytes.len().min(MOC3_HEADER_SIZE);
    // guess_format 对短输入保守返回 Unknown，无需前置判断。
    let mut report = CompatibilityReport::from_analyzer(Analyzer {
        format: Some(guess_format(moc3_bytes[..header_len].as_ref())),
    });
    report.push_note(format!(
        "model3 v{}：moc3=`{}`，纹理 {} 张{}{}",
        manifest.version,
        manifest.moc3_file,
        manifest.texture_files.len(),
        manifest
            .physics_file
            .as_ref()
            .map(|p| format!("，physics=`{p}`"))
            .unwrap_or_default(),
        manifest
            .display_info_file
            .as_ref()
            .map(|p| format!("，cdi3=`{p}`"))
            .unwrap_or_default(),
    ));
    report.push_note(format!(
        "layout: center=({},{}) size={}x{}",
        manifest.layout.center_x,
        manifest.layout.center_y,
        manifest.layout.width,
        manifest.layout.height,
    ));
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asset::ModelPackage;
    use crate::format::MocFormat;
    use crate::format::MocVersion;
    use crate::report::CompatibilityIssue;

    /// 最小合法 64 字节 MOC3 头（版本字节 `version_byte`）。
    fn moc_header(version_byte: u8) -> Vec<u8> {
        let mut header = vec![0_u8; MOC3_HEADER_SIZE];
        header[..4].copy_from_slice(b"MOC3");
        header[4] = version_byte;
        header
    }

    /// 在临时目录组装一个最小包（假 moc3 头 + 假纹理字节），返回路径。
    fn write_fixture(dir: &std::path::Path, version_byte: u8) -> std::path::PathBuf {
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
    fn compatibility_report_anchors_bai_raw_v4() {
        let dir =
            std::env::temp_dir().join(format!("l2d-asset-test-{}-{}", std::process::id(), line!()));
        let model3_path = write_fixture(&dir, 4);

        let report = ModelPackage::load(&model3_path)
            .unwrap()
            .compatibility_report();
        assert_eq!(report.format, MocFormat::Moc3(MocVersion::V4_2_0));
        assert_eq!(report.moc_version(), Some(MocVersion::V4_2_0));
        assert!(!report.has_issues());
        assert!(report.is_supported_v0(), "raw 字节 4 即 v0 锚定档位");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn compatibility_report_rejects_unanchored_versions() {
        let dir =
            std::env::temp_dir().join(format!("l2d-asset-test-{}-{}", std::process::id(), line!()));
        // raw 字节 6（Cubism 5.3）：可识别但非 v0 锚定档位。
        let model3_path = write_fixture(&dir, 6);

        let report = ModelPackage::load(&model3_path)
            .unwrap()
            .compatibility_report();
        assert_eq!(report.format, MocFormat::Moc3(MocVersion::V5_3_0));
        assert_eq!(
            report.issues(),
            &[CompatibilityIssue::UnanchoredVersion(MocVersion::V5_3_0)]
        );
        assert!(!report.is_supported_v0());

        std::fs::remove_dir_all(&dir).ok();
    }
}
