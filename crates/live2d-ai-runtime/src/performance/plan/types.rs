use serde_json::{Map, Value, json};

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
/// 与已退休 director 的异步 priority 同值（40）——它是「表演层覆盖规则层」的
/// 既有抬升口径；数字写在这里而不是从 Mod 引，避免 runtime 反向依赖 Mod crate。
/// （director 的 `plan` 模块已于 rc4 删除，这个 40 保留为 wire 契约。）
pub const PRIORITY_PERFORMANCE: u8 = 40;

/// **expression 字段的 id 能力集**（表情表只有脸，V3 / T9）。
///
/// 手势 id（nod / shake / look_* / tilt_*）**不得**出现在 expression 上：
/// 交上来一律丢该条 + warn（整份仍成功），因为渲染面的表情表只有五官。
/// none 是撤销哨兵，由校验器与 schema 单独补，不在本表里。
///
/// 数字写在这里而不是从 Mod crate 引（理由同 [PRIORITY_PERFORMANCE]）：规则表的
/// `preset_slot(id) == Face`（去掉 none）必须与这一份**逐项相等**，回归
/// `performance::rules::tests::expression_slot_matches_the_performance_field_map` 钉住。
/// （规则表已于 rc4 迁入 runtime `performance::rules`。）
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
