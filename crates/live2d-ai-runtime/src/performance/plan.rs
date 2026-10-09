//! **表演层唯一输出 schema** + 严格校验（本文件是单一真源）。
//!
//! # 契约（v1，2026-09-26 契约冻结；文档见 docs/architecture/performance-protocol-v1.md）
//!
//! 每一轮表演层必须交回**恰好一份** JSON：
//!
//! ```text
//! {
//!   "segments": ["嗯……", "我想到了。"],     // 只能切分主模型原文，逐字不变（V1）
//!   "cues": [
//!     { "field": "head", "x": 0.0, "y": 0.3, "z": 0.2, "intensity": 1, "at": "now", "hold": true },
//!     { "field": "expression", "id": "thinking", "intensity": 1, "at": "seg:2", "hold": false }
//!   ]
//! }
//! ```
//!
//! - **segments**：原文的**切分方案**（不是上屏字符串）。核心不变量：
//!   segments.concat() == 主模型原文（逐码点）——不等 = **整份失败**
//!   （performance_plan_segments_not_partition）。空原文 → segments: []。
//! - **cues**：两族表演字段 head / expression（**body 已停用**，T8：交上来就丢该条
//!   并 warn）；同类按 add 合成、立即生效不排队。[] = 本轮不动（不是撤销）。
//!   头的 x/y/z 落在现有头部角（ParamAngle*），颈随之转动——**没有新参数**。
//! - **speak**：v0 旧字段，**保留可解析（弃用，V11）**；只有 segments 缺席时走
//!   旧路径。与 segments 同现 → **segments 优先、speak 忽略 + warn**（O1）。
//! - **未知顶层键一律丢弃**（宽容）。
//!
//! # 校验纪律（整份失败，不做局部抢救）
//!
//! - 坏 JSON / 顶层不是对象 / 缺 segments（且无 speak）/ 类型不对 / 词表外 field /
//!   at 非法 / 越界锚点 / 超条数 / 超长 / **拼接不等于原文** → **整份失败** → 引擎回退；
//! - 轴值越界 / intensity 越界 / ttl_ms 越界 → **钳位**（不是失败）；
//! - expression 给 z/x/y、id 给非 expression → **丢该键 + warn**；
//! - **field=body → 丢该条 cue + warn**（T8 起只演头与表情；整份失败会连带丢语音，
//!   代价不对等，所以是丢条而不是失败）；
//! - 未知表情 id → **丢该条 cue + warn**（§2.4 #16；整份失败会连带丢语音）；
//! - **expression 的 id 只认表情槽**（[EXPRESSION_PRESET_IDS] = smile / unhappy /
//!   surprised / thinking，外加撤销哨兵 none）：手势 id（nod / tilt_left / look_up …）
//!   填进 expression → **丢该条 cue + warn**（T9；整份仍成功——表情表只有脸，
//!   手势 id 交上来在渲染面本来也会被丢掉）。
//!
//! # 问句补丁（T9，2026-10-07）
//!
//! 校验**成功之后**，[apply_question_patch](super::question::apply_question_patch)
//! 用本地词典给「第一处问句所在的那一段」补一次歪头
//! （head x=0 y=0.12 z=0.45 intensity=2 1800ms）与思考
//! （expression id=thinking intensity=2 2600ms），两样都锚在该段音频开始。
//! 该段上模型交来的 head / expression **换成词典值**；别的段原样保留；
//! segments 一个字不改、不加标签、不随机、不跨轮常驻。没有问号（只认 ？ / ?）
//! 就什么都不补。legacy speak 路径不补。
//!
//! # 为什么 schema 写在代码里
//!
//! [json_schema_strict] **由本文件的常量拼出**，同一个文件里的 [parse_plan]
//! 逐条实现同样的边界；回归 schema_and_validator_share_the_same_bounds 钉住
//! 「文档里的 schema == 发出去的 schema == 校验器认的东西」。不要在别处再抄一份。
//!
//! # 行数说明（AGENTS.md 豁免）
//!
//! 本文件 > 500 行：v1 schema / 严格校验 / 三字段 cue / wire 投影与 v0 legacy
//! 兼容面同住一处（schema 与校验器**必须同源**），按「豁免 ≤ 1000 需头注理由」保留。
//! 2026-10-07（T9）：问句词典拆去同目录的 question.rs（那里是那套值的单一真源），
//! 本文件因此回到 1000 行以内——上文那句「拆文件会被 C1 判为越权」是 4b 那一轮
//! 的授权边界记录，T9 的拆分不在它的约束范围。
//!
//! # v1 cue 的 wire 投影（B9 / V11 / D27）
//!
//! v1 的新键（`field` / `x` / `y` / `z` / `id` / `at` / `hold` /
//! `seq`）是 [PerformanceCue] 上的**显式可选字段**（`Option`，缺省 `None`）——
//! `preset_id` **只承载预设 id 本身**，绝不承载 JSON（Gate 4 D27：控制字符
//! 信封退场）。[PerformanceCue::to_json] 把它们摊平成**既有 action_cue 帧**上的
//! 可选键；既有键 sentence_seq/preset_id/intensity/ttl_ms/priority 逐字保留；
//! `field == None` = 旧 preset_id 路径，语义逐字不变（V11）。
//!
//! `epoch` **不在 cue 上**：它是 `action_cue` 帧的 payload 级字段
//!（[action_cue_payload]），前端→渲染面的 `preset` 消息按 D30 单独透传。

use serde_json::{Map, Value, json};

use super::question::apply_question_patch;

/// 一份 plan 里 cue 的条数上限（防模型灌爆）。
pub const MAX_CUES: usize = 16;
/// segments 的段数上限（O2）。
pub const MAX_SEGMENTS: usize = 64;
/// segments 的**总字符**上限（O2；超限整份失败，不截断——截断会破坏 V1 不变量）。
pub const MAX_SEGMENT_CHARS: usize = 4_000;
/// v0 遗留 speak 的字符数上限（超出按字符截断）。
pub const MAX_SPEAK_CHARS: usize = 4_000;
/// 强度下限（与渲染面 preset 的 intensity 钳位同口径）。
pub const MIN_INTENSITY: u8 = 1;
/// 强度上限。
pub const MAX_INTENSITY: u8 = 3;
/// ttl 下限（毫秒）。
pub const MIN_TTL_MS: u64 = 1;
/// ttl 上限（毫秒；与渲染面 MAX_TTL_MS 同口径）。
pub const MAX_TTL_MS: u64 = 5_000;
/// 归一化轴值下限。
pub const AXIS_MIN: f64 = -1.0;
/// 归一化轴值上限。
pub const AXIS_MAX: f64 = 1.0;
/// body 的默认 ttl_ms（O3）。
pub const DEFAULT_TTL_MS_BODY: u64 = 900;
/// head 的默认 ttl_ms（O3）。
pub const DEFAULT_TTL_MS_HEAD: u64 = 900;
/// expression 的默认 ttl_ms（O3）。
pub const DEFAULT_TTL_MS_EXPRESSION: u64 = 2_600;

/// 表演层 cue 的固定优先级（**应用层写死，模型不得自报**）。
///
/// 与 live2d-ai-mod-director::plan::PRIORITY_ASYNC 同值（40）——它是
/// 「表演层覆盖规则层」的既有抬升口径；数字写在这里而不是从 Mod 引，避免
/// runtime 反向依赖 Mod crate。
pub const PRIORITY_PERFORMANCE: u8 = 40;

/// **expression 字段的 id 能力集**（表情表只有脸，V3 / T9）。
///
/// 手势 id（nod / shake / look_* / tilt_*）**不得**出现在 expression 上：
/// 交上来一律丢该条 + warn（整份仍成功），因为渲染面的表情表只有五官。
/// none 是撤销哨兵，由校验器与 schema 单独补，不在本表里。
///
/// 数字写在这里而不是从 Mod crate 引（理由同 [PRIORITY_PERFORMANCE]）：director
/// 的 preset_slot(id) == Face（去掉 none）必须与这一份**逐项相等**，回归
/// presets::tests::expression_slot_matches_the_performance_field_map 钉住。
pub const EXPRESSION_PRESET_IDS: &[&str] = &["smile", "unhappy", "surprised", "thinking"];

/// 该 id 是否是 expression 字段可用的表情 id（none 另算）。
pub fn is_expression_preset(id: &str) -> bool {
    EXPRESSION_PRESET_IDS.contains(&id)
}

/// 三族表演字段（V2 / V3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueField {
    /// 半身摆动 / 倾斜。
    Body,
    /// 头部点头 / 摇头 / 歪头。
    Head,
    /// 只写五官（V3）。
    Expression,
}

impl CueField {
    /// wire 名。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Head => "head",
            Self::Expression => "expression",
        }
    }

    /// 省略 ttl_ms 时的默认时长（O3）。
    pub fn default_ttl_ms(&self) -> u64 {
        match self {
            Self::Body => DEFAULT_TTL_MS_BODY,
            Self::Head => DEFAULT_TTL_MS_HEAD,
            Self::Expression => DEFAULT_TTL_MS_EXPRESSION,
        }
    }
}

/// at 锚点（V6）；AfterPrev 的 O4 退化在解析期落成 now。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueAnchor {
    /// 立即生效（段 A = 墙钟；段 B = 音频时钟当前位置）。
    Now,
    /// 第 N 段音频开始播放（1-based）。
    Seg(u64),
    /// 上一条 cue 的动作做完（事件式）。
    AfterPrev,
}

/// 一条 v1 表演 cue（解析产物；sentence_seq / at_resolved 已解析）。
#[derive(Debug, Clone, PartialEq)]
pub struct FieldCue {
    /// 三族字段。
    pub field: CueField,
    /// 归一化轴值（已钳位；不适用则为 None）。
    pub x: Option<f64>,
    /// 归一化轴值。
    pub y: Option<f64>,
    /// 歪头倾斜（仅 head）。
    pub z: Option<f64>,
    /// expression 的面板 id（含撤销哨兵 none）。
    pub id: Option<String>,
    /// 强度 1..=3（已钳位）。
    pub intensity: u8,
    /// 原始锚点。
    pub at: CueAnchor,
    /// 解析后的锚点 wire 名（now / seg:N / after_prev；O4 退化后为 now）。
    pub at_resolved: String,
    /// true = 保持到下次指令（ttl_ms 被忽略）。
    pub hold: bool,
    /// 生效时长（毫秒，已钳位 / 已套默认）。
    pub ttl_ms: u64,
    /// cue 序号（与 plan 内顺序一致，从 1 起；丢条后不重排）。
    pub seq: u64,
    /// 解析后的锚段序号（1-based；seg:N ≡ sentence_seq == N）。
    pub sentence_seq: u64,
}

impl FieldCue {
    /// 投影为 wire 单元：v1 新键落在 [PerformanceCue] 的**显式可选字段**上
    /// （D27：不再编进 preset_id）；[PerformanceCue::to_json] 负责摊平。
    pub fn to_wire_cue(&self) -> PerformanceCue {
        let semantic_preset_id = match self.field {
            CueField::Expression => self.id.clone().unwrap_or_else(|| "none".to_string()),
            CueField::Body => "body".to_string(),
            CueField::Head => "head".to_string(),
        };
        PerformanceCue {
            sentence_seq: self.sentence_seq,
            preset_id: semantic_preset_id,
            intensity: self.intensity,
            ttl_ms: self.ttl_ms,
            field: Some(self.field),
            x: self.x.and_then(serde_json::Number::from_f64),
            y: self.y.and_then(serde_json::Number::from_f64),
            z: self.z.and_then(serde_json::Number::from_f64),
            id: self.id.clone(),
            at: Some(self.at_resolved.clone()),
            hold: Some(self.hold),
            seq: Some(self.seq),
        }
    }
}

/// 一条按句 cue（与 WS action_cue.payload.cues[] 的**既有键**逐字段同形）。
///
/// # v1 新键 = 显式可选字段（D27）
///
/// `field` / `x` / `y` / `z` / `id` / `at` / `hold` / `seq` 都是
/// `Option`（缺省 `None`）。`field == None` = **legacy preset_id 路径**，
/// [Self::to_json] 只出既有 5 键，语义逐字不变（V11）。轴值用
/// [serde_json::Number] 承载：既有 `derive(Eq)`（desktop 侧的
/// `ConversationUiEvent` 依赖它）不得因为多一个 `f64` 字段而被迫摘掉。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PerformanceCue {
    /// 目标句序号（与 AudioChunk.sentence_seq 同源；锚点是该句音频的 first_chunk）。
    pub sentence_seq: u64,
    /// 预设 id（**只**是 id 本身；legacy 路径直接用，v1 路径是语义占位：
    /// body/head 用字段名、expression 用表情 id）。绝不承载 JSON（D27）。
    pub preset_id: String,
    /// 强度 1..=3（已钳位）。
    pub intensity: u8,
    /// 生效时长（毫秒）1..=5000（已钳位）。
    pub ttl_ms: u64,
    /// v1：三族字段之一；`None` = 旧 preset_id 路径。
    pub field: Option<CueField>,
    /// v1：归一化 X 轴（[-1, 1]，已钳位；不适用则 `None`）。
    pub x: Option<serde_json::Number>,
    /// v1：归一化 Y 轴。
    pub y: Option<serde_json::Number>,
    /// v1：歪头倾斜（仅 head）。
    pub z: Option<serde_json::Number>,
    /// v1：expression 的面板 id（含撤销哨兵 none）。
    pub id: Option<String>,
    /// v1：解析后的锚点 wire 名（now / seg:N / after_prev）。
    pub at: Option<String>,
    /// v1：true = 保持到下次指令。
    pub hold: Option<bool>,
    /// v1：plan 内 cue 序号（从 1 起；丢条不重排）。
    pub seq: Option<u64>,
}

impl PerformanceCue {
    /// WS action_cue 单条形态。
    ///
    /// v1 新键（`field == Some(..)` 才出）**摊平**为可选键
    /// （V11：只增不改；既有键逐字保留）。legacy（`field == None`）只出
    /// 既有 5 键——与信封年代**逐字相同**（golden 回归钉住）。
    pub fn to_json(&self) -> Value {
        let mut object = Map::new();
        object.insert("sentence_seq".to_string(), json!(self.sentence_seq));
        object.insert("preset_id".to_string(), json!(self.preset_id));
        object.insert("intensity".to_string(), json!(self.intensity));
        object.insert("ttl_ms".to_string(), json!(self.ttl_ms));
        object.insert("priority".to_string(), json!(PRIORITY_PERFORMANCE));
        if let Some(field) = self.field {
            object.insert("field".to_string(), json!(field.as_str()));
            if let Some(seq) = self.seq {
                object.insert("seq".to_string(), json!(seq));
            }
            for (key, value) in [("x", &self.x), ("y", &self.y), ("z", &self.z)] {
                if let Some(number) = value {
                    object.insert(key.to_string(), Value::Number(number.clone()));
                }
            }
            if let Some(id) = &self.id {
                object.insert("id".to_string(), json!(id));
            }
            if let Some(at) = &self.at {
                object.insert("at".to_string(), json!(at));
            }
            if let Some(hold) = self.hold {
                object.insert("hold".to_string(), json!(hold));
            }
        }
        Value::Object(object)
    }

    /// 日志 / 断言的短标签（**不含全文**）。
    pub fn label(&self) -> String {
        match self.field {
            Some(field) => match &self.id {
                Some(id) => format!("{}:{id}", field.as_str()),
                None => field.as_str().to_string(),
            },
            None => self.preset_id.clone(),
        }
    }
}

/// 宽容警告（丢键 / 丢条；**不整份失败**）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanWarning {
    /// segments 与 speak 同现 → speak 被忽略（O1）。
    SpeakIgnored,
    /// 该字段不允许的轴键被丢弃（§2.4 #11）。
    AxisNotAllowed {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 被丢弃的键名。
        key: &'static str,
    },
    /// 非 expression 给了 id → 丢键。
    IdNotAllowed {
        /// 第几条 cue（0 基）。
        index: usize,
    },
    /// `field=body` 已停用 → 丢该条 cue（T8；**不整份失败**）。
    BodyNotAllowed {
        /// 第几条 cue（0 基）。
        index: usize,
    },
    /// 未知表情 id → 丢该条 cue（§2.4 #16）。
    ExpressionUnknownId {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 原始 id。
        id: String,
    },
}

impl PlanWarning {
    /// 稳定警告码（进日志；**不含正文**）。
    pub fn code(&self) -> &'static str {
        match self {
            Self::SpeakIgnored => "performance_plan_speak_ignored",
            Self::AxisNotAllowed { .. } | Self::IdNotAllowed { .. } => {
                "performance_axis_not_allowed"
            }
            Self::BodyNotAllowed { .. } => "performance_plan_body_not_allowed",
            Self::ExpressionUnknownId { .. } => "performance_expression_unknown_id",
        }
    }

    /// 给人看的一句话（**不含正文**；id 可以带）。
    pub fn message(&self) -> String {
        match self {
            Self::SpeakIgnored => "segments 与 speak 同现：按 O1 忽略 speak".to_string(),
            Self::AxisNotAllowed { index, key } => {
                format!("第 {index} 条 cue 的 {key} 对该字段不适用：已丢弃该键")
            }
            Self::IdNotAllowed { index } => {
                format!("第 {index} 条 cue 的 id 只对 expression 有效：已丢弃该键")
            }
            Self::BodyNotAllowed { index } => {
                format!("第 {index} 条 cue 的 field=body 已停用（只演头与表情）：已丢弃该条")
            }
            Self::ExpressionUnknownId { index, id } => {
                format!(
                    "第 {index} 条 cue 的 expression id={id} 不是表情槽 id（或不在能力集内）：已丢弃该条"
                )
            }
        }
    }
}

/// 一份表演层 plan。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PerformancePlan {
    /// v0 遗留：本轮要说的话；None = 不说。
    pub speak: Option<String>,
    /// v1：原文的切分方案；None = 走 legacy speak 路径。
    pub segments: Option<Vec<String>>,
    /// wire cue（legacy 路径或 v1 显式字段投影，见 [PerformanceCue]）。
    pub cues: Vec<PerformanceCue>,
    /// 宽容警告（丢键 / 丢条）。
    pub warnings: Vec<PlanWarning>,
}

impl PerformancePlan {
    /// 本轮确实什么都不做（不说也不动）。
    pub fn is_noop(&self) -> bool {
        let no_text = match &self.segments {
            Some(segments) => segments.is_empty(),
            None => self.speak.is_none(),
        };
        no_text && self.cues.is_empty()
    }

    /// 日志用摘要：**不含正文全文**（只给长度与 cue 标签列表）。
    pub fn summary(&self) -> String {
        let text = match &self.segments {
            Some(segments) => format!(
                "segments={}, seg_chars={}",
                segments.len(),
                segments.iter().map(|s| s.chars().count()).sum::<usize>()
            ),
            None => format!(
                "speak_chars={}",
                self.speak.as_ref().map_or(0, |s| s.chars().count())
            ),
        };
        let labels: Vec<String> = self.cues.iter().map(PerformanceCue::label).collect();
        format!("{text}, cues={labels:?}")
    }
}

/// 校验失败原因（[PlanError::code] 是稳定错误码，进日志与回退原因码）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// 不是合法 JSON。
    NotJson(String),
    /// 顶层不是对象。
    NotObject,
    /// 既无 segments 也无 speak（§2.4 #3）。
    MissingSegments,
    /// segments 不是数组。
    SegmentsType,
    /// segments 里有非字符串元素（第几条，0 基）。
    SegmentNotString(usize),
    /// segments 段数 / 总字符超限（O2；不截断）。
    SegmentsTooLong(String),
    /// segments.concat() != 原文（V1 核心不变量）。
    SegmentsNotPartition,
    /// speak 存在但不是 string / null。
    SpeakType,
    /// 缺 cues 字段。
    MissingCues,
    /// cues 不是数组。
    CuesType,
    /// cue 条数超过 [MAX_CUES]。
    TooManyCues(usize),
    /// cue 不是对象。
    CueNotObject(usize),
    /// cue 缺字段 / 字段类型不对（field 是字段名，index 是第几条）。
    CueField {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 字段名。
        field: &'static str,
    },
    /// field 词表外（§2.4 #10）。
    UnknownField {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 原始值。
        field: String,
    },
    /// at 非法 / 越界锚点（§2.4 #14/#15）。
    BadAnchor {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 原始值。
        at: String,
    },
    /// legacy 路径：preset_id 不在能力集内。
    UnknownPreset {
        /// 第几条 cue（0 基）。
        index: usize,
        /// 原始 id。
        preset_id: String,
    },
}

impl PlanError {
    /// 稳定错误码（进日志 / 回退原因；**不含正文**）。
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotJson(_) => "performance_plan_not_json",
            Self::NotObject => "performance_plan_not_object",
            Self::MissingSegments => "performance_plan_missing_segments",
            Self::SegmentsType => "performance_plan_segments_type",
            Self::SegmentNotString(_) => "performance_plan_segment_not_string",
            Self::SegmentsTooLong(_) => "performance_plan_segments_too_long",
            Self::SegmentsNotPartition => "performance_plan_segments_not_partition",
            Self::SpeakType => "performance_plan_speak_type",
            Self::MissingCues => "performance_plan_missing_cues",
            Self::CuesType => "performance_plan_cues_type",
            Self::TooManyCues(_) => "performance_plan_too_many_cues",
            Self::CueNotObject(_) => "performance_plan_cue_not_object",
            Self::CueField { .. } => "performance_plan_cue_field",
            Self::UnknownField { .. } => "performance_plan_unknown_field",
            Self::BadAnchor { .. } => "performance_plan_bad_anchor",
            Self::UnknownPreset { .. } => "performance_plan_unknown_preset",
        }
    }

    /// 给人看的一句话（**不含 speak / segments 全文**；id 可以带）。
    pub fn message(&self) -> String {
        match self {
            Self::NotJson(e) => format!("不是合法 JSON：{e}"),
            Self::NotObject => "顶层不是 JSON 对象".to_string(),
            Self::MissingSegments => "既缺 segments 也缺 speak".to_string(),
            Self::SegmentsType => "segments 不是数组".to_string(),
            Self::SegmentNotString(i) => format!("segments 第 {i} 个元素不是字符串"),
            Self::SegmentsTooLong(detail) => format!("segments 超限（不截断）：{detail}"),
            Self::SegmentsNotPartition => {
                "segments 拼接不等于主模型原文（V1：只能切分，不能改写）".to_string()
            }
            Self::SpeakType => "speak 既不是 string 也不是 null".to_string(),
            Self::MissingCues => "缺 cues 字段".to_string(),
            Self::CuesType => "cues 不是数组".to_string(),
            Self::TooManyCues(n) => format!("cue 条数 {n} 超过上限 {MAX_CUES}"),
            Self::CueNotObject(i) => format!("第 {i} 条 cue 不是对象"),
            Self::CueField { index, field } => {
                format!("第 {index} 条 cue 的 {field} 缺失或类型不对")
            }
            Self::UnknownField { index, field } => {
                format!("第 {index} 条 cue 的 field={field} 不在词表内")
            }
            Self::BadAnchor { index, at } => {
                format!("第 {index} 条 cue 的 at={at} 非法或越界")
            }
            Self::UnknownPreset { index, preset_id } => {
                format!("第 {index} 条 cue 的 preset_id={preset_id} 不在能力集内")
            }
        }
    }
}

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
