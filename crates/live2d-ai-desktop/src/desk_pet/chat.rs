//! 简易会话框：角色旁边一块**纯 Win32 文本小窗**（不加载 Flutter / WebView）。
//!
//! - 最近几轮「我 / AI」的句子，来自 `bridge` 的 `PetEvent`，每帧追加；
//! - `WS_OVERLAPPEDWINDOW` 自带 `WS_THICKFRAME`，**拖边缩放由系统处理**；
//! - 置顶只作用在这块小窗 HWND 上，与角色窗的置顶各存各的、互不影响；
//! - 窗口过程独立于宠物窗：`WM_DESTROY` **不** `PostQuitMessage`（否则会把
//!   宠物窗一起关掉）；用户点 X 只把菜单状态改回「关」。
//!
//! 全部失败语义与宠物窗同款：建窗/绘制失败只记 `desk_pet` 日志，窗继续跑。

#![allow(unsafe_code)]

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::menu_state;
use super::win32::*;

/// 最多留几轮（一轮 = 用户句 + AI 句，所以 12 行约 6 轮）。
const MAX_LINES: usize = 12;
const CHAT_W: i32 = 360;
const CHAT_H: i32 = 260;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Line {
    who: &'static str,
    text: String,
}

static LINES: Mutex<Vec<Line>> = Mutex::new(Vec::new());
static PET_HWND: Mutex<Option<usize>> = Mutex::new(None);
static CHAT_HWND: Mutex<Option<usize>> = Mutex::new(None);
static DIRTY: AtomicBool = AtomicBool::new(false);
static LAST_TOPMOST: AtomicBool = AtomicBool::new(false);

fn hwnd(value: usize) -> Hwnd {
    value as Hwnd
}

fn with_chat<R>(f: impl FnOnce(&mut Option<usize>) -> R) -> R {
    match CHAT_HWND.lock() {
        Ok(mut guard) => f(&mut guard),
        Err(poisoned) => f(&mut poisoned.into_inner()),
    }
}

pub fn set_pet_hwnd(h: Hwnd) {
    if let Ok(mut slot) = PET_HWND.lock() {
        *slot = Some(h as usize);
    }
}

/// 追加一句；超出上限丢最旧的。空串不进（气泡也不用画空句）。
pub fn push(who: &'static str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    if let Ok(mut lines) = LINES.lock() {
        lines.push(Line {
            who,
            text: super::bubble::truncate(text),
        });
        let len = lines.len();
        if len > MAX_LINES {
            lines.drain(..len - MAX_LINES);
        }
    }
    DIRTY.store(true, Ordering::SeqCst);
}

/// 给 `WM_PAINT` 用的纯文本（也方便单测钉住「最近几轮」）。
#[must_use]
pub fn rendered_text() -> String {
    let lines = LINES.lock().map(|g| g.clone()).unwrap_or_default();
    if lines.is_empty() {
        return "（还没有对话）".to_string();
    }
    lines
        .iter()
        .map(|line| format!("{}：{}", line.who, line.text))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 角色窗关掉时：销毁这块小窗、同步菜单状态、清掉 HWND 引用。
pub fn close() {
    let existing = with_chat(Option::take);
    if let Some(value) = existing {
        unsafe { DestroyWindow(hwnd(value)) };
    }
    if let Ok(mut slot) = PET_HWND.lock() {
        *slot = None;
    }
    menu_state::close_chat();
    DIRTY.store(false, Ordering::SeqCst);
}

unsafe extern "system" fn wnd_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    match message {
        // 注意：**不能** PostQuitMessage —— 宠物窗与这块小窗共用同一个消息循环。
        WM_DESTROY => {
            with_chat(|slot| *slot = None);
            0
        }
        WM_CLOSE => {
            menu_state::close_chat();
            unsafe { DestroyWindow(hwnd) };
            0
        }
        WM_PAINT => {
            unsafe { paint(hwnd) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// `WM_PAINT`：GDI 画字，背景由窗口类的 `COLOR_WINDOW+1` 伪刷子擦。
unsafe fn paint(hwnd: Hwnd) {
    let mut ps: PaintStruct = unsafe { std::mem::zeroed() };
    let dc = unsafe { BeginPaint(hwnd, &mut ps) };
    if dc.is_null() {
        return;
    }
    let mut client = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetClientRect(hwnd, &mut client) };
    let mut bounds = Rect {
        left: client.left + 12,
        top: client.top + 10,
        right: (client.right - 12).max(client.left + 13),
        bottom: (client.bottom - 10).max(client.top + 11),
    };
    let face = wide("Microsoft YaHei");
    let font = unsafe {
        CreateFontW(
            -16,
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
    };
    let text = wide(&rendered_text());
    unsafe {
        let old_font = SelectObject(dc, font);
        SetTextColor(dc, 0x0022_2228); // COLORREF 是 0x00BBGGRR，深色字
        SetBkMode(dc, TRANSPARENT);
        DrawTextW(
            dc,
            text.as_ptr(),
            -1,
            &mut bounds,
            DT_LEFT | DT_TOP | DT_WORDBREAK | DT_NOPREFIX,
        );
        SelectObject(dc, old_font);
        DeleteObject(font);
        EndPaint(hwnd, &ps);
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
    // 同一进程里关掉再开：类已注册（1410）不是错误。
    let registered = unsafe { RegisterClassExW(&wnd_class) } != 0;
    if !registered && last_os().raw_os_error() != Some(1410) {
        tracing::warn!(target: "desk_pet", "简易会话框窗口类注册失败");
        return None;
    }
    let mut r = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let (x, y) = if !pet.is_null() && unsafe { GetWindowRect(pet, &mut r) } != 0 {
        (r.right + 8, r.top + 120)
    } else {
        (120, 120)
    };
    let title = wide("简易会话");
    let h = unsafe {
        CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            x,
            y,
            CHAT_W,
            CHAT_H,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        )
    };
    if h.is_null() {
        tracing::warn!(target: "desk_pet", "简易会话框创建失败");
        return None;
    }
    let value = h as usize;
    with_chat(|slot| *slot = Some(value));
    LAST_TOPMOST.store(false, Ordering::SeqCst);
    DIRTY.store(true, Ordering::SeqCst);
    tracing::info!(target: "desk_pet", x, y, "简易会话框已打开（可拖边缩放）");
    Some(value)
}

/// 每帧一次（由 `companion.tick` 调）：按菜单状态开 / 关 / 置顶 / 重绘。
pub fn poll() {
    let state = menu_state::get();
    let existing = with_chat(|slot| *slot);
    match (state.chat_open, existing) {
        (true, None) => {
            let pet = PET_HWND
                .lock()
                .ok()
                .and_then(|slot| *slot)
                .map(hwnd)
                .unwrap_or(std::ptr::null_mut());
            create(pet);
        }
        (false, Some(value)) => {
            unsafe { DestroyWindow(hwnd(value)) };
            with_chat(|slot| *slot = None);
        }
        _ => {}
    }
    let Some(value) = with_chat(|slot| *slot) else {
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
        unsafe { InvalidateRect(hwnd(value), std::ptr::null(), 1) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_keeps_only_the_last_lines() {
        if let Ok(mut lines) = LINES.lock() {
            lines.clear();
        }
        push("我", "");
        assert_eq!(rendered_text(), "（还没有对话）");
        for i in 0..(MAX_LINES + 5) {
            push("我", &format!("第 {i} 句"));
        }
        let text = rendered_text();
        assert!(text.contains(&format!("第 {} 句", MAX_LINES + 4)));
        assert!(!text.contains("第 0 句"));
        assert_eq!(text.lines().count(), MAX_LINES);
    }
}
