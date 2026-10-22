//! 宠物窗本体（0.4.3 rc.2）：Win32 分层窗 + 离屏一帧皮套。
//!
//! 只在 **Windows 目标**编译（父模块 `#[cfg(windows)] mod window;`）；非 Windows
//! 不编译本文件，也不链接 user32/gdi32。裸绑定全在 `win32.rs`（本文件只写策略）。
//!
//! # 画法
//!
//! - 皮套由进程内 `OffscreenRenderer` 渲染（`render_frame` 返回**直通 alpha**
//!   的 `RgbaFrame`），经父模块的 `fill_premultiplied_bgra` 预乘后写进 32bpp、
//!   **顶朝下**的 DIB，再用 `UpdateLayeredWindow`（`ULW_ALPHA` +
//!   `AC_SRC_ALPHA`）交给 DWM；
//! - **不进 WebView、不碰 /render、不改 wasm 背景、不改口型**：WebGPU 的 surface
//!   只能 Opaque 合成（`crates/l2d-wasm-demo/src/web/surface/background.rs`），
//!   「让 canvas 透明」这条路走不通；
//! - 离屏清屏**本来就是** `LoadOp::Clear(Color::TRANSPARENT)`（
//!   `crates/l2d/src/renderer/model_core.rs` 的 `render_to_view_submit`，209-219
//!   行），读回还经 `unpremultiply(clamp=true)` 转直通 alpha ⇒ **l2d 一行未改**。
//!
//! # 失败语义
//!
//! 任何一步失败（无桌面 / 无 GPU / 皮套包缺失 / Win32 调用失败）都返回
//! `Err(String)`；调用方（`desk_pet::start_if_armed` 的线程体）只记
//! `tracing::error!`，**HTTP 服务继续、进程退出码不变**。窗口被外部关掉
//! （`WM_DESTROY`）时线程正常结束。
//!
//! **真实窗口未验**：只做过 `cargo check --target x86_64-pc-windows-gnu`；没有
//! 在 Windows 上运行过，没有人看过像素。
#![allow(unsafe_code)]

use std::ffi::c_void;
use std::time::{Duration, Instant};

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{FIXED_DT_60HZ, OffscreenRenderer, RgbaFrame};

use super::win32::*;
use super::window_rect;

/// 分层 + 点击穿透 + 置顶 + 不进 Alt+Tab + 不抢焦点（= 0x080800A8）。
const EX_STYLE: u32 =
    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
/// 60 Hz 节拍（16.667 ms）。
const TICK: Duration = Duration::from_micros(16_667);
/// 一轮循环最多补几步：慢机不追帧，但待机速度仍接近真实时间。
const MAX_STEPS_PER_FRAME: u32 = 3;
/// 单轮最多吸收的挂起时间（避免长阻塞后一次补几十步）。
const MAX_CATCH_UP: Duration = Duration::from_millis(100);

/// 窗口过程：只把 `WM_DESTROY` 变成「退出消息循环」，其余交回
/// `DefWindowProcW`（本窗没有标题栏/菜单，不需要处理别的消息）。
unsafe extern "system" fn wnd_proc(
    hwnd: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    match message {
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        WM_CLOSE => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

/// 一扇宠物窗持有的原生资源；`Drop` 按反序释放。
struct PetWindow {
    hwnd: Hwnd,
    mem_dc: Hdc,
    bitmap: Hbitmap,
    old_bitmap: Handle,
    /// DIB 像素首地址（顶朝下 32bpp BGRA），可用长度 `width*height*4`。
    bits: *mut u8,
    width: i32,
    height: i32,
    origin: Point,
}

impl PetWindow {
    /// 注册窗口类 → 建窗（`WS_POPUP|WS_VISIBLE` + 全套 ex-style）→ 建 DIB。
    fn create(width: i32, height: i32, origin: Point) -> Result<Self, String> {
        let class_name = wide(CLASS_NAME);
        let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
        if instance.is_null() {
            return Err(format!("GetModuleHandleW 失败: {}", last_os()));
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
            hbr_background: std::ptr::null_mut(),
            lpsz_menu_name: std::ptr::null(),
            lpsz_class_name: class_name.as_ptr(),
            h_icon_sm: std::ptr::null_mut(),
        };
        if unsafe { RegisterClassExW(&wnd_class) } == 0 {
            return Err(format!("RegisterClassExW 失败: {}", last_os()));
        }
        let hwnd = unsafe {
            CreateWindowExW(
                EX_STYLE,
                class_name.as_ptr(),
                std::ptr::null(),
                WS_POPUP | WS_VISIBLE,
                origin.x,
                origin.y,
                width,
                height,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err(format!("CreateWindowExW 失败: {}", last_os()));
        }
        // 屏幕 DC 只用来建内存 DC / DIB，建完立刻还回去（不长期持有）。
        let screen_dc = unsafe { GetDC(std::ptr::null_mut()) };
        if screen_dc.is_null() {
            unsafe {
                DestroyWindow(hwnd);
            }
            return Err(format!("GetDC(NULL) 失败: {}", last_os()));
        }
        let mem_dc = unsafe { CreateCompatibleDC(screen_dc) };
        if mem_dc.is_null() {
            unsafe {
                ReleaseDC(std::ptr::null_mut(), screen_dc);
                DestroyWindow(hwnd);
            }
            return Err(format!("CreateCompatibleDC 失败: {}", last_os()));
        }
        let info = BitmapInfo {
            header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: width,
                // **负高度 = 顶朝下**：第 0 行就是屏幕最上面一行，与 RgbaFrame 行序一致。
                bi_height: -height,
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: BI_RGB,
                bi_size_image: (width * height * 4) as u32,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            colors: [0],
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = unsafe {
            CreateDIBSection(
                screen_dc,
                &info,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            )
        };
        unsafe { ReleaseDC(std::ptr::null_mut(), screen_dc) };
        if bitmap.is_null() || bits.is_null() {
            let err = last_os();
            unsafe {
                if !bitmap.is_null() {
                    let _ = DeleteObject(bitmap);
                }
                let _ = DeleteDC(mem_dc);
                let _ = DestroyWindow(hwnd);
            }
            return Err(format!("CreateDIBSection 失败: {err}"));
        }
        let old_bitmap = unsafe { SelectObject(mem_dc, bitmap) };
        Ok(Self {
            hwnd,
            mem_dc,
            bitmap,
            old_bitmap,
            bits: bits.cast::<u8>(),
            width,
            height,
            origin,
        })
    }

    /// 一帧：预乘写进 DIB，再 `UpdateLayeredWindow` 交给 DWM。
    fn present(&mut self, frame: &RgbaFrame) -> Result<(), String> {
        if frame.width != self.width as u32 || frame.height != self.height as u32 {
            return Err(format!(
                "帧尺寸 {}x{} 与窗 {}x{} 不一致",
                frame.width, frame.height, self.width, self.height
            ));
        }
        let len = (self.width as usize) * (self.height as usize) * 4;
        // SAFETY: bits 来自 CreateDIBSection，长度 = width*height*4（上面刚校验过帧
        // 尺寸），且这块 DIB 只有本结构这一个写入者。
        let pixels = unsafe { std::slice::from_raw_parts_mut(self.bits, len) };
        super::fill_premultiplied_bgra(frame, pixels);
        let size = Size {
            cx: self.width,
            cy: self.height,
        };
        let src = Point { x: 0, y: 0 };
        let blend = BlendFunction {
            blend_op: AC_SRC_OVER,
            blend_flags: 0,
            source_constant_alpha: 255, // 每像素 alpha 走 DIB，不再整体淡出
            alpha_format: AC_SRC_ALPHA, // 源是预乘 alpha
        };
        let screen_dc = unsafe { GetDC(std::ptr::null_mut()) };
        if screen_dc.is_null() {
            return Err(format!("GetDC(NULL) 失败: {}", last_os()));
        }
        // pptDst = 窗左上角在屏幕上的位置；psize = 源尺寸（与 DIB 同尺寸，不缩放）；
        // pptSrc = 源位图左上角。ULW_ALPHA + AC_SRC_ALPHA ⇒ 按 DIB 的预乘 alpha 混合。
        let ok = unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                screen_dc,
                &self.origin,
                &size,
                self.mem_dc,
                &src,
                0,
                &blend,
                ULW_ALPHA,
            )
        };
        let err = if ok == 0 { Some(last_os()) } else { None };
        unsafe { ReleaseDC(std::ptr::null_mut(), screen_dc) };
        match err {
            Some(e) => Err(format!("UpdateLayeredWindow 失败: {e}")),
            None => Ok(()),
        }
    }
}

impl Drop for PetWindow {
    fn drop(&mut self) {
        // 反序释放：位图先还出 DC 再删；窗口最后销毁（幂等，已被外部关掉时返回 0）。
        unsafe {
            SelectObject(self.mem_dc, self.old_bitmap);
            let _ = DeleteObject(self.bitmap);
            let _ = DeleteDC(self.mem_dc);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// 非阻塞抽干消息队列；返回 `false` 表示收到 `WM_QUIT`（该退出了）。
///
/// `PeekMessageW` 的返回值是 BOOL：0 = 队列空，-1 = 出错（不是「有消息」）。
fn pump_messages() -> Result<bool, String> {
    loop {
        let mut msg: Msg = unsafe { std::mem::zeroed() };
        let has = unsafe { PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) };
        if has == 0 {
            return Ok(true);
        }
        if has < 0 {
            return Err(format!("PeekMessageW 失败: {}", last_os()));
        }
        if msg.message == WM_QUIT {
            return Ok(false);
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// 开窗、跑帧循环，直到窗口被关闭或出错。
///
/// 返回 `Ok(())` = 窗口被关掉（线程正常结束）；`Err` = 没开起来或中途
/// 失败（调用方只记 error，HTTP 服务继续）。
pub fn run_once() -> Result<(), String> {
    // 1) 屏幕几何 → 窗矩形（纯函数在父模块，Linux 单测钉住）。
    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let screen_h = unsafe { GetSystemMetrics(SM_CYSCREEN) };
    if screen_w <= 0 || screen_h <= 0 {
        return Err(format!(
            "拿不到屏幕尺寸（{screen_w}x{screen_h}）：没有可用桌面"
        ));
    }
    let (x, y, width, height) = window_rect(screen_w, screen_h);
    tracing::info!(
        target: "desk_pet",
        screen_w,
        screen_h,
        x,
        y,
        width,
        height,
        "宠物窗布局（真实窗口未验）"
    );

    // 2) 离屏渲染器 + 皮套包（沿用「唯一模型根」：cwd / exe 旁的 assets/models/bai）。
    let mut renderer = OffscreenRenderer::new(width as u32, height as u32)
        .map_err(|e| format!("离屏渲染器创建失败: {e}"))?;
    let model3 = crate::web_api::model_root::model_root()
        .join(crate::web_api::model_root::builtin_model3_rel_path());
    let package = ModelPackage::load(&model3)
        .map_err(|e| format!("加载皮套包失败（{}）: {e}", model3.display()))?;
    let loaded = LoadedModel::resolve(package).map_err(|e| format!("解析皮套失败: {e}"))?;
    renderer
        .load_model(&loaded)
        .map_err(|e| format!("装载皮套失败: {e}"))?;
    tracing::info!(
        target: "desk_pet",
        model = %model3.display(),
        adapter = renderer.gpu_adapter(),
        "宠物窗离屏渲染器就绪（真实窗口未验）"
    );

    // 3) 首帧先渲染、再开窗：避免「窗先出来、内容还空着」的那一小段。
    renderer
        .update(FIXED_DT_60HZ)
        .map_err(|e| format!("首帧 update 失败: {e}"))?;
    let first = renderer
        .render_frame()
        .map_err(|e| format!("首帧渲染失败: {e}"))?;
    let coverage = first.alpha_coverage();
    tracing::info!(
        target: "desk_pet",
        adapter = renderer.gpu_adapter(),
        width = first.width,
        height = first.height,
        coverage,
        bbox = ?first.visible_bbox(),
        "首帧读回（直通 alpha；真实窗口未验）"
    );
    if coverage >= 0.999 {
        tracing::warn!(
            target: "desk_pet",
            coverage,
            "首帧几乎整帧不透明：分层窗会显示一整块矩形。未改 l2d 离屏清屏（model_core 本来就是 Clear(TRANSPARENT)）；先按未验记录，等真机确认"
        );
    }

    // 4) 开窗 + 第一帧上屏 + 显示（不抢焦点）。
    let mut window = PetWindow::create(width, height, Point { x, y })?;
    window.present(&first)?;
    unsafe {
        ShowWindow(window.hwnd, SW_SHOWNOACTIVATE);
    }
    tracing::info!(
        target: "desk_pet",
        "宠物窗已创建（无边框 / 置顶 / 点击穿透 / 不进 Alt+Tab；真实窗口未验）"
    );

    // 5) 帧循环：非阻塞抽消息 + 时间累加器补 1/60 步 + 16.67ms 节拍。
    let mut next_tick = Instant::now() + TICK;
    let mut last = Instant::now();
    let mut accumulated = Duration::ZERO;
    let step = Duration::from_secs_f32(FIXED_DT_60HZ);
    let mut frames: u64 = 0;
    let mut render_time = Duration::ZERO;
    loop {
        if !pump_messages()? {
            break; // WM_QUIT：窗口已销毁
        }
        let now = Instant::now();
        accumulated += (now - last).min(MAX_CATCH_UP);
        last = now;
        let mut steps = 0;
        while accumulated >= step && steps < MAX_STEPS_PER_FRAME {
            renderer
                .update(FIXED_DT_60HZ)
                .map_err(|e| format!("update 失败: {e}"))?;
            accumulated -= step;
            steps += 1;
        }
        if steps > 0 {
            let started = Instant::now();
            let frame = renderer
                .render_frame()
                .map_err(|e| format!("渲染失败: {e}"))?;
            window.present(&frame)?;
            render_time += started.elapsed();
            frames += 1;
        }
        // 节拍：睡到下一个刻度；落后就重排基准（不补帧、不忙等）。
        let now = Instant::now();
        if next_tick > now {
            std::thread::sleep(next_tick - now);
            next_tick += TICK;
        } else {
            next_tick = now + TICK;
        }
    }
    if frames > 0 {
        let avg_ms = render_time.as_secs_f64() * 1000.0 / frames as f64;
        tracing::info!(target: "desk_pet", frames, avg_ms, "宠物窗渲染统计（真实窗口未验）");
    }
    Ok(())
}
