//! 外置动作包表的**解析与校验**（`preset/` 目录下的表解析子模块，
//! 按 AGENTS 行数纪律从原单文件拆出）。
//!
//! 这里只有「JSON → &'static PresetSpec 集合」这一段：
//! 通道白名单、按 kind 分档的幅值上限、宽容但不撒谎的校验规则。运行状态机
//! （双槽 / 强度 morph / 到点撤销）在 mod.rs。

use super::{
    BODY_LIMIT, EXPRESSION_BODY_LIMIT, EXPRESSION_HEAD_LIMIT, EXPRESSION_LIMIT, EXPRESSION_MS,
    HEAD_LIMIT, MAX_INTENSITY, MAX_TTL_MS, MOTION_MS, MotionWave, PRESETS, PresetKind, PresetMorph,
    PresetSpec, SHAKE_CYCLES, ScaleClass, scale_class,
};
// ─────────────────────────────────────────────────────────────────────
// 外置预设表（2026-09-16，P1-1）：assets/actions/presets.json
// ─────────────────────────────────────────────────────────────────────

/// 允许的动作通道白名单（面部 / 头 / 半身角度）。
///
/// **这是红线的一部分**：外置 JSON 里出现白名单外的参数（手臂 / 手指 / 特效 /
/// 口型）会被**丢掉该通道并 warn**，而不是让整张表加载失败。
pub const ALLOWED_PARAMS: &[&str] = &[
    "ParamMouthForm",
    "ParamEyeLSmile",
    "ParamEyeRSmile",
    "ParamEyeLOpen",
    "ParamEyeROpen",
    "ParamBrowLY",
    "ParamBrowRY",
    "ParamAngleX",
    "ParamAngleY",
    "ParamAngleZ",
    "ParamBodyAngleX",
    "ParamBodyAngleY",
    "ParamBodyAngleZ",
];

/// 通道幅值分档上限（身 ≤10，其余 ≤30）——**手势包**口径。
pub fn amplitude_limit(param_id: &str) -> f32 {
    if param_id.starts_with("ParamBodyAngle") {
        BODY_LIMIT
    } else {
        HEAD_LIMIT
    }
}

/// 按 **kind** 与通道取解析期幅值上限。
///
/// 表情包收紧到 `头 ≤12 / 身 ≤4 / 五官 ≤4`（v3：表情不是大姿态）；手势包用
/// `amplitude_limit`。
pub fn pack_limit(kind: PresetKind, param_id: &str) -> f32 {
    match kind {
        PresetKind::Motion => amplitude_limit(param_id),
        PresetKind::Expression => {
            if param_id.starts_with("ParamBodyAngle") {
                EXPRESSION_BODY_LIMIT
            } else if param_id.starts_with("ParamAngle") {
                EXPRESSION_HEAD_LIMIT
            } else {
                EXPRESSION_LIMIT
            }
        }
    }
}

/// 外置动作预设表：`&'static PresetSpec` 的集合（内建表或 JSON 加载结果）。
///
/// # 为什么是 `&'static`
///
/// `PresetSpec` 的 id / 参数名 / 参数切片在运行时协议里是**只读、跨帧**的引用。
/// 外置 JSON 在**启动时加载一次**，之后不再变化；因此把解析出来的 `String` /
/// `Vec` **泄漏成 `&'static`**（`Box::leak`）是这里最小、最诚实的做法。
#[derive(Debug, Clone)]
pub struct PresetTable {
    specs: Vec<&'static PresetSpec>,
    warnings: Vec<String>,
}

impl PresetTable {
    /// 内建表（fallback：JSON 缺失 / 校验失败时用）。
    pub fn builtin() -> Self {
        Self {
            specs: PRESETS.iter().collect(),
            warnings: Vec::new(),
        }
    }

    /// 全部预设（顺序 = JSON 中的顺序）。
    pub fn all(&self) -> &[&'static PresetSpec] {
        &self.specs
    }

    /// 按 id 查（未知 → None）；旧 id 不再解析，与任一未知名等价。
    pub fn get(&self, id: &str) -> Option<&'static PresetSpec> {
        self.specs.iter().find(|s| s.id == id).copied()
    }

    /// 加载期的告警（未知通道被丢 / 幅值被钳 / 单条被跳过）。
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// 从外置 JSON 解析并校验。
    ///
    /// 形状（`presets` 可省略成裸数组）：
    /// ```json
    /// { "presets": [
    ///   { "id": "nod", "kind": "motion", "duration_ms": 900,
    ///     "wave": "single",
    ///     "params": [ { "id": "ParamAngleY", "value": -20.0 } ] }
    /// ] }
    /// ```
    ///
    /// 校验规则（**宽容但不撒谎**）：
    /// - 未知通道 → 丢该通道 + warning（不失败）；
    /// - 幅值越界 → 按 `pack_limit` 钳位 + warning；
    /// - `duration_ms` 非法 → 回落该 kind 的缺省 + warning；
    /// - 表情包缺五官 / 头 / 身任一 → warning（不改形状，仍加载）；
    /// - 单条没有可用参数 / kind 非法 / id 为空 → 跳过该条 + warning；
    /// - JSON 不是合法 JSON / 没有数组 / 一条有效预设都没有 → `Err`。
    pub fn from_json(json: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(json).map_err(|e| format!("不是合法 JSON：{e}"))?;
        let arr = value
            .as_array()
            .or_else(|| value.get("presets").and_then(|p| p.as_array()))
            .ok_or_else(|| "缺少 presets 数组".to_string())?;
        let mut warnings = Vec::new();
        let mut specs: Vec<&'static PresetSpec> = Vec::new();
        for (idx, raw) in arr.iter().enumerate() {
            let id = raw.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
            if id.is_empty() {
                warnings.push(format!("第 {} 条缺 id，已跳过", idx + 1));
                continue;
            }
            if specs.iter().any(|s| s.id == id) {
                warnings.push(format!("{id} 重复，保留先出现的一条"));
                continue;
            }
            let kind = match raw.get("kind").and_then(|v| v.as_str()) {
                Some("expression") => PresetKind::Expression,
                Some("motion") => PresetKind::Motion,
                other => {
                    warnings.push(format!("{id} 的 kind 非法（{other:?}），已跳过"));
                    continue;
                }
            };
            let duration_ms = raw
                .get("duration_ms")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite() && *v > 0.0)
                .map(|v| v.min(MAX_TTL_MS))
                .unwrap_or_else(|| {
                    warnings.push(format!(
                        "{id} 的 duration_ms 非法，回落到缺省 {}ms",
                        default_duration_ms(kind)
                    ));
                    default_duration_ms(kind)
                });
            let wave = match kind {
                PresetKind::Expression => MotionWave::Single,
                PresetKind::Motion => match raw.get("wave").and_then(|v| v.as_str()) {
                    Some("oscillate") => {
                        let cycles = raw
                            .get("cycles")
                            .and_then(|v| v.as_f64())
                            .filter(|v| v.is_finite() && *v > 0.0)
                            .unwrap_or(SHAKE_CYCLES);
                        MotionWave::Oscillate { cycles }
                    }
                    // 缺省 / "single" / 未知 → 单峰（宽容，不失败）。
                    _ => MotionWave::Single,
                },
            };
            // morph：low 极 = `morph.low_params`，high 极 = `morph.high_params`。
            let (params, morph) = match raw.get("morph") {
                Some(m) if m.is_object() => {
                    let low = m
                        .get("low")
                        .and_then(|v| v.as_f64())
                        .filter(|v| v.is_finite() && *v >= 0.0)
                        .map_or(1.0, |v| v as f32);
                    let high = m
                        .get("high")
                        .and_then(|v| v.as_f64())
                        .filter(|v| v.is_finite() && *v > 0.0)
                        .map_or(MAX_INTENSITY, |v| v as f32);
                    let low_params = parse_param_list(m.get("low_params"), id, kind, &mut warnings);
                    let high_params =
                        parse_param_list(m.get("high_params"), id, kind, &mut warnings);
                    if low_params.is_empty() || high_params.is_empty() {
                        warnings.push(format!(
                            "{id}: morph 的 low/high 极不全，已按普通包处理（morph 失效）"
                        ));
                        let fallback: &'static [(&'static str, f32)] = Box::leak(
                            parse_param_list(raw.get("params"), id, kind, &mut warnings)
                                .into_boxed_slice(),
                        );
                        (fallback, None)
                    } else {
                        let low_slice: &'static [(&'static str, f32)] =
                            Box::leak(low_params.into_boxed_slice());
                        let high_slice: &'static [(&'static str, f32)] =
                            Box::leak(high_params.into_boxed_slice());
                        let morph: &'static PresetMorph = Box::leak(Box::new(PresetMorph {
                            low,
                            high,
                            high_params: high_slice,
                        }));
                        (low_slice, Some(morph))
                    }
                }
                _ => {
                    let list: &'static [(&'static str, f32)] = Box::leak(
                        parse_param_list(raw.get("params"), id, kind, &mut warnings)
                            .into_boxed_slice(),
                    );
                    (list, None)
                }
            };
            if params.is_empty() {
                warnings.push(format!("{id} 没有可用参数，已跳过"));
                continue;
            }
            warn_expression_shape(id, kind, params, morph, &mut warnings);
            specs.push(Box::leak(Box::new(PresetSpec {
                id: leak_str(id.to_string()),
                kind,
                duration_ms,
                params,
                wave,
                morph,
            })));
        }
        if specs.is_empty() {
            return Err("没有一条有效预设".to_string());
        }
        Ok(Self { specs, warnings })
    }
}

/// 解析一组参数：白名单过滤 + 按 kind 分档钳位 + warning。
fn parse_param_list(
    list: Option<&serde_json::Value>,
    id: &str,
    kind: PresetKind,
    warnings: &mut Vec<String>,
) -> Vec<(&'static str, f32)> {
    let mut params: Vec<(&'static str, f32)> = Vec::new();
    let Some(list) = list.and_then(|v| v.as_array()) else {
        return params;
    };
    for p in list {
        let pid = p.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
        let Some(value) = p.get("value").and_then(|v| v.as_f64()) else {
            warnings.push(format!("{id}: 参数 {pid:?} 的 value 不是数字，已丢弃"));
            continue;
        };
        if !ALLOWED_PARAMS.contains(&pid) {
            warnings.push(format!("{id}: 通道外参数 {pid} 已丢弃"));
            continue;
        }
        let limit = pack_limit(kind, pid);
        let mut v = value as f32;
        if !v.is_finite() {
            warnings.push(format!("{id}: {pid} 的 value 非有限，已丢弃"));
            continue;
        }
        if v.abs() > limit {
            warnings.push(format!("{id}: {pid} 幅值 {v} 超上限 {limit}，已钳位"));
            v = v.clamp(-limit, limit);
        }
        params.push((leak_str(pid.to_string()), v));
    }
    params
}

/// 表情包形状检查（v3）：五官 + 小幅头身缺任一 → warning（仍加载）。
fn warn_expression_shape(
    id: &str,
    kind: PresetKind,
    params: &[(&'static str, f32)],
    morph: Option<&'static PresetMorph>,
    warnings: &mut Vec<String>,
) {
    if kind != PresetKind::Expression {
        return;
    }
    let mut chans: Vec<&str> = params.iter().map(|(k, _)| *k).collect();
    if let Some(m) = morph {
        for (k, _) in m.high_params {
            if !chans.contains(k) {
                chans.push(k);
            }
        }
    }
    let face = chans
        .iter()
        .any(|c| scale_class(c) == ScaleClass::Expression);
    let head = chans.iter().any(|c| c.starts_with("ParamAngle"));
    let body = chans.iter().any(|c| c.starts_with("ParamBodyAngle"));
    if !face {
        warnings.push(format!("{id}: 表情包缺五官通道（v3 要求五官 + 小幅头身）"));
    }
    if !head {
        warnings.push(format!("{id}: 表情包缺小幅头部位移（ParamAngle*）"));
    }
    if !body {
        warnings.push(format!("{id}: 表情包缺小幅半身随动（ParamBodyAngle*）"));
    }
}

fn leak_str(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn default_duration_ms(kind: PresetKind) -> f64 {
    match kind {
        PresetKind::Expression => EXPRESSION_MS,
        PresetKind::Motion => MOTION_MS,
    }
}
