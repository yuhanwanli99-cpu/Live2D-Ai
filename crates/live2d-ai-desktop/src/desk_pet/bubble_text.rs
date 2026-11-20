#![allow(unsafe_code)]
//! 桌宠气泡的 GDI 文字栅格（Windows 专用；由 `bubble.rs` 以 `#[path]` 加载）。
//!
//! GDI 文字栅格：临时 32bpp DIB 上用微软雅黑画黑字，读回覆盖率当蒙版。
//! **不引新依赖**（`win32.rs` 手写 FFI），也不碰主 DIB 的 alpha 通道。
use std::ffi::c_void;

use super::super::bubble_layout::DpiMetrics;
use super::super::win32::*;
use super::{BubbleRect, INK, RgbaFrame};

pub struct TextMask {
    pub w: u32,
    pub h: u32,
    /// 每像素一个字节：255 = 全是字，0 = 底色。
    pub coverage: Vec<u8>,
}

/// 测量与最终栅格共用这一份字体创建参数，避免量一套、画一套。
unsafe fn create_font(dpi: u32) -> Handle {
    let face = wide("Microsoft YaHei");
    unsafe {
        CreateFontW(
            -DpiMetrics::new(dpi).font_px,
            0,
            0,
            0,
            FW_NORMAL,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            0,
            0,
            ANTIALIASED_QUALITY,
            0,
            face.as_ptr(),
        )
    }
}

/// 用最终绘制同款字体先量单行自然宽，再按同一内容宽量换行后的实际高。
pub fn measure(text: &str, max_w: i32, max_h: i32, dpi: u32) -> Option<(i32, i32)> {
    if text.is_empty() || max_w <= 0 || max_h <= 0 {
        return None;
    }
    let metrics = DpiMetrics::new(dpi);
    let content_max = (max_w - 2 * metrics.pad).max(1);
    unsafe {
        let screen_dc = GetDC(std::ptr::null_mut());
        if screen_dc.is_null() {
            return None;
        }
        let dc = CreateCompatibleDC(screen_dc);
        ReleaseDC(std::ptr::null_mut(), screen_dc);
        if dc.is_null() {
            return None;
        }
        let font = create_font(dpi);
        if font.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old_font = SelectObject(dc, font);
        let wide_text = wide(text);
        let mut natural = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        let natural_ok = DrawTextW(
            dc,
            wide_text.as_ptr(),
            -1,
            &mut natural,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        let content_w = (natural.right - natural.left).clamp(1, content_max);
        let mut wrapped = Rect {
            left: 0,
            top: 0,
            right: content_w,
            bottom: 0,
        };
        let text_h = DrawTextW(
            dc,
            wide_text.as_ptr(),
            -1,
            &mut wrapped,
            DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
        );
        SelectObject(dc, old_font);
        DeleteObject(font);
        DeleteDC(dc);
        if natural_ok <= 0 || text_h <= 0 {
            return None;
        }
        Some((
            (content_w + 2 * metrics.pad).min(max_w).max(1),
            (text_h + 2 * metrics.pad).min(max_h).max(1),
        ))
    }
}

/// 失败返回 `None`（调用方只记日志，气泡底色照样在）。
pub fn rasterize(text: &str, w: u32, h: u32, dpi: u32) -> Option<TextMask> {
    if w == 0 || h == 0 || text.is_empty() {
        return None;
    }
    let (iw, ih) = (w as i32, h as i32);
    let len = (w as usize).saturating_mul(h as usize).saturating_mul(4);
    // SAFETY: 所有句柄都由本函数创建并在末尾按反序释放；bits 只在
    // CreateDIBSection 返回的这块 DIB 范围内读写，长度 = w*h*4。
    unsafe {
        let screen_dc = GetDC(std::ptr::null_mut());
        if screen_dc.is_null() {
            return None;
        }
        let dc = CreateCompatibleDC(screen_dc);
        let info = BitmapInfo {
            header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: iw,
                bi_height: -ih, // 顶朝下，和 RgbaFrame 行序一致
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: BI_RGB,
                bi_size_image: w * h * 4,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            colors: [0],
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(
            screen_dc,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        ReleaseDC(std::ptr::null_mut(), screen_dc);
        if dc.is_null() || bitmap.is_null() || bits.is_null() {
            if !dc.is_null() {
                DeleteDC(dc);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            return None;
        }
        let old_bitmap = SelectObject(dc, bitmap);
        // 白底：GDI 只画字（SetBkMode TRANSPARENT），其余像素保持白。
        {
            let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), len);
            for px in pixels.as_chunks_mut::<4>().0 {
                px.copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
            }
        }
        let font = create_font(dpi);
        if font.is_null() {
            SelectObject(dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(dc);
            return None;
        }
        let old_font = SelectObject(dc, font);
        SetTextColor(dc, 0x0000_0000); // COLORREF 黑色
        SetBkMode(dc, TRANSPARENT);
        let wide_text = wide(text);
        let pad = DpiMetrics::new(dpi).pad;
        let mut bounds = Rect {
            left: pad,
            top: pad,
            right: (iw - pad).max(pad + 1),
            bottom: (ih - pad).max(pad + 1),
        };
        DrawTextW(
            dc,
            wide_text.as_ptr(),
            -1,
            &mut bounds,
            DT_LEFT | DT_TOP | DT_WORDBREAK | DT_NOPREFIX | DT_WORD_ELLIPSIS,
        );
        // 覆盖率 = 255 - 亮度（黑字→255，白底→0）；取 RGB 均值兼容
        // ClearType 的彩色亚像素。
        let coverage: Vec<u8> = std::slice::from_raw_parts(bits.cast::<u8>(), len)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|px| {
                let lum = ((u16::from(px[0]) + u16::from(px[1]) + u16::from(px[2])) / 3) as u8;
                255_u8.saturating_sub(lum)
            })
            .collect();
        SelectObject(dc, old_font);
        DeleteObject(font);
        SelectObject(dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(dc);
        Some(TextMask { w, h, coverage })
    }
}

/// 用覆盖率蒙版把深色字叠到气泡底色上（保持气泡的 alpha）。
pub fn paint(frame: &mut RgbaFrame, rect: BubbleRect, mask: &TextMask) {
    if mask.w != rect.w as u32 || mask.h != rect.h as u32 {
        return;
    }
    for ly in 0..rect.h {
        let y = rect.y + ly;
        if y < 0 || y >= frame.height as i32 {
            continue;
        }
        for lx in 0..rect.w {
            let x = rect.x + lx;
            if x < 0 || x >= frame.width as i32 {
                continue;
            }
            let c = mask.coverage[(ly as u32 * mask.w + lx as u32) as usize];
            if c == 0 {
                continue;
            }
            let idx = ((y as u32 * frame.width + x as u32) * 4) as usize;
            if idx + 4 > frame.pixels.len() {
                continue;
            }
            let inv = 255 - u16::from(c);
            for (ch, &ink) in INK.iter().enumerate() {
                let bg = u16::from(frame.pixels[idx + ch]);
                frame.pixels[idx + ch] = ((bg * inv + u16::from(ink) * u16::from(c)) / 255) as u8;
            }
            frame.pixels[idx + 3] = 238;
        }
    }
}
