//! 命中与拖动（`hit` 配置键的落地）。
//!
//! - 缺省 `opaque`：去掉常驻 `WS_EX_TRANSPARENT`，`WM_NCHITTEST` 按最近一帧的
//!   alpha 采样——alpha ≥ 16 返回 `HTCLIENT`（吃鼠标，可拖），否则 `HTTRANSPARENT`
//!   （穿透到下面的窗，桌面图标照样能点）；
//! - `through`：保持 0.4.3 的整窗 `WS_EX_TRANSPARENT`，本文件不参与；
//! - `window`：整矩形 `HTCLIENT`（空像素也吃），只给调试；注意分层窗里
//!   **alpha=0 的像素由系统判为穿透**，所以 `window.rs` 会把帧 alpha 抬到 1
//!   （不可见）——否则 `WM_NCHITTEST` 收不到空像素，整矩形吃不到鼠标；
//! - 拖动：命中像素上按住左键 → `SetCapture` + 记起点 → `WM_MOUSEMOVE` 里按光标
//!   位移调 `SetWindowPos` 并更新「窗原点」→ 松手 `ReleaseCapture`。拖动中命中测试
//!   一律 `HTCLIENT`（否则窗跟着光标离开角色后会丢捕获）。
//!
//! 纯采样/判定（`HitMap` / `verdict`）与平台无关，Linux 单测钉住；Win32 段只在
//! Windows 目标编译。本文件不碰 DIB、不碰皮套、不碰 HTTP。

#![cfg_attr(not(windows), allow(dead_code))]
#![allow(unsafe_code)]

use l2d::renderer::RgbaFrame;

use super::config::HitMode;

/// alpha ≥ 16 判为命中（计划 §2）：只要不是「几乎全透明」的像素，就允许点击/拖动。
/// 取 16 而不是 1，是为了不让描边处 1~2 级的抗锯齿残值把窗「咬」出一个假区域。
pub const ALPHA_HIT_THRESHOLD: u8 = 16;

/// 最近一帧的 alpha 采样表：每像素一个字节，行优先，长度 = 宽 × 高。
///
/// 60 Hz 下**复用同一块 `Vec`**（`refresh` 只 `clear` + `push`），避免每帧一次
/// 上兆字节的分配/释放。
#[derive(Default)]
pub struct HitMap {
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

impl HitMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// 用一帧刷新采样表（窗尺寸变了才需要重新增长 `Vec`）。
    pub fn refresh(&mut self, frame: &RgbaFrame) {
        self.width = frame.width;
        self.height = frame.height;
        let needed = (self.width as usize).saturating_mul(self.height as usize);
        self.alpha.clear();
        self.alpha
            .reserve(needed.saturating_sub(self.alpha.capacity()));
        for pixel in frame.pixels.as_chunks::<4>().0 {
            self.alpha.push(pixel[3]);
        }
    }

    /// 窗内像素 `(x, y)` 是否命中；越界一律不命中（窗外的点走不到这里，但纯函数
    /// 要自己站得住）。
    pub fn hits_at(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as u32, y as u32);
        if x >= self.width || y >= self.height {
            return false;
        }
        match self.alpha.get((y * self.width + x) as usize) {
            Some(&alpha) => alpha >= ALPHA_HIT_THRESHOLD,
            None => false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.alpha.is_empty()
    }
}

/// `WM_NCHITTEST` 的结论（纯值，便于 Linux 单测）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// 交给下面的窗（桌面图标、浏览器等照常可点）。
    Transparent,
    /// 本窗吃鼠标（可点、可拖）。
    Client,
}

/// 命中判定：模式 + 最近一帧 + 窗内坐标（客户区像素，原点在窗左上角）。
///
/// `dragging` 为真时无条件 `Client`：拖动中光标可能已经离开角色（甚至离开窗），
/// 这一小段必须把窗钉在手上的捕获里，松手后自然恢复。
pub fn verdict(mode: HitMode, map: Option<&HitMap>, cx: i32, cy: i32, dragging: bool) -> Verdict {
    if dragging {
        return Verdict::Client;
    }
    match mode {
        HitMode::Through => Verdict::Transparent,
        HitMode::Window => Verdict::Client,
        HitMode::Opaque => match map {
            Some(map) if map.hits_at(cx, cy) => Verdict::Client,
            _ => Verdict::Transparent,
        },
    }
}

#[cfg(windows)]
mod platform {
    //! Win32 段：状态、`WM_NCHITTEST` / 左键拖动。
    use std::sync::Mutex;

    use super::super::win32::*;
    use super::{HitMap, Verdict, verdict};
    use crate::desk_pet::config::{HitMode, PetConfig};

    /// 窗线程的状态（只有 `desk-pet` 线程读写：窗口过程与帧循环同线程）。
    struct State {
        mode: HitMode,
        map: HitMap,
        /// 窗左上角在屏幕上的位置（像素）；`UpdateLayeredWindow` 的 `pptDst` 用它。
        origin: Point,
        dragging: bool,
        cursor_start: Point,
        origin_start: Point,
    }

    /// 全局唯一的一扇窗：`Option` 在 `install` 之前是 `None`（此时任何消息都按
    /// 「穿透」处理，最保守）。用 `Mutex` 而不是 `static mut`：窗口过程是
    /// `unsafe extern`，这里不想再欠一笔别名安全。
    static STATE: Mutex<Option<State>> = Mutex::new(None);

    fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
        match STATE.lock() {
            Ok(mut guard) => guard.as_mut().map(f),
            // 窗口线程里 panic 过的毒锁不该再传染给下一帧：取回内值继续用。
            Err(poisoned) => poisoned.into_inner().as_mut().map(f),
        }
    }

    /// 建窗后立刻装一次（命中模式 + 初始位置）。
    pub fn install(config: PetConfig, origin: Point) {
        let state = State {
            mode: config.hit,
            map: HitMap::new(),
            origin,
            dragging: false,
            cursor_start: Point::default(),
            origin_start: origin,
        };
        match STATE.lock() {
            Ok(mut guard) => *guard = Some(state),
            Err(poisoned) => *poisoned.into_inner() = Some(state),
        }
    }

    /// 每帧把最新 alpha 采样交进来（命中测试读的永远是「最近一帧」）。
    pub fn refresh_map(frame: &l2d::renderer::RgbaFrame) {
        with_state(|state| state.map.refresh(frame));
    }

    /// 当前窗左上角（屏幕像素）。窗口创建后到 `install` 之前返回 `(0,0)`。
    pub fn origin() -> Point {
        with_state(|state| state.origin).unwrap_or_default()
    }

    /// 窗口过程里的鼠标消息；`Some(_)` = 已处理，`None` = 交回 `DefWindowProcW`。
    pub fn handle(hwnd: Hwnd, message: u32, lparam: Lparam) -> Option<Lresult> {
        match message {
            WM_NCHITTEST => Some(nchittest(lparam)),
            WM_LBUTTONDOWN => Some(start_drag(hwnd)),
            WM_MOUSEMOVE => Some(drag_move(hwnd)),
            WM_LBUTTONUP => Some(stop_drag()),
            WM_CAPTURECHANGED => {
                // 捕获被别人抢走（例如调试器/输入法）：就当松手，别让窗卡在拖动态。
                stop_drag();
                None
            }
            _ => None,
        }
    }

    /// `WM_NCHITTEST`：`lparam` 是**屏幕**坐标（两个 16 位有符号量）。
    fn nchittest(lparam: Lparam) -> Lresult {
        let screen_x = (lparam & 0xFFFF) as i16 as i32;
        let screen_y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
        let verdict = with_state(|state| {
            let map = if state.map.is_empty() {
                None
            } else {
                Some(&state.map)
            };
            verdict(
                state.mode,
                map,
                screen_x - state.origin.x,
                screen_y - state.origin.y,
                state.dragging,
            )
        })
        .unwrap_or(Verdict::Transparent);
        match verdict {
            Verdict::Client => HTCLIENT,
            Verdict::Transparent => HTTRANSPARENT,
        }
    }

    fn cursor_pos() -> Point {
        let mut point = Point::default();
        // SAFETY: GetCursorPos 只写本栈上 8 字节的 POINT；失败时点保持 (0,0)。
        unsafe { GetCursorPos(&mut point) };
        point
    }

    /// 左键按下：只有「不是在整窗穿透模式」才拖（`through` 根本收不到消息，双保险）。
    fn start_drag(hwnd: Hwnd) -> Lresult {
        let started = with_state(|state| {
            if state.mode == HitMode::Through {
                return false;
            }
            let cursor = cursor_pos();
            state.cursor_start = cursor;
            state.origin_start = state.origin;
            state.dragging = true;
            true
        })
        .unwrap_or(false);
        if started {
            // 按下即记时；是否录音由 talk.rs 的 300 ms / 8 像素状态机决定。
            // 拖拽行为一字不改（下面 SetCapture 照旧）。
            crate::desk_pet::talk::on_down();
            // SAFETY: 捕获本线程创建的窗；失败也只是拖不动，不影响其它路径。
            unsafe { SetCapture(hwnd) };
            let origin = origin();
            tracing::info!(target: "desk_pet", x = origin.x, y = origin.y, "开始拖动宠物窗");
        }
        0
    }

    /// 拖动中：按「光标相对按下点的位移」移动窗，并更新窗原点。
    fn drag_move(hwnd: Hwnd) -> Lresult {
        let moved = with_state(|state| {
            if !state.dragging {
                return None;
            }
            let cursor = cursor_pos();
            let dx = cursor.x - state.cursor_start.x;
            let dy = cursor.y - state.cursor_start.y;
            // 相对按下点的位移交给按住说话状态机：>8 px 归拖拽、不录音。
            crate::desk_pet::talk::on_move(dx, dy);
            let x = state.origin_start.x + dx;
            let y = state.origin_start.y + dy;
            if x == state.origin.x && y == state.origin.y {
                return None;
            }
            state.origin = Point { x, y };
            Some((x, y))
        })
        .flatten();
        if let Some((x, y)) = moved {
            // 只挪位置：尺寸不变、z 序不变（SWP_NOZORDER 让 HWND_TOPMOST 只用于占位）、
            // 不抢焦点（SWP_NOACTIVATE）。
            // SAFETY: hwnd 是本线程的窗；swp 参数全是常量。
            unsafe {
                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
        }
        0
    }

    /// 松手（或捕获丢失）：结束拖动并还掉捕获。
    fn stop_drag() -> Lresult {
        let was_dragging = with_state(|state| {
            let was = state.dragging;
            state.dragging = false;
            was
        })
        .unwrap_or(false);
        if was_dragging {
            // 松手：短按 / 拖拽 / 有效按住说话在这里分界（拖拽语义不变）。
            crate::desk_pet::talk::on_up();
            // SAFETY: ReleaseCapture 只影响本线程的捕获状态；没捕获时返回 0。
            unsafe { ReleaseCapture() };
            let origin = origin();
            tracing::info!(target: "desk_pet", x = origin.x, y = origin.y, "宠物窗拖动结束（松手位置即新位置）");
        }
        0
    }
}

#[cfg(windows)]
pub use platform::{handle, install, origin, refresh_map};

#[cfg(test)]
mod tests {
    use super::*;

    fn map_from(width: u32, height: u32, alphas: &[u8]) -> HitMap {
        let mut pixels = Vec::with_capacity(alphas.len() * 4);
        for alpha in alphas {
            pixels.extend_from_slice(&[0, 0, 0, *alpha]);
        }
        let frame = RgbaFrame {
            width,
            height,
            pixels,
        };
        let mut map = HitMap::new();
        map.refresh(&frame);
        map
    }

    #[test]
    fn refresh_extracts_alpha_per_pixel() {
        let map = map_from(3, 1, &[0, 16, 255]);
        assert!(!map.is_empty());
        assert!(!map.hits_at(0, 0));
        assert!(map.hits_at(1, 0)); // 恰好等于阈值算命中（≥）
        assert!(map.hits_at(2, 0));
    }

    #[test]
    fn out_of_bounds_never_hits() {
        let map = map_from(2, 2, &[255, 255, 255, 255]);
        for (x, y) in [(-1, 0), (0, -1), (2, 0), (0, 2), (99, 99)] {
            assert!(!map.hits_at(x, y), "({x},{y}) 越界必须不命中");
        }
        assert!(map.hits_at(1, 1));
    }

    #[test]
    fn refresh_reuses_the_same_buffer_on_repeat_frames() {
        let mut map = map_from(4, 4, &[255; 16]);
        let capacity = map.alpha.capacity();
        let frame = RgbaFrame {
            width: 4,
            height: 4,
            pixels: vec![0; 16 * 4],
        };
        map.refresh(&frame);
        assert_eq!(map.alpha.capacity(), capacity, "同尺寸刷新不该重新分配");
        assert!(!map.hits_at(2, 2));
    }

    #[test]
    fn verdict_respects_each_hit_mode() {
        let map = map_from(2, 1, &[0, 255]);
        // opaque：空像素穿透、实像素吃鼠标。
        assert_eq!(
            verdict(HitMode::Opaque, Some(&map), 0, 0, false),
            Verdict::Transparent
        );
        assert_eq!(
            verdict(HitMode::Opaque, Some(&map), 1, 0, false),
            Verdict::Client
        );
        // 没有采样表（首帧之前）按穿透算，最保守。
        assert_eq!(
            verdict(HitMode::Opaque, None, 1, 0, false),
            Verdict::Transparent
        );
        // through / window 不看采样表。
        assert_eq!(
            verdict(HitMode::Through, Some(&map), 1, 0, false),
            Verdict::Transparent
        );
        assert_eq!(
            verdict(HitMode::Window, Some(&map), 0, 0, false),
            Verdict::Client
        );
    }

    #[test]
    fn dragging_beats_every_mode_so_capture_is_kept() {
        let map = map_from(2, 1, &[0, 0]);
        for mode in [HitMode::Opaque, HitMode::Through, HitMode::Window] {
            assert_eq!(verdict(mode, Some(&map), 5, 5, true), Verdict::Client);
        }
    }
}
