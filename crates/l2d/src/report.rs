//! 兼容报告（类型化诊断版）。
//!
//! 语义：给定"分析器"输入（[`crate::format::guess_format`] 的探测结果），
//! 输出一份可读的 [`CompatibilityReport`]，供上层决定能否走 v0 路径播放。
//!
//! 单一事实来源：**头版本只存在于 [`MocFormat::Moc3`] 变体中**。报告不再
//! 另存重复的 `version` 字段（旧实现里 `format` 与 `version` 可能各自漂移），
//! 读取方经 [`CompatibilityReport::moc_version`] 取版本，内部直接解构格式。
//!
//! 诊断全部**类型化**（[`CompatibilityIssue`]）：任何一条存在即否决 v0 播放
//! （[`CompatibilityReport::is_supported_v0`]），不再用裸字符串警告承载判定。
//! 扩展边界：报告与问题枚举均 `#[non_exhaustive]`，后续批次可直接追加变体。

use std::fmt;

use crate::format::{MocFormat, MocVersion};

/// 分析器输入：头探测结果。
///
/// `format` 为 `None` 表示上游未能给出任何判定（如未做探测）；
/// 已做过探测的调用方应直接传入 [`crate::format::guess_format`] 的结果。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Analyzer {
    pub format: Option<MocFormat>,
}

/// 类型化兼容问题：任何一条存在即否决 v0 播放。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompatibilityIssue {
    /// 无法识别模型格式（未发现合法 MOC3 头 / 头不完整 / 字节序标志非法）。
    UnknownFormat,
    /// MOC3 magic 合法但版本字节未被任何公开实现记录（携带原字节）。
    UnrecordedVersionByte(u8),
    /// 可识别的 moc3 版本不在 v0 锚定集合内（RFC D5：v0 只保证 Bai）。
    UnanchoredVersion(MocVersion),
    /// 统一加载路径交叉核对失败：头探测版本与运行时解析版本不一致，
    /// 或其中一侧无法映射到公开版本表（`None` 一侧即不可判定）。
    RuntimeVersionMismatch {
        /// 头探测得到的版本（`None` = 头不可判定）。
        header: Option<MocVersion>,
        /// 运行时解析得到的版本（`None` = 运行时不可映射/未解析出）。
        runtime: Option<MocVersion>,
    },
}

impl fmt::Display for CompatibilityIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFormat => {
                write!(f, "无法识别模型格式（未发现合法 MOC3 头），v0 无法播放")
            }
            Self::UnrecordedVersionByte(byte) => write!(
                f,
                "MOC3 magic 合法但版本字节 {byte} 未被公开实现记录，v0 不支持"
            ),
            Self::UnanchoredVersion(version) => write!(
                f,
                "识别为 moc3 v{version}，但 v0 仅锚定 Bai 实测档位 \
                 （moc3 v{}），未验证、不支持",
                MocVersion::BAI_V4_2
            ),
            Self::RuntimeVersionMismatch { header, runtime } => write!(
                f,
                "头探测版本（{}）与运行时解析版本（{}）不一致或不可判定",
                header.map_or("<无>".to_owned(), |v| v.to_string()),
                runtime.map_or("<无>".to_owned(), |v| v.to_string()),
            ),
        }
    }
}

/// 兼容报告：格式分类 + 类型化问题 + 说明。
///
/// 边界约定：
/// - `#[non_exhaustive]`：外部 crate 不可字面量构造/穷尽匹配，后续批次可安全加字段；
/// - 版本事实唯一来源是 [`Self::format`]（`Moc3(v)` 变体携带 v）；
/// - `issues` / `notes` 经访问器只读暴露；写入仅经
///   [`Self::push_issue`] / [`Self::push_note`]。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CompatibilityReport {
    /// 探测/分析得到的格式分类（`Moc3(v)` 时 v 即头版本）。
    pub format: MocFormat,
    issues: Vec<CompatibilityIssue>,
    notes: Vec<String>,
}

impl CompatibilityReport {
    /// 直接从分析器生成报告（格式缺失记为 Unknown 并给出对应问题；
    /// 可识别但不被 v0 锚定的档位同样生成问题，保证否决原因可见）。
    pub fn from_analyzer(analyzer: Analyzer) -> Self {
        let format = analyzer.format.unwrap_or(MocFormat::Unknown);
        let mut report = Self {
            format,
            issues: Vec::new(),
            notes: Vec::new(),
        };
        match format {
            MocFormat::Unknown => report.push_issue(CompatibilityIssue::UnknownFormat),
            MocFormat::Unsupported(byte) => {
                report.push_issue(CompatibilityIssue::UnrecordedVersionByte(byte));
            }
            MocFormat::Moc3(version) if !format.is_supported_v0() => {
                report.push_issue(CompatibilityIssue::UnanchoredVersion(version));
            }
            MocFormat::Moc3(_) => {}
        }
        report
    }

    /// 头版本便捷读取（单一事实来源：解构 [`MocFormat::Moc3`]）。
    pub fn moc_version(&self) -> Option<MocVersion> {
        match self.format {
            MocFormat::Moc3(version) => Some(version),
            MocFormat::Unsupported(_) | MocFormat::Unknown => None,
        }
    }

    /// 追加类型化问题（非空 issues 否决 v0 播放判定）。
    pub fn push_issue(&mut self, issue: CompatibilityIssue) {
        self.issues.push(issue);
    }

    /// 追加说明（非致命信息，供日志/UI 展示，不影响判定）。
    pub fn push_note(&mut self, note: String) {
        self.notes.push(note);
    }

    /// 只读问题列表。
    pub fn issues(&self) -> &[CompatibilityIssue] {
        &self.issues
    }

    /// 是否存在否决性兼容问题。
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
    }

    /// 只读说明列表。
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// v0 是否可播放：格式在 v0 支持集合内且无兼容问题。
    /// 注意：这是**格式层 + 交叉核对层**的判定，真实可播放性还要等渲染验证。
    pub fn is_supported_v0(&self) -> bool {
        self.format.is_supported_v0() && self.issues.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_format_issues_and_is_not_playable() {
        let report = CompatibilityReport::from_analyzer(Analyzer::default());
        assert_eq!(report.format, MocFormat::Unknown);
        assert_eq!(report.moc_version(), None);
        assert_eq!(report.issues(), &[CompatibilityIssue::UnknownFormat]);
        assert!(!report.is_supported_v0());
    }

    #[test]
    fn bai_profile_is_playable_v0_without_issues_and_version_is_single_sourced() {
        let report = CompatibilityReport::from_analyzer(Analyzer {
            format: Some(MocFormat::Moc3(MocVersion::V4_2_0)),
        });
        // 版本只从 format 解构而来，二者恒一致。
        assert_eq!(report.moc_version(), Some(MocVersion::V4_2_0));
        assert!(report.issues().is_empty());
        assert!(report.is_supported_v0());
    }

    #[test]
    fn recognized_but_unanchored_version_is_typed_issue() {
        // 旧版 Cubism 3.0 档位：可识别，但 v0 未验证。
        let report = CompatibilityReport::from_analyzer(Analyzer {
            format: Some(MocFormat::Moc3(MocVersion::V3_0_0)),
        });
        assert_eq!(
            report.issues(),
            &[CompatibilityIssue::UnanchoredVersion(MocVersion::V3_0_0)]
        );
        assert_eq!(report.moc_version(), Some(MocVersion::V3_0_0));
        assert!(!report.is_supported_v0());
    }

    #[test]
    fn unrecorded_version_byte_carries_raw_byte() {
        let report = CompatibilityReport::from_analyzer(Analyzer {
            format: Some(MocFormat::Unsupported(9)),
        });
        assert_eq!(
            report.issues()[0],
            CompatibilityIssue::UnrecordedVersionByte(9)
        );
        assert_eq!(
            report.issues()[0].to_string(),
            "MOC3 magic 合法但版本字节 9 未被公开实现记录，v0 不支持"
        );
        assert!(!report.is_supported_v0());
    }

    #[test]
    fn runtime_mismatch_issue_displays_both_sides() {
        let issue = CompatibilityIssue::RuntimeVersionMismatch {
            header: Some(MocVersion::V4_2_0),
            runtime: Some(MocVersion::V5_0_0),
        };
        assert!(issue.to_string().contains("4.2.0"));
        assert!(issue.to_string().contains("5.0.0"));

        let issue_unmappable = CompatibilityIssue::RuntimeVersionMismatch {
            header: Some(MocVersion::V4_2_0),
            runtime: None,
        };
        assert!(issue_unmappable.to_string().contains("<无>"));
    }

    #[test]
    fn pushed_issue_vetoes_support_but_notes_do_not() {
        let mut report = CompatibilityReport::from_analyzer(Analyzer {
            format: Some(MocFormat::Moc3(MocVersion::V4_2_0)),
        });
        assert!(report.is_supported_v0());
        report.push_issue(CompatibilityIssue::RuntimeVersionMismatch {
            header: Some(MocVersion::V4_2_0),
            runtime: None,
        });
        report.push_note("备注不影响判定。".to_owned());
        assert_eq!(report.notes().len(), 1);
        assert!(report.has_issues());
        assert!(!report.is_supported_v0());
    }
}
