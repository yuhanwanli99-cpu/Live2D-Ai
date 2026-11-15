//! 事实种类、保留率与加固（遗忘曲线）。
//!
//! 每条事实有强度 `S`（**小时**）与上次加固时间；距今 `t` 小时：
//!
//! ```text
//! R = e^(-t / S)
//! ```
//!
//! `t = S` 时约剩 0.37。分档与取舍在 [crate::hotmem::card] / [crate::hotmem::pin]，
//! 本模块只负责「一条事实现在多新鲜」。

use serde::{Deserialize, Serialize};

/// episode（闲聊）的起始强度：18 小时。
pub const EPISODE_INITIAL_HOURS: f64 = 18.0;
/// episode 再被提到时的强度倍率。
pub const EPISODE_GROWTH: f64 = 1.6;
/// episode 的强度上限：60 天。
pub const EPISODE_MAX_HOURS: f64 = 60.0 * 24.0;
/// episode 低于这个保留率**且**超过 [EPISODE_FORGET_AGE_HOURS] 才删。
pub const EPISODE_FORGET_R: f64 = 0.05;
/// episode 的删除年龄门槛：180 天。
pub const EPISODE_FORGET_AGE_HOURS: f64 = 180.0 * 24.0;

/// durable（耐久）的起始强度：14 天。
pub const DURABLE_INITIAL_HOURS: f64 = 14.0 * 24.0;
/// durable 再被提到时的强度倍率。
pub const DURABLE_GROWTH: f64 = 1.8;
/// durable 的强度上限：约 2 年（**按 730 天写死**，测试按这个常量断言，
/// 不按闰年日历猜——这是上限不是日期）。
pub const DURABLE_MAX_HOURS: f64 = 730.0 * 24.0;

/// 一条事实的种类。落盘 / 抽取结果用 `snake_case` 字面量。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FactKind {
    /// 闲聊片段：起始 18 小时，180 天且 `R < 0.05` 才删。
    Episode,
    /// 耐久事实：起始 14 天，**不参与遗忘清理**（只随用户清理删）。
    Durable,
}

impl FactKind {
    /// 解析抽取结果里的 `kind` 字符串；未知 / 空 = [FactKind::Episode]。
    ///
    /// 「抽失败就只把这一轮原文记成 episode」——所以这个回落方向是**产品语义**，
    /// 不是偷懒：认不出来的东西当闲聊，宁可淡忘也不当耐久事实钉住。
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("durable") => Self::Durable,
            _ => Self::Episode,
        }
    }

    /// 稳定码（落盘 / 日志 / 状态面）。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Episode => "episode",
            Self::Durable => "durable",
        }
    }

    /// 起始强度（小时）。
    pub const fn initial_hours(self) -> f64 {
        match self {
            Self::Episode => EPISODE_INITIAL_HOURS,
            Self::Durable => DURABLE_INITIAL_HOURS,
        }
    }

    /// 再被提到时的强度倍率。
    pub const fn growth(self) -> f64 {
        match self {
            Self::Episode => EPISODE_GROWTH,
            Self::Durable => DURABLE_GROWTH,
        }
    }

    /// 强度上限（小时）。
    pub const fn max_hours(self) -> f64 {
        match self {
            Self::Episode => EPISODE_MAX_HOURS,
            Self::Durable => DURABLE_MAX_HOURS,
        }
    }
}

/// 保留率 `R = e^(-t/S)`。
///
/// 边界（**不 panic、不产出 NaN**）：
/// - `s_hours <= 0` / 非有限 → 0.0（强度坏了当作已经忘光，宁可少说）；
/// - `t_hours < 0`（时钟回拨）→ 按 0 处理，即 `R = 1.0`；
/// - 结果恒在 `[0, 1]`。
pub fn retention(t_hours: f64, s_hours: f64) -> f64 {
    if !s_hours.is_finite() || s_hours <= 0.0 || !t_hours.is_finite() {
        return 0.0;
    }
    let t = if t_hours < 0.0 { 0.0 } else { t_hours };
    let r = (-t / s_hours).exp();
    if r.is_finite() {
        r.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// 一条被再提到后新的强度（倍率 × 当前，封顶 [FactKind::max_hours]）。
///
/// 当前强度坏了（`<= 0` / 非有限）→ 从该类别的起始强度重新起算，不返回 0。
pub fn strengthen(kind: FactKind, s_hours: f64) -> f64 {
    let base = if s_hours.is_finite() && s_hours > 0.0 {
        s_hours
    } else {
        kind.initial_hours()
    };
    (base * kind.growth()).min(kind.max_hours())
}

/// 一条事实（内存投影；落盘形状见 `hotmem.jsonl` / `hotmem-pinned.json`）。
#[derive(Debug, Clone, PartialEq)]
pub struct HotFact {
    /// 稳定 id（进程内自增；读回的记录按落盘 id 继续）。
    pub id: u64,
    /// 事实正文（说话口径，**不含**分档标记）。
    pub text: String,
    /// episode / durable。
    pub kind: FactKind,
    /// 强度（小时）。
    pub s_hours: f64,
    /// 距上次加固的小时数（**由调用方给**，本模块不读时钟）。
    pub age_hours: f64,
}

impl HotFact {
    /// 一条新事实：强度取该类别起始值、年龄 0。
    pub fn new(id: u64, text: impl Into<String>, kind: FactKind) -> Self {
        Self {
            id,
            text: text.into(),
            kind,
            s_hours: kind.initial_hours(),
            age_hours: 0.0,
        }
    }

    /// 当前保留率。
    pub fn retention(&self) -> f64 {
        retention(self.age_hours, self.s_hours)
    }

    /// 现在的档位（见 [crate::hotmem::card::CardTier]）；`R < 0.2` → None（不进热卡）。
    pub fn tier(&self) -> Option<crate::hotmem::card::CardTier> {
        crate::hotmem::card::CardTier::from_retention(self.retention())
    }

    /// 再被提到：强度按倍率加固、年龄归零（时间基点 = 「刚被加固」）。
    pub fn reinforce(&mut self) {
        self.s_hours = strengthen(self.kind, self.s_hours);
        self.age_hours = 0.0;
    }

    /// 是否该被遗忘（**只对 episode 生效**；durable 只在用户清理时删）。
    pub fn is_expired(&self) -> bool {
        self.kind == FactKind::Episode
            && self.retention() < EPISODE_FORGET_R
            && self.age_hours > EPISODE_FORGET_AGE_HOURS
    }
}
