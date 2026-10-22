//! 0.4.3 桌宠 demo：只把皮套透出来（`--desk-pet`，仅 Windows）。
//!
//! 双击（无参数）与 `--web` 行为**一字不变**：听端口、开启动器、不开宠物窗。
//! 只有 `--desk-pet` 会 arm 本模块；`web_api::start_server` 在 **bind 成功且
//! 已打印「已监听」之后**调 start_if_armed，于是 HTTP 服务照常跑、旁边多一扇
//! 无边框置顶、点击穿透的分层窗，只画皮套。
//!
//! # 为什么本文件平台无关
//!
//! Win32 全在 `window.rs`（`#[cfg(windows)] mod window;`）：**非 Windows 不编译
//! 窗口模块**，也不链接 user32/gdi32。留在本文件的是两件可纯函数化的事——
//! 窗口几何 window_rect 与预乘 fill_premultiplied_bgra——它们的单测在 Linux 上
//! 也真跑（窗口本体在真机之前没有任何自动化覆盖，所以能把这两块钉住就别放过）。
//!
//! # 失败语义（硬约束）
//!
//! 宠物窗是**旁路**：无桌面 / 无 GPU / 皮套包缺失 / Windows 调用失败，一律只
//! `tracing::error!(target: "desk_pet", …)`，**HTTP 服务继续、进程退出码不变**。
//! 窗口线程带 `catch_unwind` 兜底，且从不 unwrap 到服务线程上。
//!
//! **真实窗口未验**：本模块与 `window.rs` 只做过 Linux 单测 +
//! `cargo check --target x86_64-pc-windows-gnu`；没有在 Windows 上运行过，
//! 没有人看过像素。

// 非 Windows：纯函数只被单测使用（窗口模块根本不编译）。
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::atomic::{AtomicBool, Ordering};

use l2d::renderer::RgbaFrame;

/// Win32 裸绑定（类型 / 常量 / 结构体 / extern；只在 Windows 目标编译）。
#[cfg(windows)]
mod win32;
/// Win32 窗口策略（窗口类、DIB、UpdateLayeredWindow、帧循环；仅 Windows）。
#[cfg(windows)]
mod window;

/// 窗高 = 屏高 3/4（0.4.3 rc.2 的构图策略；真机嫌小就调这一个常量）。
pub const WINDOW_SCREEN_HEIGHT_RATIO: f32 = 0.75;
/// bai 画布 4000×6000 ⇒ 宽:高 = 2:3（与 `aspect_fit_transform` 配合，像素不拉伸）。
pub const CANVAS_ASPECT_W_OVER_H: f32 = 2.0 / 3.0;
/// 右下角留边的最小值（像素）。
pub const MARGIN_MIN_PX: i32 = 24;
/// 缩进屏幕时的最小边下限（只为避免 0/负尺寸，不参与构图）。
const MIN_SIDE_PX: i32 = 16;

/// 进程级开关：只有生产 `--desk-pet` 启动路径会打开它（与 browser / restart
/// 同一纪律——测试与库调用不 arm，否则跑测试也会去开窗）。
static ARMED: AtomicBool = AtomicBool::new(false);

/// 打开「监听成功后开宠物窗」（只在 `main` 的 `Command::DeskPet` 分支调一次）。
pub fn arm() {
    ARMED.store(true, Ordering::SeqCst);
}

/// 监听成功后开宠物窗；未 arm → 什么都不做。
///
/// 用 `swap(false)` 而不是 `load`：一扇窗只开一次（自重启的替身是**另一个
/// 进程**，本进程不该被重复调用开出第二扇窗）。
///
/// 线程名固定 `desk-pet`，线程体用 `catch_unwind` 包住：宠物窗失败（含
/// panic）只留一条 error 日志，**不碰 HTTP 线程、不改退出码**。
///
/// 非 Windows：`--desk-pet` 在解析期就被拒（退出码 2），这里没有任何窗口可开
/// ——只读一次 ARMED，让两侧共用同一份 arm 语义。
pub fn start_if_armed() {
    if !ARMED.swap(false, Ordering::SeqCst) {
        return;
    }
    spawn_pet_thread();
}

/// 起 `desk-pet` 线程（Windows）；非 Windows 是空实现——`--desk-pet` 在解析期
/// 就被拒（退出码 2），这里没有任何窗口可开，只留一条 debug 说明。
///
/// 抽成单独函数是为了让「只在 Windows 有事做」这件事对 clippy 也成立：
/// 非 Windows 的 `start_if_armed` 末尾不留下一个孤零零的 `return`。
fn spawn_pet_thread() {
    #[cfg(windows)]
    {
        let spawned = std::thread::Builder::new()
            .name("desk-pet".to_string())
            .spawn(|| {
                let outcome =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(window::run_once));
                match outcome {
                    Ok(Ok(())) => {
                        tracing::info!(target: "desk_pet", "宠物窗已关闭；HTTP 服务继续");
                    }
                    Ok(Err(e)) => {
                        tracing::error!(target: "desk_pet", "宠物窗未打开/已退出：{e}（HTTP 服务继续）");
                    }
                    Err(_) => {
                        tracing::error!(target: "desk_pet", "宠物窗线程 panic（已捕获；HTTP 服务继续）");
                    }
                }
            });
        if let Err(e) = spawned {
            tracing::error!(target: "desk_pet", "宠物窗线程起不来：{e}（HTTP 服务继续）");
        }
    }
    #[cfg(not(windows))]
    {
        tracing::debug!(target: "desk_pet", "非 Windows：没有宠物窗可开（--desk-pet 已在解析期被拒）");
    }
}

/// 窗口矩形 `(x, y, width, height)`（屏幕像素，左上角原点）。
///
/// 策略：窗高 = 屏高 3/4（WINDOW_SCREEN_HEIGHT_RATIO）；宽 = 高 × 2/3
/// （CANVAS_ASPECT_W_OVER_H，bai 画布 4000×6000）；贴**右下角**，留边
/// `max(24, 屏高/40)`；屏幕太小时整体等比缩到可用区域，**不越界、不出现
/// 负坐标或 0 尺寸**。
///
/// 纯函数（不读真实屏幕）：Windows 上由 `window::run_once` 用
/// `GetSystemMetrics(SM_CXSCREEN/SM_CYSCREEN)` 的结果喂进来。
pub fn window_rect(screen_w: i32, screen_h: i32) -> (i32, i32, i32, i32) {
    let sw = screen_w.max(1);
    let sh = screen_h.max(1);
    let margin = (sh / 40).max(MARGIN_MIN_PX);
    // 目标尺寸：屏高 3/4，宽按画布 2:3。
    let mut height = (sh as f32 * WINDOW_SCREEN_HEIGHT_RATIO).round() as i32;
    let mut width = (height as f32 * CANVAS_ASPECT_W_OVER_H).round() as i32;
    // 缩到「屏幕减两侧留边」以内（小屏/异常值也不越界），比例优先保宽。
    let max_w = (sw - 2 * margin).max(MIN_SIDE_PX);
    let max_h = (sh - 2 * margin).max(MIN_SIDE_PX);
    if width > max_w {
        let scale = max_w as f32 / width as f32;
        width = max_w;
        height = (height as f32 * scale).round() as i32;
    }
    if height > max_h {
        let scale = max_h as f32 / height as f32;
        height = max_h;
        width = (width as f32 * scale).round() as i32;
    }
    let width = width.max(1);
    let height = height.max(1);
    let x = (sw - width - margin).max(0);
    let y = (sh - height - margin).max(0);
    (x, y, width, height)
}

/// 直通 alpha 的 RGBA 帧 → **预乘 BGRA**（Windows 分层窗 AC_SRC_ALPHA 要求）。
///
/// - 目标 DIB 是 32bpp、**B 在最低字节**，所以这里换位：`out = [B*A, G*A, R*A, A]`；
/// - 预乘用 round-to-nearest `(c * a + 127) / 255`：a=255 时原值不变、a=0 时
///   三个分量全 0（半透明边缘不发灰）；
/// - out 必须与 `frame.pixels` 等长（`w*h*4`）：不等是编程错误，直接 assert
///   ——它发生在宠物窗线程内，被 start_if_armed 的 catch_unwind 收住，不会碰到
///   HTTP 线程。
pub fn fill_premultiplied_bgra(frame: &RgbaFrame, out: &mut [u8]) {
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
        let premultiply = |channel: u8| -> u8 { ((u32::from(channel) * alpha + 127) / 255) as u8 };
        dst[0] = premultiply(src[2]); // B
        dst[1] = premultiply(src[1]); // G
        dst[2] = premultiply(src[0]); // R
        dst[3] = src[3]; // A
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1080p：窗 540×810，右下留边 27（= max(24, 1080/40)）。
    #[test]
    fn window_rect_is_three_quarters_height_twothirds_width_at_bottom_right() {
        assert_eq!(window_rect(1920, 1080), (1353, 243, 540, 810));
        // 4K：1620 = 2160*3/4；宽 1080 = 1620*2/3；留边 54。
        assert_eq!(window_rect(3840, 2160), (2706, 486, 1080, 1620));
    }

    /// 小屏不能越界：尺寸缩进可用区域，坐标不为负，且整扇窗仍在屏幕内。
    #[test]
    fn window_rect_never_goes_negative_or_off_screen() {
        for (sw, sh) in [(800, 600), (320, 240), (100, 100), (1, 1), (0, 0)] {
            let (x, y, w, h) = window_rect(sw, sh);
            assert!(w >= 1 && h >= 1, "尺寸必须为正: {sw}x{sh} → {w}x{h}");
            assert!(x >= 0 && y >= 0, "坐标必须非负: {sw}x{sh} → ({x},{y})");
            assert!(
                x + w <= sw.max(1) && y + h <= sh.max(1),
                "整扇窗必须留在屏幕内: {sw}x{sh} → ({x},{y},{w},{h})"
            );
        }
    }

    /// 预乘：A=0 全 0、A=255 原值换位、A=128 逐通道 round-to-nearest，B 在最低字节。
    #[test]
    fn premultiply_is_bgra_round_to_nearest() {
        let frame = RgbaFrame {
            width: 3,
            height: 1,
            pixels: vec![
                255, 0, 0, 128, // R=255,A=128 → B/G=0, R=(255*128+127)/255=128
                10, 20, 30, 0, // 全透明 → 三个分量全 0
                10, 20, 30, 255, // 不透明 → BGRA 换位
            ],
        };
        let mut out = vec![0_u8; frame.pixels.len()];
        fill_premultiplied_bgra(&frame, &mut out);
        assert_eq!(&out[0..4], &[0, 0, 128, 128]);
        assert_eq!(&out[4..8], &[0, 0, 0, 0]);
        assert_eq!(&out[8..12], &[30, 20, 10, 255]);
    }

    /// 半透明的小值也要 round（(1*128+127)/255 = 1），不是截断成 0。
    #[test]
    fn premultiply_rounds_instead_of_truncating() {
        let frame = RgbaFrame {
            width: 1,
            height: 1,
            pixels: vec![1, 1, 1, 128],
        };
        let mut out = vec![0_u8; 4];
        fill_premultiplied_bgra(&frame, &mut out);
        assert_eq!(out, vec![1, 1, 1, 128]);
    }

    #[test]
    #[should_panic(expected = "预乘目标长度")]
    fn premultiply_rejects_wrong_length() {
        let frame = RgbaFrame {
            width: 1,
            height: 1,
            pixels: vec![1, 2, 3, 4],
        };
        let mut out = vec![0_u8; 3];
        fill_premultiplied_bgra(&frame, &mut out);
    }

    /// 非 Windows：arm + start_if_armed 立即返回，不起线程、不 panic
    /// （解析期已拒绝 `--desk-pet`，这条钉住「不启动任何东西」）。
    #[cfg(not(windows))]
    #[test]
    fn start_if_armed_is_a_noop_off_windows() {
        arm();
        start_if_armed();
        start_if_armed(); // swap 语义：第二次无需再做事
    }
}
