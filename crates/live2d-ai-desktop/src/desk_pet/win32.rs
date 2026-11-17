//! Win32 裸绑定（0.4.3 rc.2 桌宠窗）——**只有声明，没有策略**。
//!
//! 手写 `unsafe extern "system"` + `#[link]`，**不引 windows-sys**：
//! desktop 顶层 `[dependencies]` 仍 18 条（与 `folder_job.rs` 同一条纪律）。
//! 只在 Windows 目标编译（`desk_pet/mod.rs` 里 `#[cfg(windows)]`）。
//!
//! 取值与字段布局逐项对过 mingw-w64 头文件（`winuser.h` / `wingdi.h`；
//! 本机在 `/mnt/c/msys64/mingw64/include`），64 位 `sizeof` 实测：
//! `WNDCLASSEXW`=80、`MSG`=48、`POINT`=8、`SIZE`=8、
//! `BLENDFUNCTION`=4、`BITMAPINFOHEADER`=40、`BITMAPINFO`=44。
//!
//! 0.4.3 之后新增两组声明（命中/拖动与 DPI）：`WM_NCHITTEST` 系列消息、
//! `HTTRANSPARENT`/`HTCLIENT`、`SetCapture`/`ReleaseCapture`/`GetCursorPos`/
//! `SetWindowPos`，以及 `SetProcessDpiAwarenessContext` / `SetThreadDpiAwarenessContext`
//! / `SetProcessDPIAware` / `GetDpiForSystem`。符号可链接性用
//! `x86_64-w64-mingw32-gcc` 实测过（四个 DPI 符号都在 user32 的导入库里）。
//!
//! 真机证据：`D:\live2dai\plan\VERIFY-desk-pet-win.md`（H1 基线 / H3 命中拖动 /
//! H4 DPI 的截图与日志原文）。
#![allow(unsafe_code)]

use std::ffi::c_void;

// ── 类型别名（BOOL=i32；WPARAM/LPARAM 按 64 位声明） ──
pub(super) type Handle = *mut c_void;
pub(super) type Hwnd = *mut c_void;
pub(super) type Hdc = *mut c_void;
pub(super) type Hbitmap = *mut c_void;
pub(super) type Hmenu = *mut c_void;
pub(super) type Hinstance = *mut c_void;
pub(super) type Lresult = isize;
pub(super) type Wparam = usize;
pub(super) type Lparam = isize;
pub(super) type Bool = i32;

// ── 窗口类名与常量（mingw-w64 头文件核对值） ──
pub(super) const CLASS_NAME: &str = "live2d-ai-desk-pet";
pub(super) const WS_POPUP: u32 = 0x8000_0000;
pub(super) const WS_VISIBLE: u32 = 0x1000_0000;
pub(super) const WS_EX_LAYERED: u32 = 0x0008_0000;
pub(super) const WS_EX_TRANSPARENT: u32 = 0x0000_0020;
pub(super) const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub(super) const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub(super) const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
pub(super) const ULW_ALPHA: u32 = 0x0000_0002;
pub(super) const AC_SRC_OVER: u8 = 0x00;
pub(super) const AC_SRC_ALPHA: u8 = 0x01;
pub(super) const BI_RGB: u32 = 0;
pub(super) const DIB_RGB_COLORS: u32 = 0;
pub(super) const PM_REMOVE: u32 = 0x0001;
pub(super) const WM_DESTROY: u32 = 0x0002;
pub(super) const WM_CLOSE: u32 = 0x0010;
pub(super) const WM_QUIT: u32 = 0x0012;
pub(super) const SW_SHOWNOACTIVATE: i32 = 4;
pub(super) const SM_CXSCREEN: i32 = 0;
pub(super) const SM_CYSCREEN: i32 = 1;

// ── 命中测试 / 鼠标 / 拖动（0.4.3 之后） ──
pub(super) const WM_NCHITTEST: u32 = 0x0084;
pub(super) const WM_MOUSEMOVE: u32 = 0x0200;
pub(super) const WM_LBUTTONDOWN: u32 = 0x0201;
pub(super) const WM_LBUTTONUP: u32 = 0x0202;
pub(super) const WM_CAPTURECHANGED: u32 = 0x0215;
// ── 右键菜单（2026-11-17「回到网页」） ──
pub(super) const WM_NULL: u32 = 0x0000;
pub(super) const WM_RBUTTONDOWN: u32 = 0x0204;
pub(super) const WM_RBUTTONUP: u32 = 0x0205;
/// `MF_STRING` / `MF_BYCOMMAND` / `TPM_LEFTALIGN` 在 winuser.h 里都是 0。
pub(super) const MF_STRING: u32 = 0x0000;
pub(super) const TPM_RIGHTBUTTON: u32 = 0x0002;
pub(super) const TPM_LEFTALIGN: u32 = 0x0000;
/// 带这一位时 `TrackPopupMenu` 返回选中的命令 ID（而不是 BOOL），且不再发 WM_COMMAND。
pub(super) const TPM_RETURNCMD: u32 = 0x0100;
/// `WM_NCHITTEST` 答复：这个点不属于我，交给下面的窗。
pub(super) const HTTRANSPARENT: Lresult = -1;
/// `WM_NCHITTEST` 答复：客户区（本窗吃这个点）。
pub(super) const HTCLIENT: Lresult = 1;
pub(super) const SWP_NOSIZE: u32 = 0x0001;
pub(super) const SWP_NOZORDER: u32 = 0x0004;
pub(super) const SWP_NOACTIVATE: u32 = 0x0010;
/// `SetWindowPos` 的 `hWndInsertAfter` 占位（配合 `SWP_NOZORDER` 时 z 序不变）。
pub(super) const HWND_TOPMOST: Hwnd = -1_isize as Hwnd;

// ── DPI（建窗前设进程感知；失败退线程级，再失败就用系统虚拟化坐标） ──
/// `DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2`（值 -4）。
pub(super) const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: Handle = -4_isize as Handle;

// ── 气泡文字栅格（2026-11-17）：临时 DIB 上用 GDI 画字，读回覆盖率 ──
pub(super) const DT_WORDBREAK: u32 = 0x0010;
pub(super) const DT_NOPREFIX: u32 = 0x0800;
pub(super) const DT_LEFT: u32 = 0x0000;
pub(super) const DT_TOP: u32 = 0x0000;
pub(super) const TRANSPARENT: i32 = 1;
pub(super) const FW_NORMAL: i32 = 400;
pub(super) const DEFAULT_CHARSET: u32 = 1;
pub(super) const ANTIALIASED_QUALITY: u32 = 4;

// ── 结构体（字段顺序与头文件一致；构造在 window.rs / hit.rs，所以字段 pub(super)） ──
#[repr(C)]
pub(super) struct WndClassExW {
    pub(super) cb_size: u32,
    pub(super) style: u32,
    pub(super) lpfn_wnd_proc:
        Option<unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult>,
    pub(super) cb_cls_extra: i32,
    pub(super) cb_wnd_extra: i32,
    pub(super) h_instance: Hinstance,
    pub(super) h_icon: Handle,
    pub(super) h_cursor: Handle,
    pub(super) hbr_background: Handle,
    pub(super) lpsz_menu_name: *const u16,
    pub(super) lpsz_class_name: *const u16,
    pub(super) h_icon_sm: Handle,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct Point {
    pub(super) x: i32,
    pub(super) y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct Size {
    pub(super) cx: i32,
    pub(super) cy: i32,
}

/// `RECT`（DrawTextW 的排版边界）。
#[repr(C)]
pub(super) struct Rect {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) right: i32,
    pub(super) bottom: i32,
}

#[repr(C)]
pub(super) struct Msg {
    pub(super) hwnd: Hwnd,
    pub(super) message: u32,
    pub(super) w_param: Wparam,
    pub(super) l_param: Lparam,
    pub(super) time: u32,
    pub(super) pt: Point,
}

#[repr(C)]
pub(super) struct BlendFunction {
    pub(super) blend_op: u8,
    pub(super) blend_flags: u8,
    pub(super) source_constant_alpha: u8,
    pub(super) alpha_format: u8,
}

#[repr(C)]
pub(super) struct BitmapInfoHeader {
    pub(super) bi_size: u32,
    pub(super) bi_width: i32,
    pub(super) bi_height: i32,
    pub(super) bi_planes: u16,
    pub(super) bi_bit_count: u16,
    pub(super) bi_compression: u32,
    pub(super) bi_size_image: u32,
    pub(super) bi_x_pels_per_meter: i32,
    pub(super) bi_y_pels_per_meter: i32,
    pub(super) bi_clr_used: u32,
    pub(super) bi_clr_important: u32,
}

#[repr(C)]
pub(super) struct BitmapInfo {
    pub(super) header: BitmapInfoHeader,
    pub(super) colors: [u32; 1],
}

/// UTF-16 + NUL（宽字符 API 的入参形态）。
pub(super) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 最近的 Win32 错误（不声明 `GetLastError`：标准库已经包好了）。
pub(super) fn last_os() -> std::io::Error {
    std::io::Error::last_os_error()
}

// ── 手写 FFI（链接名 = Windows 导入库） ──
#[link(name = "user32")]
unsafe extern "system" {
    pub(super) fn RegisterClassExW(class: *const WndClassExW) -> u16;
    pub(super) fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: Handle,
        instance: Hinstance,
        param: *mut c_void,
    ) -> Hwnd;
    pub(super) fn DefWindowProcW(
        hwnd: Hwnd,
        message: u32,
        wparam: Wparam,
        lparam: Lparam,
    ) -> Lresult;
    pub(super) fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> Bool;
    pub(super) fn TranslateMessage(msg: *const Msg) -> Bool;
    pub(super) fn DispatchMessageW(msg: *const Msg) -> Lresult;
    pub(super) fn ShowWindow(hwnd: Hwnd, cmd: i32) -> Bool;
    pub(super) fn DestroyWindow(hwnd: Hwnd) -> Bool;
    pub(super) fn PostQuitMessage(code: i32);
    pub(super) fn GetSystemMetrics(index: i32) -> i32;
    pub(super) fn GetDC(hwnd: Hwnd) -> Hdc;
    pub(super) fn ReleaseDC(hwnd: Hwnd, hdc: Hdc) -> i32;
    pub(super) fn UpdateLayeredWindow(
        hwnd: Hwnd,
        screen_dc: Hdc,
        dst: *const Point,
        size: *const Size,
        src_dc: Hdc,
        src: *const Point,
        key: u32,
        blend: *const BlendFunction,
        flags: u32,
    ) -> Bool;
    // 命中 / 拖动（0.4.3 之后）
    pub(super) fn SetCapture(hwnd: Hwnd) -> Hwnd;
    pub(super) fn ReleaseCapture() -> Bool;
    pub(super) fn GetCursorPos(point: *mut Point) -> Bool;
    pub(super) fn SetWindowPos(
        hwnd: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Bool;
    // DPI（0.4.3 之后；Win10 1703+ / Vista+）
    pub(super) fn SetProcessDpiAwarenessContext(context: Handle) -> Bool;
    pub(super) fn SetThreadDpiAwarenessContext(context: Handle) -> Handle;
    pub(super) fn SetProcessDPIAware() -> Bool;
    pub(super) fn GetDpiForSystem() -> u32;
    // 关窗：PostMessageW(WM_CLOSE) 复用现成的退出链，不新造停止协议。
    pub(super) fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: Wparam, lparam: Lparam) -> Bool;
    // 右键菜单（TPM_RETURNCMD 路线，不接 WM_COMMAND）。
    pub(super) fn ClientToScreen(hwnd: Hwnd, point: *mut Point) -> Bool;
    pub(super) fn CreatePopupMenu() -> Hmenu;
    pub(super) fn DestroyMenu(menu: Hmenu) -> Bool;
    pub(super) fn AppendMenuW(menu: Hmenu, flags: u32, id: usize, text: *const u16) -> Bool;
    pub(super) fn TrackPopupMenu(
        menu: Hmenu,
        flags: u32,
        x: i32,
        y: i32,
        reserved: i32,
        hwnd: Hwnd,
        rect: *const Rect,
    ) -> Bool;
    pub(super) fn SetForegroundWindow(hwnd: Hwnd) -> Bool;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    pub(super) fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
    pub(super) fn CreateDIBSection(
        hdc: Hdc,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: Handle,
        offset: u32,
    ) -> Hbitmap;
    pub(super) fn SelectObject(hdc: Hdc, object: Handle) -> Handle;
    pub(super) fn DeleteObject(object: Handle) -> Bool;
    pub(super) fn DeleteDC(hdc: Hdc) -> Bool;
    // 气泡文字栅格：临时 DIB 上画黑字，读回像素当覆盖率蒙版。
    pub(super) fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike_out: u32,
        char_set: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch_and_family: u32,
        face_name: *const u16,
    ) -> Handle;
    pub(super) fn DrawTextW(
        hdc: Hdc,
        text: *const u16,
        count: i32,
        rect: *mut Rect,
        format: u32,
    ) -> i32;
    pub(super) fn SetTextColor(hdc: Hdc, color: u32) -> u32;
    pub(super) fn SetBkMode(hdc: Hdc, mode: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub(super) fn GetModuleHandleW(name: *const u16) -> Hinstance;
}
