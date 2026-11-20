//! 简易会话框：角色旁边的纯 Win32 小窗，正文是真正的只读多行 EDIT。
//!
//! - `WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY` 提供真滚动；
//!   新句写入后 `EM_SETSEL(end,end) + EM_SCROLLCARET` 自动滚到底；
//! - 父窗仍用 `WS_OVERLAPPEDWINDOW`，拖边缩放时 EDIT 跟随客户区；
//! - 置顶只作用于这块小窗，与角色窗互不影响；
//! - `WM_DESTROY` 不 `PostQuitMessage`，用户点 X 或角色关闭都不会误杀别的窗。
//!
//! 历史与工作区几何在平台无关模块；本文件只保留 Win32 行为。

#![allow(unsafe_code)]

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::bubble_layout::dpi_scale;
use super::chat_history::History;
use super::chat_layout::{self, Rect as LayoutRect};
use super::menu_state;
use super::win32::*;

const CHAT_W: i32 = 360;
const CHAT_H: i32 = 260;

#[derive(Default)]
struct Native {
    pet: Option<usize>,
    chat: Option<usize>,
    edit: Option<usize>,
    font: Option<usize>,
}

static HISTORY: Mutex<History> = Mutex::new(History::new());
static NATIVE: Mutex<Native> = Mutex::new(Native {
    pet: None,
    chat: None,
    edit: None,
    font: None,
});
static DIRTY: AtomicBool = AtomicBool::new(false);
static LAST_TOPMOST: AtomicBool = AtomicBool::new(false);

fn hwnd(value: usize) -> Hwnd {
    value as Hwnd
}

fn with_native<R>(f: impl FnOnce(&mut Native) -> R) -> R {
    match NATIVE.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

fn with_history<R>(f: impl FnOnce(&mut History) -> R) -> R {
    match HISTORY.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

pub fn set_pet_hwnd(h: Hwnd) {
    with_native(|native| native.pet = Some(h as usize));
}

/// 追加完整一句；只有很大的安全上限防无界，气泡省略策略不参与。
pub fn push(who: &'static str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    with_history(|history| history.push(who, text));
    DIRTY.store(true, Ordering::SeqCst);
}

#[must_use]
pub fn rendered_text() -> String {
    with_history(|history| history.rendered_text())
}

/// 角色窗关掉时销毁小窗并同步菜单；EDIT 与字体由父窗 WM_DESTROY 收尾。
pub fn close() {
    let existing = with_native(|native| native.chat.take());
    if let Some(value) = existing {
        unsafe { DestroyWindow(hwnd(value)) };
    }
    with_native(|native| native.pet = None);
    menu_state::close_chat();
    DIRTY.store(false, Ordering::SeqCst);
}

fn current_edit() -> Option<Hwnd> {
    with_native(|native| native.edit.map(hwnd))
}

fn current_dpi(window: Hwnd) -> u32 {
    let dpi = unsafe { GetDpiForWindow(window) };
    if dpi == 0 {
        unsafe { GetDpiForSystem() }
    } else {
        dpi
    }
}

fn scroll_to_bottom() {
    let Some(edit) = current_edit() else {
        return;
    };
    unsafe {
        let end = GetWindowTextLengthW(edit).max(0);
        SendMessageW(edit, EM_SETSEL, end as usize, end as isize);
        SendMessageW(edit, EM_SCROLLCARET, 0, 0);
    }
}

fn update_text() {
    let Some(edit) = current_edit() else {
        return;
    };
    // EDIT 只认 CRLF；History 内部正文仍保留原始内容。
    let text = rendered_text()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n");
    let text = wide(&text);
    unsafe {
        SetWindowTextW(edit, text.as_ptr());
    }
    scroll_to_bottom();
}

fn resize_edit(parent: Hwnd) {
    let Some(edit) = current_edit() else {
        return;
    };
    let mut client = Rect::default();
    unsafe {
        GetClientRect(parent, &mut client);
        SetWindowPos(
            edit,
            std::ptr::null_mut(),
            0,
            0,
            (client.right - client.left).max(1),
            (client.bottom - client.top).max(1),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    scroll_to_bottom();
}

fn create_chat_font(dpi: u32) -> Handle {
    let face = wide("Microsoft YaHei");
    unsafe {
        CreateFontW(
            -dpi_scale(16, dpi),
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

fn refresh_font(parent: Hwnd) {
    let Some(edit) = current_edit() else {
        return;
    };
    let font = create_chat_font(current_dpi(parent));
    if font.is_null() {
        return;
    }
    unsafe {
        SendMessageW(edit, WM_SETFONT, font as usize, 1);
    }
    let old = with_native(|native| native.font.replace(font as usize));
    if let Some(old) = old {
        unsafe { DeleteObject(old as Handle) };
    }
}

unsafe extern "system" fn wnd_proc(
    window: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    match message {
        WM_DESTROY => {
            let font = with_native(|native| {
                native.chat = None;
                native.edit = None;
                native.font.take()
            });
            if let Some(font) = font {
                unsafe { DeleteObject(font as Handle) };
            }
            0
        }
        WM_CLOSE => {
            menu_state::close_chat();
            unsafe { DestroyWindow(window) };
            0
        }
        WM_SIZE => {
            resize_edit(window);
            0
        }
        WM_DPICHANGED => {
            let suggested = lparam as *const Rect;
            if !suggested.is_null() {
                let rect = unsafe { *suggested };
                unsafe {
                    SetWindowPos(
                        window,
                        std::ptr::null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }
            }
            refresh_font(window);
            resize_edit(window);
            0
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

fn work_area(pet: Hwnd) -> Rect {
    if !pet.is_null() {
        let monitor = unsafe { MonitorFromWindow(pet, MONITOR_DEFAULTTONEAREST) };
        if !monitor.is_null() {
            let mut info = MonitorInfo {
                cb_size: std::mem::size_of::<MonitorInfo>() as u32,
                monitor: Rect::default(),
                work: Rect::default(),
                flags: 0,
            };
            if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
                return info.work;
            }
        }
    }
    let mut work = Rect::default();
    if unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut Rect).cast(), 0) } != 0
    {
        return work;
    }
    Rect {
        left: 0,
        top: 0,
        right: unsafe { GetSystemMetrics(SM_CXSCREEN) },
        bottom: unsafe { GetSystemMetrics(SM_CYSCREEN) },
    }
}

fn layout_rect(rect: Rect) -> LayoutRect {
    LayoutRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

fn create(pet: Hwnd) -> Option<usize> {
    let class_name = wide(CHAT_CLASS_NAME);
    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    if instance.is_null() {
        return None;
    }
    let wnd_class = WndClassExW {
        cb_size: std::mem::size_of::<WndClassExW>() as u32,
        style: 0,
        lpfn_wnd_proc: Some(wnd_proc),
        cb_cls_extra: 0,
        cb_wnd_extra: 0,
        h_instance: instance,
        h_icon: std::ptr::null_mut(),
        h_cursor: std::ptr::null_mut(),
        hbr_background: COLOR_WINDOW_BRUSH,
        lpsz_menu_name: std::ptr::null(),
        lpsz_class_name: class_name.as_ptr(),
        h_icon_sm: std::ptr::null_mut(),
    };
    let registered = unsafe { RegisterClassExW(&wnd_class) } != 0;
    if !registered && last_os().raw_os_error() != Some(1410) {
        tracing::warn!(target: "desk_pet", "简易会话框窗口类注册失败");
        return None;
    }

    let mut pet_rect = Rect::default();
    let has_pet = !pet.is_null() && unsafe { GetWindowRect(pet, &mut pet_rect) } != 0;
    let dpi = current_dpi(pet);
    let placement = chat_layout::place(
        layout_rect(work_area(pet)),
        has_pet.then(|| layout_rect(pet_rect)),
        dpi_scale(CHAT_W, dpi),
        dpi_scale(CHAT_H, dpi),
        dpi_scale(8, dpi),
        dpi_scale(120, dpi),
    );
    let p = placement.rect;
    let title = wide("简易会话");
    let window = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            p.left,
            p.top,
            p.right - p.left,
            p.bottom - p.top,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        )
    };
    if window.is_null() {
        tracing::warn!(target: "desk_pet", "简易会话框创建失败");
        return None;
    }
    let edit_class = wide("EDIT");
    let edit = unsafe {
        CreateWindowExW(
            WS_EX_CLIENTEDGE,
            edit_class.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_VSCROLL | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY,
            0,
            0,
            p.right - p.left,
            p.bottom - p.top,
            window,
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        )
    };
    if edit.is_null() {
        tracing::warn!(target: "desk_pet", "只读多行 EDIT 创建失败");
        unsafe { DestroyWindow(window) };
        return None;
    }
    let value = window as usize;
    with_native(|native| {
        native.chat = Some(value);
        native.edit = Some(edit as usize);
    });
    LAST_TOPMOST.store(false, Ordering::SeqCst);
    refresh_font(window);
    resize_edit(window);
    update_text();
    DIRTY.store(false, Ordering::SeqCst);
    tracing::info!(target: "desk_pet", x = p.left, y = p.top, side = ?placement.side, dpi, "简易会话框已打开（只读 EDIT / 可滚动 / 自动到底）");
    Some(value)
}

/// 每帧一次：按菜单状态开 / 关 / 独立置顶；有新历史时更新并滚到底。
pub fn poll() {
    let state = menu_state::get();
    let existing = with_native(|native| native.chat);
    match (state.chat_open, existing) {
        (true, None) => {
            let pet = with_native(|native| native.pet.map(hwnd)).unwrap_or(std::ptr::null_mut());
            create(pet);
        }
        (false, Some(value)) => {
            unsafe { DestroyWindow(hwnd(value)) };
        }
        _ => {}
    }
    let Some(value) = with_native(|native| native.chat) else {
        return;
    };
    if LAST_TOPMOST.swap(state.chat_topmost, Ordering::SeqCst) != state.chat_topmost {
        let after = if state.chat_topmost {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
        unsafe {
            let _ = SetWindowPos(
                hwnd(value),
                after,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        tracing::info!(target: "desk_pet", on = state.chat_topmost, "简易会话框置顶");
    }
    if DIRTY.swap(false, Ordering::SeqCst) {
        update_text();
    }
}
