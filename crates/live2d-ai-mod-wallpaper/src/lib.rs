//! live2d-ai-mod-wallpaper（Wave 1，2026-09-14）——**壁纸策略 v0**。
//!
//! 链路定位：本 Mod 只回答「**何时换壁纸 / 要不要和舞台同步**」，**不画图**。
//! 真正的像素路径（stage-bg 命令 → wasm 预通道 → framebuffer）在 0.1.0-rc.5
//! 已经定案，本 crate **一行不碰**（见 `docs/architecture/wallpaper-mod-v0.md` §2）。
//!
//! # 三档模式（`settings_spec` v1）
//!
//! | key | 语义 |
//! |---|---|
//! | `mode` | `off`（缺省）＝ 不接管；`follow_stage` ＝ 壳跟随舞台；`interval` ＝ 定时切换 |
//! | `interval_secs` | `interval` 模式的切换间隔（秒；钳在 5..=86400，永不 0） |
//!
//! schema 里**没有**第二个 `enabled`：启停唯一真源是 Mod manifest 的 `enabled`
//!（与 external-input / voice-input 同口径，见 `docs/architecture/mod-product-chain.md` §4）。
//!
//! # 边界（Wave 1；半成品是刻意的）
//!
//! - **不注册**进 `AVAILABLE_MOD_FACTORIES`：注册由集成 PR 按
//!   `docs/plans/parallel-mods/REGISTER-wallpaper.md` 统一做；本分支不得碰
//!   FACTORIES / `mod_count_*` / 缺省 manifest / 全局版本（PARALLEL-PROTOCOL §3）。
//! - **不做大轮播**：不持有图片、不引入图库 / 在线拉图 / 轮播 UI；`interval` 只按
//!   **调用方给的列表长度**推进一个游标（纯逻辑在 [`strategy`]）。
//! - **不接**动作通道（`action_tx` 自 rc.2 起休眠）；v0 **不订阅** host 事件
//!   （Mod 事件主题里没有时钟，节拍由集成方驱动 [`WallpaperRuntime::tick`]）。
//!
//! # 换图落点（v0 占位，明文）
//!
//! `DisplayPrefs`（`shellImage` / `stageImage` / `syncShellStageBg`）与 stage-bg
//! 通道**都在 Flutter / wasm 侧**，Mod API 目前没有壁纸写入口。因此本 crate 产出
//! [`WallpaperDecision`] 后，[`WallpaperRuntime::apply_decision`] **只记一条 warn
//! 并返回 `false`**——这是**刻意的占位**，不是遗忘：集成方按 REGISTER 文件把决策接到
//! 既有 `DisplayPrefs` / `sendStageBg` 通道即可（`follow_stage` → `syncShellStageBg = true`，
//! `Advance { index }` → 播放列表第 `index` 张图）。**不得**为此新增 wasm / framebuffer 路径。

pub mod strategy;

pub use strategy::{
    DEFAULT_INTERVAL_SECS, MAX_INTERVAL_SECS, MIN_INTERVAL_SECS, WallpaperConfig,
    WallpaperDecision, WallpaperMode, WallpaperStrategy, interval_secs_from_config,
};

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "wallpaper",
    name: "壁纸策略",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// 壁纸设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
///
/// 字段顺序与 `mode` 选项顺序都是契约（单测钉住）。
pub fn wallpaper_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::Select {
                key: "mode".to_string(),
                label: "壁纸模式（off = 不接管；follow_stage = 与舞台同步；interval = 定时切换）"
                    .to_string(),
                options: vec![
                    SelectOption {
                        value: WallpaperMode::Off.as_str().to_string(),
                        label: "关闭（不接管）".to_string(),
                    },
                    SelectOption {
                        value: WallpaperMode::FollowStage.as_str().to_string(),
                        label: "跟随舞台（与舞台同步）".to_string(),
                    },
                    SelectOption {
                        value: WallpaperMode::Interval.as_str().to_string(),
                        label: "定时切换".to_string(),
                    },
                ],
            },
            ModSettingField::Number {
                key: "interval_secs".to_string(),
                label: "切换间隔（秒；仅 interval 模式生效，5–86400）".to_string(),
                min: MIN_INTERVAL_SECS as f64,
                max: MAX_INTERVAL_SECS as f64,
            },
        ],
    }
}

/// 壁纸 Mod 运行时：注入的 host services + namespaced config + 纯策略状态机。
pub struct WallpaperRuntime {
    services: ModServices,
    config: serde_json::Value,
    strategy: WallpaperStrategy,
    /// settings schema 是否已注册（`start` 成功标志）。
    registered: bool,
}

impl WallpaperRuntime {
    /// 用注入的 host services + namespaced config 构造。
    ///
    /// v0 的播放列表长度从 0 起（图库由集成方经 [`Self::set_playlist_len`] 告知）。
    pub fn new(services: ModServices, config: serde_json::Value) -> Self {
        let strategy = WallpaperStrategy::from_config_json(&config, 0);
        Self {
            services,
            config,
            strategy,
            registered: false,
        }
    }

    /// 是否已启用（`start` 成功）。
    pub fn is_ready(&self) -> bool {
        self.registered
    }

    /// 当前 namespaced 配置。
    pub fn config(&self) -> &serde_json::Value {
        &self.config
    }

    /// 当前模式。
    pub fn mode(&self) -> WallpaperMode {
        self.strategy.mode()
    }

    /// 当前间隔（秒，已钳）。
    pub fn interval_secs(&self) -> u64 {
        self.strategy.config().interval_secs
    }

    /// 只读策略（集成方/单测观察游标）。
    pub fn strategy(&self) -> &WallpaperStrategy {
        &self.strategy
    }

    /// 图库变化时更新播放列表长度。
    pub fn set_playlist_len(&mut self, len: usize) {
        self.strategy.set_playlist_len(len);
    }

    /// 用新的 namespaced config 重配置（计时 / 同步标志复位）。
    pub fn reconfigure(&mut self, config: serde_json::Value) {
        self.strategy
            .reconfigure(WallpaperConfig::from_json(&config));
        self.config = config;
    }

    /// 推进节拍并返回决策；**非 `None` 的决策会留一条 info 日志**。
    ///
    /// 节拍来源由集成方决定（主循环 timer / 下一次 turn 事件等）——v0 不在 Mod 内起线程。
    pub fn tick(&mut self, delta_ms: u64) -> WallpaperDecision {
        let decision = self.strategy.tick(delta_ms);
        match decision {
            WallpaperDecision::None => {}
            WallpaperDecision::SyncStage => self
                .services
                .logger
                .info("壁纸策略：壳跟随舞台（等价 DisplayPrefs.syncShellStageBg = true）"),
            WallpaperDecision::Advance { index } => self.services.logger.info(&format!(
                "壁纸策略：切到第 {index} 张（列表 {} 张）",
                self.strategy.playlist_len()
            )),
        }
        decision
    }

    /// 把决策落到真实显示层——**v0 占位**。
    ///
    /// - `None` → `true`（无事可做即成功，幂等）；
    /// - `SyncStage` / `Advance` → 记一条 warn 并返回 `false`：Mod API 没有壁纸写入口，
    ///   集成方按 `docs/plans/parallel-mods/REGISTER-wallpaper.md` 接到既有
    ///   `DisplayPrefs` / stage-bg 通道。
    pub fn apply_decision(&self, decision: WallpaperDecision) -> bool {
        match decision {
            WallpaperDecision::None => true,
            WallpaperDecision::SyncStage | WallpaperDecision::Advance { .. } => {
                self.services.logger.warn(
                    "壁纸策略 v0：换图落点未接线（应由集成方经 DisplayPrefs / stage-bg 执行）",
                );
                false
            }
        }
    }
}

/// 壁纸工厂。
pub struct WallpaperFactory;

impl ModFactory for WallpaperFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// 静态 schema：未启用也能渲染配置表单（rc.4 M2 语义）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(wallpaper_settings_spec())
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(WallpaperRuntime::new(services, config)))
    }
}

impl ModRuntime for WallpaperRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        registrar.register_settings(wallpaper_settings_spec())?;
        // v0 不订阅 host 事件：Mod 事件主题里没有时钟，节拍由集成方驱动 tick。
        self.registered = true;
        self.services.logger.info(&format!(
            "wallpaper Mod 已启动（mode={}, interval={}s）",
            self.mode().as_str(),
            self.interval_secs()
        ));
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.registered = false;
        self.services.logger.info("wallpaper Mod 已关闭");
        Ok(())
    }
}

/// 供集成 PR 装配 `AVAILABLE_MOD_FACTORIES` 的工厂单例（**本分支不注册**）。
pub static FACTORY: WallpaperFactory = WallpaperFactory;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// 测试用日志记录。
    type Recorded = Arc<Mutex<Vec<String>>>;

    fn recording_services() -> (ModServices, Recorded) {
        let logs: Recorded = Arc::new(Mutex::new(Vec::new()));
        let l = logs.clone();
        let services = ModServices::new(
            ModActionSender::new(|_| false),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(move |_lvl, msg| l.lock().unwrap().push(msg.to_string())),
        );
        (services, logs)
    }

    fn noop_services() -> ModServices {
        ModServices::new(
            ModActionSender::new(|_| false),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        )
    }

    /// 只接受 settings 注册；**任何 subscribe 都 panic**（v0 不该订阅）。
    #[derive(Default)]
    struct MockRegistrar {
        specs: Vec<ModSettingsSpec>,
    }
    impl ModRegistrar for MockRegistrar {
        fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
            self.specs.push(spec);
            Ok(())
        }
        fn subscribe(&mut self, _t: ModEventTopic) -> Result<SubscriptionId, ModError> {
            panic!("v0 不应订阅任何 host 事件（无时钟主题）")
        }
        fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
            Ok(())
        }
    }

    #[test]
    fn descriptor_and_factory_identity() {
        assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
        assert_eq!(FACTORY.descriptor().id, "wallpaper");
        assert_eq!(FACTORY.descriptor().name, "壁纸策略");
        let rt: Box<dyn ModRuntime> = FACTORY
            .create(noop_services(), serde_json::json!({}))
            .expect("create 应成功");
        drop(rt);
    }

    #[test]
    fn start_registers_settings_only() {
        let mut rt = WallpaperRuntime::new(noop_services(), serde_json::json!({}));
        let mut reg = MockRegistrar::default();
        rt.start(&mut reg).expect("start 应成功");
        assert!(rt.is_ready());
        assert_eq!(reg.specs, vec![wallpaper_settings_spec()]);
        rt.shutdown().unwrap();
        assert!(!rt.is_ready(), "shutdown 后不得再报 ready");
    }

    /// 静态 schema 字段齐全；**不含 `enabled`**（唯一开关是 manifest）。
    #[test]
    fn static_spec_fields_and_no_enabled() {
        let spec = FACTORY.settings_spec().expect("静态 schema");
        assert!(spec.validate().is_ok(), "字段 key 不得重复");
        let keys: Vec<&str> = spec.fields.iter().map(|f| f.key()).collect();
        assert_eq!(keys, vec!["mode", "interval_secs"]);
        assert!(!keys.contains(&"enabled"), "启停只由 Mod manifest 表达");
        match &spec.fields[0] {
            ModSettingField::Select { options, .. } => {
                let values: Vec<&str> = options.iter().map(|o| o.value.as_str()).collect();
                assert_eq!(values, vec!["off", "follow_stage", "interval"]);
            }
            other => panic!("mode 应为 Select，实际 {other:?}"),
        }
        match &spec.fields[1] {
            ModSettingField::Number { min, max, .. } => {
                assert_eq!(*min, MIN_INTERVAL_SECS as f64);
                assert_eq!(*max, MAX_INTERVAL_SECS as f64);
            }
            other => panic!("interval_secs 应为 Number，实际 {other:?}"),
        }
    }

    #[test]
    fn runtime_reads_mode_and_interval_from_config() {
        let rt = WallpaperRuntime::new(
            noop_services(),
            serde_json::json!({"mode": "interval", "interval_secs": 42}),
        );
        assert_eq!(rt.mode(), WallpaperMode::Interval);
        assert_eq!(rt.interval_secs(), 42);
        assert_eq!(rt.config()["mode"], serde_json::json!("interval"));
        // 缺省 = off（缺省不接管）。
        let d = WallpaperRuntime::new(noop_services(), serde_json::json!({}));
        assert_eq!(d.mode(), WallpaperMode::Off);
        assert_eq!(d.interval_secs(), DEFAULT_INTERVAL_SECS);
    }

    #[test]
    fn runtime_tick_logs_decisions_and_advances() {
        let (services, logs) = recording_services();
        let mut rt = WallpaperRuntime::new(
            services,
            serde_json::json!({"mode": "interval", "interval_secs": 10}),
        );
        rt.set_playlist_len(3);
        assert_eq!(rt.tick(0), WallpaperDecision::Advance { index: 0 });
        assert_eq!(rt.tick(10_000), WallpaperDecision::Advance { index: 1 });
        // 未到点：无新日志。
        let before = logs.lock().unwrap().len();
        assert_eq!(rt.tick(1), WallpaperDecision::None);
        assert_eq!(logs.lock().unwrap().len(), before, "None 不写日志");
        assert!(
            logs.lock().unwrap().iter().any(|m| m.contains("第 0 张")),
            "切图应留 info 日志"
        );
    }

    #[test]
    fn runtime_follow_stage_logs_and_respects_off() {
        let (services, logs) = recording_services();
        let mut rt = WallpaperRuntime::new(services, serde_json::json!({"mode": "follow_stage"}));
        assert_eq!(rt.tick(0), WallpaperDecision::SyncStage);
        assert_eq!(rt.tick(0), WallpaperDecision::None);
        assert!(
            logs.lock().unwrap().iter().any(|m| m.contains("跟随舞台")),
            "同步应留 info 日志"
        );

        let mut off = WallpaperRuntime::new(noop_services(), serde_json::json!({}));
        assert_eq!(off.tick(999_999), WallpaperDecision::None);
    }

    #[test]
    fn reconfigure_applies_new_config() {
        let mut rt = WallpaperRuntime::new(noop_services(), serde_json::json!({}));
        assert_eq!(rt.mode(), WallpaperMode::Off);
        rt.reconfigure(serde_json::json!({"mode": "interval", "interval_secs": 0}));
        assert_eq!(rt.mode(), WallpaperMode::Interval);
        assert_eq!(rt.interval_secs(), MIN_INTERVAL_SECS, "重配置也走钳位");
        assert_eq!(rt.config()["mode"], serde_json::json!("interval"));
    }

    /// v0 明文占位：`None` 幂等成功，其余决策未接线（false + 一条 warn）。
    #[test]
    fn apply_decision_is_documented_placeholder() {
        let (services, logs) = recording_services();
        let rt = WallpaperRuntime::new(services, serde_json::json!({}));
        assert!(rt.apply_decision(WallpaperDecision::None), "None 幂等成功");
        assert!(!rt.apply_decision(WallpaperDecision::SyncStage));
        assert!(!rt.apply_decision(WallpaperDecision::Advance { index: 2 }));
        assert!(logs.lock().unwrap().iter().any(|m| m.contains("未接线")));
    }
}
