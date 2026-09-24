//! **表演层唯一输出 schema** + 严格校验（本文件是单一真源）。
//!
//! # 契约（2026-09-22 用户敲定；文档终稿见 docs/architecture/performance-layer-v0.md）
//!
//! 每一轮表演层必须交回**恰好一份** JSON：
//!
//! ```json
//! {"speak": string|null,
//!  "cues": [{"sentence_seq": u64, "preset_id": string, "intensity": number, "ttl_ms": number}]}
//! ```
//!
//! - **speak**：本轮要说的话（= 送 TTS / 上屏的唯一真源）。`null` 或 `""` = 本轮不说；
//! - **cues**：按句动作 cue。`[]` = 本轮不动；`preset_id` 必须在本模型能力集内；
//! - **noop 合法**：`speak=null/"" 且 cues=[]` = 本轮不说也不动（不是失败）；
//! - **只说** = `cues=[]`；**只动** = `speak` 空（引擎给一条无声锚句）。
//!
//! # 校验纪律（整份失败，不做局部抢救）
//!
//! - 坏 JSON / 顶层不是对象 / 缺字段 / 类型不对 / `preset_id` 不在能力集 /
//!   字段数超限 → **整份失败** → 引擎回退（`speak=clean_for_tts(原文)` + 规则 cue）；
//! - `intensity` / `ttl_ms` 越界 → **钳位**（不是失败）；
//! - **未知字段一律丢弃**（宽容：模型多写一个 `reason` 不该让整轮没有语音）；
//! - 为什么不做「丢一条坏 cue、留其余」：局部抢救会让「模型到底演了什么」变成
//!   不可解释的混合体。宁可整份回退到确定性规则层（可解释、可单测）。
//!
//! # 为什么 schema 写在代码里
//!
//! [json_schema_strict] **由本文件的常量拼出**，同一个文件里的 [parse_plan]
//! 逐条实现同样的边界；回归 `schema_and_validator_share_the_same_bounds` 钉住
//! 「文档里的 schema == 发出去的 schema == 校验器认的东西」。不要在别处再抄一份。

use serde_json::{Value, json};

/// 一份 plan 里 cue 的条数上限（防模型灌爆）。
pub const MAX_CUES: usize = 16;
/// 强度下限（与渲染面 preset 的 intensity 钳位同口径）。
pub const MIN_INTENSITY: u8 = 1;
/// 强度上限。
pub const MAX_INTENSITY: u8 = 3;
/// ttl 下限（毫秒）。
pub const MIN_TTL_MS: u64 = 1;
/// ttl 上限（毫秒；与渲染面 MAX_TTL_MS 同口径）。
pub const MAX_TTL_MS: u64 = 5_000;
/// `speak` 的字符数上限（超出按字符截断；防无标点长文一次灌爆 TTS 队列）。
pub const MAX_SPEAK_CHARS: usize = 4_000;

/// 表演层 cue 的固定优先级（**应用层写死，模型不得自报**）。
///
/// 与 `live2d-ai-mod-director::plan::PRIORITY_ASYNC` 同值（40）——它是
/// 「表演层覆盖规则层」的既有抬升口径；数字写在这里而不是从 Mod 引，避免
/// runtime 反向依赖 Mod crate。
pub const PRIORITY_PERFORMANCE: u8 = 40;

/// 一条按句 cue（与 WS `action_cue.payload.cues[]` 逐字段同形）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceCue {
    /// 目标句序号（与 `AudioChunk.sentence_seq` 同源；锚点是该句音频的 first_chunk）。
    pub sentence_seq: u64,
    /// 预设 id（必须在本模型能力集内）。
    pub preset_id: String,
    /// 强度 1..=3（已钳位）。
    pub intensity: u8,
    /// 生效时长（毫秒）1..=5000（已钳位）。
    pub ttl_ms: u64,
}

impl PerformanceCue {
    /// WS `action_cue` 单条形态。
    pub fn to_json(&self) -> Value {
        json!({
            "sentence_seq": self.sentence_seq,
            "preset_id": self.preset_id,
            "intensity": self.intensity,
            "ttl_ms": self.ttl_ms,
            "priority": PRIORITY_PERFORMANCE,
        })
    }
}

/// 一份表演层 plan。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PerformancePlan {
    /// 本轮要说的话；`None` / 空白 = 不说（noop 的一半）。
    pub speak: Option<String>,
    /// 按句 cue；空 = 不动。
    pub cues: Vec<PerformanceCue>,
}

impl PerformancePlan {
    /// 本轮确实什么都不做（不说也不动）。
    pub fn is_noop(&self) -> bool {
        self.speak.is_none() && self.cues.is_empty()
    }

    /// 日志用摘要：**不含正文全文**（只给长度与 preset id 列表）。
    pub fn summary(&self) -> String {
        let ids: Vec<&str> = self.cues.iter().map(|c| c.preset_id.as_str()).collect();
        format!(
            "speak_chars={}, cues={:?}",
            self.speak.as_ref().map_or(0, |s| s.chars().count()),
            ids
        )
    }
}

/// 校验失败原因（[PlanError::code] 是稳定错误码，进日志与回退原因码）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// 不是合法 JSON。
    NotJson(String),
    /// 顶层不是对象。
    NotObject,
    /// 缺 `speak` 字段。
    MissingSpeak,
    /// `speak` 不是 string / null。
    SpeakType,
    /// 缺 `cues` 字段。
    MissingCues,
    /// `cues` 不是数组。
    CuesType,
    /// cue 条数超过 [MAX_CUES]。
    TooManyCues(usize),
    /// cue 不是对象。
    CueNotObject(usize),
    /// cue 缺字段 / 字段类型不对（`field` 是字段名，`index` 是第几条）。
    CueField {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 字段名。
        field: &'static str,
    },
    /// `preset_id` 不在能力集内。
    UnknownPreset {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 原始 id（可能是任意字符串；日志里会带出来）。
        preset_id: String,
    },
}

impl PlanError {
    /// 稳定错误码（进日志 / 回退原因；**不含正文**）。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotJson(_) => "performance_plan_not_json",
            Self::NotObject => "performance_plan_not_object",
            Self::MissingSpeak => "performance_plan_missing_speak",
            Self::SpeakType => "performance_plan_speak_type",
            Self::MissingCues => "performance_plan_missing_cues",
            Self::CuesType => "performance_plan_cues_type",
            Self::TooManyCues(_) => "performance_plan_too_many_cues",
            Self::CueNotObject(_) => "performance_plan_cue_not_object",
            Self::CueField { .. } => "performance_plan_cue_field",
            Self::UnknownPreset { .. } => "performance_plan_unknown_preset",
        }
    }

    /// 给人看的一句话（**不含 speak 全文**；preset id 可以带）。
    pub fn message(&self) -> String {
        match self {
            Self::NotJson(e) => format!("不是合法 JSON：{e}"),
            Self::NotObject => "顶层不是 JSON 对象".to_string(),
            Self::MissingSpeak => "缺 speak 字段".to_string(),
            Self::SpeakType => "speak 既不是 string 也不是 null".to_string(),
            Self::MissingCues => "缺 cues 字段".to_string(),
            Self::CuesType => "cues 不是数组".to_string(),
            Self::TooManyCues(n) => format!("cue 条数 {n} 超过上限 {MAX_CUES}"),
            Self::CueNotObject(i) => format!("第 {i} 条 cue 不是对象"),
            Self::CueField { index, field } => {
                format!("第 {index} 条 cue 的 {field} 缺失或类型不对")
            }
            Self::UnknownPreset { index, preset_id } => {
                format!("第 {index} 条 cue 的 preset_id={preset_id} 不在能力集内")
            }
        }
    }
}

/// **严格校验**一份表演层响应；返回 plan 或整份失败原因。
///
/// 边界与 [json_schema_strict] 逐条一致（回归钉住）。
pub fn parse_plan(raw: &str, allow: &[String]) -> Result<PerformancePlan, PlanError> {
    let value: Value = serde_json::from_str(raw).map_err(|e| PlanError::NotJson(e.to_string()))?;
    let object = value.as_object().ok_or(PlanError::NotObject)?;
    // speak：**必须存在**；null / string 二者之一。
    let speak_value = object.get("speak").ok_or(PlanError::MissingSpeak)?;
    let speak = match speak_value {
        Value::Null => None,
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.chars().take(MAX_SPEAK_CHARS).collect::<String>())
            }
        }
        _ => return Err(PlanError::SpeakType),
    };
    // cues：**必须存在**且是数组。
    let cues_value = object.get("cues").ok_or(PlanError::MissingCues)?;
    let cue_list = cues_value.as_array().ok_or(PlanError::CuesType)?;
    if cue_list.len() > MAX_CUES {
        return Err(PlanError::TooManyCues(cue_list.len()));
    }
    let mut cues = Vec::with_capacity(cue_list.len());
    for (index, cue) in cue_list.iter().enumerate() {
        let cue = cue.as_object().ok_or(PlanError::CueNotObject(index))?;
        // sentence_seq：必须是 >=1 的整数（0 / 缺失 / 类型不对都算坏字段）。
        let sentence_seq = cue
            .get("sentence_seq")
            .and_then(Value::as_u64)
            .filter(|v| *v >= 1)
            .ok_or(PlanError::CueField {
                index,
                field: "sentence_seq",
            })?;
        let preset_id =
            cue.get("preset_id")
                .and_then(Value::as_str)
                .ok_or(PlanError::CueField {
                    index,
                    field: "preset_id",
                })?;
        if !allow.iter().any(|a| a == preset_id) {
            return Err(PlanError::UnknownPreset {
                index,
                preset_id: preset_id.to_string(),
            });
        }
        // intensity / ttl_ms：必须是数字；越界**钳位**（不是失败）。
        let intensity_raw =
            cue.get("intensity")
                .and_then(Value::as_u64)
                .ok_or(PlanError::CueField {
                    index,
                    field: "intensity",
                })?;
        let ttl_raw = cue
            .get("ttl_ms")
            .and_then(Value::as_u64)
            .ok_or(PlanError::CueField {
                index,
                field: "ttl_ms",
            })?;
        cues.push(PerformanceCue {
            sentence_seq,
            preset_id: preset_id.to_string(),
            intensity: (intensity_raw as u8).clamp(MIN_INTENSITY, MAX_INTENSITY),
            ttl_ms: ttl_raw.clamp(MIN_TTL_MS, MAX_TTL_MS),
        });
    }
    Ok(PerformancePlan { speak, cues })
}

/// 发给支持 structured output 的 provider 的 **json_schema strict** 定义。
///
/// 由本文件的常量拼出——能力集就是 host 注入的那一份（单一真源）。
pub fn json_schema_strict(allow: &[String]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["speak", "cues"],
        "properties": {
            "speak": {
                "type": ["string", "null"],
                "description": "本轮要说的话（送 TTS / 上屏的唯一真源）；null 或空串 = 本轮不说。"
            },
            "cues": {
                "type": "array",
                "maxItems": MAX_CUES,
                "description": "按句动作 cue；空数组 = 本轮不动。",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["sentence_seq", "preset_id", "intensity", "ttl_ms"],
                    "properties": {
                        "sentence_seq": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "目标句序号（从 1 开始，对应 speak 切出的句子）。"
                        },
                        "preset_id": {
                            "type": "string",
                            "enum": allow,
                            "description": "动作预设 id，只能取能力集里的值。"
                        },
                        "intensity": {
                            "type": "integer",
                            "minimum": MIN_INTENSITY,
                            "maximum": MAX_INTENSITY
                        },
                        "ttl_ms": {
                            "type": "integer",
                            "minimum": MIN_TTL_MS,
                            "maximum": MAX_TTL_MS
                        }
                    }
                }
            }
        }
    })
}

/// 把 plan 的 cue 列表封成 WS `action_cue` 的 payload 形态。
pub fn action_cue_payload(epoch: u64, covers_upto_seq: u64, cues: &[PerformanceCue]) -> Value {
    json!({
        "epoch": epoch,
        "covers_upto_seq": covers_upto_seq,
        "cues": cues.iter().map(PerformanceCue::to_json).collect::<Vec<_>>(),
    })
}
