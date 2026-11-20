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

pub use super::bubble_layout::BubbleRect;
#[cfg(windows)]
use super::bubble_layout::{self, DpiMetrics};

/// 仅对异常长句省略；普通长句交给 GDI 实测后换行、增高。
pub const MAX_CHARS: usize = 240;
/// 没有 TTS 时气泡停留 4 秒。
pub const TEXT_ONLY_TTL: Duration = Duration::from_secs(4);
/// 播放中的保险丝（声卡异常、VoiceEnded 丢失时不让气泡永远挂着）。
pub const PLAYING_MAX_TTL: Duration = Duration::from_secs(120);
/// 圆角矩形底色：浅色、**直通 alpha**；A=238 ≥ `ALPHA_HIT_THRESHOLD`。
const BG: [u8; 4] = [250, 250, 252, 238];
/// 深色字。
const INK: [u8; 3] = [32, 34, 40];

/// 超过安全字符上限才省略；空串 / 纯空白一律返回空串（不画）。
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
///
/// `current` 是「已经交给合成的那一句」（AI 回复）；`user` 是「用户刚说的
/// 那句」，两者分别由右键菜单的输入消息 / AI 回复开关决定是否画。
#[derive(Debug, Default)]
pub struct BubbleState {
    current: Option<Bubble>,
    user: Option<Bubble>,
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

    /// 用户刚说的那句（ASR 识别结果 / 网页里发的消息）。只出文字 4 秒，
    /// 与 AI 气泡各占一块，不互相覆盖。
    pub fn on_user_line(&mut self, text: &str, now: Instant) {
        let shown = truncate(text);
        if shown.is_empty() {
            self.user = None;
            return;
        }
        self.user = Some(Bubble {
            text: shown,
            shown_at: now,
            ttl: TEXT_ONLY_TTL,
            mode: Mode::TextOnly,
        });
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

    /// 每帧调用：到期就撤（两块各自计时）。
    pub fn tick(&mut self, now: Instant) {
        if self.current.as_ref().is_some_and(|b| b.expired(now)) {
            self.current = None;
        }
        if self.user.as_ref().is_some_and(|b| b.expired(now)) {
            self.user = None;
        }
    }

    pub fn text(&self) -> Option<&str> {
        self.current.as_ref().map(|b| b.text.as_str())
    }

    pub fn user_text(&self) -> Option<&str> {
        self.user.as_ref().map(|b| b.text.as_str())
    }

    #[allow(dead_code)]
    pub fn visible(&self) -> bool {
        self.current.is_some()
    }
}

fn inside_rounded(x: i32, y: i32, w: i32, h: i32, r: i32) -> bool {
    let r = r.min(w / 2).min(h / 2).max(0);
    let cx = x.clamp(r, (w - 1 - r).max(r));
    let cy = y.clamp(r, (h - 1 - r).max(r));
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

/// 把圆角矩形写进帧（直通 alpha）。半径与文字一样按当前 DPI 缩放。
pub fn paint_background(frame: &mut RgbaFrame, rect: BubbleRect, radius: i32) {
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
            if !inside_rounded(lx, ly, rect.w, rect.h, radius) {
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
#[path = "bubble_text.rs"]
mod text;

/// 同一句、同一 DPI 与同一可用宽度只测量一次；蒙版只在文字或尺寸变化时重画。
/// 位置不参与蒙版键，因此角色 bbox 每帧移动会挪泡，但不会重复栅格化。
#[cfg(windows)]
pub struct TextCache {
    rendered: String,
    dpi: u32,
    max_w: i32,
    max_h: i32,
    size: (i32, i32),
    raster_size: Option<(i32, i32)>,
    mask: Option<text::TextMask>,
}

#[cfg(windows)]
impl TextCache {
    pub fn new() -> Self {
        Self {
            rendered: String::new(),
            dpi: 0,
            max_w: 0,
            max_h: 0,
            size: (1, 1),
            raster_size: None,
            mask: None,
        }
    }

    pub fn measure(
        &mut self,
        shown: &str,
        max_w: i32,
        max_h: i32,
        dpi: u32,
        metrics: DpiMetrics,
    ) -> (i32, i32) {
        if self.rendered != shown || self.dpi != dpi || self.max_w != max_w || self.max_h != max_h {
            self.rendered = shown.to_string();
            self.dpi = dpi;
            self.max_w = max_w;
            self.max_h = max_h;
            let measured = text::measure(shown, max_w, max_h, dpi);
            if measured.is_none() {
                tracing::warn!(target: "desk_pet", "气泡文字测量失败，使用可见降级尺寸");
            }
            self.size = measured
                .unwrap_or_else(|| bubble_layout::fallback_size(shown, max_w, max_h, metrics));
            self.raster_size = None;
            self.mask = None;
        }
        self.size
    }

    fn place_at(&mut self, shown: &str, rect: BubbleRect, dpi: u32) {
        let size = (rect.w, rect.h);
        if self.raster_size != Some(size) {
            self.raster_size = Some(size);
            self.mask = text::rasterize(shown, rect.w as u32, rect.h as u32, dpi);
            if self.mask.is_none() {
                tracing::warn!(target: "desk_pet", "气泡文字栅格失败（只画底色，不关 HTTP）");
            }
        }
    }

    pub fn mask(&self) -> Option<&text::TextMask> {
        self.mask.as_ref()
    }
}

#[cfg(windows)]
fn paint_one(
    frame: &mut RgbaFrame,
    shown: &str,
    rect: BubbleRect,
    cache: &mut TextCache,
    dpi: u32,
    metrics: DpiMetrics,
) {
    cache.place_at(shown, rect, dpi);
    paint_background(frame, rect, metrics.radius);
    if let Some(mask) = cache.mask() {
        text::paint(frame, rect, mask);
    }
}

/// 每帧重读角色 bbox，并按两块各自的实测高堆叠；不再有首次位置或 88px 缓存。
#[cfg(windows)]
pub fn overlay_pair(
    frame: &mut RgbaFrame,
    state: &BubbleState,
    ai_cache: &mut TextCache,
    user_cache: &mut TextCache,
    show_user: bool,
    show_ai: bool,
    dpi: u32,
) {
    let ai = if show_ai { state.text() } else { None };
    let user = if show_user { state.user_text() } else { None };
    if ai.is_none() && user.is_none() {
        return;
    }
    let metrics = DpiMetrics::new(dpi);
    let max_w = metrics.max_bubble_width(frame.width as i32);
    let max_h = metrics.max_bubble_height(frame.height as i32);
    let ai_size = ai.map(|shown| ai_cache.measure(shown, max_w, max_h, dpi, metrics));
    let user_size = user.map(|shown| user_cache.measure(shown, max_w, max_h, dpi, metrics));
    let placed = bubble_layout::place_pair(
        frame.width as i32,
        frame.height as i32,
        frame.visible_bbox(),
        ai_size,
        user_size,
        metrics,
    );
    if let (Some(shown), Some(rect)) = (ai, placed.ai) {
        paint_one(frame, shown, rect, ai_cache, dpi, metrics);
    }
    if let (Some(shown), Some(rect)) = (user, placed.user) {
        paint_one(frame, shown, rect, user_cache, dpi, metrics);
    }
}

#[cfg(test)]
#[path = "bubble_tests.rs"]
mod tests;
