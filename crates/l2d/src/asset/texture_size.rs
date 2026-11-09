//! 贴图尺寸探测（0.5.0 rc.6 / task-14）：只读 PNG 头，不解码像素。
//!
//! # 为什么需要
//!
//! 渲染档位（[`crate::renderer::RenderTier`]）在 native headless 路径里直接决定
//! `max_texture_dimension_2d`。真实模型（大肥鱼皮肤）的 texture_00 是
//! **4096×8192**，默认 4096 档会以 `TextureTooLarge` 装载失败。自动选档只需要
//! 「贴图最长边」，不需要解码整张图——PNG 的 IHDR 前 24 字节就够（44 MB 的
//! 贴图集全解码代价明显不值得在装载前重复一次）。
//!
//! # 规则
//!
//! - 只认 PNG 签名 + 第一块必须是 `IHDR`；宽高从 IHDR 读大端 u32；
//! - 其它格式 / 截断 / 尺寸为 0 → `None`（调用方按「尺寸未知」处理，不猜）；
//! - 纯函数、无 panic，可在原生 `cargo test` 覆盖。

/// PNG 签名（8 字节）。
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// 读 PNG 宽高（`(width, height)`）；不是合法 PNG 头 → `None`。
pub fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || bytes.get(..8)? != PNG_SIGNATURE {
        return None;
    }
    // 8..12 = 第一块长度（IHDR 固定 13），12..16 = 块类型。
    if bytes.get(12..16)? != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes.get(16..20)?.try_into().ok()?);
    let height = u32::from_be_bytes(bytes.get(20..24)?.try_into().ok()?);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

/// 读 PNG 的**最长边**；不是合法 PNG 头 → `None`。
pub fn png_max_edge(bytes: &[u8]) -> Option<u32> {
    png_dimensions(bytes).map(|(width, height)| width.max(height))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 合成最小 PNG 头（签名 + IHDR 长度/类型 + 宽高）。
    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PNG_SIGNATURE);
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn reads_png_width_height_and_max_edge() {
        assert_eq!(png_dimensions(&png_header(4096, 8192)), Some((4096, 8192)));
        assert_eq!(png_max_edge(&png_header(4096, 8192)), Some(8192));
        assert_eq!(png_max_edge(&png_header(4096, 4096)), Some(4096));
        assert_eq!(png_max_edge(&png_header(1, 16384)), Some(16384));
    }

    #[test]
    fn rejects_non_png_truncated_or_zero_sized_headers() {
        assert_eq!(png_max_edge(b""), None);
        assert_eq!(png_max_edge(b"not a png at all, definitely"), None);
        assert_eq!(png_max_edge(&png_header(0, 4096)), None, "0 宽不合法");
        assert_eq!(png_max_edge(&png_header(4096, 0)), None, "0 高不合法");
        // 头截断（不足 24 字节）。
        let mut short = png_header(4096, 4096);
        short.truncate(20);
        assert_eq!(png_max_edge(&short), None);
        // 第一块不是 IHDR（例如被改写成 IDAT）。
        let mut wrong = png_header(4096, 4096);
        wrong[12..16].copy_from_slice(b"IDAT");
        assert_eq!(png_max_edge(&wrong), None);
    }

    #[test]
    fn reads_the_real_bai_texture() {
        // bai 白模随仓库分发：用真实 PNG 验证头解析（4096×4096）。
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("assets/models/bai/runtime/bai.16384/texture_00_4096.png");
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("读不到 bai 贴图 {}: {e}", path.display()));
        assert_eq!(png_dimensions(&bytes), Some((4096, 4096)));
        assert_eq!(png_max_edge(&bytes), Some(4096));
    }
}
