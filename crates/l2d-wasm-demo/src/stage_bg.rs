//! 舞台背景的**纯逻辑**：dataURL → 图片字节、cover 的 UV 变换、纯色底取值。
//!
//! 刻意**不**放在 wasm-only 的 `web::surface` 里——本仓库已经因为
//! 「换算藏在 `#[cfg(target_arch = "wasm32")]` 后面、原生 `cargo test` 编译不到」
//! 栽过一次（`mouth.rs` 的口型映射）——同一个坑不再踩第三次。这里的三个函数
//! 都能在原生 target 上单测：base64 解码、cover 比例、纯色底回退。
//!
//! # 为什么舞台背景改成画进 framebuffer（2026-09-14 rc.5 定）
//!
//! Web 端 WebGPU 的 canvas 只能**不透明**合成（wgpu 29 的 webgpu 后端
//! `get_capabilities` 只报 `CompositeAlphaMode::Opaque`，`wgpu-core` 又拒绝
//! caps 之外的 alpha_mode）。于是「canvas 用 CSS 背景、模型画在透明像素上」这条
//! 路**结构性地不可达**：透明 clear 的像素会被合成为不透明黑，盖住 CSS 背景。
//! 所以舞台底色与背景图都必须由渲染面自己填进 framebuffer。

/// 无 `stageColor` 时的暗色兜底（与历史 CSS 回退值一致）。
pub const DEFAULT_STAGE_DARK: &str = "#101418";
/// 无 `stageColor` 时的亮色兜底。
pub const DEFAULT_STAGE_LIGHT: &str = "#e8ecf1";

/// 舞台纯色底的 RGBA（0..1，a 恒 1）。
///
/// `stage_color` 是父页 `sync.stageColor` 的原样字符串（通常已是 `#rrggbb`）；
/// 非法 / 缺失时回落到 `dark` 对应的历史默认色——**绝不**让舞台变成未定义颜色。
pub fn clear_color(stage_color: Option<&str>, dark: bool) -> [f64; 4] {
    let fallback = if dark {
        DEFAULT_STAGE_DARK
    } else {
        DEFAULT_STAGE_LIGHT
    };
    let hex = stage_color
        .map(str::trim)
        .filter(|s| parse_hex(s).is_some())
        .unwrap_or(fallback);
    parse_hex(hex).unwrap_or([0.0, 0.0, 0.0, 1.0])
}

/// 解析 `#rgb` / `#rrggbb` 为 RGBA（0..1）。非法返回 `None`。
pub fn parse_hex(raw: &str) -> Option<[f64; 4]> {
    let hex = raw.trim().strip_prefix('#')?;
    let (r, g, b) = match hex.len() {
        3 => {
            let mut it = hex.chars().map(|c| c.to_digit(16).map(|v| (v * 17) as u8));
            (it.next()??, it.next()??, it.next()??)
        }
        6 => {
            let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
            (byte(0)?, byte(2)?, byte(4)?)
        }
        _ => return None,
    };
    Some([
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
        1.0,
    ])
}

/// `background-size: cover` 的 UV 变换：返回 `[scale_x, scale_y, offset_x, offset_y]`。
///
/// 采样时 `uv = quad_uv * scale + offset`，于是图**铺满且保比**、多出的边被裁掉
/// （而不是拉伸）。任一边长非正时退化为恒等变换（不裁不缩）。
pub fn cover_uv(image_w: f32, image_h: f32, view_w: f32, view_h: f32) -> [f32; 4] {
    if !(image_w > 0.0 && image_h > 0.0 && view_w > 0.0 && view_h > 0.0) {
        return [1.0, 1.0, 0.0, 0.0];
    }
    let image_aspect = image_w / image_h;
    let view_aspect = view_w / view_h;
    if image_aspect > view_aspect {
        // 图比视口更宽 → 高度铺满，左右各裁一半。
        let sx = view_aspect / image_aspect;
        [sx, 1.0, (1.0 - sx) * 0.5, 0.0]
    } else {
        // 图比视口更高（或等比）→ 宽度铺满，上下各裁一半。
        let sy = image_aspect / view_aspect;
        [1.0, sy, 0.0, (1.0 - sy) * 0.5]
    }
}

/// 背景纹理的长边上限（物理像素）。
///
/// 用户随手选的照片常见 4000×3000——全尺寸解码是 ~48 MB RGBA + 同量级 GPU
/// 纹理，浏览器标签有被撑爆的风险。2048 对「舞台背景」足够（画布本身按
/// dpr≤1.5 钳制），且显著缩短解码与上传时间。
pub const MAX_BACKGROUND_SIDE: u32 = 2048;

/// 等比缩放到最长边 `max_side` 以内；已在范围内（或输入非法）原样返回。
pub fn fit_within(width: u32, height: u32, max_side: u32) -> (u32, u32) {
    if width == 0 || height == 0 || max_side == 0 {
        return (width.max(1), height.max(1));
    }
    let longest = width.max(height);
    if longest <= max_side {
        return (width, height);
    }
    let k = f64::from(max_side) / f64::from(longest);
    (
        ((f64::from(width) * k).round() as u32).max(1),
        ((f64::from(height) * k).round() as u32).max(1),
    )
}

/// 从 `data:image/...;base64,....` 取出**解码后的图片字节**。
///
/// 不是 dataURL / 没有 base64 段 / 载荷非法一律 `None`（调用方退回纯色底），
/// **不抛**——一份被外部塞坏的值不该让舞台渲染不出来。
pub fn base64_decode_data_url(data_url: &str) -> Option<Vec<u8>> {
    let comma = data_url.find(',')?;
    let meta = &data_url[..comma];
    if !meta.contains("base64") {
        return None;
    }
    base64_decode(&data_url[comma + 1..])
}

/// 标准 base64 解码（忽略空白；容忍丢失的 `=` 填充）。
pub fn base64_decode(input: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some(u32::from(b - b'A')),
            b'a'..=b'z' => Some(u32::from(b - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(b - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &b in input.as_bytes() {
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'=' {
            break;
        }
        let v = val(b)?;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 纯色底 ─────────────────────────────────────────────────────────

    #[test]
    fn clear_color_parses_hex_and_normalizes_to_unit_range() {
        assert_eq!(clear_color(Some("#000000"), true), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(clear_color(Some("#ffffff"), true), [1.0, 1.0, 1.0, 1.0]);
        // #061223 → r=0x06 g=0x12 b=0x23。
        let blue = clear_color(Some("#061223"), true);
        assert!((blue[0] - 0x06 as f64 / 255.0).abs() < 1e-9, "{blue:?}");
        assert!((blue[1] - 0x12 as f64 / 255.0).abs() < 1e-9, "{blue:?}");
        assert!((blue[2] - 0x23 as f64 / 255.0).abs() < 1e-9, "{blue:?}");
        assert_eq!(blue[3], 1.0);
        // 三位简写按 CSS 语义展开（f → ff）。
        assert_eq!(clear_color(Some("#fff"), true), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn clear_color_falls_back_by_dark_flag_not_to_undefined() {
        // 白主题的兜底必须是亮的——否则「白主题舞台为白」这条验收根本立不住。
        assert_eq!(
            clear_color(None, false),
            parse_hex(DEFAULT_STAGE_LIGHT).unwrap()
        );
        assert_eq!(
            clear_color(None, true),
            parse_hex(DEFAULT_STAGE_DARK).unwrap()
        );
        for bad in [
            "",
            "#",
            "#12",
            "#12345",
            "red",
            "url(x)",
            "expression(alert(1))",
        ] {
            // 任何非法值都回落兜底，绝不进 GPU。
            assert!(parse_hex(bad).is_none(), "{bad:?}");
            assert_eq!(
                clear_color(Some(bad), true),
                parse_hex(DEFAULT_STAGE_DARK).unwrap()
            );
        }
    }

    // ── cover 变换 ─────────────────────────────────────────────────────

    #[test]
    fn cover_uv_crops_the_longer_axis_and_keeps_the_other_full() {
        // 图更宽（2:1）放进 1:1 视口 → 横向各裁 1/4，纵向铺满。
        let uv = cover_uv(200.0, 100.0, 100.0, 100.0);
        assert!((uv[0] - 0.5).abs() < 1e-6, "{uv:?}");
        assert_eq!(uv[1], 1.0);
        assert!((uv[2] - 0.25).abs() < 1e-6, "{uv:?}");
        assert_eq!(uv[3], 0.0);

        // 图更高（1:2）放进 1:1 视口 → 纵向各裁 1/4，横向铺满。
        let uv = cover_uv(100.0, 200.0, 100.0, 100.0);
        assert_eq!(uv[0], 1.0);
        assert!((uv[1] - 0.5).abs() < 1e-6, "{uv:?}");
        assert_eq!(uv[2], 0.0);
        assert!((uv[3] - 0.25).abs() < 1e-6, "{uv:?}");
    }

    #[test]
    fn cover_uv_is_identity_when_aspects_match_or_inputs_are_bogus() {
        assert_eq!(cover_uv(160.0, 90.0, 320.0, 180.0), [1.0, 1.0, 0.0, 0.0]);
        for (iw, ih, vw, vh) in [
            (0.0, 100.0, 100.0, 100.0),
            (100.0, 0.0, 100.0, 100.0),
            (100.0, 100.0, 0.0, 100.0),
            (100.0, 100.0, 100.0, 0.0),
            (f32::NAN, 100.0, 100.0, 100.0),
        ] {
            assert_eq!(
                cover_uv(iw, ih, vw, vh),
                [1.0, 1.0, 0.0, 0.0],
                "{iw},{ih},{vw},{vh}"
            );
        }
    }

    // ── 背景纹理尺寸钳制 ───────────────────────────────────────────────

    #[test]
    fn fit_within_caps_the_long_side_and_keeps_the_aspect_ratio() {
        // 典型 4000×3000 照片 → 长边 2048，比例不变。
        assert_eq!(fit_within(4000, 3000, 2048), (2048, 1536));
        // 竖图同理。
        assert_eq!(fit_within(3000, 4000, 2048), (1536, 2048));
        // 已在范围内不动（不放大）。
        assert_eq!(fit_within(800, 600, 2048), (800, 600));
        // 边界：正好等于上限。
        assert_eq!(fit_within(2048, 1000, 2048), (2048, 1000));
        // 输入非法时不 panic，也不返回 0 维。
        assert_eq!(fit_within(0, 0, 2048), (1, 1));
        assert_eq!(fit_within(100, 100, 0), (100, 100));
    }

    // ── dataURL / base64 ───────────────────────────────────────────────

    #[test]
    fn data_url_extracts_and_decodes_the_base64_payload() {
        // "PNG" 的 base64 是 "UE5H"。
        assert_eq!(
            base64_decode_data_url("data:image/png;base64,UE5H").as_deref(),
            Some(&b"PNG"[..])
        );
        // 空载荷解出空字节（上层据此判定「没有图」）。
        assert_eq!(
            base64_decode_data_url("data:image/png;base64,").as_deref(),
            Some(&b""[..])
        );
    }

    #[test]
    fn data_url_rejects_non_base64_and_malformed_input() {
        for bad in [
            "",
            "not a data url",
            "data:image/png,rawbytes",   // 没有 base64 段
            "data:image/png;base64,!!!", // 非法字符
        ] {
            assert!(base64_decode_data_url(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn base64_decode_round_trips_a_known_png_header() {
        // 1×1 PNG 的魔数前 8 字节：89 50 4E 47 0D 0A 1A 0A。
        let decoded = base64_decode("iVBORw0KGgo=").unwrap();
        assert_eq!(
            decoded,
            vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        // 缺 `=` 填充也要能解（dataURL 有时被裁剪过）。
        assert_eq!(base64_decode("iVBORw0KGgo").unwrap(), decoded);
        // 空白容忍。
        assert_eq!(base64_decode(" iVBO Rw0K Ggo= ").unwrap(), decoded);
    }
}
