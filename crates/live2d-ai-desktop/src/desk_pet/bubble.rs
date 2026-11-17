//! 桌宠气泡：画在**同一张分层 DIB** 上，只显示「已经交给合成的那一句」。
//!
//! # 画在哪
//!
//! [`overlay`] 在 `window.rs` 里 `renderer.render_frame()` 之后、
//! `hit::refresh_map` / `UpdateLayeredWindow` 之前，把气泡写进同一份
//! `RgbaFrame.pixels`（直通 alpha RGBA）。父模块 `fill_premultiplied_bgra`
//! 随后把整帧转成**预乘 BGRA** 交给分层窗——气泡和皮套共用的是那一张
//! DIB，不是第二扇窗、不是 egui、不是 Flutter 叠层。顺带一个好处：气泡
//! 像素天然进了 `hit.rs` 的 alpha 采样表，「点在字上等于点在角色上」。
//!
//! # 生命周期
//!
//! - 新的一句**整块换掉**上一句（不是追加）；空串不画；
//! - 这句在 cpal 里开始播 → 进入 Playing，播完 / `/stop` → 消失；
//! - 没有 TTS（只出文字）→ 4 秒后消失；notice（没配钥匙 / 没听清）按自己
//!   的 ttl 消失；Playing 另有 120 秒保险丝，防止声卡异常时气泡挂着不走。
//!
//! 失败只记 `desk_pet` 日志，不关 HTTP、不改退出码。

// 非 Windows：绘制段不编译，纯状态机与矩形填充仍供单测使用。
#![cfg_attr(not(windows), allow(dead_code))]

use std::time::{Duration, Instant};

use l2d::renderer::RgbaFrame;

/// 最多 48 个字，超了截断并加「…」。
pub const MAX_CHARS: usize = 48;
/// 没有 TTS 时气泡停留 4 秒。
pub const TEXT_ONLY_TTL: Duration = Duration::from_secs(4);
/// 播放中的保险丝（声卡异常、VoiceEnded 丢失时不让气泡永远挂着）。
pub const PLAYING_MAX_TTL: Duration = Duration::from_secs(120);
/// 圆角矩形底色：浅色、**直通 alpha**；A=238 ≥ `ALPHA_HIT_THRESHOLD`。
const BG: [u8; 4] = [250, 250, 252, 238];
/// 深色字。
const INK: [u8; 3] = [32, 34, 40];
/// 圆角半径 / 内边距 / 字号（物理像素）。
const RADIUS: i32 = 14;
const PAD: i32 = 12;
const FONT_PX: i32 = 16;

/// 超过 48 字截断并加「…」；空串 / 纯空白一律返回空串（不画）。
#[must_use]
pub fn truncate(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= MAX_CHARS {
        return text.to_string();
    }
    let mut out: String = chars[..MAX_CHARS].iter().collect();
    out.push('…');
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 只出文字，按 4 秒计时。
    TextOnly,
    /// 这句正在 cpal 里播，等 VoiceEnded。
    Playing,
    /// 提示（没配识别钥匙 / 没听清），按自己的 ttl 计时。
    Notice,
}

#[derive(Debug, Clone)]
struct Bubble {
    text: String,
    shown_at: Instant,
    ttl: Duration,
    mode: Mode,
}

impl Bubble {
    fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.shown_at) >= self.ttl
    }
}

/// 气泡状态机（平台无关，Linux/Windows 单测都真跑）。
#[derive(Debug, Default)]
pub struct BubbleState {
    current: Option<Bubble>,
}

impl BubbleState {
    /// 一句已经交给合成：`playing` 由 `bridge::is_playing()` 给出，避免
    /// `VoiceStarted` 早于 `SentenceVoiced` 时把正在播的句子误判成只出文字。
    pub fn on_sentence(&mut self, text: &str, now: Instant, playing: bool) {
        let shown = truncate(text);
        if shown.is_empty() {
            self.current = None;
            return;
        }
        self.current = Some(Bubble {
            text: shown,
            shown_at: now,
            ttl: if playing {
                PLAYING_MAX_TTL
            } else {
                TEXT_ONLY_TTL
            },
            mode: if playing {
                Mode::Playing
            } else {
                Mode::TextOnly
            },
        });
    }

    pub fn on_voice_started(&mut self) {
        if let Some(b) = self.current.as_mut()
            && b.mode != Mode::Notice
        {
            b.mode = Mode::Playing;
            b.ttl = PLAYING_MAX_TTL;
        }
    }

    /// 播完 / 排空 / `/stop`；notice 不跟着撤。
    pub fn on_voice_ended(&mut self) {
        if self
            .current
            .as_ref()
            .is_some_and(|b| b.mode != Mode::Notice)
        {
            self.current = None;
        }
    }

    pub fn on_notice(&mut self, text: &str, ttl: Duration, now: Instant) {
        let shown = truncate(text);
        if shown.is_empty() {
            return;
        }
        self.current = Some(Bubble {
            text: shown,
            shown_at: now,
            ttl,
            mode: Mode::Notice,
        });
    }

    /// 每帧调用：到期就撤。
    pub fn tick(&mut self, now: Instant) {
        if self.current.as_ref().is_some_and(|b| b.expired(now)) {
            self.current = None;
        }
    }

    pub fn text(&self) -> Option<&str> {
        self.current.as_ref().map(|b| b.text.as_str())
    }

    #[allow(dead_code)]
    pub fn visible(&self) -> bool {
        self.current.is_some()
    }
}

/// 气泡矩形（客户区像素，`x/y` 允许为负，绘制时逐像素裁到帧内）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BubbleRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

fn inside_rounded(x: i32, y: i32, w: i32, h: i32, r: i32) -> bool {
    let r = r.min(w / 2).min(h / 2).max(0);
    let cx = x.clamp(r, (w - 1 - r).max(r));
    let cy = y.clamp(r, (h - 1 - r).max(r));
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

/// 在角色上方（有可见包围盒时）或窗顶居中放一条气泡；小屏也不越界。
#[must_use]
pub fn layout(frame: &RgbaFrame, _text_chars: usize) -> BubbleRect {
    let fw = frame.width as i32;
    let fh = frame.height as i32;
    let margin = 10;
    let w = if fw > 2 * margin {
        (fw - 2 * margin).min(420)
    } else {
        fw.max(1)
    };
    let h = fh.clamp(1, 88);
    let x = ((fw - w) / 2).max(0);
    let mut y = 12.min((fh - h).max(0));
    if let Some((_x0, y0, _x1, _y1)) = frame.visible_bbox() {
        let above = y0 as i32 - h - 12;
        if above > 0 && above + h <= fh {
            y = above;
        }
    }
    BubbleRect { x, y, w, h }
}

/// 把圆角矩形写进帧（直通 alpha）。返回值只是便于单测定位。
pub fn paint_background(frame: &mut RgbaFrame, rect: BubbleRect) {
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
            if !inside_rounded(lx, ly, rect.w, rect.h, RADIUS) {
                continue;
            }
            let idx = ((y as u32 * frame.width + x as u32) * 4) as usize;
            if idx + 4 <= frame.pixels.len() {
                frame.pixels[idx..idx + 4].copy_from_slice(&BG);
            }
        }
    }
}

#[cfg(windows)]
mod text {
    #![allow(unsafe_code)]
    //! GDI 文字栅格：临时 32bpp DIB 上用微软雅黑画黑字，读回覆盖率当蒙版。
    //! **不引新依赖**（`win32.rs` 手写 FFI），也不碰主 DIB 的 alpha 通道。
    use std::ffi::c_void;

    use super::super::win32::*;
    use super::{BubbleRect, FONT_PX, INK, PAD, RgbaFrame};

    pub struct TextMask {
        pub w: u32,
        pub h: u32,
        /// 每像素一个字节：255 = 全是字，0 = 底色。
        pub coverage: Vec<u8>,
    }

    /// 失败返回 `None`（调用方只记日志，气泡底色照样在）。
    pub fn rasterize(text: &str, w: u32, h: u32) -> Option<TextMask> {
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
            let face = wide("Microsoft YaHei");
            let font = CreateFontW(
                -FONT_PX,
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
            );
            let old_font = SelectObject(dc, font);
            SetTextColor(dc, 0x0000_0000); // COLORREF 黑色
            SetBkMode(dc, TRANSPARENT);
            let wide_text = wide(text);
            let mut bounds = Rect {
                left: PAD,
                top: PAD,
                right: (iw - PAD).max(PAD + 1),
                bottom: (ih - PAD).max(PAD + 1),
            };
            DrawTextW(
                dc,
                wide_text.as_ptr(),
                -1,
                &mut bounds,
                DT_LEFT | DT_TOP | DT_WORDBREAK | DT_NOPREFIX,
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
                    frame.pixels[idx + ch] =
                        ((bg * inv + u16::from(ink) * u16::from(c)) / 255) as u8;
                }
                frame.pixels[idx + 3] = 238;
            }
        }
    }
}

/// 文字栅格缓存：一句只排版 / 栅格化一次。
///
/// `layout` 要读一次整帧的 `visible_bbox`（找角色头顶），60 Hz 里每帧扫
/// 一百万像素太浪费；窗尺寸固定、同一句期间角色位置近似不变，所以把矩形
/// 与蒙版一起按「上台词文本」缓存。
#[cfg(windows)]
pub struct TextCache {
    rendered: String,
    rect: Option<BubbleRect>,
    mask: Option<text::TextMask>,
}

#[cfg(windows)]
impl TextCache {
    pub fn new() -> Self {
        Self {
            rendered: String::new(),
            rect: None,
            mask: None,
        }
    }

    /// 换了一句才重新排版 + 栅格化；同一句每帧复用同一个矩形/蒙版。
    pub fn place(&mut self, shown: &str, frame: &RgbaFrame) -> BubbleRect {
        if self.rendered != shown || self.rect.is_none() {
            let rect = layout(frame, shown.chars().count());
            self.rendered = shown.to_string();
            self.rect = Some(rect);
            self.mask = text::rasterize(shown, rect.w as u32, rect.h as u32);
            if self.mask.is_none() {
                tracing::warn!(target: "desk_pet", "气泡文字栅格失败（只画底色，不关 HTTP）");
            }
        }
        self.rect.unwrap_or(BubbleRect {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        })
    }

    pub fn mask(&self) -> Option<&text::TextMask> {
        self.mask.as_ref()
    }
}

/// 每帧：把当前气泡叠进帧。无气泡 = 一字节不改。
#[cfg(windows)]
pub fn overlay(frame: &mut RgbaFrame, state: &BubbleState, cache: &mut TextCache) {
    let Some(shown) = state.text() else {
        return;
    };
    let rect = cache.place(shown, frame);
    paint_background(frame, rect);
    if let Some(mask) = cache.mask() {
        text::paint(frame, rect, mask);
    }
}

#[cfg(test)]
#[path = "bubble_tests.rs"]
mod tests;
