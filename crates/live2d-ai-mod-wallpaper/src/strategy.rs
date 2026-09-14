//! 壁纸策略**纯逻辑**（无 host 依赖、无时钟、无 IO）——本 Mod 的核心可测面。
//!
//! 本文件刻意不 `use live2d_ai_mod_system`：策略只做「根据已过去的时长 + 播放列表
//! 长度，决定现在该不该换、换成哪张」。时间由调用方以**增量毫秒**喂入
//!（[`WallpaperStrategy::tick`]），因此单测不需要 sleep、也不需要假时钟。
//!
//! # 三档模式
//!
//! | mode | 决策 | 语义 |
//! |---|---|---|
//! | `off` | [`WallpaperDecision::None`] | **不接管**壁纸（产品既有行为保持不变） |
//! | `follow_stage` | [`WallpaperDecision::SyncStage`]（启动后**只发一次**） | 壳跟随舞台：等价 `DisplayPrefs.syncShellStageBg = true` |
//! | `interval` | [`WallpaperDecision::Advance`] | 每 `interval_secs` 前进播放列表一格 |
//!
//! # 刻意不做
//!
//! - **不追帧**：一次 `tick` 至多换一张；长时间挂起后不会「补播」一大串
//!   （与「不做大轮播」同一条纪律，见 `docs/architecture/wallpaper-mod-v0.md` §4）。
//! - **不持有图片**：只持有播放列表**长度**与游标；图从哪来由集成方（现有
//!   `DisplayPrefs` / stage-bg 通道）决定。本 crate 不读文件、不存 dataURL。
//! - **不读进程环境 / 不碰密钥**：配置只来自注入的 namespaced JSON。

/// 缺省切换间隔（秒）。缺字段 / 非法值回落它。
pub const DEFAULT_INTERVAL_SECS: u64 = 300;

/// 间隔下限（秒）：低于此值退化成闪烁，钳到下限。
pub const MIN_INTERVAL_SECS: u64 = 5;

/// 间隔上限（秒，24h）：避免「等于关不掉」的荒唐配置。
pub const MAX_INTERVAL_SECS: u64 = 86_400;

/// 壁纸模式（配置值 `off` / `follow_stage` / `interval`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperMode {
    /// 不接管。
    Off,
    /// 与舞台同步（壳背景 = 舞台背景，一份真相）。
    FollowStage,
    /// 定时切换播放列表。
    Interval,
}

impl WallpaperMode {
    /// 稳定字符串 id（配置值 / 日志 / 前端 select 选项同源）。
    pub const fn as_str(self) -> &'static str {
        match self {
            WallpaperMode::Off => "off",
            WallpaperMode::FollowStage => "follow_stage",
            WallpaperMode::Interval => "interval",
        }
    }

    /// 从 Mod config JSON 读取 `mode`。
    ///
    /// **宽容口径**：未知值 / 非字符串 / 缺失 → [`WallpaperMode::Off`]。
    /// 配置写错不该把壁纸变成不可预期的行为（fail-safe 到「不接管」）。
    pub fn from_config(config: &serde_json::Value) -> Self {
        match config.get("mode").and_then(|v| v.as_str()) {
            Some("follow_stage") => WallpaperMode::FollowStage,
            Some("interval") => WallpaperMode::Interval,
            _ => WallpaperMode::Off,
        }
    }

    /// 该模式是否会产出决策（`off` 永远不动作）。
    pub const fn is_active(self) -> bool {
        !matches!(self, WallpaperMode::Off)
    }
}

/// 生效配置（模式 + 已钳的间隔）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WallpaperConfig {
    /// 生效模式。
    pub mode: WallpaperMode,
    /// 生效间隔（秒，已钳在 `[MIN_INTERVAL_SECS, MAX_INTERVAL_SECS]`）。
    pub interval_secs: u64,
}

impl WallpaperConfig {
    /// 从 Mod config JSON 构造（`mode` + `interval_secs`）。
    pub fn from_json(config: &serde_json::Value) -> Self {
        Self {
            mode: WallpaperMode::from_config(config),
            interval_secs: interval_secs_from_config(config),
        }
    }

    /// 间隔的毫秒形态（饱和乘法；`interval_secs` 已非 0）。
    pub const fn interval_ms(&self) -> u64 {
        self.interval_secs.saturating_mul(1000)
    }
}

/// 从 Mod config JSON 读取 `interval_secs` 并钳位。
///
/// - 接受整数与整数形态的浮点（`300` / `300.0`）；
/// - 负数 / `NaN` / 无穷 / 非数字 / 缺失 → [`DEFAULT_INTERVAL_SECS`]；
/// - 结果钳在 `[MIN_INTERVAL_SECS, MAX_INTERVAL_SECS]`（**永不返回 0**，
///   于是 [`WallpaperConfig::interval_ms`] 恒非 0，策略里没有除零路径）。
pub fn interval_secs_from_config(config: &serde_json::Value) -> u64 {
    let raw = config.get("interval_secs").and_then(|v| {
        v.as_u64().or_else(|| {
            v.as_f64()
                .filter(|f| f.is_finite() && *f >= 0.0)
                .map(|f| f as u64)
        })
    });
    raw.unwrap_or(DEFAULT_INTERVAL_SECS)
        .clamp(MIN_INTERVAL_SECS, MAX_INTERVAL_SECS)
}

/// 一次 `tick` 的决策（纯数据；集成方把它翻译成 `DisplayPrefs` / stage-bg 动作）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallpaperDecision {
    /// 不动作（`off`、`follow_stage` 已同步过、或 `interval` 未到点）。
    None,
    /// 壳跟随舞台：`DisplayPrefs.syncShellStageBg = true`。
    SyncStage,
    /// 切到播放列表第 `index` 张（`0 <= index < playlist_len`）。
    Advance {
        /// 目标下标（从 0 起，已按列表长度取模）。
        index: usize,
    },
}

/// 壁纸策略状态机（纯逻辑，可脱机单测）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperStrategy {
    config: WallpaperConfig,
    /// 播放列表长度（0 = 没有可切的图，`interval` 模式保持不动）。
    playlist_len: usize,
    /// 当前下标。
    cursor: usize,
    /// 距上次换图累计的毫秒（`interval` 模式）。
    elapsed_ms: u64,
    /// `follow_stage` 是否已发过 `SyncStage`（只发一次）。
    synced: bool,
    /// `interval` 是否已完成「首帧」（启动即显示第 0 张）。
    started: bool,
}

impl WallpaperStrategy {
    /// 用生效配置 + 播放列表长度构造。游标从 0 起。
    pub fn new(config: WallpaperConfig, playlist_len: usize) -> Self {
        Self {
            config,
            playlist_len,
            cursor: 0,
            elapsed_ms: 0,
            synced: false,
            started: false,
        }
    }

    /// 从 namespaced config JSON 直接构造（供 host / 单测）。
    pub fn from_config_json(config: &serde_json::Value, playlist_len: usize) -> Self {
        Self::new(WallpaperConfig::from_json(config), playlist_len)
    }

    /// 当前生效配置。
    pub const fn config(&self) -> WallpaperConfig {
        self.config
    }

    /// 当前模式。
    pub const fn mode(&self) -> WallpaperMode {
        self.config.mode
    }

    /// 当前播放列表下标（`playlist_len == 0` 时恒 0）。
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// 播放列表长度。
    pub const fn playlist_len(&self) -> usize {
        self.playlist_len
    }

    /// **重配置**：换模式 / 换间隔后计时与同步标志全部复位（游标回 0）。
    ///
    /// 复位是有意的：`interval` 换间隔后不应沿用旧模式的累计时长，
    /// `follow_stage` 换进来后必须重新发一次 `SyncStage`（配置改了就重新同步）。
    pub fn reconfigure(&mut self, config: WallpaperConfig) {
        self.config = config;
        self.cursor = 0;
        self.elapsed_ms = 0;
        self.synced = false;
        self.started = false;
    }

    /// 更新播放列表长度（图库变化时由集成方调用）。
    ///
    /// - 长度 0 → 游标归 0、`started` 复位（下次有图时重新首帧）；
    /// - 长度变小 → 游标钳到 `len - 1`（不越界）。
    pub fn set_playlist_len(&mut self, len: usize) {
        self.playlist_len = len;
        if len == 0 {
            self.cursor = 0;
            self.started = false;
        } else if self.cursor >= len {
            self.cursor = len - 1;
        }
    }

    /// 推进 `delta_ms` 毫秒并给出决策。
    ///
    /// 各模式语义见模块头注。要点：
    /// - `off` 恒 `None`；
    /// - `follow_stage` 首次 `tick` 发 `SyncStage`，之后恒 `None`；
    /// - `interval` 首帧发 `Advance { index: 0 }`，之后每满一个间隔前进一格并取模；
    ///   列表为空时保持不动（`None`）；
    /// - 累计用**饱和加法**；换图后计时归零，**不做追帧**。
    pub fn tick(&mut self, delta_ms: u64) -> WallpaperDecision {
        match self.config.mode {
            WallpaperMode::Off => WallpaperDecision::None,
            WallpaperMode::FollowStage => {
                if self.synced {
                    WallpaperDecision::None
                } else {
                    self.synced = true;
                    WallpaperDecision::SyncStage
                }
            }
            WallpaperMode::Interval => {
                if self.playlist_len == 0 {
                    return WallpaperDecision::None;
                }
                if !self.started {
                    self.started = true;
                    self.elapsed_ms = 0;
                    self.cursor = 0;
                    return WallpaperDecision::Advance { index: self.cursor };
                }
                self.elapsed_ms = self.elapsed_ms.saturating_add(delta_ms);
                if self.elapsed_ms >= self.config.interval_ms() {
                    // 换后重新计时：一次 tick 至多换一张，长挂起也不补播。
                    self.elapsed_ms = 0;
                    self.cursor = (self.cursor + 1) % self.playlist_len;
                    WallpaperDecision::Advance { index: self.cursor }
                } else {
                    WallpaperDecision::None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(mode: WallpaperMode, secs: u64) -> WallpaperConfig {
        WallpaperConfig {
            mode,
            interval_secs: secs,
        }
    }

    // ---------------------------------------------------- 配置解析

    #[test]
    fn mode_from_config_maps_known_values_and_defaults_off() {
        let m = WallpaperMode::from_config;
        assert_eq!(m(&serde_json::json!({})), WallpaperMode::Off);
        assert_eq!(m(&serde_json::json!({"mode": "off"})), WallpaperMode::Off);
        assert_eq!(
            m(&serde_json::json!({"mode": "follow_stage"})),
            WallpaperMode::FollowStage
        );
        assert_eq!(
            m(&serde_json::json!({"mode": "interval"})),
            WallpaperMode::Interval
        );
        // 未知 / 非字符串 / 大小写不符 → off（宽容，fail-safe）。
        assert_eq!(
            m(&serde_json::json!({"mode": "carousel"})),
            WallpaperMode::Off
        );
        assert_eq!(
            m(&serde_json::json!({"mode": "Follow_Stage"})),
            WallpaperMode::Off
        );
        assert_eq!(m(&serde_json::json!({"mode": 7})), WallpaperMode::Off);
        assert!(!WallpaperMode::Off.is_active());
        assert!(WallpaperMode::FollowStage.is_active());
        assert!(WallpaperMode::Interval.is_active());
        assert_eq!(WallpaperMode::Off.as_str(), "off");
        assert_eq!(WallpaperMode::FollowStage.as_str(), "follow_stage");
        assert_eq!(WallpaperMode::Interval.as_str(), "interval");
    }

    #[test]
    fn interval_secs_defaults_and_clamps() {
        let s = interval_secs_from_config;
        assert_eq!(s(&serde_json::json!({})), DEFAULT_INTERVAL_SECS);
        assert_eq!(s(&serde_json::json!({"interval_secs": 60})), 60);
        // 整数形态浮点也接受。
        assert_eq!(s(&serde_json::json!({"interval_secs": 90.0})), 90);
        assert_eq!(s(&serde_json::json!({"interval_secs": 90.9})), 90);
        // 过小 / 0 / 负数 → 钳到下限（**永不 0**）。
        assert_eq!(
            s(&serde_json::json!({"interval_secs": 0})),
            MIN_INTERVAL_SECS
        );
        assert_eq!(
            s(&serde_json::json!({"interval_secs": 1})),
            MIN_INTERVAL_SECS
        );
        assert_eq!(
            s(&serde_json::json!({"interval_secs": -5})),
            DEFAULT_INTERVAL_SECS
        );
        // 过大 → 钳到上限。
        assert_eq!(
            s(&serde_json::json!({"interval_secs": 999_999_999u64})),
            MAX_INTERVAL_SECS
        );
        // 非数字 → 缺省。
        assert_eq!(
            s(&serde_json::json!({"interval_secs": "60"})),
            DEFAULT_INTERVAL_SECS
        );
        assert_eq!(
            s(&serde_json::json!({"interval_secs": null})),
            DEFAULT_INTERVAL_SECS
        );
        // 钳位后 interval_ms 恒非 0。
        let c = WallpaperConfig::from_json(&serde_json::json!({"interval_secs": 0}));
        assert!(c.interval_ms() > 0);
    }

    // ---------------------------------------------------- off

    #[test]
    fn off_never_acts() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Off, 5), 3);
        for _ in 0..100 {
            assert_eq!(s.tick(10_000), WallpaperDecision::None);
        }
        assert_eq!(s.cursor(), 0, "off 不改游标");
    }

    // ---------------------------------------------------- follow_stage

    #[test]
    fn follow_stage_emits_sync_exactly_once() {
        let mut s =
            WallpaperStrategy::from_config_json(&serde_json::json!({"mode": "follow_stage"}), 3);
        assert_eq!(s.mode(), WallpaperMode::FollowStage);
        assert_eq!(s.tick(0), WallpaperDecision::SyncStage);
        assert_eq!(s.tick(0), WallpaperDecision::None);
        assert_eq!(s.tick(60_000), WallpaperDecision::None);
    }

    #[test]
    fn follow_stage_resyncs_after_reconfigure() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::FollowStage, 300), 0);
        assert_eq!(s.tick(0), WallpaperDecision::SyncStage);
        assert_eq!(s.tick(0), WallpaperDecision::None);
        s.reconfigure(cfg(WallpaperMode::FollowStage, 300));
        assert_eq!(s.tick(0), WallpaperDecision::SyncStage, "重配置后重新同步");
    }

    // ---------------------------------------------------- interval

    #[test]
    fn interval_first_tick_shows_first_image() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 10), 4);
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
        assert_eq!(s.cursor(), 0);
    }

    #[test]
    fn interval_advances_only_when_full_interval_elapsed() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 10), 3);
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
        // 差 1ms 不换。
        assert_eq!(s.tick(9_999), WallpaperDecision::None);
        // 第 10000ms 换。
        assert_eq!(s.tick(1), WallpaperDecision::Advance { index: 1 });
        assert_eq!(s.cursor(), 1);
    }

    #[test]
    fn interval_wraps_around_playlist() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 1), 2);
        let step = s.config().interval_ms();
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
        assert_eq!(s.tick(step), WallpaperDecision::Advance { index: 1 });
        assert_eq!(
            s.tick(step),
            WallpaperDecision::Advance { index: 0 },
            "取模回绕"
        );
    }

    #[test]
    fn interval_holds_when_playlist_empty() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 1), 0);
        assert_eq!(s.tick(0), WallpaperDecision::None);
        assert_eq!(s.tick(999_999), WallpaperDecision::None);
        assert_eq!(s.cursor(), 0);
    }

    /// 长时间挂起（单次巨大 delta）只换一张——不追帧、不补播。
    #[test]
    fn interval_does_not_catch_up_after_long_suspend() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 1), 3);
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
        // 等于 1000 个间隔；仍只前进一格。
        let d = s.tick(s.config().interval_ms() * 1_000);
        assert_eq!(d, WallpaperDecision::Advance { index: 1 });
        // 紧接一次极小 tick 不再换（计时已归零）。
        assert_eq!(s.tick(0), WallpaperDecision::None);
    }

    #[test]
    fn interval_overflow_is_saturating_not_wrapping() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 5), 2);
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
        // u64::MAX 饱和后仍判定「到点」，且不 panic。
        assert_eq!(s.tick(u64::MAX), WallpaperDecision::Advance { index: 1 });
        // 下一 tick 立即到点（饱和值未被 clamp，仍 >= interval）。
        let _ = s.tick(u64::MAX);
    }

    #[test]
    fn set_playlist_len_clamps_and_resets() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 1), 5);
        let step = s.config().interval_ms();
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
        for _ in 0..4 {
            let _ = s.tick(step);
        }
        assert_eq!(s.cursor(), 4);
        // 缩短列表 → 游标钳入界。
        s.set_playlist_len(2);
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.tick(step), WallpaperDecision::Advance { index: 0 });
        // 清空 → 挂起；重新有图 → 重新首帧。
        s.set_playlist_len(0);
        assert_eq!(s.tick(step), WallpaperDecision::None);
        s.set_playlist_len(3);
        assert_eq!(s.tick(step), WallpaperDecision::Advance { index: 0 });
    }

    #[test]
    fn reconfigure_resets_timing_and_cursor() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Interval, 1), 3);
        let step = s.config().interval_ms();
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
        assert_eq!(s.tick(step), WallpaperDecision::Advance { index: 1 });
        s.reconfigure(cfg(WallpaperMode::Interval, 1));
        assert_eq!(s.cursor(), 0, "重配置后游标回 0");
        assert_eq!(s.tick(0), WallpaperDecision::Advance { index: 0 });
    }

    #[test]
    fn mode_switch_between_off_and_follow_stage() {
        let mut s = WallpaperStrategy::new(cfg(WallpaperMode::Off, 10), 0);
        assert_eq!(s.tick(0), WallpaperDecision::None);
        s.reconfigure(cfg(WallpaperMode::FollowStage, 10));
        assert_eq!(s.tick(0), WallpaperDecision::SyncStage);
        s.reconfigure(cfg(WallpaperMode::Off, 10));
        assert_eq!(s.tick(0), WallpaperDecision::None);
    }
}
