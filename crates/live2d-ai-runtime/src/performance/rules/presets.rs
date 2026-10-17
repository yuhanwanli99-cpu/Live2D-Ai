//! 动作包表（2026-09-15，L1「导演动作预设」）——`emotion|intent → preset_id`。
//!
//! # 范围（严格按任务书）
//!
//! v3（2026-09-23）把「包」收成两族；T9（2026-10-07）加上 `thinking` /
//! `look_up` / `look_down`，主 allowlist 12 条：
//! - **表情包**（expression）：`smile` / `unhappy`（intensity morph sad→angry）/
//!   `surprised` / `thinking`（T9，2026-10-07）；
//! - **手势包**（motion）：`nod` / `shake` / `look_left` / `look_right` /
//!   `tilt_left` / `tilt_right` / `look_up` / `look_down`（T9）。
//!
//! 具体参数写在外置 `assets/actions/presets.json` 里，本表只持 **id 契约**
//! （allowlist + 面板选项 + 二路能力集三处共用同一个常量）。v2 的旧 id 已于
//! 2026-09-23 **整体删除**：配置解析回落缺省、表演层校验直接拒绝。
//!
//! **不做**：手臂 / 手指 / 特效 / 改 TTS pitch·speed 实写 /
//! 复活旧 Action 工具调用。
//!
//! 2026-10-17（rc4）：本表从 `live2d-ai-mod-director` 迁入核心，是主链按句
//! 第二路的**失败回退**（`rule_cues_for_sentence`）单一真源；director 侧只留
//! `pub use` 兼容层。
//!
//! # 为什么是这张表而不是分类器
//!
//! 情绪来自 [`super::decision::derive`]（确定性纯函数，词表打分）。预设表只是
//! 一张**映射**：同一输入恒等输出，可单测、可在面板改、没有网络与随机。

use serde_json::{Value, json};

use super::decision::{EmotionHint, IntentHint};

/// 「不投递动作」的哨兵值（也是每个键的合法取值）。
pub const PRESET_NONE: &str = "none";

/// **主 allowlist**（精简后的 id 契约，v3）：面板 Select + 二路能力集 +
/// 表演层 schema 的可见集合。**不要**在别处再抄一份 id 列表。
pub const PRESET_IDS: &[&str] = &[
    PRESET_NONE,
    "smile",
    "unhappy",
    "surprised",
    "thinking",
    "nod",
    "shake",
    "look_left",
    "look_right",
    "look_up",
    "look_down",
    "tilt_left",
    "tilt_right",
];

/// 槽位（与渲染面的 `kind` 对齐）：表情与手势可同轮并存。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresetSlot {
    /// 表情包（渲染面 kind=expression）。
    Face,
    /// 手势包（渲染面 kind=motion）。
    Gesture,
}

/// 该 id 的槽位（未知 id → [`PresetSlot::Face`]，也就是「按表情处理」）。
pub fn preset_slot(id: &str) -> PresetSlot {
    match id {
        "nod" | "shake" | "look_left" | "look_right" | "tilt_left" | "tilt_right" | "look_up"
        | "look_down" => PresetSlot::Gesture,
        _ => PresetSlot::Face,
    }
}

/// 该 id 是否可被**配置 / 表演层**接受（只认主 allowlist）。
///
/// 旧的 v2 id 已删除：一律 `false`。
pub fn is_known_preset(id: &str) -> bool {
    PRESET_IDS.contains(&id)
}

/// 表演层 / 校验用的可接受集合 = 主 allowlist。
///
/// 用途：`supervisor` 装配表演层时作为能力集（与面板 Select / 配置解析同一份
/// 常量；旧 id 已删除，不在集合里）。
pub fn accepted_preset_ids() -> Vec<String> {
    PRESET_IDS.iter().map(|s| (*s).to_string()).collect()
}

/// 生效的预设表（emotion 六档 + intent 两档）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetTable {
    pub happy: String,
    pub sad: String,
    pub angry: String,
    pub surprised: String,
    pub anxious: String,
    pub affectionate: String,
    pub greeting: String,
    pub farewell: String,
}

impl Default for PresetTable {
    /// **内置默认映射（6 条非 none，≤8）**：
    ///
    /// | 触发 | 预设 | 通道 |
    /// | --- | --- | --- |
    /// | happy | `smile` | 表情 |
    /// | affectionate | `smile` | 表情 |
    /// | sad | `unhappy`（intensity 1 = 难过相） | 表情 |
    /// | angry | `unhappy`（intensity 3 = 生气相） | 表情 |
    /// | surprised | `surprised` | 表情 |
    /// | greeting | `nod` | 手势 |
    ///
    /// `anxious` / `farewell` 缺省 `none`：焦虑与告别没有公认的短动作，
    /// 宁可不做也不乱做（不做 > 做错）。
    fn default() -> Self {
        Self {
            happy: "smile".to_string(),
            sad: "unhappy".to_string(),
            angry: "unhappy".to_string(),
            surprised: "surprised".to_string(),
            anxious: PRESET_NONE.to_string(),
            affectionate: "smile".to_string(),
            greeting: "nod".to_string(),
            farewell: PRESET_NONE.to_string(),
        }
    }
}

impl PresetTable {
    /// 从 namespaced JSON 读表：**未知 id / 类型不对 → 用缺省**，绝不失败。
    pub fn from_value(value: &Value) -> Self {
        let d = Self::default();
        Self {
            happy: pick(value, "preset_happy", &d.happy),
            sad: pick(value, "preset_sad", &d.sad),
            angry: pick(value, "preset_angry", &d.angry),
            surprised: pick(value, "preset_surprised", &d.surprised),
            anxious: pick(value, "preset_anxious", &d.anxious),
            affectionate: pick(value, "preset_affectionate", &d.affectionate),
            greeting: pick(value, "preset_greeting", &d.greeting),
            farewell: pick(value, "preset_farewell", &d.farewell),
        }
    }

    /// **本轮选哪一条**：`intent` 命中非 `none` 优先，否则看 `emotion`。
    ///
    /// 为什么 intent 优先：问候/告别是「这一轮在干什么」，比情绪更具体。
    /// 需要**表情 + 手势同轮**时用 [Self::resolve_slots]。
    ///
    /// 返回 `None` = 本轮**不投递任何预设**（两侧都是 `none`）。
    pub fn resolve(&self, emotion: EmotionHint, intent: IntentHint) -> Option<&str> {
        let by_intent = match intent {
            IntentHint::Greeting => Some(self.greeting.as_str()),
            IntentHint::Farewell => Some(self.farewell.as_str()),
            _ => None,
        };
        if let Some(id) = by_intent.filter(|id| *id != PRESET_NONE) {
            return Some(id);
        }
        let by_emotion = self.emotion_id(emotion);
        (by_emotion != PRESET_NONE).then_some(by_emotion)
    }

    /// **双槽解析（v3）**：intent → 手势，emotion → 表情；同一槽只保留一条。
    ///
    /// 例：「你好呀，今天真开心！」→ `[nod, smile]`（手势 + 表情同轮）。
    /// 顺序 = 手势在前（intent 优先），表情在后。
    pub fn resolve_slots(&self, emotion: EmotionHint, intent: IntentHint) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        let by_intent = match intent {
            IntentHint::Greeting => Some(self.greeting.as_str()),
            IntentHint::Farewell => Some(self.farewell.as_str()),
            _ => None,
        };
        if let Some(id) = by_intent.filter(|id| *id != PRESET_NONE) {
            out.push(id);
        }
        let by_emotion = self.emotion_id(emotion);
        if by_emotion != PRESET_NONE
            && !out
                .iter()
                .any(|existing| preset_slot(existing) == preset_slot(by_emotion))
        {
            out.push(by_emotion);
        }
        out
    }

    /// emotion 档 → 预设 id（可能是 `none`）。
    fn emotion_id(&self, emotion: EmotionHint) -> &str {
        match emotion {
            EmotionHint::Neutral => PRESET_NONE,
            EmotionHint::Happy => self.happy.as_str(),
            EmotionHint::Sad => self.sad.as_str(),
            EmotionHint::Angry => self.angry.as_str(),
            EmotionHint::Surprised => self.surprised.as_str(),
            EmotionHint::Anxious => self.anxious.as_str(),
            EmotionHint::Affectionate => self.affectionate.as_str(),
        }
    }

    /// `state_json.presets`：面板据此显示「现在映射到哪条」。
    pub fn to_json(&self) -> Value {
        json!({
            "happy": self.happy,
            "sad": self.sad,
            "angry": self.angry,
            "surprised": self.surprised,
            "anxious": self.anxious,
            "affectionate": self.affectionate,
            "greeting": self.greeting,
            "farewell": self.farewell,
        })
    }
}

/// 读一个键：非字符串 / 未知 id（含已删除的旧 id）→ 回落缺省。
fn pick(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| is_known_preset(s))
        .unwrap_or(fallback)
        .to_string()
}

/// Select 选项（settings_spec 用同一份，避免两处 id 列表漂移）。
///
/// 就一份主 allowlist——旧的 v2 id 已删除，面板与配置都不认识它们。
pub fn preset_options() -> Vec<String> {
    PRESET_IDS.iter().map(|s| (*s).to_string()).collect()
}

/// **规则回退**（2026-09-22，表演层的失败路径）：正文 → 至多两条按句 cue。
///
/// 口径：
/// - 纯函数（无 IO / 无网络 / 无时钟 / 无随机），同一输入恒等输出；
/// - 用 [super::decision::derive]（词表打分）+ 缺省映射表，经
///   [PresetTable::resolve_slots] 给出**手势 + 表情**（v3 允许同轮双槽）；
/// - **句号固定为 1**；按句对齐的入口是 [`rule_cues_for_sentence`]；
/// - 选不出（中性闲聊 / 焦虑 / 告别缺省 none）→ 空表；
/// - **T9（2026-10-07）**：正文含 ？ / ? 时，还空着的槽补默认值——手势槽空 →
///   tilt_left，表情槽空 → thinking（已有的问候点头 / 情绪脸优先，不被问号挤掉）。
///   问句判据与 runtime 的问句补丁**共用同一份**（has_question_mark，只认 ？/?），
///   所以这里补出来的两条与表演层的问句补丁是同一对 id。
///
/// 单一真源说明：预设 id 集合与映射表都住本模块（[`PRESET_IDS`] /
/// [`PresetTable::default`]）；主链引擎直接调用，director 只做 re-export。
pub fn rule_cues_for_text(text: &str) -> Vec<crate::performance::PerformanceCue> {
    let decision = super::decision::derive(text, super::decision::Lexicon::Builtin);
    let table = PresetTable::default();
    let mut ids = table.resolve_slots(decision.emotion, decision.intent);
    // T9：问句补默认值（只补空槽；已有的手势 / 表情一律留着）。判据与
    // runtime 的问句补丁同源，表情 id 也直接引那边那一个常量。
    if crate::performance::has_question_mark(text) {
        if !ids.iter().any(|id| preset_slot(id) == PresetSlot::Gesture) {
            ids.push("tilt_left");
        }
        if !ids.iter().any(|id| preset_slot(id) == PresetSlot::Face) {
            ids.push(crate::performance::QUESTION_EXPRESSION_ID);
        }
    }
    ids.into_iter()
        .map(|id| crate::performance::PerformanceCue {
            sentence_seq: 1,
            preset_id: id.to_string(),
            intensity: 1,
            ttl_ms: 2_000,
            ..Default::default()
        })
        .collect()
}

/// **按句规则回退**（T13，2026-10-13；rc4 起是主链常驻路径）：二路失败 /
/// 不可用时，用**这一句原文**出规则 cue，句号改成参数。
///
/// 内部仍调 [`rule_cues_for_text`]（同一张映射表），只把每条 `sentence_seq`
/// 改写成参数值。`rule_cues_for_text` 保持「seq = 1」的旧契约与旧测试不动。
/// 签名里**不出现用户句**——调用方只能把这一句一路原文传进来。
pub fn rule_cues_for_sentence(
    text: &str,
    sentence_seq: u64,
) -> Vec<crate::performance::PerformanceCue> {
    rule_cues_for_text(text)
        .into_iter()
        .map(|mut cue| {
            cue.sentence_seq = sentence_seq;
            cue
        })
        .collect()
}
