//! 运行时/声明性能力数据：会话线索、三态能力状态、声明性/实际能力表。
//!
//! 本子模块专注于"什么能力可用/不可用/未探测"的簿记口径，不涉及窗口后端
//! 检测与定位算法（见 [`super::window`]）。
//!
//! 关键类型四层语义（参见父模块文档）：
//! 1. [`LinuxSessionHint`] —— 环境变量**线索**；
//! 2. [`DeclaredCapabilities`] —— 由 hint 推导的**期望**表；
//! 3. [`super::window::WindowBackendKind`] —— 由 RawWindowHandle 实测分类的后端；
//! 4. [`RuntimeCapabilities`] —— 真实初始化结果簿记。
//!
//! 头注豁免：本文件 ~535 行（≤1000 头注豁免阈），原因是 7 个能力相关数据
//! 类型同源（hint/状态/枚举/表）、测试就近保留，强行再拆会引入跨文件
//! 重复 `use super::{...}` 列表与更深的目录层级，得不偿失。

/// 环境值"有效"：存在且 `trim` 后非空。
fn is_meaningful(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.trim().is_empty())
}

/// Linux 图形会话类型线索（由环境变量推导，非实测连接）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxSessionHint {
    /// X11 会话线索（Xorg 或 XWayland）。
    X11,
    /// Wayland 合成器会话线索。
    Wayland,
    /// 无法从显式环境输入识别（按最保守策略处理）。
    Unknown,
}

impl std::fmt::Display for LinuxSessionHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LinuxSessionHint::X11 => "x11",
            LinuxSessionHint::Wayland => "wayland",
            LinuxSessionHint::Unknown => "unknown",
        })
    }
}

impl LinuxSessionHint {
    /// 从显式传入的环境变量值检测会话类型线索（纯函数，不读取进程环境）。
    ///
    /// 优先级：
    /// 1. `xdg_session_type`：`wayland`/`x11`（大小写不敏感、忽略首尾空白）直接命中；
    ///    其他非空值（如 `tty`）不作为结论，继续按下方线索判定；
    /// 2. `wayland_display` 非空 → [`LinuxSessionHint::Wayland`]；
    /// 3. `display` 非空 → [`LinuxSessionHint::X11`]；
    /// 4. 均不可识别 → [`LinuxSessionHint::Unknown`]。
    pub fn detect_from(
        xdg_session_type: Option<&str>,
        wayland_display: Option<&str>,
        display: Option<&str>,
    ) -> LinuxSessionHint {
        // 1) XDG_SESSION_TYPE 优先。
        if let Some(raw) = xdg_session_type.map(str::trim).filter(|v| !v.is_empty()) {
            if raw.eq_ignore_ascii_case("wayland") {
                return LinuxSessionHint::Wayland;
            }
            if raw.eq_ignore_ascii_case("x11") {
                return LinuxSessionHint::X11;
            }
            // 未识别的会话值：不武断归为未知，继续按 WAYLAND_DISPLAY / DISPLAY 判定。
        }
        // 2) WAYLAND_DISPLAY。
        if is_meaningful(wayland_display) {
            return LinuxSessionHint::Wayland;
        }
        // 3) DISPLAY。
        if is_meaningful(display) {
            return LinuxSessionHint::X11;
        }
        // 4) 无可用线索：最保守。
        LinuxSessionHint::Unknown
    }

    /// 可读的降级/限制说明：对当前会话线索下预期受限的能力逐条输出。
    ///
    /// 输出描述的是"该会话下这些能力需要额外工作/协议支持"，
    /// 不是"它们已被禁用"的最终结论——最终以真实初始化结果为准。
    pub fn degradation_reasons(self) -> Vec<&'static str> {
        match self {
            LinuxSessionHint::X11 => vec![
                "x11：置顶/全局定位/点击穿透/拖动均按 RawWindowHandle 实测结果记录（本批已实现探测与落地）",
            ],
            LinuxSessionHint::Wayland => vec![
                "always_on_top：Wayland 置顶依赖合成器扩展（如 zwlr_layer_shell_v1），winit 为无操作，预期 unavailable",
                "global_position：Wayland 由合成器管理窗口布局，客户端全局定位无协议保证，预期 unavailable",
                "click_through/drag_move：经 winit 原生 input region / xdg 交互实现，两种后端均可期待",
            ],
            LinuxSessionHint::Unknown => vec![
                "全部能力 unknown/unavailable：会话线索未知（XDG_SESSION_TYPE/WAYLAND_DISPLAY/DISPLAY 缺失或不可识别），无法建立窗口",
            ],
        }
    }
}

/// 单项能力的三态记录：比 bool 更诚实——区分"没测过"和"测过不行"。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CapabilityState {
    /// 尚未初始化/未探测：不能断言支持也不能断言不支持。
    #[default]
    Unknown,
    /// 已通过真实初始化确认可用。
    Available,
    /// 已尝试但确认不可用（协议限制或初始化失败）。
    Unavailable,
}

impl std::fmt::Display for CapabilityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            CapabilityState::Available => "available",
            CapabilityState::Unavailable => "unavailable",
            CapabilityState::Unknown => "unknown",
        })
    }
}

/// 实际探测到的 surface alpha 合成模式（自有枚举，隔离 wgpu 类型）。
///
/// 透明性结论只认 PreMultiplied / PostMultiplied / Inherit 这类尊重 alpha 的模式；
/// 仅 Opaque 时透明 alpha 视为不可用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedAlphaMode {
    /// 合成器尊重 alpha，期望应用输出预乘值。
    PreMultiplied,
    /// 合成器尊重 alpha，直通（非预乘）值即可。
    PostMultiplied,
    /// 平台默认合成行为（inherit/auto）：大概率可用但未经逐平台确认。
    Inherit,
    /// 合成器忽略 alpha：透明不可用。
    Opaque,
}

impl std::fmt::Display for ObservedAlphaMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ObservedAlphaMode::PreMultiplied => "premultiplied",
            ObservedAlphaMode::PostMultiplied => "postmultiplied",
            ObservedAlphaMode::Inherit => "inherit",
            ObservedAlphaMode::Opaque => "opaque",
        })
    }
}

impl ObservedAlphaMode {
    /// 该模式是否让合成器尊重帧缓冲 alpha（Inherit 按保守口径不算确认可用）。
    pub const fn respects_alpha(self) -> bool {
        matches!(self, Self::PreMultiplied | Self::PostMultiplied)
    }
}

/// 从候选列表中选择优先级最高的 alpha 模式（纯函数）。
///
/// 偏好顺序（与未来 l2d 直通 alpha 渲染输出对齐）：
/// [`PostMultiplied`](ObservedAlphaMode::PostMultiplied) >
/// [`PreMultiplied`](ObservedAlphaMode::PreMultiplied) >
/// [`Inherit`](ObservedAlphaMode::Inherit) >
/// [`Opaque`](ObservedAlphaMode::Opaque)。
///
/// 返回 `None` 表示候选列表为空（surface 与 adapter 不兼容等异常场景）。
pub fn choose_alpha_mode(supported: &[ObservedAlphaMode]) -> Option<ObservedAlphaMode> {
    const PREFERENCE: [ObservedAlphaMode; 4] = [
        ObservedAlphaMode::PostMultiplied,
        ObservedAlphaMode::PreMultiplied,
        ObservedAlphaMode::Inherit,
        ObservedAlphaMode::Opaque,
    ];
    PREFERENCE.into_iter().find(|mode| supported.contains(mode))
}

/// 会话类型 → **声明性期望**能力表。
///
/// ⚠️ 这是由 [`LinuxSessionHint`] 推导的期望值，用于日志展示与 UX 预期管理，
/// **不是实际能力 gate**：任何功能开关都不应读本表做判断；
/// 实际能力一律查询 [`RuntimeCapabilities`]（来自真实窗口/surface 初始化结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeclaredCapabilities {
    /// 透明窗口（桌面宠基础能力）。
    pub transparent_window: bool,
    /// 窗口置顶（期望；仅 X11 协议具备）。
    pub always_on_top: bool,
    /// 全局定位（期望；仅 X11 协议具备）。
    pub global_position: bool,
    /// 点击穿透（期望；winit 原生 input region 两种后端均具备）。
    pub click_through: bool,
    /// 交互态拖动窗口（期望；winit 两种后端均实现）。
    pub drag_move: bool,
    /// 系统托盘（期望；ksni 走 SNI，依赖桌面 host/扩展显示）。
    pub tray: bool,
}

impl Default for DeclaredCapabilities {
    /// 最保守默认：全部不承诺。
    fn default() -> Self {
        Self {
            transparent_window: false,
            always_on_top: false,
            global_position: false,
            click_through: false,
            drag_move: false,
            tray: false,
        }
    }
}

impl DeclaredCapabilities {
    /// 会话类型 → 声明性期望映射（纯函数）。
    ///
    /// - [`LinuxSessionHint::X11`]：协议上具备完整桌宠条件；
    /// - [`LinuxSessionHint::Wayland`]：透明窗口 + 点击穿透 + 拖动 + 托盘可期待；
    ///   置顶/全局定位受协议限制；
    /// - [`LinuxSessionHint::Unknown`]：最保守，全不承诺。
    pub fn for_session(session: LinuxSessionHint) -> Self {
        match session {
            LinuxSessionHint::X11 => Self {
                transparent_window: true,
                always_on_top: true,
                global_position: true,
                click_through: true,
                drag_move: true,
                tray: true,
            },
            LinuxSessionHint::Wayland => Self {
                transparent_window: true,
                always_on_top: false,
                global_position: false,
                // winit 原生 set_cursor_hittest / drag_window 两个后端都实现。
                click_through: true,
                drag_move: true,
                tray: true,
            },
            LinuxSessionHint::Unknown => Self::default(),
        }
    }

    /// 能力名 → 期望值，固定顺序（供 CLI / 日志逐项输出）。
    pub fn as_table(&self) -> [(&'static str, bool); 6] {
        [
            ("transparent_window", self.transparent_window),
            ("always_on_top", self.always_on_top),
            ("global_position", self.global_position),
            ("click_through", self.click_through),
            ("drag_move", self.drag_move),
            ("tray", self.tray),
        ]
    }
}

/// **实际**运行时能力簿记：来自真实窗口/surface 初始化与桌宠 API 调用结果。
///
/// 构造约束（需求 2 口径）：
/// - 只能在对应初始化/API **真实调用成功**后把相应字段置为
///   [`CapabilityState::Available`]；
/// - `always_on_top` / `global_position` 仅在实测后端为 X11 时可 Available；
///   Wayland 上 winit 对应实现为无操作/合成器管理 → 显式 `Unavailable`
///   （判定依据是 RawWindowHandle 实测分类，不是环境 hint）；
/// - `click_through` / `drag_move` 两种后端都支持：状态只由**该 API 的实际调用结果**
///   决定（成功 → Available；未调用过保持 Unknown）；
/// - `tray` 由 ksni spawn 的真实确认结果决定；
/// - 绝不允许从 [`LinuxSessionHint`] 推导本结构的字段值。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeCapabilities {
    /// winit 窗口对象是否创建成功。
    pub window_created: bool,
    /// wgpu surface 是否创建并完成首次 configure。
    pub surface_initialized: bool,
    /// 是否向 winit 请求了透明窗口（请求 ≠ 生效，生效看 alpha_mode/transparent_alpha）。
    pub transparent_window_requested: bool,
    /// 实际协商出的 alpha 合成模式（None = 未完成 surface 初始化）。
    pub alpha_mode: Option<ObservedAlphaMode>,
    /// 透明 alpha 是否确认可用（Opaque → Unavailable；Premultiplied/Post → Available；其余 Unknown）。
    pub transparent_alpha: CapabilityState,
    /// 实测窗口系统后端（RawWindowHandle 分类；None = 未探测/无法识别）。
    pub backend: Option<super::window::WindowBackendKind>,
    /// 窗口置顶：仅 X11 且 `set_window_level` 实调后记录。
    pub always_on_top: CapabilityState,
    /// 置顶当前是否生效（运行态镜像，供报告与托盘簿记对账）。
    pub always_on_top_active: bool,
    /// 全局定位：仅 X11 且初始定位经回读验证后记录。
    pub global_position: CapabilityState,
    /// 点击穿透：`set_cursor_hittest` 实调成功后记录（两种后端均可）。
    pub click_through: CapabilityState,
    /// 点击穿透当前是否生效（true = 鼠标穿透中）。
    pub click_through_active: bool,
    /// 交互态拖动：`drag_window` 实调成功后记录。
    pub drag_move: CapabilityState,
    /// 系统托盘：ksni spawn 真实确认后记录。
    pub tray: CapabilityState,
}

impl RuntimeCapabilities {
    /// 未探测状态的便捷构造（等同 `Default`），供无窗口路径打印。
    pub fn not_probed() -> Self {
        Self::default()
    }

    /// 能力名 → 实际状态文本，固定顺序（供 CLI / 日志逐项输出）。
    pub fn as_table(&self) -> Vec<(&'static str, String)> {
        let mut table = vec![
            ("window_created", self.window_created.to_string()),
            ("surface_initialized", self.surface_initialized.to_string()),
            (
                "transparent_window_requested",
                self.transparent_window_requested.to_string(),
            ),
            (
                "alpha_mode",
                self.alpha_mode
                    .as_ref()
                    .map_or_else(|| "n/a".to_string(), ToString::to_string),
            ),
            ("transparent_alpha", self.transparent_alpha.to_string()),
            (
                "backend",
                self.backend
                    .as_ref()
                    .map_or_else(|| "n/a".to_string(), ToString::to_string),
            ),
        ];
        for (name, state) in [
            ("always_on_top", self.always_on_top),
            ("global_position", self.global_position),
            ("click_through", self.click_through),
            ("drag_move", self.drag_move),
            ("tray", self.tray),
        ] {
            table.push((name, state.to_string()));
        }
        table.push((
            "always_on_top_active",
            self.always_on_top_active.to_string(),
        ));
        table.push((
            "click_through_active",
            self.click_through_active.to_string(),
        ));
        table
    }

    /// 一行摘要（供 smoke 结束后的 RunReport 使用）。
    pub fn summarize(&self) -> String {
        format!(
            "window={} surface={} transparent_requested={} alpha_mode={} \
             transparent_alpha={} backend={} aot={} ct={}",
            self.window_created,
            self.surface_initialized,
            self.transparent_window_requested,
            self.alpha_mode
                .as_ref()
                .map_or_else(|| "n/a".to_string(), ToString::to_string),
            self.transparent_alpha,
            self.backend
                .as_ref()
                .map_or_else(|| "n/a".to_string(), ToString::to_string),
            self.always_on_top,
            self.click_through
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CapabilityState, DeclaredCapabilities, LinuxSessionHint, ObservedAlphaMode,
        RuntimeCapabilities, choose_alpha_mode,
    };
    use crate::platform::window::WindowBackendKind;

    #[test]
    fn detect_from_prefers_xdg_session_type() {
        assert_eq!(
            LinuxSessionHint::detect_from(Some("wayland"), Some("wayland-0"), Some(":0")),
            LinuxSessionHint::Wayland
        );
        assert_eq!(
            LinuxSessionHint::detect_from(Some("x11"), Some("wayland-0"), None),
            LinuxSessionHint::X11
        );
        // 大小写不敏感、忽略首尾空白。
        assert_eq!(
            LinuxSessionHint::detect_from(Some("  Wayland "), None, None),
            LinuxSessionHint::Wayland
        );
    }

    #[test]
    fn detect_from_falls_back_to_display_vars() {
        assert_eq!(
            LinuxSessionHint::detect_from(None, Some("wayland-1"), None),
            LinuxSessionHint::Wayland
        );
        assert_eq!(
            LinuxSessionHint::detect_from(None, None, Some(":0")),
            LinuxSessionHint::X11
        );
        // XDG 不可识别（如 tty）不作为结论，落到 DISPLAY。
        assert_eq!(
            LinuxSessionHint::detect_from(Some("tty"), None, Some(":0")),
            LinuxSessionHint::X11
        );
        // 全部缺失 → Unknown；空白值等价于缺失。
        assert_eq!(
            LinuxSessionHint::detect_from(None, None, None),
            LinuxSessionHint::Unknown
        );
        assert_eq!(
            LinuxSessionHint::detect_from(Some(""), Some(" "), Some("")),
            LinuxSessionHint::Unknown
        );
    }

    #[test]
    fn declared_capabilities_map_per_session_hint_only() {
        let x11 = DeclaredCapabilities::for_session(LinuxSessionHint::X11);
        assert!(x11.transparent_window && x11.tray);
        // 声明表：X11 协议上完整桌宠条件……
        assert!(x11.always_on_top && x11.global_position && x11.click_through && x11.drag_move);

        let wayland = DeclaredCapabilities::for_session(LinuxSessionHint::Wayland);
        assert!(wayland.transparent_window && wayland.tray);
        // 置顶/全局定位受协议限制；穿透与拖动经 winit 原生实现可期待（本批口径）。
        assert!(!wayland.always_on_top && !wayland.global_position);
        assert!(wayland.click_through && wayland.drag_move);
        assert_eq!(LinuxSessionHint::Wayland.degradation_reasons().len(), 3);

        let unknown = DeclaredCapabilities::for_session(LinuxSessionHint::Unknown);
        assert_eq!(unknown, DeclaredCapabilities::default());
        assert!(!LinuxSessionHint::Unknown.degradation_reasons().is_empty());
    }

    #[test]
    fn runtime_capabilities_default_is_honest_not_probed() {
        let caps = RuntimeCapabilities::not_probed();
        // 事实字段：未初始化即 false。
        assert!(!caps.window_created);
        assert!(!caps.surface_initialized);
        assert!(!caps.transparent_window_requested);
        assert_eq!(caps.alpha_mode, None);
        assert_eq!(caps.backend, None);
        assert!(!caps.always_on_top_active);
        assert!(!caps.click_through_active);
        // 能力字段：未探测即 Unknown，绝不因 X11 线索变 true/available。
        for state in [
            caps.transparent_alpha,
            caps.always_on_top,
            caps.global_position,
            caps.click_through,
            caps.drag_move,
            caps.tray,
        ] {
            assert_eq!(state, CapabilityState::Unknown);
        }
    }

    #[test]
    fn runtime_capabilities_table_and_summary_cover_all_fields() {
        let caps = RuntimeCapabilities {
            window_created: true,
            surface_initialized: true,
            transparent_window_requested: true,
            alpha_mode: Some(ObservedAlphaMode::PostMultiplied),
            transparent_alpha: CapabilityState::Available,
            backend: Some(WindowBackendKind::X11),
            always_on_top: CapabilityState::Available,
            always_on_top_active: true,
            global_position: CapabilityState::Available,
            click_through: CapabilityState::Available,
            click_through_active: false,
            drag_move: CapabilityState::Unknown,
            tray: CapabilityState::Unavailable,
        };
        let table = caps.as_table();
        // 6 个事实字段 + 5 个能力字段 + 2 个运行态镜像。
        assert_eq!(table.len(), 13);
        assert!(table.iter().any(|(k, v)| *k == "backend" && v == "x11"));
        assert!(
            table
                .iter()
                .any(|(k, v)| *k == "always_on_top" && v == "available")
        );
        assert!(
            table
                .iter()
                .any(|(k, v)| *k == "tray" && v == "unavailable")
        );
        assert!(
            table
                .iter()
                .any(|(k, v)| *k == "click_through_active" && v == "false")
        );
        let summary = caps.summarize();
        assert!(summary.contains("postmultiplied"));
        assert!(summary.contains("transparent_alpha=available"));
        assert!(summary.contains("backend=x11"));
        assert!(summary.contains("aot=available"));
        assert!(summary.contains("ct=available"));
    }

    #[test]
    fn choose_alpha_mode_prefers_post_multiplied_for_straight_alpha_output() {
        let all = [
            ObservedAlphaMode::Opaque,
            ObservedAlphaMode::Inherit,
            ObservedAlphaMode::PreMultiplied,
            ObservedAlphaMode::PostMultiplied,
        ];
        assert_eq!(
            choose_alpha_mode(&all),
            Some(ObservedAlphaMode::PostMultiplied)
        );
        // 只有预乘时也接受（清屏全透明阶段两者等价）。
        assert_eq!(
            choose_alpha_mode(&[ObservedAlphaMode::Opaque, ObservedAlphaMode::PreMultiplied]),
            Some(ObservedAlphaMode::PreMultiplied)
        );
        // 只有不透明 → Opaque 且不算透明可用。
        assert_eq!(
            choose_alpha_mode(&[ObservedAlphaMode::Opaque]),
            Some(ObservedAlphaMode::Opaque)
        );
        assert!(!ObservedAlphaMode::Opaque.respects_alpha());
        // 空列表 = surface/adapter 不兼容。
        assert_eq!(choose_alpha_mode(&[]), None);
        assert!(ObservedAlphaMode::PostMultiplied.respects_alpha());
        assert!(ObservedAlphaMode::PreMultiplied.respects_alpha());
        assert!(!ObservedAlphaMode::Inherit.respects_alpha());
    }

    #[test]
    fn capability_state_display_is_stable_cli_vocabulary() {
        assert_eq!(CapabilityState::Available.to_string(), "available");
        assert_eq!(CapabilityState::Unavailable.to_string(), "unavailable");
        assert_eq!(CapabilityState::Unknown.to_string(), "unknown");
        assert_eq!(ObservedAlphaMode::Inherit.to_string(), "inherit");
    }
}
