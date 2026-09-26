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
//!
//! # v1 三族表演字段（V12 事实：director **只产 cues**）
//!
//! 协议 v1（`docs/architecture/performance-protocol-v1.md` §2.2）给一条 cue 加了
//! 三族语义字段 `field` / `x` / `y` / `z` / `id` / `at` / `hold`（+ `seq`）。
//! 本模块是 director 的**二路产出面**，因此 [Cue] 能表达它：
//!
//! - **只增不改（V11）**：`preset_id` 仍是必填的 legacy 键（旧路径逐字不变）；
//!   v1 新键在 [Cue::to_json] 里**摊平成可选键**——前端 `ActionCue.fromJson`
//!   遇缺省即旧语义；
//! - **V12 事实**：director **只产 cues，不产文本**。这里**没有**、也不会有
//!   `segments` / `speak` 字段——切分原文是表演层（runtime）的职责；
//! - **零下行通道**：本模块不调用 `ModServices.action_tx` / `apply_settings`
//!   （回归 `tests::action_tx_and_apply_settings_are_never_called`）。
//!
//! `sentence_seq` 仍是导演的锚（导演不产 segments，「第 N 句」就是锚点）；
//! v1 的 `at` 缺省由它推出（`seg:N`），不是模型自报。

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
///
/// **legacy 路径**（`field == None`）：`preset_id` + intensity/ttl，逐字段与
/// v0 相同。**v1 路径**（`field == Some(..)`）：额外携带三族字段与锚点；
/// legacy 键仍在（V11：只增不改）。
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    /// 目标句序号（与 AudioChunk.sentence_seq 同源；导演不产生新序号）。
    pub sentence_seq: u64,
    /// 预设 id（legacy 路径必填；v1 路径 = 语义 id，expression 时即面板 id）。
    pub preset_id: String,
    /// 强度 1..=3。
    pub intensity: u8,
    /// 生效时长（毫秒）1..=5000。
    pub ttl_ms: u64,
    /// 优先级（应用层写死，不来自 LLM）。
    pub priority: u8,
    /// v1 三族字段（协议 §2.2）；`None` = 旧 `preset_id` 路径，逐字不变。
    pub field: Option<live2d_ai_runtime::performance::CueField>,
    /// 归一化轴值（已钳 `[-1, 1]`；仅 v1 路径有）。
    pub x: Option<f64>,
    /// 归一化轴值。
    pub y: Option<f64>,
    /// 歪头倾斜（仅 head；body/expression 给了会被丢键 + warning）。
    pub z: Option<f64>,
    /// 锚点 wire 名（now / seg:N / after_prev；缺省由 sentence_seq 推出 seg:N）。
    pub at: Option<String>,
    /// 保持到下次指令（v1 必填；legacy 路径为 None）。
    pub hold: Option<bool>,
    /// plan 内 cue 序号（从 1 起；v1 路径才有）。
    pub seq: Option<u64>,
}

impl Cue {
    /// 一条 legacy cue（规则层 / 旧二路路径；v1 键全缺省）。
    pub fn legacy(
        sentence_seq: u64,
        preset_id: impl Into<String>,
        intensity: u8,
        ttl_ms: u64,
        priority: u8,
    ) -> Self {
        Self {
            sentence_seq,
            preset_id: preset_id.into(),
            intensity,
            ttl_ms,
            priority,
            field: None,
            x: None,
            y: None,
            z: None,
            at: None,
            hold: None,
            seq: None,
        }
    }

    /// JSON 形态（WS action_cue 的 cues 元素）。
    ///
    /// legacy 键 `sentence_seq` / `preset_id` / `intensity` / `ttl_ms` /
    /// `priority` **永远在**（V11）；v1 新键只在 `field` 存在时摊平，
    /// 缺省 = 旧语义。
    pub fn to_json(&self) -> serde_json::Value {
        let mut object = serde_json::Map::new();
        object.insert(
            "sentence_seq".to_string(),
            serde_json::json!(self.sentence_seq),
        );
        object.insert("preset_id".to_string(), serde_json::json!(self.preset_id));
        object.insert("intensity".to_string(), serde_json::json!(self.intensity));
        object.insert("ttl_ms".to_string(), serde_json::json!(self.ttl_ms));
        object.insert("priority".to_string(), serde_json::json!(self.priority));
        let Some(field) = self.field else {
            return serde_json::Value::Object(object);
        };
        object.insert("field".to_string(), serde_json::json!(field.as_str()));
        if field == live2d_ai_runtime::performance::CueField::Expression {
            // expression 的面板 id 与 legacy preset_id 同源（V11 兼容面）。
            object.insert("id".to_string(), serde_json::json!(self.preset_id));
        }
        for (key, value) in [("x", self.x), ("y", self.y), ("z", self.z)] {
            if let Some(value) = value {
                object.insert(key.to_string(), serde_json::json!(value));
            }
        }
        if let Some(at) = &self.at {
            object.insert("at".to_string(), serde_json::json!(at));
        }
        if let Some(hold) = self.hold {
            object.insert("hold".to_string(), serde_json::json!(hold));
        }
        if let Some(seq) = self.seq {
            object.insert("seq".to_string(), serde_json::json!(seq));
        }
        serde_json::Value::Object(object)
    }
}

/// 一份导演 plan。
///
/// **只有 cues，没有文本**（V12）：导演不产 `segments` / `speak`；切分原文
/// 是表演层（runtime）的职责，导演是备注不是誊写员。
#[derive(Debug, Clone, PartialEq)]
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
        let cues = vec![Cue::legacy(
            1,
            preset_id.to_string(),
            intensity.clamp(1, MAX_INTENSITY),
            ttl_ms.clamp(1, MAX_TTL_MS),
            PRIORITY_RULE,
        )];
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
            // v1 三族字段形态（协议 §2.2）：给了 field 就走 v1 键；否则旧路径。
            if let Some(field_raw) = cue.get("field").and_then(serde_json::Value::as_str) {
                let index = cues.len() as u64 + 1;
                let parsed = cue.as_object().and_then(|obj| {
                    parse_field_cue(obj, seq, index, field_raw, allow, &mut warnings)
                });
                if let Some(parsed) = parsed {
                    cues.push(parsed);
                }
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
            cues.push(Cue::legacy(
                seq,
                preset_id.to_string(),
                intensity,
                ttl_ms,
                PRIORITY_ASYNC,
            ));
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

/// 解析一条 v1 三族字段 cue（协议 §2.2）；不合格 → 丢该条 + warning。
///
/// 口径（与 [parse_plan] 的「丢条不失败」纪律一致）：
/// - `field` 词表外 → 丢；`expression` 缺 `id` / id 不在能力集（`none` 除外）→ 丢；
/// - `hold` **必填 bool**：它决定「到点回不回」，猜一个默认值就是猜语义 → 缺则丢；
/// - 轴值给了就**钳到 [-1, 1]**；`body` / `expression` 给 `z` → 丢该键 + warning；
/// - `at` 缺省由 `sentence_seq` 推成 `seg:N`（导演的锚就是句号，不是时钟）；
/// - `preset_id` = 语义 id（body/head = 字段名；expression = 面板 id）——
///   与 runtime 的 `FieldCue::to_wire_cue` 同一口径，保证两条通道同形。
fn parse_field_cue(
    cue: &serde_json::Map<String, serde_json::Value>,
    seq: u64,
    index: u64,
    field_raw: &str,
    allow: &[&str],
    warnings: &mut Vec<String>,
) -> Option<Cue> {
    use live2d_ai_runtime::performance::CueField;
    let field = match field_raw {
        "body" => CueField::Body,
        "head" => CueField::Head,
        "expression" => CueField::Expression,
        other => {
            warnings.push(format!("cue seq={seq} 的 field={other} 不在词表内，已丢弃"));
            return None;
        }
    };
    let semantic_id = if field == CueField::Expression {
        let Some(id) = cue.get("id").and_then(serde_json::Value::as_str) else {
            warnings.push(format!("cue seq={seq} 的 expression 缺 id，已丢弃"));
            return None;
        };
        // none = 撤销哨兵（永远允许）；其余必须在能力集内。
        if id != "none" && !allow.contains(&id) {
            warnings.push(format!("cue seq={seq} 的表达式 {id} 不在能力集，已丢弃"));
            return None;
        }
        id.to_string()
    } else {
        field.as_str().to_string()
    };
    let Some(hold) = cue.get("hold").and_then(serde_json::Value::as_bool) else {
        warnings.push(format!(
            "cue seq={seq} 缺 hold，已丢弃（hold 决定到点回不回，猜不得）"
        ));
        return None;
    };
    let mut z = axis(cue, "z");
    if field != CueField::Head && z.is_some() {
        z = None;
        warnings.push(format!(
            "cue seq={seq} 的字段 {field_raw} 不接受 z，已丢弃该键"
        ));
    }
    let at = cue
        .get("at")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("seg:{seq}"));
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
    Some(Cue {
        sentence_seq: seq,
        preset_id: semantic_id,
        intensity,
        ttl_ms,
        priority: PRIORITY_ASYNC,
        field: Some(field),
        x: axis(cue, "x"),
        y: axis(cue, "y"),
        z,
        at: Some(at),
        hold: Some(hold),
        seq: Some(index),
    })
}

/// 读一个归一化轴值：缺席 / 非数字 → None；给了就钳到 `[-1, 1]`。
fn axis(cue: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<f64> {
    cue.get(key)
        .and_then(serde_json::Value::as_f64)
        .map(|v| v.clamp(-1.0, 1.0))
}
