//! 导演 plan（P1-3，2026-09-16）：异步第二路 LLM 的输出契约 + 严格解析。
//!
//! 形状：{epoch, covers_upto_seq, cues:[{sentence_seq, preset_id, intensity?, ttl_ms?}]}。
//!
//! 红线：
//! - **priority 由应用层写死**（规则 10 / 异步 40 / 标签 80），LLM 不得自报；
//! - preset_id 不在能力集 -> 丢该条（不报错）；
//! - intensity 钳 1..=3；ttl_ms 钳 1..=5000；cues 上限 16 条；
//! - epoch 不匹配由 arbiter 处理（整份丢弃、零副作用）。
//!
//! **撤销哨兵（D10，2026-09-24）**：`cues[].preset_id == "none"` = 该句音频开始时
//! **撤销两个槽**（与渲染面 `preset{id:"none"}` 同义）。它与「`cues: []` = 本轮不动」
//! 是两个不同的语义：空表是不动，`none` cue 是显式归零。撤销只走 action_cue 这一条
//! 通道（前端拉取 `latest.preset_id` 的驱动通道已退役；`latest` 仅面板展示）。
//!
//! 「导演是备注，不是誊写员」：本模块只产出 cue，**绝不改**送 TTS 的文本。

/// 规则层优先级（最低，常开兜底）。
pub const PRIORITY_RULE: u8 = 10;
/// 异步第二路 LLM 的优先级。
pub const PRIORITY_ASYNC: u8 = 40;
/// 内联标签的优先级（最高；本波只预留解析器，默认关）。
pub const PRIORITY_LABEL: u8 = 80;

/// 单份 plan 的 cue 数上限（防 LLM 灌爆）。
pub const MAX_CUES: usize = 16;
/// 强度上限（与渲染面 preset 的 intensity 钳位同口径）。
pub const MAX_INTENSITY: u8 = 3;
/// ttl 上限（毫秒；与渲染面 MAX_TTL_MS 同口径）。
pub const MAX_TTL_MS: u64 = 5_000;

/// 一条按句 cue。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    /// 目标句序号（与 AudioChunk.sentence_seq 同源；导演不产生新序号）。
    pub sentence_seq: u64,
    /// 预设 id（必须在本模型能力集内）。
    pub preset_id: String,
    /// 强度 1..=3。
    pub intensity: u8,
    /// 生效时长（毫秒）1..=5000。
    pub ttl_ms: u64,
    /// 优先级（应用层写死，不来自 LLM）。
    pub priority: u8,
}

impl Cue {
    /// JSON 形态（WS action_cue 的 cues 元素）。
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "sentence_seq": self.sentence_seq,
            "preset_id": self.preset_id,
            "intensity": self.intensity,
            "ttl_ms": self.ttl_ms,
            "priority": self.priority,
        })
    }
}

/// 一份导演 plan。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectorPlan {
    /// 该 plan 属于哪一轮。
    pub epoch: u64,
    /// 这份 plan 的效力终点（超过它的句子交回规则层）。
    pub covers_upto_seq: u64,
    /// 按句 cue。
    pub cues: Vec<Cue>,
}

impl DirectorPlan {
    /// 规则层 plan：给第一句一条 cue（来源 = 本轮规则决策的 preset）。
    ///
    /// **`none` cue = 撤销哨兵（D10，2026-09-24）**：preset 为空 / `"none"` 时**不再
    /// 返回空 plan**，而是产出一条 `preset_id == "none"` 的 cue——该句音频开始时前端
    /// 撤销两个槽（与渲染面 `preset{id:"none"}` 同义）。中性轮因此也有**显式撤销锚点**
    /// （performance 关时的 director 规则路径）；不能拿「空 cues」代替撤销，
    /// 那在语义上是「本轮不动」（见模块头注 D10/D11）。
    ///
    /// 为什么 `"none"` 可直接承载：
    /// - [`crate::arbiter::Arbiter::apply`] **不校验 preset id**（只做 epoch 硬闸 +
    ///   按句 upsert）；
    /// - 规则路径**不经** [`parse_plan`] 的能力集校验（那是异步二路 LLM 的第二道闸，
    ///   用 [`crate::presets::PRESET_IDS`] 当 allowlist 丢掉未知 id）。
    ///
    /// 两点都有回归钉住（`tests_staging.rs::rule_none_cue_is_the_revoke_sentinel`）。
    ///
    /// 有非 none 预设时行为**逐字段不变**：`sentence_seq=1` / `preset_id` 原样 /
    /// `intensity` 钳 1..=3 / `ttl_ms` 钳 1..=5000 / `priority=PRIORITY_RULE`。
    pub fn rule(epoch: u64, preset_id: Option<&str>, intensity: u8, ttl_ms: u64) -> Self {
        let preset_id = preset_id
            .filter(|id| !id.is_empty())
            .unwrap_or(crate::presets::PRESET_NONE);
        let cues = vec![Cue {
            sentence_seq: 1,
            preset_id: preset_id.to_string(),
            intensity: intensity.clamp(1, MAX_INTENSITY),
            ttl_ms: ttl_ms.clamp(1, MAX_TTL_MS),
            priority: PRIORITY_RULE,
        }];
        Self {
            epoch,
            covers_upto_seq: 1,
            cues,
        }
    }

    /// WS action_cue 的 payload 形态。
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "epoch": self.epoch,
            "covers_upto_seq": self.covers_upto_seq,
            "cues": self.cues.iter().map(Cue::to_json).collect::<Vec<_>>(),
        })
    }
}

/// 解析异步第二路 LLM 的 plan。
///
/// 返回 (plan, warnings)：JSON 坏 / 缺 epoch -> Err；未知 preset / 非法句号 ->
/// 丢该条 + warning（不失败）。priority 一律写 PRIORITY_ASYNC。
pub fn parse_plan(raw: &str, allow: &[&str]) -> Result<(DirectorPlan, Vec<String>), String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("plan 不是合法 JSON：{e}"))?;
    let epoch = value
        .get("epoch")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "plan 缺 epoch".to_string())?;
    let covers_upto_seq = value
        .get("covers_upto_seq")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let mut warnings = Vec::new();
    let mut cues = Vec::new();
    if let Some(list) = value.get("cues").and_then(serde_json::Value::as_array) {
        for cue in list {
            if cues.len() >= MAX_CUES {
                warnings.push(format!("cue 超过上限 {MAX_CUES}，其余丢弃"));
                break;
            }
            let seq = cue
                .get("sentence_seq")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            if seq == 0 {
                warnings.push("cue 缺 sentence_seq，已丢弃".to_string());
                continue;
            }
            let Some(preset_id) = cue.get("preset_id").and_then(serde_json::Value::as_str) else {
                warnings.push(format!("cue seq={seq} 缺 preset_id，已丢弃"));
                continue;
            };
            if !allow.contains(&preset_id) {
                warnings.push(format!(
                    "cue seq={seq} 的 preset {preset_id} 不在能力集，已丢弃"
                ));
                continue;
            }
            let intensity = cue
                .get("intensity")
                .and_then(serde_json::Value::as_u64)
                .map(|v| (v as u8).clamp(1, MAX_INTENSITY))
                .unwrap_or(1);
            let ttl_ms = cue
                .get("ttl_ms")
                .and_then(serde_json::Value::as_u64)
                .map(|v| v.clamp(1, MAX_TTL_MS))
                .unwrap_or(2_000);
            cues.push(Cue {
                sentence_seq: seq,
                preset_id: preset_id.to_string(),
                intensity,
                ttl_ms,
                priority: PRIORITY_ASYNC,
            });
        }
    }
    Ok((
        DirectorPlan {
            epoch,
            covers_upto_seq,
            cues,
        },
        warnings,
    ))
}
