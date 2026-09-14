//! live2d-ai-mod-pet-desktop（节点 E E7；Wave 2 更新 2026-09-14）。
//!
//! 桌宠窗口 Mod——**配置 / 事件态的可测面 v1**。
//!
//! # 范围声明（Wave 2：**窗口未开**）
//!
//! 本 crate **不创建任何窗口**，`state_json()` 恒返回
//! `"window": {"opened": false, "reason": "native_shell_dormant"}`。
//!
//! 为什么：桌宠悬浮窗（总在最前 / 点击穿透 / 不透明度）是**原生能力**，
//! 它的实现在 `live2d-ai-desktop` 的 **egui 原生壳**
//! （`src/app/` + `src/tray.rs` + `src/platform.rs`）。而那份原生壳自 rc.3 起被
//! 裁决为**休眠保留**——唯一产品链路是 **`--web` + Flutter Web `/app/`**
//! （`scripts/ignite.sh` 点火）。
//! 完整台账见 `docs/architecture/core-chain-baseline.md` **§3.6「原生第二壳
//! （egui / `--chat`）：非主线」**与 `AGENTS.md`「原生第二壳的归属」一节；
//! 唤醒原生壳必须先单独论证「**谁来维护第二个 UI 壳**」。
//!
//! 边界一览（**不允许第三种含糊表述**）：
//!
//! | 对象 | 状态 | 本 crate 的关系 |
//! | --- | --- | --- |
//! | `--web` + Flutter `/app/` | **主线** | 桌宠窗口 UI 在它里面**不存在**；本 Mod 只经 host 的 `GET /api/v1/mods/pet-desktop/state` 出状态 |
//! | egui 原生壳（窗口 / 托盘 / 桌宠穿透） | 休眠保留、不验收 | **不接线**；本 crate 不是它的驱动方 |
//! | `--chat` 终端壳 | 休眠保留、不验收 | 无关 |
//!
//! 本 crate **不引入** `winit` / `egui` / `wgpu` 等第二套渲染依赖（Wave 2 红线），
//! 也不重写 desktop 壳。因此本 Mod 的验收面是**数据**（配置 + 事件态），不是像素。
//!
//! # 配置字段（`settings_spec` v1）
//!
//! | key | kind | 语义 | 缺省 |
//! | --- | --- | --- | --- |
//! | `always_on_top` | bool | 窗口总在最前 | `true` |
//! | `click_through` | bool | 鼠标点击穿透 | `false` |
//! | `opacity` | number | 窗口不透明度，钳在 `0.1..=1.0` | `0.95` |
//!
//! schema 里**没有**第二个 `enabled`：启停唯一真源是 Mod manifest 的 `enabled`
//!（与 external-input / voice-input / wallpaper 同口径，见
//! `docs/architecture/mod-product-chain.md` §4）。pet-desktop **缺省停用**
//!（`cli_entry::default_mods_manifest` 不收录），要看到状态面需先启用它。
//!
//! # 状态面契约（`ModRuntime::state_json`）
//!
//! ```json
//! {
//!   "always_on_top": true,
//!   "click_through": false,
//!   "opacity": 0.95,
//!   "voice_active": false,
//!   "window": { "opened": false, "reason": "native_shell_dormant" }
//! }
//! ```
//!
//! - **读取来源**：`create(services, config)` 拿到的 **namespaced Mod config JSON**
//!   ——与 `POST /api/v1/mods/pet-desktop/config` 写的是同一份（host 会
//!   `reload_config` + `restart`，于是新快照立刻生效）。字段缺失 / 类型不对 → 回落
//!   上表缺省；`opacity` 非有限值或越界 → 钳位（非有限值回落缺省）。
//! - `voice_active`：由 `VoiceStarted`（true）/ `VoiceEnded`（false）翻转，
//!   `shutdown` 复位；语义与 rc.1 起的 v1 骨架**完全一致**，只是现在读得到。
//! - `window.opened` 恒 `false`（见上「范围声明」）；`reason` 是稳定字符串
//!   [`WINDOW_REASON_NATIVE_SHELL_DORMANT`]，前端据此区分「窗口关着」与
//!   「这个 Mod 没实现状态面」。
//! - **只读、不写盘、不阻塞**：实现只读内存字段，无 IO、无锁等待、无网络
//!   （契约见 `live2d-ai-mod-system` 的 `ModRuntime::state_json` 头注）。
//!   经 host 暴露为 `GET /api/v1/mods/{id}/state`（在册但暂不可读 = 503）。
//!
//! # 版本
//!
//! `api_version` 声明为 [`MOD_API_VERSION`]（对齐 `live2d-ai-mod-system`）。

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "pet-desktop",
    name: "桌宠窗口",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// `always_on_top` 缺省值。
pub const DEFAULT_ALWAYS_ON_TOP: bool = true;
/// `click_through` 缺省值。
pub const DEFAULT_CLICK_THROUGH: bool = false;
/// `opacity` 缺省值。
pub const DEFAULT_OPACITY: f64 = 0.95;
/// `opacity` 下限。
pub const MIN_OPACITY: f64 = 0.1;
/// `opacity` 上限。
pub const MAX_OPACITY: f64 = 1.0;
/// `state_json().window.reason` 的稳定值：原生壳休眠，窗口未开。
pub const WINDOW_REASON_NATIVE_SHELL_DORMANT: &str = "native_shell_dormant";

/// 桌宠窗口设置 schema（**静态**：未启用也拿得到，前端可「先填再启用」）。
///
/// `PetDesktopFactory::settings_spec()` 与 `start` 注册的是**同一份**（同一个
/// 函数调用），单测钉住「两处不分叉」——旧骨架只在 `start` 里注册，于是
/// 「开关还关着」时前端拿不到表单。
pub fn pet_desktop_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::Bool {
                key: "always_on_top".to_string(),
                label: "总在最前".to_string(),
                default: DEFAULT_ALWAYS_ON_TOP,
            },
            ModSettingField::Bool {
                key: "click_through".to_string(),
                label: "点击穿透".to_string(),
                default: DEFAULT_CLICK_THROUGH,
            },
            ModSettingField::Number {
                key: "opacity".to_string(),
                label: "窗口不透明度（0.1–1.0）".to_string(),
                min: MIN_OPACITY,
                max: MAX_OPACITY,
            },
        ],
    }
}

/// `opacity` 钳位：非有限值（NaN / ±∞）回落 [`DEFAULT_OPACITY`]，其余钳进
/// `MIN_OPACITY..=MAX_OPACITY`。**永不返回越界值**。
pub fn clamp_opacity(value: f64) -> f64 {
    if !value.is_finite() {
        return DEFAULT_OPACITY;
    }
    value.clamp(MIN_OPACITY, MAX_OPACITY)
}

/// 从 Mod config JSON 读 `always_on_top`；缺失 / 非 bool → [`DEFAULT_ALWAYS_ON_TOP`]。
pub fn always_on_top_from_config(config: &serde_json::Value) -> bool {
    config
        .get("always_on_top")
        .and_then(|v| v.as_bool())
        .unwrap_or(DEFAULT_ALWAYS_ON_TOP)
}

/// 从 Mod config JSON 读 `click_through`；缺失 / 非 bool → [`DEFAULT_CLICK_THROUGH`]。
pub fn click_through_from_config(config: &serde_json::Value) -> bool {
    config
        .get("click_through")
        .and_then(|v| v.as_bool())
        .unwrap_or(DEFAULT_CLICK_THROUGH)
}

/// 从 Mod config JSON 读 `opacity`；缺失 / 非数字 → [`DEFAULT_OPACITY`]，
/// 数字一律走 [`clamp_opacity`]。
pub fn opacity_from_config(config: &serde_json::Value) -> f64 {
    config
        .get("opacity")
        .and_then(|v| v.as_f64())
        .map(clamp_opacity)
        .unwrap_or(DEFAULT_OPACITY)
}

/// 纯函数：由 config + `voice_active` 构造状态面 JSON。
///
/// 抽出来的目的与 `wallpaper` 的纯策略相同：**不需要 host 就能单测状态形状**
///（runtime 版本只是转调）。`window` 恒定休眠（见 crate 头注「范围声明」）。
pub fn build_state_json(config: &serde_json::Value, voice_active: bool) -> serde_json::Value {
    serde_json::json!({
        "always_on_top": always_on_top_from_config(config),
        "click_through": click_through_from_config(config),
        "opacity": opacity_from_config(config),
        "voice_active": voice_active,
        "window": {
            "opened": false,
            "reason": WINDOW_REASON_NATIVE_SHELL_DORMANT,
        },
    })
}

/// 桌宠窗口 Mod 运行时状态。
pub struct PetDesktopRuntime {
    services: ModServices,
    /// namespaced Mod config（`create` 注入；HTTP `/config` 改的就是它，
    /// 改完 host 会 restart 出一个带新 config 的实例）。
    config: serde_json::Value,
    /// settings schema 是否已注册（`start` 成功标志）。
    registered: bool,
    /// 语音是否活跃（`VoiceStarted` = true / `VoiceEnded` = false）。
    /// v1 起就是「供未来原生窗口读取口型」的语义——现在经 `state_json` 可读。
    voice_active: bool,
}

impl PetDesktopRuntime {
    /// 用注入的 host services + namespaced config 构造。
    pub fn new(services: ModServices, config: serde_json::Value) -> Self {
        Self {
            services,
            config,
            registered: false,
            voice_active: false,
        }
    }

    /// 是否已启动（`start` 成功）。
    pub fn is_ready(&self) -> bool {
        self.registered
    }

    /// 当前 namespaced 配置。
    pub fn config(&self) -> &serde_json::Value {
        &self.config
    }

    /// 语音是否活跃（口型联动信号）。
    pub fn voice_active(&self) -> bool {
        self.voice_active
    }

    /// 当前生效的 `always_on_top`。
    pub fn always_on_top(&self) -> bool {
        always_on_top_from_config(&self.config)
    }

    /// 当前生效的 `click_through`。
    pub fn click_through(&self) -> bool {
        click_through_from_config(&self.config)
    }

    /// 当前生效的 `opacity`（已钳位）。
    pub fn opacity(&self) -> f64 {
        opacity_from_config(&self.config)
    }

    /// 同进程热改配置（不改盘）。
    ///
    /// HTTP 路径**不**走这里：`POST /api/v1/mods/pet-desktop/config` 由 host
    /// `reload_config` + `restart` 造一个新实例（写盘 + 快照一致）。本方法只服务
    /// 「集成方在同一进程内换一份 namespaced config」的场景，并在下次
    /// `state_json` 立刻反映。
    pub fn reconfigure(&mut self, config: serde_json::Value) {
        self.config = config;
    }
}

impl Default for PetDesktopRuntime {
    fn default() -> Self {
        Self::new(
            ModServices::new(
                ModActionSender::new(|_| false),
                SaySender::new(|_| true),
                ModEventSender::new(|_topic, _payload| true),
                ModLogger::new(|_lvl, _msg| {}),
            ),
            serde_json::json!({}),
        )
    }
}

/// 桌宠窗口 Mod 工厂。
pub struct PetDesktopFactory;

impl ModFactory for PetDesktopFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// **静态** schema：窗口关着 / Mod 未启用也能渲染配置表单（rc.4 M2 语义）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(pet_desktop_settings_spec())
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(PetDesktopRuntime::new(services, config)))
    }
}

impl ModRuntime for PetDesktopRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 与 `PetDesktopFactory::settings_spec()` 同一份（单测钉住不分叉）。
        registrar.register_settings(pet_desktop_settings_spec())?;
        // 订阅语音事件：翻转 `voice_active`（口型联动信号，v1 语义不变）。
        registrar.subscribe(ModEventTopic::VoiceStarted)?;
        registrar.subscribe(ModEventTopic::VoiceEnded)?;
        self.registered = true;
        self.services
            .logger
            .info("pet-desktop Mod 已启动（窗口未开：原生壳休眠，状态经 state_json 可读）");
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        match topic {
            ModEventTopic::VoiceStarted => {
                self.voice_active = true;
                self.services
                    .logger
                    .info("桌宠口型联动（VoiceStarted → voice_active=true）");
            }
            ModEventTopic::VoiceEnded => {
                self.voice_active = false;
                self.services
                    .logger
                    .info("桌宠口型联动（VoiceEnded → voice_active=false）");
            }
            _ => {}
        }
        // v1 契约不变：payload 不参与状态（`VoiceStarted`/`VoiceEnded` 的
        // payload 目前为空串，未来即便非空也不得影响这里）。
        let _ = payload;
        Ok(())
    }

    /// 收尾：复位「已注册」与口型信号（窗口本来就没开，无需关窗）。
    fn shutdown(&mut self) -> Result<(), ModError> {
        self.registered = false;
        self.voice_active = false;
        self.services.logger.info("pet-desktop Mod 已关闭");
        Ok(())
    }

    /// 只读运行态快照（Wave 2）——形状与来源见 crate 头注「状态面契约」。
    ///
    /// 只读内存字段：不写盘、不取锁、不发网络请求（host 在 web_api 线程调用）。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(build_state_json(&self.config, self.voice_active))
    }
}

/// 供 host 装配 `AVAILABLE_MOD_FACTORIES` 的工厂单例（**已在表里**，Wave 2 不新增）。
pub static FACTORY: PetDesktopFactory = PetDesktopFactory;
