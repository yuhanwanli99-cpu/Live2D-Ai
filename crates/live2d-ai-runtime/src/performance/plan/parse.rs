use serde_json::{Map, Value, json};

use crate::performance::question::apply_question_patch;

use super::types::*;

/// **严格校验**一份表演层响应；source = 主模型本轮原文（V1 拼接基准）。
///
/// 边界与 [json_schema_strict] 逐条一致（回归钉住）。
pub fn parse_plan(raw: &str, allow: &[String], source: &str) -> Result<PerformancePlan, PlanError> {
    let value: Value = serde_json::from_str(raw).map_err(|e| PlanError::NotJson(e.to_string()))?;
    let object = value.as_object().ok_or(PlanError::NotObject)?;
    if object.get("segments").is_none() {
        // v0 旧路径（V11：字段一律不删、缺省即旧语义）。
        if object.get("speak").is_none() {
            return Err(PlanError::MissingSegments);
        }
        return parse_legacy(object, allow);
    }
    // 校验**成功之后**才补问句（T9）：失败路径一个字都不补，legacy 路径不走这里。
    Ok(apply_question_patch(parse_v1(object, allow, source)?))
}

/// v0 legacy 路径：speak + 按句 preset cue。
fn parse_legacy(
    object: &Map<String, Value>,
    allow: &[String],
) -> Result<PerformancePlan, PlanError> {
    let speak = match object.get("speak") {
        Some(Value::Null) => None,
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.chars().take(MAX_SPEAK_CHARS).collect::<String>())
            }
        }
        _ => return Err(PlanError::SpeakType),
    };
    let cues_value = object.get("cues").ok_or(PlanError::MissingCues)?;
    let cue_list = cues_value.as_array().ok_or(PlanError::CuesType)?;
    if cue_list.len() > MAX_CUES {
        return Err(PlanError::TooManyCues(cue_list.len()));
    }
    let mut cues = Vec::with_capacity(cue_list.len());
    for (index, cue) in cue_list.iter().enumerate() {
        let cue = cue.as_object().ok_or(PlanError::CueNotObject(index))?;
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
        // legacy 路径：v1 可选字段全部缺省 None（= 只出既有 5 键）。
        cues.push(PerformanceCue {
            sentence_seq,
            preset_id: preset_id.to_string(),
            intensity: (intensity_raw as u8).clamp(MIN_INTENSITY, MAX_INTENSITY),
            ttl_ms: ttl_raw.clamp(MIN_TTL_MS, MAX_TTL_MS),
            ..Default::default()
        });
    }
    Ok(PerformancePlan {
        speak,
        segments: None,
        cues,
        warnings: Vec::new(),
    })
}

/// v1 路径：segments（原文切分）+ 三字段 cues。
fn parse_v1(
    object: &Map<String, Value>,
    allow: &[String],
    source: &str,
) -> Result<PerformancePlan, PlanError> {
    let mut warnings = Vec::new();
    if object.get("speak").is_some() {
        // O1：segments 优先，speak 忽略 + warn（不整份失败）。
        warnings.push(PlanWarning::SpeakIgnored);
    }
    let segments_value = object.get("segments").ok_or(PlanError::MissingSegments)?;
    let segment_list = segments_value.as_array().ok_or(PlanError::SegmentsType)?;
    if segment_list.len() > MAX_SEGMENTS {
        return Err(PlanError::SegmentsTooLong(format!(
            "段数 {} 超过上限 {MAX_SEGMENTS}",
            segment_list.len()
        )));
    }
    let mut segments = Vec::with_capacity(segment_list.len());
    let mut total_chars = 0usize;
    for (index, segment) in segment_list.iter().enumerate() {
        let segment = segment.as_str().ok_or(PlanError::SegmentNotString(index))?;
        total_chars += segment.chars().count();
        segments.push(segment.to_string());
    }
    if total_chars > MAX_SEGMENT_CHARS {
        return Err(PlanError::SegmentsTooLong(format!(
            "总字符 {total_chars} 超过上限 {MAX_SEGMENT_CHARS}"
        )));
    }
    // V1 核心不变量：逐码点拼接恒等；空原文必须是空切分方案。
    if segments.concat() != source || (source.is_empty() && !segments.is_empty()) {
        return Err(PlanError::SegmentsNotPartition);
    }
    let cues_value = object.get("cues").ok_or(PlanError::MissingCues)?;
    let cue_list = cues_value.as_array().ok_or(PlanError::CuesType)?;
    if cue_list.len() > MAX_CUES {
        return Err(PlanError::TooManyCues(cue_list.len()));
    }
    let seg_count = segments.len() as u64;
    let mut parsed: Vec<FieldCue> = Vec::with_capacity(cue_list.len());
    for (index, cue) in cue_list.iter().enumerate() {
        let cue = cue.as_object().ok_or(PlanError::CueNotObject(index))?;
        let field = match cue.get("field").and_then(Value::as_str) {
            Some("head") => CueField::Head,
            Some("expression") => CueField::Expression,
            // 2026-10-07（T8）：`body` 停用（只演头与表情）。丢该条 + warn，
            // **不整份失败**——整份失败会连带丢掉本轮语音分段，代价远大于少一条身段。
            Some("body") => {
                warnings.push(PlanWarning::BodyNotAllowed { index });
                continue;
            }
            Some(other) => {
                return Err(PlanError::UnknownField {
                    index,
                    field: other.to_string(),
                });
            }
            None => {
                return Err(PlanError::CueField {
                    index,
                    field: "field",
                });
            }
        };
        let intensity_raw =
            cue.get("intensity")
                .and_then(Value::as_u64)
                .ok_or(PlanError::CueField {
                    index,
                    field: "intensity",
                })?;
        let hold = cue
            .get("hold")
            .and_then(Value::as_bool)
            .ok_or(PlanError::CueField {
                index,
                field: "hold",
            })?;
        let at_raw = cue
            .get("at")
            .and_then(Value::as_str)
            .ok_or(PlanError::CueField { index, field: "at" })?;
        let at = parse_anchor(at_raw, seg_count).ok_or_else(|| PlanError::BadAnchor {
            index,
            at: at_raw.to_string(),
        })?;
        // 轴键：按字段裁剪不允许的键（丢键 + warn，不整份失败）。
        let allow_x = matches!(field, CueField::Body | CueField::Head);
        let allow_z = matches!(field, CueField::Head);
        let mut axes = [
            ("x", axis_value(cue, "x", index)?),
            ("y", axis_value(cue, "y", index)?),
            ("z", axis_value(cue, "z", index)?),
        ];
        for (key, value) in axes.iter_mut() {
            let permitted = match *key {
                "x" | "y" => allow_x,
                _ => allow_z,
            };
            if value.is_some() && !permitted {
                warnings.push(PlanWarning::AxisNotAllowed { index, key });
                *value = None;
            }
        }
        let [x, y, z] = axes.map(|(_, value)| value);
        // id：expression 必填且必须在能力集（none 恒合法）；其余字段给 id 丢键。
        let id = match cue.get("id") {
            Some(Value::String(s)) => Some(s.clone()),
            Some(_) => {
                return Err(PlanError::CueField { index, field: "id" });
            }
            None => None,
        };
        let id = if field == CueField::Expression {
            let id = id.ok_or(PlanError::CueField { index, field: "id" })?;
            // T9：能力集 ∩ **表情槽**。手势 id 填进 expression 同样落这一条
            // （丢该条 + warn，整份仍成功）——表情表只有脸，写了也会被渲染面丢掉。
            if id != "none" && !(is_expression_preset(&id) && allow.iter().any(|a| a == &id)) {
                warnings.push(PlanWarning::ExpressionUnknownId { index, id });
                continue;
            }
            Some(id)
        } else {
            if id.is_some() {
                warnings.push(PlanWarning::IdNotAllowed { index });
            }
            None
        };
        let ttl_ms = match cue.get("ttl_ms") {
            Some(value) => value
                .as_u64()
                .ok_or(PlanError::CueField {
                    index,
                    field: "ttl_ms",
                })?
                .clamp(MIN_TTL_MS, MAX_TTL_MS),
            None => field.default_ttl_ms(),
        };
        parsed.push(FieldCue {
            field,
            x,
            y,
            z,
            id,
            intensity: (intensity_raw as u8).clamp(MIN_INTENSITY, MAX_INTENSITY),
            at,
            at_resolved: String::new(),
            hold,
            ttl_ms,
            seq: index as u64 + 1,
            sentence_seq: 0,
        });
    }
    // 锚点解析：now → 第 1 段；seg:N → 第 N 段；after_prev → 上一条锚段，
    // 若上一条 hold=true（没有「做完」点，O4）或没有上一条 → 退化为 now，
    // 但仍留在上一 cue 的锚段（不得跳回、更不得「永不生效」）。
    let mut previous: Option<(u64, bool)> = None;
    for cue in parsed.iter_mut() {
        let (sentence_seq, at_resolved) = match cue.at {
            CueAnchor::Now => (1, "now".to_string()),
            CueAnchor::Seg(n) => (n, format!("seg:{n}")),
            CueAnchor::AfterPrev => match previous {
                Some((prev_seq, false)) => (prev_seq, "after_prev".to_string()),
                Some((prev_seq, true)) => (prev_seq, "now".to_string()),
                None => (1, "now".to_string()),
            },
        };
        cue.sentence_seq = sentence_seq;
        cue.at_resolved = at_resolved;
        previous = Some((sentence_seq, cue.hold));
    }
    let cues = parsed.iter().map(FieldCue::to_wire_cue).collect();
    Ok(PerformancePlan {
        speak: None,
        segments: Some(segments),
        cues,
        warnings,
    })
}

/// 读一个轴键：缺席 None，数字**钳位**到 [AXIS_MIN, AXIS_MAX]，类型不对整份失败。
fn axis_value(
    cue: &Map<String, Value>,
    key: &'static str,
    index: usize,
) -> Result<Option<f64>, PlanError> {
    match cue.get(key) {
        None => Ok(None),
        Some(Value::Number(n)) => {
            let value = n
                .as_f64()
                .ok_or(PlanError::CueField { index, field: key })?;
            Ok(Some(value.clamp(AXIS_MIN, AXIS_MAX)))
        }
        Some(_) => Err(PlanError::CueField { index, field: key }),
    }
}

/// 解析 at 锚点（seg:N 1-based 且 N <= seg_count）。
fn parse_anchor(raw: &str, seg_count: u64) -> Option<CueAnchor> {
    match raw {
        "now" => Some(CueAnchor::Now),
        "after_prev" => Some(CueAnchor::AfterPrev),
        other => {
            let n = other.strip_prefix("seg:")?.parse::<u64>().ok()?;
            (n >= 1 && n <= seg_count).then_some(CueAnchor::Seg(n))
        }
    }
}

/// 发给支持 structured output 的 provider 的 **json_schema strict** 定义。
///
/// 由本文件的常量拼出——边界就是校验器认的那一份（单一真源）。
pub fn json_schema_strict(allow: &[String]) -> Value {
    // T9：schema 的 expression enum **只留表情槽的 id**（能力集 ∩ 表情槽）+ none。
    // 手势 id 不进 enum——它进了 enum 就等于邀请模型把它填到 id 上。
    let mut expression_ids: Vec<Value> = allow
        .iter()
        .filter(|id| is_expression_preset(id))
        .map(|id| json!(id))
        .collect();
    expression_ids.push(json!("none"));
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["segments", "cues"],
        "properties": {
            "segments": {
                "type": "array",
                "maxItems": MAX_SEGMENTS,
                "description": format!(
                    "把【主模型原文】切成若干段：只能切分，逐字不变，拼接必须与原文完全相同；总字符不超过 {MAX_SEGMENT_CHARS}。空原文给空数组。"
                ),
                "items": { "type": "string" }
            },
            "speak": {
                "type": ["string", "null"],
                "description": "已弃用（V11 保留）：与 segments 同现时被忽略。"
            },
            "cues": {
                "type": "array",
                "maxItems": MAX_CUES,
                "description": "表演 cue；空数组 = 本轮不动（不是撤销）。",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["field", "intensity", "at", "hold"],
                    "properties": {
                        "field": {
                            "type": "string",
                            "description": "只演头与表情；body 已停用（交上来会被丢掉）。",
                            "enum": ["head", "expression"]
                        },
                        "x": { "type": "number", "minimum": AXIS_MIN, "maximum": AXIS_MAX },
                        "y": { "type": "number", "minimum": AXIS_MIN, "maximum": AXIS_MAX },
                        "z": { "type": "number", "minimum": AXIS_MIN, "maximum": AXIS_MAX },
                        "id": {
                            "type": "string",
                            "enum": expression_ids,
                            "description": "expression 专用；none = 撤销哨兵。"
                        },
                        "intensity": {
                            "type": "integer",
                            "minimum": MIN_INTENSITY,
                            "maximum": MAX_INTENSITY
                        },
                        "at": {
                            "type": "string",
                            "description": "now / seg:N（1-based，N <= segments 段数）/ after_prev"
                        },
                        "hold": { "type": "boolean" },
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

/// 把 plan 的 cue 列表封成 WS action_cue 的 payload 形态。
pub fn action_cue_payload(epoch: u64, covers_upto_seq: u64, cues: &[PerformanceCue]) -> Value {
    json!({
        "epoch": epoch,
        "covers_upto_seq": covers_upto_seq,
        "cues": cues.iter().map(PerformanceCue::to_json).collect::<Vec<_>>(),
    })
}
