//! `preset` 协议解析（自 `preset/mod.rs` 机械拆出）。

use super::data::{MAX_INTENSITY, MAX_TTL_MS, PresetSpec, REVOKE_ID, preset};

/// 预设来源（P0-1；仅用于 HUD / 调试面板显示「谁在演」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetSource {
    /// 开发工具「动作调试」直接下发。
    Debug,
    /// 导演 Mod 经 Flutter 转发。
    Director,
    /// 缺省 / 不认识的来源。
    Unknown,
}

impl PresetSource {
    /// 稳定字符串（HUD / 状态行回显）。
    pub const fn as_str(self) -> &'static str {
        match self {
            PresetSource::Debug => "debug",
            PresetSource::Director => "director",
            PresetSource::Unknown => "unknown",
        }
    }

    /// 解析来源字符串；未知一律 `PresetSource::Unknown`（宽容，不失败）。
    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "debug" | "ui" => PresetSource::Debug,
            "director" => PresetSource::Director,
            _ => PresetSource::Unknown,
        }
    }
}

/// 一条解析后的 `preset` 指令（P0-1）。
#[derive(Debug, Clone, PartialEq)]
pub enum PresetCommand {
    /// 应用一条预设。
    Apply {
        spec: &'static PresetSpec,
        intensity: f32,
        ttl_ms: f64,
        source: PresetSource,
    },
    /// `id:"none"`：撤销（按槽从屏幕值放回，见 `release`）。
    Revoke,
    /// 未知 / 缺失 id：静默忽略（不动任何参数，不回执）。
    Ignore,
}

/// 从原始字段解析一条 `preset` 指令（**纯函数**，原生可测；内建表）。
///
/// 缺省与钳位（P0-1，兼容旧 `{id}`）：
/// - `intensity` 缺省 / 非有限 → `1.0`，钳 `[0, MAX_INTENSITY]`；
/// - `ttl_ms` 缺省 / 非正 → 该预设的 `duration_ms`，钳 `(0, MAX_TTL_MS]`；
/// - `source` 缺省 / 未知 → `PresetSource::Unknown`；
/// - `id:"none"` → `PresetCommand::Revoke`；未知（含已删除的旧 id）→ `PresetCommand::Ignore`。
// 内建表版的协议解析：wasm 侧走 PresetRuntime::resolve（外置表）；测试仍用这个。
#[allow(dead_code)]
pub fn parse_command(
    id: Option<&str>,
    intensity: Option<f64>,
    ttl_ms: Option<f64>,
    source: Option<&str>,
) -> PresetCommand {
    parse_command_with(
        |id| preset(id).map(|s| (s, None)),
        id,
        intensity,
        ttl_ms,
        source,
    )
}

/// 同 parse_command，但用**给定的查找函数**解析 id。
///
/// PresetRuntime::resolve 用它接外置表（&'static PresetSpec 是同一类型，
/// 只是来源不同）。查找返回的 `Option<f32>` 保留给「查表默认强度」这一扩展点；
/// 当前所有实现都返回 `None`（旧 id 的别名默认强度已删除）。
pub(super) fn parse_command_with(
    lookup: impl Fn(&str) -> Option<(&'static PresetSpec, Option<f32>)>,
    id: Option<&str>,
    intensity: Option<f64>,
    ttl_ms: Option<f64>,
    source: Option<&str>,
) -> PresetCommand {
    let Some(raw_id) = id else {
        return PresetCommand::Ignore;
    };
    let id = raw_id.trim();
    if id == REVOKE_ID {
        return PresetCommand::Revoke;
    }
    let Some((spec, lookup_default)) = lookup(id) else {
        return PresetCommand::Ignore;
    };
    // 显式给了合法 intensity 就用它；缺省 / 非有限 → 查表默认（当前恒 1.0）。
    let intensity = match intensity {
        Some(v) if v.is_finite() => v as f32,
        _ => lookup_default.unwrap_or(1.0),
    }
    .clamp(0.0, MAX_INTENSITY);
    let ttl_ms = ttl_ms
        .filter(|v| v.is_finite() && *v > 0.0)
        .map_or(spec.duration_ms, |v| v.min(MAX_TTL_MS));
    let source = source.map_or(PresetSource::Unknown, PresetSource::parse);
    PresetCommand::Apply {
        spec,
        intensity,
        ttl_ms,
        source,
    }
}
