//! OBS 试玩捕获：只在写 DIB 时选择透明或绿幕，不改原始 RGBA 帧。
//!
//! `LIVE2D_AI_DESK_PET_CHROMA` 只在开窗时读取一次。缺省 / `transparent`
//! 逐字节沿用旧预乘路径；`green` 把直通 RGBA 全帧合成到 `#00FF00`，并令
//! 源 alpha 与 constant opacity 都为 1。原始帧仍供命中表使用，因此绿幕下
//! 空白区域是否穿透仍由原来的 alpha 决定。

use l2d::renderer::RgbaFrame;

pub const CHROMA_ENV: &str = "LIVE2D_AI_DESK_PET_CHROMA";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromaMode {
    Transparent,
    Green,
}

impl ChromaMode {
    /// 开窗边界读取一次；帧循环不读环境变量。
    pub fn from_env() -> Self {
        let raw = std::env::var_os(CHROMA_ENV);
        let text = raw.as_ref().map(|value| value.to_string_lossy());
        let mode = Self::parse(text.as_deref());
        if mode == Self::Green {
            tracing::info!(
                target: "desk_pet",
                "OBS 绿幕已启用：DIB 全帧合成到 #00FF00，alpha=255，constant opacity=1；原始帧与 alpha 命中表不变"
            );
        }
        mode
    }

    fn parse(value: Option<&str>) -> Self {
        match value {
            None | Some("transparent") => Self::Transparent,
            Some("green") => Self::Green,
            Some(other) => {
                tracing::warn!(
                    target: "desk_pet",
                    value = %other,
                    "非法 LIVE2D_AI_DESK_PET_CHROMA 值；回退 transparent（仅接受 transparent / green）"
                );
                Self::Transparent
            }
        }
    }
}

/// 原始直通 RGBA → 捕获用 DIB。绿幕只写 `out`，绝不回写 `frame`。
pub fn fill_dib(frame: &RgbaFrame, out: &mut [u8], chroma: ChromaMode) {
    match chroma {
        ChromaMode::Transparent => super::fill_premultiplied_bgra(frame, out),
        ChromaMode::Green => fill_green_bgra(frame, out),
    }
}

/// `src*a + green*(1-a)`，然后输出不透明 BGRA（alpha=255，无需再预乘）。
fn fill_green_bgra(frame: &RgbaFrame, out: &mut [u8]) {
    assert_eq!(
        out.len(),
        frame.pixels.len(),
        "预乘目标长度必须等于 RGBA 字节数"
    );
    for (src, dst) in frame
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(out.as_chunks_mut::<4>().0.iter_mut())
    {
        let alpha = u32::from(src[3]);
        let composite = |channel: u8, background: u8| -> u8 {
            ((u32::from(channel) * alpha + u32::from(background) * (255 - alpha) + 127) / 255) as u8
        };
        dst[0] = composite(src[2], 0);
        dst[1] = composite(src[1], 255);
        dst[2] = composite(src[0], 0);
        dst[3] = 255;
    }
}

/// 透明模式保留旧的 constant opacity；绿幕必须为 1，避免 OBS 收到半透明绿底。
pub fn source_constant_alpha(opacity: f32, chroma: ChromaMode) -> u8 {
    match chroma {
        ChromaMode::Green => 255,
        ChromaMode::Transparent => {
            ((opacity.clamp(0.0, 1.0) * 255.0).round() as i32).clamp(1, 255) as u8
        }
    }
}

#[cfg(windows)]
pub fn blend_function(opacity: f32, chroma: ChromaMode) -> super::win32::BlendFunction {
    super::win32::BlendFunction {
        blend_op: super::win32::AC_SRC_OVER,
        blend_flags: 0,
        source_constant_alpha: source_constant_alpha(opacity, chroma),
        alpha_format: super::win32::AC_SRC_ALPHA,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(pixels: Vec<u8>) -> RgbaFrame {
        RgbaFrame {
            width: (pixels.len() / 4) as u32,
            height: 1,
            pixels,
        }
    }

    #[test]
    fn transparent_is_byte_for_byte_compatible_with_legacy_path() {
        let frame = frame(vec![
            255, 0, 0, 128, // 半透明
            10, 20, 30, 0, // 全透明
            10, 20, 30, 255, // 不透明
            1, 1, 1, 128, // 旧路径的 round-to-nearest 边界
        ]);
        let mut legacy = vec![0_u8; frame.pixels.len()];
        let mut actual = vec![0_u8; frame.pixels.len()];
        crate::desk_pet::fill_premultiplied_bgra(&frame, &mut legacy);
        fill_dib(&frame, &mut actual, ChromaMode::Transparent);
        assert_eq!(actual, legacy);
        assert_eq!(
            actual,
            vec![0, 0, 128, 128, 0, 0, 0, 0, 30, 20, 10, 255, 1, 1, 1, 128]
        );
    }

    #[test]
    fn green_composites_transparent_opaque_and_half_transparent_pixels() {
        let frame = frame(vec![
            10, 20, 30, 0, // 空像素也必须成为纯绿，而非仅改 alpha
            10, 20, 30, 255, // 不透明像素原样换成 BGR
            200, 100, 50, 128, // src*a + green*(1-a)
        ]);
        let mut out = vec![0_u8; frame.pixels.len()];
        fill_dib(&frame, &mut out, ChromaMode::Green);
        assert_eq!(
            out,
            vec![0, 255, 0, 255, 30, 20, 10, 255, 25, 177, 100, 255]
        );
    }

    #[test]
    fn green_never_mutates_the_source_frame_used_by_hit_testing() {
        let frame = frame(vec![10, 20, 30, 0, 200, 100, 50, 128]);
        let original = frame.pixels.clone();
        let mut out = vec![0_u8; frame.pixels.len()];
        fill_dib(&frame, &mut out, ChromaMode::Green);
        assert_eq!(frame.pixels, original);
    }

    #[test]
    fn invalid_chroma_values_fall_back_to_transparent() {
        assert_eq!(ChromaMode::parse(None), ChromaMode::Transparent);
        assert_eq!(
            ChromaMode::parse(Some("transparent")),
            ChromaMode::Transparent
        );
        assert_eq!(ChromaMode::parse(Some("green")), ChromaMode::Green);
        for invalid in ["", "GREEN", "blue", " green", "green "] {
            assert_eq!(
                ChromaMode::parse(Some(invalid)),
                ChromaMode::Transparent,
                "非法值必须回退：{invalid:?}"
            );
        }
    }

    #[test]
    fn green_forces_constant_opacity_one_without_changing_transparent_policy() {
        for opacity in [0.0, 0.1, 0.5, 1.0] {
            assert_eq!(source_constant_alpha(opacity, ChromaMode::Green), 255);
        }
        assert_eq!(source_constant_alpha(0.5, ChromaMode::Transparent), 128);
        assert_eq!(source_constant_alpha(1.0, ChromaMode::Transparent), 255);
        // 旧路径即使收到异常 0，也保持最低 1 的防御行为。
        assert_eq!(source_constant_alpha(0.0, ChromaMode::Transparent), 1);
    }
}
