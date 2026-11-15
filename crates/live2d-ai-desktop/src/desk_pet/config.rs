//! 桌宠窗配置（`PetConfig`）：纯解析 + 钳位，**Win32 无关**，Linux 单测能跑。
//!
//! 合同见 `D:\live2dai\plan\PLAN-wsl-desk-pet-mod-2026-11-16.md` §3（两边同一张表）。
//! 四个键住在 `mods.json` 的 `mods.desk-pet.config`：
//!
//! | key | 类型 | 缺省 | 边界 |
//! | --- | --- | --- | --- |
//! | `hit` | string | `opaque` | `through` / `opaque` / `window` |
//! | `opacity` | number | `1.0` | `0.1..=1.0` |
//! | `always_on_top` | bool | `true` | |
//! | `height_ratio` | number | `0.75` | `0.25..=0.9` |
//!
//! **坏配置不是错误**：缺键、坏类型、未知字符串、NaN、越界，一律**该字段回落
//! 缺省**、其它字段照常，永不向调用方返回 `Err`——宠物窗是 HTTP 服务的旁路，
//! 一份坏 JSON 不该阻止开窗，更不该让进程退出码变化。
//!
//! 本文件（H2）还不读文件：调用方传缺省值。读 `mods.json` 是配对 WSL 计划
//! rc.2 的事，它把文件里的对象直接交给 `PetConfig::from_json`（同一套回落）。

// 解析入口这一步只有单测在调：读 `mods.json` 是配对 WSL 计划 rc.2 的事，在那之前
// 命令行路径只用 `PetConfig::default()`。留着它们是为了 rc.2 直接接上同一份实现。
#![allow(dead_code)]

use serde_json::Value;

/// 命中模式（`hit` 键）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HitMode {
    /// 按最近一帧的 alpha 命中：alpha ≥ 16 的像素吃鼠标，其余穿透（缺省）。
    #[default]
    Opaque,
    /// 整窗穿透（0.4.3 的行为；恢复常驻 `WS_EX_TRANSPARENT`）。
    Through,
    /// 整矩形吃鼠标，空像素也吃——只给调试，产品缺省不用。
    Window,
}

impl HitMode {
    /// 解析 `hit` 字符串；未知字符串返回 `None`（调用方回落缺省）。
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "opaque" => Some(Self::Opaque),
            "through" => Some(Self::Through),
            "window" => Some(Self::Window),
            _ => None,
        }
    }

    /// 日志用短名（与 `parse` 的表逐字一致）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::Through => "through",
            Self::Window => "window",
        }
    }
}

/// `hit` 缺省。
pub const HIT_DEFAULT: HitMode = HitMode::Opaque;
/// `opacity` 缺省（不淡出）。
pub const OPACITY_DEFAULT: f32 = 1.0;
/// `opacity` 下界：**永不为 0**（0 等于「窗还在但看不见」，比关窗更难查）。
pub const OPACITY_MIN: f32 = 0.1;
/// `opacity` 上界。
pub const OPACITY_MAX: f32 = 1.0;
/// `always_on_top` 缺省。
pub const ALWAYS_ON_TOP_DEFAULT: bool = true;
/// `height_ratio` 缺省（0.4.3 的「屏高 3/4」构图）。
pub const HEIGHT_RATIO_DEFAULT: f32 = 0.75;
/// `height_ratio` 下界。
pub const HEIGHT_RATIO_MIN: f32 = 0.25;
/// `height_ratio` 上界。
pub const HEIGHT_RATIO_MAX: f32 = 0.9;

/// 一扇宠物窗的全部可调项。`Default` 与上表的缺省列逐字一致。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PetConfig {
    pub hit: HitMode,
    pub opacity: f32,
    pub always_on_top: bool,
    pub height_ratio: f32,
}

impl Default for PetConfig {
    fn default() -> Self {
        Self {
            hit: HIT_DEFAULT,
            opacity: OPACITY_DEFAULT,
            always_on_top: ALWAYS_ON_TOP_DEFAULT,
            height_ratio: HEIGHT_RATIO_DEFAULT,
        }
    }
}

impl PetConfig {
    /// 从 `mods.desk-pet.config` 的 JSON 值解析；**任何毛病都只回落该字段**。
    ///
    /// 传入非对象（`null` / 数组 / 字符串 / 数字）等于「整段缺失」⇒ 全缺省。
    pub fn from_json(value: &Value) -> Self {
        let mut config = Self::default();
        let Some(object) = value.as_object() else {
            return config;
        };
        if let Some(mode) = object
            .get("hit")
            .and_then(Value::as_str)
            .and_then(HitMode::parse)
        {
            config.hit = mode;
        }
        if let Some(opacity) = object.get("opacity").and_then(Value::as_f64) {
            config.opacity = clamp_closed(opacity, OPACITY_MIN, OPACITY_MAX, OPACITY_DEFAULT);
        }
        if let Some(always_on_top) = object.get("always_on_top").and_then(Value::as_bool) {
            config.always_on_top = always_on_top;
        }
        if let Some(ratio) = object.get("height_ratio").and_then(Value::as_f64) {
            config.height_ratio = clamp_closed(
                ratio,
                HEIGHT_RATIO_MIN,
                HEIGHT_RATIO_MAX,
                HEIGHT_RATIO_DEFAULT,
            );
        }
        config
    }
}

/// 闭区间钳位；NaN / 无穷回落缺省（`f32::clamp` 遇 NaN 会把 NaN 传下去，不能直接用）。
///
/// 值合法但越界 ⇒ 钳到边界（计划的「越界回落」要的是「不报错」，钳位比丢掉
/// 用户意图更可预测：写 1.5 的人想要「尽量不透明」）。
fn clamp_closed(value: f64, min: f32, max: f32, fallback: f32) -> f32 {
    if !value.is_finite() {
        return fallback;
    }
    (value as f32).clamp(min, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_matches_contract_table() {
        let config = PetConfig::default();
        assert_eq!(config.hit, HitMode::Opaque);
        assert_eq!(config.opacity, 1.0);
        assert!(config.always_on_top);
        assert_eq!(config.height_ratio, 0.75);
    }

    #[test]
    fn empty_object_and_non_object_fall_back_to_default() {
        for value in [json!({}), json!(null), json!([]), json!("opaque"), json!(7)] {
            assert_eq!(
                PetConfig::from_json(&value),
                PetConfig::default(),
                "{value}"
            );
        }
    }

    #[test]
    fn parses_all_four_keys() {
        let config = PetConfig::from_json(&json!({
            "hit": "through",
            "opacity": 0.5,
            "always_on_top": false,
            "height_ratio": 0.4,
        }));
        assert_eq!(config.hit, HitMode::Through);
        assert_eq!(config.opacity, 0.5);
        assert!(!config.always_on_top);
        assert_eq!(config.height_ratio, 0.4);
    }

    #[test]
    fn unknown_or_wrong_typed_hit_keeps_default() {
        for value in [
            json!({"hit": "click"}),
            json!({"hit": 1}),
            json!({"hit": null}),
        ] {
            assert_eq!(PetConfig::from_json(&value).hit, HitMode::Opaque, "{value}");
        }
        assert_eq!(
            PetConfig::from_json(&json!({"hit": "window"})).hit,
            HitMode::Window
        );
    }

    #[test]
    fn opacity_and_ratio_clamp_instead_of_failing() {
        let low = PetConfig::from_json(&json!({"opacity": 0.0, "height_ratio": 0.01}));
        assert_eq!(low.opacity, OPACITY_MIN);
        assert_eq!(low.height_ratio, HEIGHT_RATIO_MIN);
        let high = PetConfig::from_json(&json!({"opacity": 9.0, "height_ratio": 5.0}));
        assert_eq!(high.opacity, OPACITY_MAX);
        assert_eq!(high.height_ratio, HEIGHT_RATIO_MAX);
        // 永不把 opacity 收成 0：负数也只到 0.1。
        assert_eq!(
            PetConfig::from_json(&json!({"opacity": -3.0})).opacity,
            OPACITY_MIN
        );
    }

    #[test]
    fn bad_types_keep_their_own_default_only() {
        let config = PetConfig::from_json(&json!({
            "hit": "window",
            "opacity": "0.5",
            "always_on_top": "no",
            "height_ratio": true,
        }));
        assert_eq!(config.hit, HitMode::Window);
        assert_eq!(config.opacity, OPACITY_DEFAULT);
        assert!(config.always_on_top);
        assert_eq!(config.height_ratio, HEIGHT_RATIO_DEFAULT);
    }

    #[test]
    fn nan_and_infinite_fall_back_to_default() {
        // serde_json 的 Number 装不下 NaN，但 f64 路径仍要防一手。
        assert_eq!(
            clamp_closed(f64::NAN, OPACITY_MIN, OPACITY_MAX, OPACITY_DEFAULT),
            OPACITY_DEFAULT
        );
        assert_eq!(
            clamp_closed(f64::INFINITY, OPACITY_MIN, OPACITY_MAX, OPACITY_DEFAULT),
            OPACITY_DEFAULT
        );
    }

    #[test]
    fn integers_are_accepted_as_numbers() {
        let config = PetConfig::from_json(&json!({"opacity": 1, "height_ratio": 0}));
        assert_eq!(config.opacity, 1.0);
        assert_eq!(config.height_ratio, HEIGHT_RATIO_MIN);
    }

    #[test]
    fn hit_mode_round_trips_through_parse_and_as_str() {
        for mode in [HitMode::Opaque, HitMode::Through, HitMode::Window] {
            assert_eq!(HitMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(HitMode::parse("OPAQUE"), None);
    }
}
