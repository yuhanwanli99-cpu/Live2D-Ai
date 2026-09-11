//! ksni（纯 Rust SNI）系统托盘：菜单回调**只**经 [`EventLoopProxy`] 发用户事件。
//!
//! 设计约束（RFC §4 批次 6 / 需求 3-4）：
//! - 所有 `Window::*` 调用留在事件循环线程；托盘线程/DBus 线程绝不直接触窗，
//!   回调里唯一的动作是 `proxy.send_event(PetUserEvent)`；
//! - 初始化失败（无 D-Bus / 无 StatusNotifierWatcher / 无 StatusNotifierHost）
//!   必须**可见降级且不阻塞启动**：看护线程 + 确认窗口超时，超时/失败都照常继续；
//! - 菜单模型构建是纯函数（[`build_menu_model`]），与 ksni 类型解耦、可单测；
//!   ksni 胶水层只做「模型 → 菜单项」的机械映射（[`menu`] 子模块）；
//! - 图标不依赖主题包：程序化生成 ARGB32 圆形像素（[`render_icon_argb`]，纯函数）。

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tracing::{info, warn};
use winit::event_loop::EventLoopProxy;

use crate::app_event::{AppEvent, send_app_event};
use crate::user_event::PetUserEvent;

pub mod menu;

pub use menu::{TraySnapshot, build_menu_model};

/// 托盘初始化确认等待窗口：超过即按「不可用」降级继续启动（绝不阻塞主流程）。
///
/// ksni 的 `spawn()` 在返回前会同步完成 D-Bus 连接 + Watcher 注册往返，
/// 正常桌面环境为毫秒级；该窗口只兜底异常总线（挂起的 socket 等）。
pub const TRAY_CONFIRM_WAIT: Duration = Duration::from_millis(1500);

/// 托盘共享簿记：事件循环线程写，ksni 服务线程读（菜单渲染时取快照）。
#[derive(Debug, Default)]
pub struct TraySharedState {
    /// 当前是否点击穿透。
    click_through: std::sync::atomic::AtomicBool,
    /// 当前是否置顶。
    always_on_top: std::sync::atomic::AtomicBool,
    /// 当前窗口是否可见。
    visible: std::sync::atomic::AtomicBool,
}

impl TraySharedState {
    pub fn set_click_through(&self, value: bool) {
        self.click_through
            .store(value, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_always_on_top(&self, value: bool) {
        self.always_on_top
            .store(value, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn set_visible(&self, value: bool) {
        self.visible
            .store(value, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> TraySnapshot {
        TraySnapshot {
            click_through: self
                .click_through
                .load(std::sync::atomic::Ordering::Relaxed),
            always_on_top: self
                .always_on_top
                .load(std::sync::atomic::Ordering::Relaxed),
            visible: self.visible.load(std::sync::atomic::Ordering::Relaxed),
        }
    }
}

// ---------------------------------------------------------------------- 图标

/// 程序化生成 ARGB32（网络字节序）圆形桌宠图标（纯函数，零资产依赖）。
///
/// 形状：实心圆盘 + 同心描边；圆外全透明。尺寸任意（SNI host 会自行缩放）。
pub fn render_icon_argb(size: u32) -> Vec<u8> {
    // 主色（Bai 系粉色）与描边色。
    const FILL: [u8; 3] = [0xFF, 0x9E, 0xB4]; // RGB
    const EDGE: [u8; 3] = [0xE5, 0x6B, 0x8C];
    let n = size.max(1) as usize;
    let mut data = Vec::with_capacity(n * n * 4);
    let center = (n as f64 - 1.0) / 2.0;
    let radius = n as f64 * 0.42;
    let inner_edge = radius * 0.80;
    for y in 0..n {
        for x in 0..n {
            let dx = x as f64 - center;
            let dy = y as f64 - center;
            let dist = dx.hypot(dy);
            let (rgb, alpha) = if dist <= radius {
                if dist > inner_edge {
                    (EDGE, 255u8)
                } else {
                    (FILL, 255u8)
                }
            } else {
                (FILL, 0u8)
            };
            // ARGB32 网络字节序：A, R, G, B。
            data.extend_from_slice(&[alpha, rgb[0], rgb[1], rgb[2]]);
        }
    }
    data
}

// ------------------------------------------------------------------- ksni 胶水

/// 实现 [`ksni::Tray`] 的托盘本体；所有回调只发 [`PetUserEvent`]。
struct PetTray {
    state: Arc<TraySharedState>,
    proxy: EventLoopProxy<AppEvent>,
}

impl PetTray {
    fn send(&self, event: PetUserEvent) {
        // 事件循环已退出时 send 失败：静默即可（进程正在收尾）。
        send_app_event(&self.proxy, AppEvent::Tray(event));
    }

    /// 把纯模型映射成 ksni 菜单（唯一允许出现 ksni 类型的位置之一）。
    fn map_entry(
        entry: &menu::MenuEntrySpec,
        proxy: &EventLoopProxy<AppEvent>,
    ) -> ksni::MenuItem<Self> {
        let proxy = proxy.clone();
        let event = entry.event;
        match entry.checked {
            Some(checked) => ksni::menu::CheckmarkItem {
                label: entry.label.clone(),
                checked,
                enabled: entry.enabled,
                activate: Box::new(move |_| {
                    send_app_event(&proxy, AppEvent::Tray(event));
                }),
                ..Default::default()
            }
            .into(),
            None => ksni::menu::StandardItem {
                label: entry.label.clone(),
                enabled: entry.enabled,
                icon_name: if entry.event == PetUserEvent::TrayExit {
                    "application-exit".to_string()
                } else {
                    String::new()
                },
                activate: Box::new(move |_| {
                    send_app_event(&proxy, AppEvent::Tray(event));
                }),
                ..Default::default()
            }
            .into(),
        }
    }
}

impl ksni::Tray for PetTray {
    fn id(&self) -> String {
        "live2d-ai-desktop".to_string()
    }

    fn title(&self) -> String {
        "Live2D-Ai 桌宠".to_string()
    }

    fn icon_name(&self) -> String {
        // 主题图标仅作 host 兜底；真实图标走 icon_pixmap（不依赖主题包）。
        "face-smile".to_string()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        const SIZE: u32 = 48;
        vec![ksni::Icon {
            width: SIZE as i32,
            height: SIZE as i32,
            data: render_icon_argb(SIZE),
        }]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: self.icon_name(),
            icon_pixmap: Vec::new(),
            title: "Live2D-Ai 桌宠".to_string(),
            description: "菜单：点击穿透 / 置顶 / 显隐 / 退出".to_string(),
        }
    }

    /// 托盘图标左键激活 = 显示/隐藏切换（隐藏后的主要恢复入口之一）。
    fn activate(&mut self, _x: i32, _y: i32) {
        let visible = self.state.snapshot().visible;
        self.send(PetUserEvent::TraySetVisible(!visible));
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        build_menu_model(self.state.snapshot())
            .iter()
            .map(|entry| Self::map_entry(entry, &self.proxy))
            .collect()
    }
}

// ---------------------------------------------------------------- 启动编排

/// ksni 错误 → 可读降级说明（纯函数；文案进日志/stderr，供用户可见降级）。
pub fn classify_ksni_error(err: &ksni::Error) -> String {
    // ksni::Error 标记为 non_exhaustive：必须保留通配分支。
    match err {
        ksni::Error::Dbus(e) => {
            format!("无可用 D-Bus session bus（无桌面会话或沙箱未配置总线）: {e}")
        }
        ksni::Error::Watcher(e) => {
            format!("StatusNotifierWatcher 不可达（当前桌面环境不支持 SNI 托盘或扩展未启用）: {e}")
        }
        ksni::Error::WontShow => concat!(
            "托盘已注册但无 StatusNotifierHost 显示它",
            "（如 GNOME 未启用 AppIndicator 扩展）——图标不会出现"
        )
        .to_string(),
        other => format!("未知托盘错误（ksni 新变体？）: {other:?}"),
    }
}

/// 托盘运行时句柄：无论初始化成败都存在，供事件循环刷新菜单与退出清理。
pub struct TrayRuntime {
    /// 已确认成功 spawn 的句柄集合（含「确认窗口超时后才迟到就绪」的句柄）。
    handles: Arc<Mutex<Vec<ksni::blocking::Handle<PetTray>>>>,
    /// 共享状态（事件循环线程在每次窗口态变更后写入）。
    state: Option<Arc<TraySharedState>>,
    /// 初始化结论说明：None = 已确认就绪；Some = 可见降级原因。
    pub degradation_note: Option<String>,
}

impl std::fmt::Debug for TrayRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayRuntime")
            .field("handles_confirmed", &self.handles.lock().map(|h| h.len()))
            .field("state", &self.state.is_some())
            .field("degradation_note", &self.degradation_note)
            .finish()
    }
}

impl TrayRuntime {
    /// 托盘是否已确认就绪（pet-mode 自动穿透的前置条件）。
    pub fn is_ready(&self) -> bool {
        self.degradation_note.is_none()
    }

    /// 共享状态访问（供事件循环线程同步布尔态）。
    pub fn state(&self) -> Option<&Arc<TraySharedState>> {
        self.state.as_ref()
    }

    /// 通知 SNI host 重取菜单/属性（窗口态变化后调用；失败静默）。
    pub fn refresh(&self) {
        if let Ok(handles) = self.handles.lock() {
            for handle in handles.iter() {
                handle.update(|_tray: &mut PetTray| {});
            }
        }
    }

    /// 关闭全部托盘服务（事件循环 exiting 时调用；幂等）。
    pub fn shutdown(&self) {
        if let Ok(mut handles) = self.handles.lock() {
            for handle in handles.drain(..) {
                handle.shutdown().wait();
            }
        }
    }
}

/// 尝试启动托盘并等待确认窗口；**永不阻塞超过 `wait`**，失败/超时均可见降级。
pub fn spawn_pet_tray(proxy: EventLoopProxy<AppEvent>, wait: Duration) -> TrayRuntime {
    let handles: Arc<Mutex<Vec<ksni::blocking::Handle<PetTray>>>> =
        Arc::new(Mutex::new(Vec::new()));
    // 共享簿记在主线程创建：无论确认是否迟到，运行时都持有同一份状态。
    let state = Arc::new(TraySharedState::default());
    let (result_tx, result_rx) = std::sync::mpsc::channel::<Result<(), String>>();

    let handles_for_thread = Arc::clone(&handles);
    let state_for_thread = Arc::clone(&state);
    let spawn_result = thread::Builder::new()
        .name("l2d-tray-spawn".to_string())
        .spawn(move || {
            let tray = PetTray {
                state: state_for_thread,
                proxy,
            };
            // ksni 内部偶发 panic（异常总线环境）也不许带崩主进程：
            // catch_unwind 后归一为失败字符串。
            let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                use ksni::blocking::TrayMethods as _;
                tray.spawn()
            }));
            match attempt {
                Ok(Ok(handle)) => {
                    // 先入集合再报喜：即使确认窗口已过（迟到就绪），句柄仍可被
                    // refresh/shutdown 管理，服务不会泄漏。
                    if let Ok(mut slot) = handles_for_thread.lock() {
                        slot.push(handle);
                    }
                    let _ = result_tx.send(Ok(()));
                }
                Ok(Err(e)) => {
                    let _ = result_tx.send(Err(classify_ksni_error(&e)));
                }
                Err(payload) => {
                    let msg = payload
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| {
                            payload
                                .downcast_ref::<&'static str>()
                                .map(|s| (*s).to_string())
                        })
                        .unwrap_or_else(|| "<non-string panic>".to_string());
                    let _ = result_tx.send(Err(format!("托盘线程 panic（已隔离）: {msg}")));
                }
            }
        });

    let mut runtime = TrayRuntime {
        handles,
        state: Some(state),
        degradation_note: None,
    };

    match spawn_result {
        Err(e) => {
            // 线程都起不来（极端环境）：可见降级，绝不阻塞启动。
            runtime.degradation_note = Some(format!("托盘线程创建失败: {e}"));
            warn!(reason = %runtime.degradation_note.as_deref().unwrap_or(""), "tray 不可用");
        }
        Ok(_join) => match result_rx.recv_timeout(wait) {
            Ok(Ok(())) => {
                info!("托盘就绪（StatusNotifierItem 已注册）");
            }
            Ok(Err(reason)) => {
                runtime.degradation_note = Some(reason.clone());
                warn!(%reason, "tray 初始化失败（可见降级，不影响启动）");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                let note = format!(
                    "托盘初始化超过 {wait:.1?} 未确认（D-Bus 总线可能挂起）——按不可用降级，\
                     迟到就绪的实例仍会被接管"
                );
                runtime.degradation_note = Some(note.clone());
                warn!("{note}");
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                let note = "托盘线程意外提前退出（无确认结果）".to_string();
                runtime.degradation_note = Some(note.clone());
                warn!("{note}");
            }
        },
    }
    runtime
}

#[cfg(test)]
mod tests {
    use super::{TraySharedState, TraySnapshot, classify_ksni_error, render_icon_argb};

    #[test]
    fn shared_state_snapshot_round_trip() {
        // Default = 全 false；经 setter 修改后快照必须如实反映。
        let state = TraySharedState::default();
        assert_eq!(
            state.snapshot(),
            TraySnapshot {
                click_through: false,
                always_on_top: false,
                visible: false
            }
        );
        state.set_click_through(true);
        state.set_always_on_top(true);
        state.set_visible(true);
        assert_eq!(
            state.snapshot(),
            TraySnapshot {
                click_through: true,
                always_on_top: true,
                visible: true
            }
        );
        state.set_always_on_top(false);
        assert!(!state.snapshot().always_on_top);
    }

    #[test]
    fn menu_model_reflects_state_changes() {
        let state = TraySharedState::default();
        let before = super::build_menu_model(state.snapshot());
        assert_eq!(before[0].checked, Some(false));
        state.set_click_through(true);
        let after = super::build_menu_model(state.snapshot());
        assert_eq!(after[0].checked, Some(true));
    }

    #[test]
    fn icon_renderer_shape_and_size() {
        let size = 24;
        let data = render_icon_argb(size);
        assert_eq!(data.len(), (size * size * 4) as usize);
        let px = |x: u32, y: u32| -> [u8; 4] {
            let off = ((y * size + x) * 4) as usize;
            [data[off], data[off + 1], data[off + 2], data[off + 3]]
        };
        // 四角透明、中心不透明（ARGB 首字节是 A）。
        for (x, y) in [(0u32, 0u32), (23, 0), (0, 23), (23, 23)] {
            assert_eq!(px(x, y)[0], 0, "corner ({x},{y}) 应透明");
        }
        let center = px(12, 12);
        assert_eq!(center[0], 255);
        // 描边环采样：半径 ~0.42*size 处应为描边色。
        let edge = px(2, 12);
        assert_eq!(edge[0], 255);
        assert_eq!([edge[1], edge[2], edge[3]], [0xE5, 0x6B, 0x8C]);
        // size=0 也安全（钳到 1 像素）。
        assert_eq!(render_icon_argb(0).len(), 4);
    }

    #[test]
    fn ksni_error_classification_is_user_visible_chinese_reasons() {
        // WontShow 无载荷可直接构造（GNOME 缺 AppIndicator 扩展的典型场景）。
        let wont_show = classify_ksni_error(&ksni::Error::WontShow);
        assert!(wont_show.contains("StatusNotifierHost"), "{wont_show}");
        // D-Bus/Watcher 分支需要构造 zbus 错误类型，本 crate 不直接依赖 zbus，
        // 其文案由真实运行日志验证（见 verification 记录）。
    }
}
