//! live2d-ai-mod-wallpaper（Wave 1，2026-09-14）——**壁纸策略 v0**。
//!
//! 链路定位：本 Mod 只回答「**何时换壁纸 / 要不要和舞台同步**」，**不画图**。
//! 真正的像素路径（stage-bg 命令 → wasm 预通道 → framebuffer）在 0.1.0-rc.5
//! 已经定案，本 crate **一行不碰**（见 `docs/architecture/wallpaper-mod-v0.md` §2）。
//!
//! # 三档模式（`settings_spec` v1：两个**可编辑**字段）
//!
//! | key | 语义 |
//! |---|---|
//! | `mode` | `off`（缺省）＝ 不接管；`follow_stage` ＝ 壳跟随舞台；`interval` ＝ 定时切换 |
//! | `interval_secs` | `interval` 模式的切换间隔（秒；钳在 5..=86400，永不 0） |
//!
//! schema 里**没有**第二个 `enabled`：启停唯一真源是 Mod manifest 的 `enabled`
//!（与 external-input / voice-input 同口径，见 `docs/architecture/mod-product-chain.md` §4）。
//!
//! `playlist_len` **不是**可编辑字段（2026-09-14 Wave 3 删除）：它是**运行值**，
//! 真源 = Flutter `DisplayPrefs.stagePlaylist.length`，由前端经既有 config 端点写回；
//! 留在表单里手填必被下一次写回覆盖（自相矛盾），所以它只**只读**出现在
//! [`WallpaperRuntime::state_json`] 与 `GET /api/v1/mods` 的 `config` 里。
//!
//! # 边界（Wave 2，2026-09-14）
//!
//! - **已注册**进 `AVAILABLE_MOD_FACTORIES`（`0.2.0-rc.2` 集成，**缺省停用**）；
//!   本 crate **不**碰 FACTORIES / `mod_count_*` / 缺省 manifest / 全局版本
//!   （PARALLEL-PROTOCOL §3）——注册状态由集成方维护。
//! - **不做大轮播**：不持有图片、不引入图库 / 在线拉图 / 轮播 UI；`interval` 只按
//!   **config 里的 `playlist_len`** 推进一个游标（纯逻辑在 [`strategy`]），
//!   列表本身住 Flutter 的 `DisplayPrefs.stagePlaylist`。
//! - **不接**动作通道（`action_tx` 自 rc.2 起休眠）；**不订阅** host 事件
//!   （Mod 事件主题里没有时钟，节拍由 [`ModRuntime::state_json`] 用 `Instant` 驱动）。
//!
//! # 换图落点（Wave 2：**已接线**）
//!
//! `DisplayPrefs`（`shellImage` / `stageImage` / `syncShellStageBg`）与 stage-bg
//! 通道都在 Flutter / wasm 侧，Mod API 没有壁纸写入口——所以本 crate 不「执行」
//! 换图，而是把决策**投影成一份纯数据 patch**（[`WallpaperDecision::to_prefs_patch`]），
//! 由 `WallpaperRuntime::state_json` 经基座 `GET /api/v1/mods/wallpaper/state` 交给
//! Flutter，Flutter 的纯函数 `applyWallpaperPatch` 落到 `DisplayPrefs`，再走**既有**
//! `Live2DStage.sendStageBg`。契约与预算见
//! `docs/architecture/wallpaper-mod-v0.md` §5。
//!
//! **不再有占位**：早期版本 `apply_decision` 对 `SyncStage` / `Advance` 只记一条
//! warn 并返回 `false`（「落点未接线」）——那条假话已随本版删除，取而代之的是
//! 可单测的真投影。

pub mod strategy;

pub use strategy::{
    DEFAULT_INTERVAL_SECS, MAX_INTERVAL_SECS, MAX_PLAYLIST_LEN, MIN_INTERVAL_SECS, WallpaperConfig,
    WallpaperDecision, WallpaperMode, WallpaperStrategy, interval_secs_from_config,
    playlist_len_from_config,
};

use std::time::Instant;

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
///
/// **只有两个可编辑字段**：`playlist_len` 是运行值（真源 = Flutter
/// `DisplayPrefs.stagePlaylist.length`），只读出现在 `state_json` 与 `config`，
/// 不在这里——放进来手填必被前端写回覆盖（Wave 3 §3B 必做 2）。
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
    /// 上一次 `state_json` 的时刻（**唯一的时钟源**）。
    ///
    /// 策略时钟只在 `state_json` 里推进：不在 Mod 内起线程、不订阅事件
    /// （Mod 事件主题里没有时钟）。第一次调用时为 `None` → 增量按 0 算，
    /// 于是 `interval` 模式的第一帧立刻上屏（与纯策略的首帧语义一致）。
    last_state_at: Option<Instant>,
}

impl WallpaperRuntime {
    /// 用注入的 host services + namespaced config 构造。
    ///
    /// 播放列表长度从 config 的 `playlist_len` 读（钳位，缺省 0）——Flutter 在
    /// 列表变化时用既有 `POST /api/v1/mods/wallpaper/config` 写回，服务端
    /// reload + restart 后本构造读到新值。
    pub fn new(services: ModServices, config: serde_json::Value) -> Self {
        let playlist_len = playlist_len_from_config(&config);
        let strategy = WallpaperStrategy::from_config_json(&config, playlist_len);
        Self {
            services,
            config,
            strategy,
            registered: false,
            last_state_at: None,
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

    /// 当前播放列表长度（来自 config，已钳）。
    pub fn playlist_len(&self) -> usize {
        self.strategy.playlist_len()
    }

    /// 只读策略（集成方/单测观察游标）。
    pub fn strategy(&self) -> &WallpaperStrategy {
        &self.strategy
    }

    /// 图库变化时更新播放列表长度。
    pub fn set_playlist_len(&mut self, len: usize) {
        self.strategy.set_playlist_len(len.min(MAX_PLAYLIST_LEN));
    }

    /// 用新的 namespaced config 重配置（计时 / 同步标志复位）。
    ///
    /// `playlist_len` 也从新 config 重读（不是只在构造时读一次）。
    pub fn reconfigure(&mut self, config: serde_json::Value) {
        self.strategy
            .reconfigure(WallpaperConfig::from_json(&config));
        self.strategy
            .set_playlist_len(playlist_len_from_config(&config));
        self.config = config;
    }

    /// 推进节拍并返回决策；**非 `None` 的决策会留一条 info 日志**。
    ///
    /// 节拍来源：生产路径是 [`ModRuntime::state_json`]（用 `Instant` 算增量），
    /// 纯逻辑单测直接喂增量毫秒。**不在 Mod 内起线程**。
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

    /// **只读运行态快照**（基座 Wave 2 API）：决策 + 落点 patch + 原因。
    ///
    /// 契约（`ModRuntime::state_json` 头注）：只读、脱敏、不写盘、不阻塞。
    /// 本实现的全部副作用只有一条：**推进策略时钟**——用内部 `Instant` 算
    /// 距上次调用的增量毫秒喂既有纯策略 `tick(delta_ms)`。这正是契约允许的
    /// 「为计算快照而推进内部游标」，也是本 Mod **唯一**的节拍来源
    /// （不起线程、不订阅事件、不在 `state_json` 里写盘）。
    ///
    /// 返回形状（`docs/plans/parallel-mods/PARALLEL-WAVE2-2026-09-14.md` §3B）：
    ///
    /// ```json
    /// {"mode":"interval","active":true,"playlist_len":3,
    ///  "decision":{"kind":"advance","index":1},
    ///  "prefs_patch":{"stage_index":1},
    ///  "reason":"advance"}
    /// ```
    ///
    /// `prefs_patch` 为 `null` 时前端**明确跳过**（`skipped`），不是「假装成功」。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        // 首帧：没有上一次时刻 → 增量 0（interval 立即上屏第 0 张）。
        let delta_ms = match self.last_state_at {
            Some(previous) => previous.elapsed().as_millis().min(u64::MAX as u128) as u64,
            None => 0,
        };
        self.last_state_at = Some(Instant::now());

        let decision = self.tick(delta_ms);
        let mode = self.mode();
        let playlist_len = self.strategy.playlist_len();
        Some(serde_json::json!({
            "mode": mode.as_str(),
            "active": mode.is_active(),
            "playlist_len": playlist_len,
            "decision": decision.to_json(),
            "prefs_patch": decision.to_prefs_patch(),
            "reason": decision.reason(mode, playlist_len),
        }))
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

    /// 静态 schema 字段齐全；**不含 `enabled`**（唯一开关是 manifest），
    /// 也**不含 `playlist_len`**（Wave 3：运行值不进表单——手填必被前端覆盖）。
    ///
    /// `start` 注册的就是这一份（与 `settings_spec()` 不分叉）。
    #[test]
    fn static_spec_fields_and_no_enabled() {
        let spec = FACTORY.settings_spec().expect("静态 schema");
        assert!(spec.validate().is_ok(), "字段 key 不得重复");
        let keys: Vec<&str> = spec.fields.iter().map(|f| f.key()).collect();
        assert_eq!(keys, vec!["mode", "interval_secs"]);
        assert!(!keys.contains(&"enabled"), "启停只由 Mod manifest 表达");
        assert!(
            !keys.contains(&"playlist_len"),
            "playlist_len 是运行值，不得再作为可编辑字段（Wave 3 §3B 必做 2）"
        );
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

    /// **只读面**：`playlist_len` 从可编辑 schema 删除后，数值仍可从
    /// `config`（`GET /api/v1/mods` 原样回）与 `state_json` 读到——
    /// 删的是「手填入口」，不是「可观察性」。
    #[test]
    fn playlist_len_is_run_value_read_from_config_and_state() {
        let config = serde_json::json!({"mode": "interval", "interval_secs": 5, "playlist_len": 4});
        let rt = WallpaperRuntime::new(noop_services(), config);
        // 只读面 ①：config 原样持有（GET /mods 的 config 字段）。
        assert_eq!(rt.config()["playlist_len"], serde_json::json!(4));
        // 只读面 ②：state_json 顶部（数值真源 = Flutter stagePlaylist.length）。
        let mut rt = rt;
        let state = rt.state_json().unwrap();
        assert_eq!(state["playlist_len"], serde_json::json!(4));
        assert_eq!(state["decision"]["kind"], serde_json::json!("advance"));
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

    // ---------------------------------------------------- state_json（Wave 2）

    /// §3B 形状：六个键齐全，`decision` 是对象、`prefs_patch` 与它同源。
    #[test]
    fn state_json_shape_matches_contract() {
        let mut rt = WallpaperRuntime::new(
            noop_services(),
            serde_json::json!({"mode": "interval", "interval_secs": 300, "playlist_len": 3}),
        );
        let state = rt.state_json().expect("本 Mod 恒有可公开状态");
        assert_eq!(state["mode"], serde_json::json!("interval"));
        assert_eq!(state["active"], serde_json::json!(true));
        assert_eq!(state["playlist_len"], serde_json::json!(3));
        assert_eq!(
            state["decision"],
            serde_json::json!({"kind": "advance", "index": 0}),
            "interval 首帧应上屏第 0 张"
        );
        assert_eq!(
            state["prefs_patch"],
            serde_json::json!({"stage_index": 0}),
            "预投影必须与 decision 同源"
        );
        assert_eq!(state["reason"], serde_json::json!("advance"));
        // 顶层恰好这六个键（多一个字段 = 前端契约漂移）。
        // 顺序不比：serde_json 的 Map 按 key 排序，顺序不是契约。
        let mut keys: Vec<&str> = state
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "active",
                "decision",
                "mode",
                "playlist_len",
                "prefs_patch",
                "reason"
            ]
        );
    }

    /// 时钟只在 `state_json` 里推进：两次调用之间累计的增量喂给纯策略。
    ///
    /// 这里不 sleep：直接手工把上一次时刻**拨回**（等价于「5 秒前的快照」），
    /// 断言第二次调用确实到下一点——若时钟没推进，第二次会仍是 `interval_not_due`。
    #[test]
    fn state_json_advances_the_clock_between_calls() {
        let mut rt = WallpaperRuntime::new(
            noop_services(),
            serde_json::json!({"mode": "interval", "interval_secs": 5, "playlist_len": 2}),
        );
        let first = rt.state_json().unwrap();
        assert_eq!(first["decision"]["index"], serde_json::json!(0));
        // 未到点：紧接着再取一次仍是 none/interval_not_due（不是每次都换）。
        let immediate = rt.state_json().unwrap();
        assert_eq!(immediate["decision"]["kind"], serde_json::json!("none"));
        assert_eq!(immediate["reason"], serde_json::json!("interval_not_due"));
        // 把上次时刻拨回 6 秒（> interval）→ 下一次快照必定前进一格。
        rt.last_state_at = Some(Instant::now() - std::time::Duration::from_secs(6));
        let advanced = rt.state_json().unwrap();
        assert_eq!(advanced["decision"]["kind"], serde_json::json!("advance"));
        assert_eq!(advanced["decision"]["index"], serde_json::json!(1));
        assert_eq!(advanced["reason"], serde_json::json!("advance"));
    }

    /// **interval 的 state 轨迹**（Wave 3 §3B 必做 3，可脚本验证）：
    /// 相隔一个 `interval_secs` 的快照序列按
    /// `advance 0 → none → advance 1 → advance 2 → advance 0` 推进，
    /// `prefs_patch` 与 `reason` 逐拍对应（含取模回绕）。
    ///
    /// 不 sleep：直接拨 `last_state_at`（生产路径 `Instant` 增量的等价物）。
    #[test]
    fn state_trajectory_interval_advances_index_over_time() {
        let mut rt = WallpaperRuntime::new(
            noop_services(),
            serde_json::json!({"mode": "interval", "interval_secs": 5, "playlist_len": 3}),
        );
        let mut kinds: Vec<String> = Vec::new();
        let mut indices: Vec<Option<u64>> = Vec::new();
        let mut patches: Vec<serde_json::Value> = Vec::new();
        let mut reasons: Vec<String> = Vec::new();
        for step in 0..5 {
            rt.last_state_at = match step {
                // 首帧（无上一次时刻）→ 增量 0 → 立即上屏第 0 张。
                0 => None,
                // 相隔 0 ms → 未到点。
                1 => Some(Instant::now()),
                // 相隔 6 s > 5 s → 前进一格。
                _ => Some(Instant::now() - std::time::Duration::from_secs(6)),
            };
            let state = rt.state_json().unwrap();
            kinds.push(
                state["decision"]["kind"]
                    .as_str()
                    .unwrap_or("?")
                    .to_string(),
            );
            indices.push(state["decision"]["index"].as_u64());
            patches.push(state["prefs_patch"].clone());
            reasons.push(state["reason"].as_str().unwrap_or("?").to_string());
        }
        assert_eq!(
            kinds,
            vec!["advance", "none", "advance", "advance", "advance"]
        );
        assert_eq!(indices, vec![Some(0), None, Some(1), Some(2), Some(0)]);
        assert_eq!(
            reasons,
            vec![
                "advance",
                "interval_not_due",
                "advance",
                "advance",
                "advance"
            ]
        );
        assert_eq!(
            patches,
            vec![
                serde_json::json!({"stage_index": 0}),
                serde_json::Value::Null,
                serde_json::json!({"stage_index": 1}),
                serde_json::json!({"stage_index": 2}),
                serde_json::json!({"stage_index": 0}),
            ],
            "prefs_patch 必须与 decision 同源（含回绕）"
        );
    }

    /// **follow_stage 的 state 轨迹**：首次快照给出 `sync_shell_stage_bg` 决策
    /// （patch = `{"sync_shell_stage_bg":true}`），之后恒 none；重配置后重新同步。
    #[test]
    fn state_trajectory_follow_stage_syncs_then_resyncs_after_reconfigure() {
        let mut rt =
            WallpaperRuntime::new(noop_services(), serde_json::json!({"mode": "follow_stage"}));
        let first = rt.state_json().unwrap();
        assert_eq!(first["decision"]["kind"], serde_json::json!("sync_stage"));
        assert_eq!(
            first["prefs_patch"],
            serde_json::json!({"sync_shell_stage_bg": true})
        );
        let second = rt.state_json().unwrap();
        assert_eq!(second["decision"]["kind"], serde_json::json!("none"));
        assert!(second["prefs_patch"].is_null());
        // 重配置（用户改了配置）→ 轨迹重新从头走一遍。
        rt.reconfigure(serde_json::json!({"mode": "follow_stage"}));
        rt.last_state_at = None;
        let third = rt.state_json().unwrap();
        assert_eq!(third["decision"]["kind"], serde_json::json!("sync_stage"));
        assert_eq!(
            third["prefs_patch"],
            serde_json::json!({"sync_shell_stage_bg": true})
        );
    }

    /// `off` 模式：不动作、无 patch（`prefs_patch` 明确是 `null`，
    /// 前端据此跳过——**不是**假装成功）。
    #[test]
    fn state_json_off_mode_never_acts() {
        let mut rt = WallpaperRuntime::new(noop_services(), serde_json::json!({"playlist_len": 3}));
        for _ in 0..3 {
            let state = rt.state_json().unwrap();
            assert_eq!(state["mode"], serde_json::json!("off"));
            assert_eq!(state["active"], serde_json::json!(false));
            assert_eq!(state["decision"], serde_json::json!({"kind": "none"}));
            assert!(state["prefs_patch"].is_null(), "off 不得产出 patch");
            assert_eq!(state["reason"], serde_json::json!("mode_off"));
        }
    }

    /// `follow_stage`：首次给同步 patch，之后恒 none（不重复同步）。
    #[test]
    fn state_json_follow_stage_syncs_once() {
        let mut rt =
            WallpaperRuntime::new(noop_services(), serde_json::json!({"mode": "follow_stage"}));
        let first = rt.state_json().unwrap();
        assert_eq!(first["decision"]["kind"], serde_json::json!("sync_stage"));
        assert_eq!(
            first["prefs_patch"],
            serde_json::json!({"sync_shell_stage_bg": true})
        );
        assert_eq!(first["reason"], serde_json::json!("sync_stage"));
        let second = rt.state_json().unwrap();
        assert_eq!(second["decision"]["kind"], serde_json::json!("none"));
        assert!(second["prefs_patch"].is_null());
        assert_eq!(second["reason"], serde_json::json!("already_synced"));
    }

    /// 列表为空：`interval` 不动作，原因是 `playlist_empty`（可诊断，不是哑掉）。
    #[test]
    fn state_json_interval_with_empty_playlist_reports_reason() {
        let mut rt = WallpaperRuntime::new(
            noop_services(),
            serde_json::json!({"mode": "interval", "interval_secs": 5}),
        );
        let state = rt.state_json().unwrap();
        assert_eq!(state["playlist_len"], serde_json::json!(0));
        assert_eq!(state["decision"], serde_json::json!({"kind": "none"}));
        assert!(state["prefs_patch"].is_null());
        assert_eq!(state["reason"], serde_json::json!("playlist_empty"));
    }

    /// `playlist_len` 从 config 读并**两端钳位**（负数 → 0，超上限 → 上限）。
    #[test]
    fn playlist_len_from_config_is_clamped_at_runtime() {
        let rt = WallpaperRuntime::new(
            noop_services(),
            serde_json::json!({"mode": "interval", "playlist_len": 9999}),
        );
        assert_eq!(rt.playlist_len(), MAX_PLAYLIST_LEN);
        let zero = WallpaperRuntime::new(
            noop_services(),
            serde_json::json!({"mode": "interval", "playlist_len": -3}),
        );
        assert_eq!(zero.playlist_len(), 0);
        // 缺字段 = 0。
        assert_eq!(
            WallpaperRuntime::new(noop_services(), serde_json::json!({})).playlist_len(),
            0
        );
    }

    /// `reconfigure` 也重读 `playlist_len`（不只是构造时读一次）。
    #[test]
    fn reconfigure_rereads_playlist_len() {
        let mut rt = WallpaperRuntime::new(noop_services(), serde_json::json!({}));
        assert_eq!(rt.playlist_len(), 0);
        rt.reconfigure(serde_json::json!({"mode": "interval", "playlist_len": 4}));
        assert_eq!(rt.playlist_len(), 4);
        // 新列表长度 0 时策略挂起（不会拿着旧长度空转）。
        rt.reconfigure(serde_json::json!({"mode": "interval", "playlist_len": 0}));
        assert_eq!(rt.playlist_len(), 0);
        assert_eq!(rt.tick(999_999), WallpaperDecision::None);
    }

    /// `set_playlist_len` 走同一条上限（集成方多写一个 0 不会无界膨胀）。
    #[test]
    fn set_playlist_len_is_capped() {
        let mut rt = WallpaperRuntime::new(noop_services(), serde_json::json!({}));
        rt.set_playlist_len(999);
        assert_eq!(rt.playlist_len(), MAX_PLAYLIST_LEN);
    }
}
