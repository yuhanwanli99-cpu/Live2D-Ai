//! 导演决策的**纯函数**层（无 IO / 无网络 / 无时钟 / 无随机）。
//!
//! 输入 = 本轮输入正文 + 情绪词表档位；输出 = [`Decision`]（情绪 / 意图 /
//! TTS **建议**参数）。同一输入恒等输出，单测不需要 mock / sleep。
//!
//! **纪律**（与 crate 头注同一条）：
//!
//! - 这里算出的 `suggested_tts` 是**建议**，只进日志与 `state_json`；
//!   **绝不** `apply_settings`、**绝不** `action_tx`、**绝不**改主链任何字段；
//! - 没有证据不是故障：未知词 / 空串 / 纯空白 / 纯标点 / 超长截断后无命中
//!   → `Neutral`（永不返回「错误」）。

/// 情绪提示。`as_str` 的字符串是稳定契约（进 `state_json` / 日志）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmotionHint {
    /// 无证据（fail-safe 落点）。
    Neutral,
    Happy,
    Sad,
    Angry,
    Surprised,
    Anxious,
    Affectionate,
}

/// 情绪平局裁决优先级（**先出现者优先**）；[`score_emotions`] 也按此序返回。
pub const EMOTION_PRIORITY: [EmotionHint; 6] = [
    EmotionHint::Happy,
    EmotionHint::Sad,
    EmotionHint::Angry,
    EmotionHint::Surprised,
    EmotionHint::Anxious,
    EmotionHint::Affectionate,
];

impl EmotionHint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Happy => "happy",
            Self::Sad => "sad",
            Self::Angry => "angry",
            Self::Surprised => "surprised",
            Self::Anxious => "anxious",
            Self::Affectionate => "affectionate",
        }
    }
}

/// 意图提示。`Silence` 只在清洗后正文为空时出现，且**不产生决策**。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntentHint {
    Chat,
    Question,
    Greeting,
    Farewell,
    Request,
    Complaint,
    Silence,
}

impl IntentHint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Question => "question",
            Self::Greeting => "greeting",
            Self::Farewell => "farewell",
            Self::Request => "request",
            Self::Complaint => "complaint",
            Self::Silence => "silence",
        }
    }
}

/// 情绪词表档位（`settings_spec` 的 `emotion_lexicon` Select）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lexicon {
    /// 内置词表：弱（权重 1）+ 强（权重 2）关键词都算命中。
    #[default]
    Builtin,
    /// 保守档：**只有强关键词**（权重 ≥ 2）算命中。
    Strict,
}

impl Lexicon {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Strict => "strict",
        }
    }

    /// 解析配置字符串；未知 / 缺失 → 缺省档（[`Lexicon::Builtin`]）。
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("strict") => Self::Strict,
            _ => Self::Builtin,
        }
    }

    /// 本档位采纳的最小权重（1 = 弱强都算，2 = 只算强）。
    const fn min_weight(self) -> u32 {
        match self {
            Self::Builtin => 1,
            Self::Strict => 2,
        }
    }
}

/// TTS **建议**参数（速度 / 音高，1.0 = 不变）。
///
/// 只是建议：骨架**不**把它投递给任何通道（见 crate 头注）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TtsSuggestion {
    pub speed: f64,
    pub pitch: f64,
}

/// 情绪 → 速度基准增量（在 1.0 之上 / 之下）。
const EMOTION_SPEED_DELTA: [(EmotionHint, f64); 6] = [
    (EmotionHint::Happy, 0.08),
    (EmotionHint::Sad, -0.10),
    (EmotionHint::Angry, 0.12),
    (EmotionHint::Surprised, 0.05),
    (EmotionHint::Anxious, 0.06),
    (EmotionHint::Affectionate, -0.05),
];

/// 情绪 → 音高基准增量。
const EMOTION_PITCH_DELTA: [(EmotionHint, f64); 6] = [
    (EmotionHint::Happy, 0.10),
    (EmotionHint::Sad, -0.08),
    (EmotionHint::Angry, 0.06),
    (EmotionHint::Surprised, 0.15),
    (EmotionHint::Anxious, 0.04),
    (EmotionHint::Affectionate, 0.08),
];

/// 速度缺省值。
pub const DEFAULT_SPEED: f64 = 1.0;
/// 音高缺省值。
pub const DEFAULT_PITCH: f64 = 1.0;
/// 速度下限。
pub const MIN_SPEED: f64 = 0.5;
/// 速度上限。
pub const MAX_SPEED: f64 = 1.5;
/// 音高下限。
pub const MIN_PITCH: f64 = 0.5;
/// 音高上限。
pub const MAX_PITCH: f64 = 1.5;
/// 每个强调级（`!` / `！`）追加的音高增量。
pub const PITCH_PER_EMPHASIS: f64 = 0.05;
/// 强调级上限（再多也只是这一档）。
pub const MAX_EMPHASIS: u8 = 2;
/// 推导前截断的最大**字符**数（按字符，不按字节，避免劈开多字节字符）。
pub const MAX_TEXT_CHARS: usize = 2000;

/// 一次推导的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decision {
    pub emotion: EmotionHint,
    pub intent: IntentHint,
    pub suggested_tts: TtsSuggestion,
}

impl Decision {
    /// 空输入落点：中性 + 静默 + 缺省参数（**不产生决策**，也不写配置）。
    pub const fn silent() -> Self {
        Self {
            emotion: EmotionHint::Neutral,
            intent: IntentHint::Silence,
            suggested_tts: TtsSuggestion {
                speed: DEFAULT_SPEED,
                pitch: DEFAULT_PITCH,
            },
        }
    }

    /// 是否为「静默轮」（正文清洗后为空）。
    pub const fn is_silent(&self) -> bool {
        matches!(self.intent, IntentHint::Silence)
    }
}

/// 清洗：去掉首尾空白；超长按**字符**截断。永不失败、永不 panic。
pub fn clean_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= MAX_TEXT_CHARS {
        trimmed.to_string()
    } else {
        trimmed.chars().take(MAX_TEXT_CHARS).collect()
    }
}

/// 否定前缀：紧跟关键词前一个字时，该次命中作废（`不开心` 不算 Happy）。
const NEGATIONS: [char; 6] = ['不', '没', '未', '别', '无', '莫'];

/// 关键词命中次数：**逐次匹配**，每次都要过两道闸——
/// ① ASCII 关键词必须有词边界（`unhappy` 不算 `happy`）；
/// ② 命中点前一个字不是否定前缀（`不开心` 不算 `开心`）。
pub fn count_keyword(haystack: &str, needle: &str) -> u32 {
    if needle.is_empty() {
        return 0;
    }
    let bytes = haystack.as_bytes();
    let ascii = needle.is_ascii();
    let mut hits = 0_u32;
    let mut from = 0_usize;
    while let Some(rel) = haystack[from..].find(needle) {
        let idx = from + rel;
        // 前进**一个字符**（不是一字节）：中文关键词占多字节，idx + 1 会落在字符内部。
        from = idx + haystack[idx..].chars().next().map_or(1, char::len_utf8);
        if ascii {
            let before_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
            let end = idx + needle.len();
            let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();
            if !(before_ok && after_ok) {
                continue;
            }
        }
        let negated = haystack[..idx]
            .chars()
            .next_back()
            .is_some_and(|c| NEGATIONS.contains(&c));
        if !negated {
            hits += 1;
        }
    }
    hits
}

/// 是否包含关键词（[`count_keyword`] 的布尔包装）。
pub fn contains_keyword(haystack: &str, needle: &str) -> bool {
    count_keyword(haystack, needle) > 0
}

/// 命中任一关键词。
fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| contains_keyword(haystack, n))
}

const HAPPY_WORDS: &[(&str, u32)] = &[
    ("开心", 1),
    ("高兴", 1),
    ("愉快", 1),
    ("太好了", 2),
    ("哈哈", 1),
    ("棒", 1),
    ("happy", 1),
    ("great", 1),
];
const SAD_WORDS: &[(&str, u32)] = &[
    ("难过", 1),
    ("伤心", 2),
    ("委屈", 1),
    ("想哭", 2),
    ("低落", 1),
    ("sad", 1),
    ("哭", 1),
];
const ANGRY_WORDS: &[(&str, u32)] = &[
    ("生气", 1),
    ("愤怒", 2),
    ("讨厌", 1),
    ("好烦", 2),
    ("气死", 2),
    ("烦", 1),
    ("angry", 1),
];
const SURPRISED_WORDS: &[(&str, u32)] = &[
    ("惊讶", 1),
    ("居然", 1),
    ("竟然", 1),
    ("天呐", 2),
    ("哇", 1),
    ("wow", 1),
    ("surprise", 1),
];
const ANXIOUS_WORDS: &[(&str, u32)] = &[
    ("焦虑", 2),
    ("紧张", 1),
    ("担心", 1),
    ("害怕", 2),
    ("不安", 1),
    ("anxious", 1),
    ("worried", 1),
];
const AFFECTIONATE_WORDS: &[(&str, u32)] = &[
    ("喜欢", 1),
    ("爱你", 2),
    ("抱抱", 1),
    ("亲亲", 1),
    ("想你", 1),
    ("love", 1),
    ("miss", 1),
];

const QUESTION_WORDS: &[&str] = &["吗", "呢", "什么", "怎么", "为什么", "如何", "哪"];
const GREETING_WORDS: &[&str] = &[
    "你好",
    "您好",
    "早上好",
    "晚上好",
    "嗨",
    "哈喽",
    "hello",
    "hi",
    "hey",
];
const FAREWELL_WORDS: &[&str] = &["再见", "拜拜", "晚安", "bye", "goodbye", "see you"];
const REQUEST_WORDS: &[&str] = &[
    "请",
    "帮我",
    "帮忙",
    "能不能",
    "可以吗",
    "麻烦",
    "please",
    "help",
];
const COMPLAINT_WORDS: &[&str] = &["讨厌", "烦", "累", "难受", "不舒服", "生气", "抱怨"];

/// 单表评分：权重 ≥ 档位阈值的关键词命中数 × 权重，求和。
fn score_words(text: &str, words: &[(&str, u32)], lexicon: Lexicon) -> u32 {
    let min = lexicon.min_weight();
    words
        .iter()
        .filter(|(_, w)| *w >= min)
        .map(|(word, w)| count_keyword(text, word) * w)
        .sum()
}

/// 六种情绪各自得分，**按 [`EMOTION_PRIORITY`] 顺序**返回（平局靠这个顺序裁决）。
pub fn score_emotions(text: &str, lexicon: Lexicon) -> [(EmotionHint, u32); 6] {
    [
        (EmotionHint::Happy, score_words(text, HAPPY_WORDS, lexicon)),
        (EmotionHint::Sad, score_words(text, SAD_WORDS, lexicon)),
        (EmotionHint::Angry, score_words(text, ANGRY_WORDS, lexicon)),
        (
            EmotionHint::Surprised,
            score_words(text, SURPRISED_WORDS, lexicon),
        ),
        (
            EmotionHint::Anxious,
            score_words(text, ANXIOUS_WORDS, lexicon),
        ),
        (
            EmotionHint::Affectionate,
            score_words(text, AFFECTIONATE_WORDS, lexicon),
        ),
    ]
}

/// 取最高分；全 0 → [`EmotionHint::Neutral`]。平局取优先级靠前者。
pub fn pick_emotion(scores: &[(EmotionHint, u32)]) -> EmotionHint {
    let mut best = EmotionHint::Neutral;
    let mut best_score = 0_u32;
    for (emotion, score) in scores {
        if *score > best_score {
            best = *emotion;
            best_score = *score;
        }
    }
    best
}

/// 意图推导：**首个匹配者胜**，顺序 Question > Greeting > Farewell > Request >
/// Complaint > Chat（空输入 → Silence）。
pub fn derive_intent(text: &str) -> IntentHint {
    if text.is_empty() {
        return IntentHint::Silence;
    }
    if text.contains('？') || text.contains('?') || contains_any(text, QUESTION_WORDS) {
        return IntentHint::Question;
    }
    if contains_any(text, GREETING_WORDS) {
        return IntentHint::Greeting;
    }
    if contains_any(text, FAREWELL_WORDS) {
        return IntentHint::Farewell;
    }
    if contains_any(text, REQUEST_WORDS) {
        return IntentHint::Request;
    }
    if contains_any(text, COMPLAINT_WORDS) {
        return IntentHint::Complaint;
    }
    IntentHint::Chat
}

/// 强调级：`!` / `！` 的个数，封顶 [`MAX_EMPHASIS`]。
pub fn emphasis_level(text: &str) -> u8 {
    let n = text
        .chars()
        .filter(|c| *c == '!' || *c == '！')
        .count()
        .min(MAX_EMPHASIS as usize);
    n as u8
}

/// 由情绪 + 强调级算**建议**参数。强调只抬音高（顿挫），不改语速。
pub fn suggest_tts(emotion: EmotionHint, emphasis: u8) -> TtsSuggestion {
    let speed_delta = EMOTION_SPEED_DELTA
        .iter()
        .find(|(e, _)| *e == emotion)
        .map(|(_, d)| *d)
        .unwrap_or(0.0);
    let pitch_delta = EMOTION_PITCH_DELTA
        .iter()
        .find(|(e, _)| *e == emotion)
        .map(|(_, d)| *d)
        .unwrap_or(0.0);
    let emphasis = f64::from(emphasis.min(MAX_EMPHASIS));
    TtsSuggestion {
        speed: round2((DEFAULT_SPEED + speed_delta).clamp(MIN_SPEED, MAX_SPEED)),
        pitch: round2(
            (DEFAULT_PITCH + pitch_delta + PITCH_PER_EMPHASIS * emphasis)
                .clamp(MIN_PITCH, MAX_PITCH),
        ),
    }
}

/// 保留两位小数（让 `state_json` 里的数值稳定可断言，不出现 1.1500000000000001）。
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// **纯函数入口**：正文 + 词表档位 → [`Decision`]。
///
/// 同一输入恒等输出；空 / 纯空白 → [`Decision::silent`]。
pub fn derive(text: &str, lexicon: Lexicon) -> Decision {
    let cleaned = clean_text(text);
    if cleaned.is_empty() {
        return Decision::silent();
    }
    let emotion = pick_emotion(&score_emotions(&cleaned, lexicon));
    let intent = derive_intent(&cleaned);
    Decision {
        emotion,
        intent,
        suggested_tts: suggest_tts(emotion, emphasis_level(&cleaned)),
    }
}
