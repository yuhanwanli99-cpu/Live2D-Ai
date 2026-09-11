//! 皮套格式类型与保守探测（第一批：仅类型与纯函数）。
//!
//! 探测依据（**公开实现 + 本仓库 Bai 实测头交叉验证**，非猜测）：
//!
//! - MOC3 头为固定 **64 字节**：`bytes[0..4] == b"MOC3"`，
//!   `bytes[4]` 为版本字节（u8），`bytes[5]` 为字节序标志（0=LE，1=BE），其余保留为零；
//!   参见纯 Rust runtime [Mocari](https://github.com/Eatgrapes/Mocari) `src/moc3/header.rs`
//!   与 [py-moc3](https://github.com/Ludentes/py-moc3) `Moc3Header`（两者一致，后者另含
//!   OpenL2D/moc3ingbird 同表结论）。
//! - 公开已知的版本字节：`1→3.0.0`、`2→3.3.0`、`3→4.0.0`、`4→4.2.0`（py-moc3/Mocari 一致）、
//!   `5→5.0.0`（py-moc3）、`6→5.3.0`（Mocari）。表外字节一律视为未记录版本。
//! - 本仓库 v0 唯一实测样本 `bai.moc3` 头 8 字节为 `4D 4F 43 33 04 00 00 00`
//!   （版本字节 4 → Cubism 4.2.0，小端），与上表吻合。
//!
//! 纪律：探测只做**白名单匹配**；任何不完整/字段非法的输入一律保守地判为
//! [`MocFormat::Unknown`]，绝不推断。

use std::fmt;

/// MOC3 magic（头前 4 字节）。
pub const MOC3_MAGIC: [u8; 4] = *b"MOC3";

/// MOC3 固定头长度（公开实现均按 64 字节解析；不足即视为无法保守确认）。
pub const MOC3_HEADER_SIZE: usize = 64;

/// MOC3 头内字节序标志（`bytes[5]`）：0 = little-endian。
const ENDIAN_LITTLE: u8 = 0;
/// MOC3 头内字节序标志（`bytes[5]`）：1 = big-endian。
const ENDIAN_BIG: u8 = 1;

/// 已被公开实现记录的 MOC3 版本（封闭集合，来自版本字节白名单）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MocVersion {
    /// 版本字节 1 — Cubism 3.0.0。
    V3_0_0,
    /// 版本字节 2 — Cubism 3.3.0。
    V3_3_0,
    /// 版本字节 3 — Cubism 4.0.0。
    V4_0_0,
    /// 版本字节 4 — Cubism 4.2.0（本仓库 Bai 实测档位）。
    V4_2_0,
    /// 版本字节 5 — Cubism 5.0.0（py-moc3 记录）。
    V5_0_0,
    /// 版本字节 6 — Cubism 5.3.0（Mocari 记录）。
    V5_3_0,
}

impl MocVersion {
    /// 从头内版本字节映射已知版本；表外字节返回 `None`（不猜）。
    pub const fn from_raw_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::V3_0_0),
            2 => Some(Self::V3_3_0),
            3 => Some(Self::V4_0_0),
            4 => Some(Self::V4_2_0),
            5 => Some(Self::V5_0_0),
            6 => Some(Self::V5_3_0),
            _ => None,
        }
    }

    /// 头内版本字节原值。
    pub const fn raw_byte(self) -> u8 {
        match self {
            Self::V3_0_0 => 1,
            Self::V3_3_0 => 2,
            Self::V4_0_0 => 3,
            Self::V4_2_0 => 4,
            Self::V5_0_0 => 5,
            Self::V5_3_0 => 6,
        }
    }

    /// Cubism 编辑器版本三元组 `(major, minor, patch)`。
    pub const fn as_tuple(self) -> (u32, u32, u32) {
        match self {
            Self::V3_0_0 => (3, 0, 0),
            Self::V3_3_0 => (3, 3, 0),
            Self::V4_0_0 => (4, 0, 0),
            Self::V4_2_0 => (4, 2, 0),
            Self::V5_0_0 => (5, 0, 0),
            Self::V5_3_0 => (5, 3, 0),
        }
    }

    /// 本仓库 v0 唯一锚定的档位：Bai 实测的 Cubism 4.2.0（RFC D5「v0 只保证 Bai」）。
    pub const BAI_V4_2: Self = Self::V4_2_0;
}

impl fmt::Display for MocVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (major, minor, patch) = self.as_tuple();
        write!(f, "{major}.{minor}.{patch}")
    }
}

/// 探测结果格式分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MocFormat {
    /// 可识别的 MOC3：magic 合法、版本字节命中公开白名单、字节序标志合法。
    Moc3(MocVersion),
    /// 有合法 magic 但版本字节未被任何公开实现记录（未来版本或损坏文件）——不支持。
    Unsupported(u8),
    /// 无法保守识别：缺 magic / 头不完整 / 字节序标志非法。调用方应按未知处理。
    Unknown,
}

impl MocFormat {
    /// 该格式是否在 v0 支持集合内。
    ///
    /// v0 仅锚定 Bai 实测档位（moc3 v4.2，RFC D5）；其余版本等真实 runtime
    /// 解析落地后再逐档验证放行，第一批绝不预先承诺。
    pub fn is_supported_v0(self) -> bool {
        matches!(self, Self::Moc3(v) if v == MocVersion::BAI_V4_2)
    }
}

/// 保守的 MOC3 格式探测（纯函数，永不 panic，不读文件）。
///
/// 规则（全部为白名单校验，无推断）：
/// 1. 输入不足 [`MOC3_HEADER_SIZE`] 字节或前 4 字节非 `MOC3` → [`MocFormat::Unknown`]；
/// 2. `bytes[5]` 字节序标志非 0/1 → [`MocFormat::Unknown`]（头非法，参照 Mocari 拒绝语义）；
/// 3. `bytes[4]` 命中 [`MocVersion::from_raw_byte`] 白名单 → [`MocFormat::Moc3`]；
/// 4. 否则 → [`MocFormat::Unsupported`]（携带版本字节原值，供报告与日志）。
pub fn guess_format(header: &[u8]) -> MocFormat {
    if header.len() < MOC3_HEADER_SIZE || header[..4] != MOC3_MAGIC {
        return MocFormat::Unknown;
    }
    if !matches!(header[5], ENDIAN_LITTLE | ENDIAN_BIG) {
        return MocFormat::Unknown;
    }
    match MocVersion::from_raw_byte(header[4]) {
        Some(version) => MocFormat::Moc3(version),
        None => MocFormat::Unsupported(header[4]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个最小合法 MOC3 头（64 字节，版本字节 `version_byte`，字节序标志 `endian_byte`）。
    fn header_with(version_byte: u8, endian_byte: u8) -> [u8; MOC3_HEADER_SIZE] {
        let mut header = [0_u8; MOC3_HEADER_SIZE];
        header[..4].copy_from_slice(&MOC3_MAGIC);
        header[4] = version_byte;
        header[5] = endian_byte;
        header
    }

    /// 本仓库 `bai/runtime/bai.moc3` 的真实头（只读 hexdump 取证：
    /// `4d4f 4333 0400 ...` 全零至 64 字节）。
    const BAI_HEADER: [u8; MOC3_HEADER_SIZE] = {
        let mut header = [0_u8; MOC3_HEADER_SIZE];
        header[0] = b'M';
        header[1] = b'O';
        header[2] = b'C';
        header[3] = b'3';
        header[4] = 4;
        header
    };

    #[test]
    fn bai_actual_header_is_recognized_v4_2() {
        assert_eq!(
            guess_format(&BAI_HEADER),
            MocFormat::Moc3(MocVersion::V4_2_0)
        );
        // v0 只保证 Bai → Bai 档位必须被判为支持。
        assert!(guess_format(&BAI_HEADER).is_supported_v0());
    }

    #[test]
    fn known_version_table_matches_public_sources() {
        for (byte, expected) in [
            (1_u8, MocVersion::V3_0_0),
            (2, MocVersion::V3_3_0),
            (3, MocVersion::V4_0_0),
            (4, MocVersion::V4_2_0),
            (5, MocVersion::V5_0_0),
            (6, MocVersion::V5_3_0),
        ] {
            assert_eq!(MocVersion::from_raw_byte(byte), Some(expected));
            assert_eq!(expected.raw_byte(), byte);
        }
        // 表外字节不猜。
        assert_eq!(MocVersion::from_raw_byte(0), None);
        assert_eq!(MocVersion::from_raw_byte(7), None);
        assert_eq!(MocVersion::from_raw_byte(255), None);
    }

    #[test]
    fn unrecognized_version_is_unsupported_not_unknown() {
        let header = header_with(7, ENDIAN_LITTLE);
        assert_eq!(guess_format(&header), MocFormat::Unsupported(7));
        assert!(!MocFormat::Unsupported(7).is_supported_v0());
    }

    #[test]
    fn truncated_or_garbage_input_is_unknown() {
        // 有 magic 但头不完整：不能保守确认。
        assert_eq!(guess_format(b"MOC3\x04\x00"), MocFormat::Unknown);
        assert_eq!(guess_format(&BAI_HEADER[..63]), MocFormat::Unknown);
        // 无 magic / 空 / 全零垃圾输入。
        assert_eq!(guess_format(b""), MocFormat::Unknown);
        assert_eq!(
            guess_format(b"MOC3-xyz-not-even-close-payload-padding!!"),
            MocFormat::Unknown
        );
        assert_eq!(guess_format(&[0_u8; MOC3_HEADER_SIZE]), MocFormat::Unknown);
    }

    #[test]
    fn illegal_endian_flag_is_unknown() {
        let header = header_with(4, 2);
        assert_eq!(guess_format(&header), MocFormat::Unknown);
    }

    #[test]
    fn other_known_versions_are_not_v0_guaranteed() {
        for version in [
            MocVersion::V3_0_0,
            MocVersion::V3_3_0,
            MocVersion::V4_0_0,
            MocVersion::V5_0_0,
            MocVersion::V5_3_0,
        ] {
            let header = header_with(version.raw_byte(), ENDIAN_LITTLE);
            assert_eq!(guess_format(&header), MocFormat::Moc3(version));
            assert!(
                !guess_format(&header).is_supported_v0(),
                "{version} 不在 v0 锚定集合"
            );
        }
    }

    #[test]
    fn display_renders_cubism_triple() {
        assert_eq!(MocVersion::V4_2_0.to_string(), "4.2.0");
        assert_eq!(MocVersion::V3_0_0.as_tuple(), (3, 0, 0));
    }
}
