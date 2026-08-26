//! `l2d` — Live2D 皮套格式、兼容报告与离屏渲染封装。
//!
//! - 格式/版本类型（MOC3 版本字节白名单，依据公开实现 + Bai 实测头，见 [`format`]）；
//! - 兼容报告（`format` → `report`，`#[non_exhaustive]` 预留 capability 扩展边界；
//!   头版本唯一事实来源是 [`format::MocFormat::Moc3`] 变体；诊断全部类型化，
//!   见 [`report::CompatibilityIssue`]）；
//! - 统一加载路径（[`model::LoadedModel`]）：包 → 运行时句柄原子解析 + 兼容报告 +
//!   「头探测 × 运行时版本」交叉核对（不一致即类型化否决）；
//! - 姿态分层栈（[`pose_stack`]）：base < idle < input < physics < final_override，
//!   纯逻辑、无 GPU 可测；
//! - 保守的格式探测（白名单匹配，永不 panic、不推断）。
//!
//! Runtime/render 底座（RFC D4 Bakeoff 结论：Ayagami，rev 固定 pin，
//! 决策记录见 `docs/verification/rust-bakeoff-decision.md` 与根 `SOURCES.md`）
//! 经以下自有封装暴露，**第三方类型不进入公开 API**：
//! - [`asset`]：model3 皮套包加载——磁盘入口 [`asset::ModelPackage::load`] 与
//!   内存入口 [`asset::ModelPackage::from_memory`]/[`asset::ModelPackage::from_memory_map`]
//!   共用同一「解析清单 → 校验引用 → 严格取 moc/纹理/physics/cdi」组装逻辑；
//!   相对路径统一规范化加固（拒绝绝对路径与 `..` 上跳，错误携带资源名）；
//! - [`model`]：moc3 运行时句柄（画布/版本/规模，自有类型）+ 绑定的 [`model::LoadedModel`]；
//! - [`renderer`]：headless 离屏渲染——共享 [`renderer::GpuContext`]（支持已有
//!   device/queue 接入）、目标无关的 [`renderer::ModelRendererCore`]（render_to_view）、
//!   便利层 [`renderer::OffscreenRenderer`]（固定 dt 更新 → RGBA/PNG 出帧）。
//!
//! 许可：**MIT OR Apache-2.0**（见 `LICENSE-MIT`、`LICENSE-APACHE`）。

pub mod asset;
pub mod format;
pub mod model;
pub mod pose_stack;
pub mod renderer;
pub mod report;

/// crate 版本（供兼容报告等消费）。
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 当前 crate 面向 v0 支持的格式集合。
///
/// RFC D5「v0 只保证 Bai」：v0 唯一锚定的是本仓库实测的 `bai.moc3`
/// （版本字节 4 → moc3 v4.2）。其余档位待真实 runtime 落地后逐档验证放行。
pub const V0_SUPPORTED_FORMATS: &[format::MocFormat] =
    &[format::MocFormat::Moc3(format::MocVersion::BAI_V4_2)];

#[cfg(test)]
mod tests {
    use super::format::{MOC3_HEADER_SIZE, MocFormat, MocVersion, guess_format};
    use super::report::{Analyzer, CompatibilityIssue, CompatibilityReport};

    /// 构造最小合法 64 字节 MOC3 头（小端）。
    fn header(version_byte: u8) -> [u8; MOC3_HEADER_SIZE] {
        let mut header = [0_u8; MOC3_HEADER_SIZE];
        header[..4].copy_from_slice(b"MOC3");
        header[4] = version_byte;
        header
    }

    /// v0 支持集合：Bai 实测档位（版本字节 4）可识别且可播放；常量表一致。
    #[test]
    fn v0_supports_bai_profile_only() {
        let bai = header(4);
        assert_eq!(guess_format(&bai), MocFormat::Moc3(MocVersion::V4_2_0));
        assert!(guess_format(&bai).is_supported_v0());
        assert_eq!(super::V0_SUPPORTED_FORMATS.len(), 1);
        assert!(super::V0_SUPPORTED_FORMATS[0].is_supported_v0());

        // 其他已知/未知档位一律不在 v0 集合内。
        assert!(!guess_format(&header(3)).is_supported_v0());
        assert!(!guess_format(&header(6)).is_supported_v0());
        assert!(!guess_format(&header(7)).is_supported_v0());
    }

    /// 端到端：Bai 头 → Analyzer → 报告可播放且版本单一事实来源；
    /// 垃圾输入 → Unknown 且不可播放。
    #[test]
    fn end_to_end_report_flow() {
        let supported = CompatibilityReport::from_analyzer(Analyzer {
            format: Some(guess_format(&header(4))),
        });
        assert_eq!(supported.format, MocFormat::Moc3(MocVersion::V4_2_0));
        assert_eq!(supported.moc_version(), Some(MocVersion::V4_2_0));
        assert!(supported.is_supported_v0());

        let unknown = CompatibilityReport::from_analyzer(Analyzer {
            format: Some(guess_format(b"NOT A MOC")),
        });
        assert_eq!(unknown.format, MocFormat::Unknown);
        assert_eq!(unknown.issues(), &[CompatibilityIssue::UnknownFormat]);
        assert!(!unknown.is_supported_v0());
    }
}
