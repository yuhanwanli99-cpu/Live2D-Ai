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
//! 声明面另用 `rustc --target x86_64-pc-windows-gnu --emit=metadata` 编过。
//!
//! **真实窗口未验**：这里只有编译期证据，没有在 Windows 上运行过。
#![allow(unsafe_code)]

use std::ffi::c_void;

// ── 类型别名（BOOL=i32；WPARAM/LPARAM 按 64 位声明） ──
pub(super) type Handle = *mut c_void;
pub(super) type Hwnd = *mut c_void;
pub(super) type Hdc = *mut c_void;
pub(super) type Hbitmap = *mut c_void;
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

// ── 结构体（字段顺序与头文件一致；构造在 window.rs，所以字段 pub(super)） ──
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
}

#[link(name = "kernel32")]
unsafe extern "system" {
    pub(super) fn GetModuleHandleW(name: *const u16) -> Hinstance;
}
