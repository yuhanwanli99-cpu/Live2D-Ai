//! `cleaning` 的内联单测（经 `#[path = "cleaning_tests.rs"]` 挂回，保持父文件 ≤500 行）。

use super::*;
use crate::performance::client::{PerformanceFuture, PerformanceReply};

/// 固定回一份 raw 的测试替身（`reply=None` = 请求失败）。
#[derive(Debug)]
struct StubCleaner {
    reply: Option<String>,
}

impl PerformanceClient for StubCleaner {
    fn enabled(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "injected"
    }
    fn has_api_key(&self) -> bool {
        false
    }
    fn request<'a>(
        &'a self,
        _system: &'a str,
        _user: &'a str,
        _timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
        let reply = self.reply.clone().map(|raw| PerformanceReply {
            raw,
            structured: false,
        });
        Box::pin(async move { reply })
    }
}

/// **计划口径的那份假响应**：TTS 文本 `你好`、上屏文本 `你好😄`。
#[tokio::test]
async fn accepted_plan_splits_display_and_speech() {
    let cleaner = SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: Some(r#"{"display":"你好😄","speech":"你好"}"#.to_string()),
        }),
        CLEAN_TIMEOUT_MS,
    );
    let cleaned = cleaner.clean("你好😄").await;
    assert_eq!(cleaned.speech, "你好", "送 TTS 的必须只有可念文本");
    assert_eq!(cleaned.display, "你好😄", "上屏必须保留 emoji");
}

/// 空 `speech` = 这一句不发 TTS（引擎侧走静音句），上屏照旧。
#[tokio::test]
async fn empty_speech_is_kept_and_display_survives() {
    let cleaner = SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: Some(r#"{"display":"（挥手）","speech":""}"#.to_string()),
        }),
        CLEAN_TIMEOUT_MS,
    );
    let cleaned = cleaner.clean("（挥手）").await;
    assert_eq!(cleaned.speech, "");
    assert_eq!(cleaned.display, "（挥手）");
}

/// `display` 不是原文切片 → 整句作废，改送原文（上屏也是原文）。
#[tokio::test]
async fn display_must_be_a_contiguous_slice_of_the_sentence() {
    let cleaner = SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: Some(r#"{"display":"你好啊","speech":"你好啊"}"#.to_string()),
        }),
        CLEAN_TIMEOUT_MS,
    );
    let cleaned = cleaner.clean("你好😄").await;
    assert_eq!(
        cleaned,
        CleanedSentence::raw("你好😄"),
        "不是切片就要整句作废"
    );
}

/// 请求失败 / 坏 JSON / 缺字段 → 一律原文（**不新写正则去猜**）。
#[tokio::test]
async fn failures_fall_back_to_the_raw_sentence() {
    for reply in [
        None,
        Some("这不是 JSON".to_string()),
        Some(r#"{"display":"你好"}"#.to_string()),
        Some(r#"{"speech":"你好"}"#.to_string()),
        Some(r#"{"display":"","speech":"你好"}"#.to_string()),
    ] {
        let cleaner = SentenceCleaner::new(
            Box::new(StubCleaner {
                reply: reply.clone(),
            }),
            CLEAN_TIMEOUT_MS,
        );
        let cleaned = cleaner.clean("你好😄（挥手）").await;
        assert_eq!(
            cleaned,
            CleanedSentence::raw("你好😄（挥手）"),
            "reply={reply:?}"
        );
    }
}

/// 记录 system 提示的替身（区分动作开关走哪一份提示词）。
#[derive(Debug)]
struct SystemRecordingCleaner {
    reply: Option<String>,
    systems: Arc<std::sync::Mutex<Vec<String>>>,
}

impl PerformanceClient for SystemRecordingCleaner {
    fn enabled(&self) -> bool {
        true
    }
    fn kind(&self) -> &'static str {
        "injected"
    }
    fn has_api_key(&self) -> bool {
        false
    }
    fn request<'a>(
        &'a self,
        system: &'a str,
        _user: &'a str,
        _timeout_ms: u64,
    ) -> PerformanceFuture<'a, Option<PerformanceReply>> {
        self.systems
            .lock()
            .expect("poison")
            .push(system.to_string());
        let reply = self.reply.clone().map(|raw| PerformanceReply {
            raw,
            structured: false,
        });
        Box::pin(async move { reply })
    }
}

/// T11 / T17-1：动作门**关**着 → 旧提示词；`cues` 字段被忽略（`cue_ids = None`）。
#[tokio::test]
async fn cleaner_off_keeps_two_field_json() {
    let systems = Arc::new(std::sync::Mutex::new(Vec::new()));
    let cleaner = SentenceCleaner::new(
        Box::new(SystemRecordingCleaner {
            reply: Some(r#"{"display":"你好","speech":"你好"}"#.to_string()),
            systems: Arc::clone(&systems),
        }),
        CLEAN_TIMEOUT_MS,
    );
    assert!(!cleaner.motion());
    let cleaned = cleaner.clean("你好").await;
    assert_eq!(cleaned.display, "你好");
    assert_eq!(cleaned.speech, "你好");
    assert_eq!(cleaned.cue_ids, None, "关着时不解析 cues");
    let sys = systems.lock().expect("poison").clone();
    assert_eq!(sys.as_slice(), [CLEAN_SYSTEM_PROMPT], "关着时用旧提示词");
}

/// T11：动作门**开**着 → 用带 `cues` 的新提示词（能力集写进提示词）。
#[tokio::test]
async fn cleaner_on_asks_for_cues_in_the_prompt() {
    let systems = Arc::new(std::sync::Mutex::new(Vec::new()));
    let cleaner = SentenceCleaner::new(
        Box::new(SystemRecordingCleaner {
            reply: Some(r#"{"display":"你好","speech":"你好","cues":[]}"#.to_string()),
            systems: Arc::clone(&systems),
        }),
        CLEAN_TIMEOUT_MS,
    )
    .with_motion(
        vec!["nod".to_string(), "smile".to_string()],
        MotionBias::new("测试子句：nod, smile".to_string(), false),
    );
    assert!(cleaner.motion());
    let cleaned = cleaner.clean("你好").await;
    assert_eq!(cleaned.cue_ids, Some(Vec::new()), "空数组 = 这一句不动");
    let sys = systems.lock().expect("poison").clone();
    assert_eq!(sys.len(), 1);
    assert!(sys[0].contains("cues"), "开着时提示词必须提 cues");
    assert!(
        sys[0].contains("nod") && sys[0].contains("smile"),
        "能力集进提示词"
    );
}

/// T12 / T17-6：`cues` 里未知 id 被丢掉（只收能力集里的 preset_id）。
#[tokio::test]
async fn unknown_preset_id_is_dropped() {
    let allowed = vec!["nod".to_string(), "smile".to_string(), "none".to_string()];
    let cleaner = SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: Some(
                r#"{"display":"你好","speech":"你好","cues":["nod","不存在","smile"]}"#.to_string(),
            ),
        }),
        CLEAN_TIMEOUT_MS,
    )
    .with_motion(allowed, restrained_bias());
    let cleaned = cleaner.clean("你好").await;
    assert_eq!(
        cleaned.cue_ids,
        Some(vec!["nod".to_string(), "smile".to_string()]),
        "未知 id 必须丢，合法 id 保留"
    );
}

/// T12 / T17-7：结果 id 全在能力集里，且**没有嘴部参数**（口型只归 TTS）。
#[tokio::test]
async fn cue_preset_ids_are_not_mouth() {
    let allowed = vec!["nod".to_string(), "smile".to_string(), "none".to_string()];
    let cleaner = SentenceCleaner::new(
        Box::new(StubCleaner {
            reply: Some(
                r#"{"display":"你好","speech":"你好","cues":["nod","ParamMouthOpenY","smile"]}"#
                    .to_string(),
            ),
        }),
        CLEAN_TIMEOUT_MS,
    )
    .with_motion(allowed.clone(), restrained_bias());
    let cleaned = cleaner.clean("你好").await;
    let cue_ids = cleaned.cue_ids.expect("动作门开着必须解析 cues");
    for id in &cue_ids {
        assert!(allowed.contains(id), "id 必须在能力集里：{id}");
        assert!(!id.starts_with("Param"), "cue 不得写参数：{id}");
    }
    // 组装出的 legacy cue 也不含嘴部参数。
    for cue in cues_from_ids(&cue_ids, 1) {
        assert!(!cue.preset_id.starts_with("Param"));
    }
}

/// 没接二路（disabled）→ 引擎走既有路径：`enabled() == false`。
#[test]
fn disabled_cleaner_is_off_and_from_llm_respects_the_flag() {
    assert!(!SentenceCleaner::disabled().enabled());
    let mut llm = LlmConfig::new("https://api.deepseek.com/v1", "deepseek-flash");
    assert!(
        !SentenceCleaner::from_llm(&llm).enabled(),
        "clean_tts=false"
    );
    llm.clean_tts = true;
    let on = SentenceCleaner::from_llm(&llm);
    assert!(on.enabled());
    assert_eq!(on.model(), "deepseek-flash");
    // 端点非法也不失败：转 disabled（回原文）。
    let mut bad = LlmConfig::new("", "deepseek-flash");
    bad.clean_tts = true;
    assert!(!SentenceCleaner::from_llm(&bad).enabled());
}

// ---- 表演倾向（2026-10-13 执行单 T3）-----------------------------------------

/// 克制档投影（空数组**不**回退；既有测试共用）。
fn restrained_bias() -> MotionBias {
    MotionBias::new("测试子句（克制）".to_string(), false)
}

/// `bias=None` = 既有「克制」文案逐字不变（`cues：` 那一句结尾是能力集 + 句号）。
#[test]
fn motion_system_prompt_without_bias_keeps_the_legacy_clause() {
    let allowed = vec!["nod".to_string(), "smile".to_string()];
    let prompt = motion_system_prompt(&allowed, None);
    assert!(
        prompt.contains(
            "cues：只根据用户给你的这一句原文，从下面这组 id 里选 0~2 个动作预设，\
不要根据别的句子或上下文，选不出就给空数组 []：nod, smile。"
        ),
        "既有克制文案必须逐字保留：{prompt}"
    );
}

/// `bias=Some` 只把 `cues：` 那一句换成 cue_clause；display / speech 两句保持原样。
#[test]
fn motion_system_prompt_replaces_only_the_cues_clause() {
    let allowed = vec!["nod".to_string(), "smile".to_string()];
    let none = motion_system_prompt(&allowed, None);
    let bias = motion_system_prompt(
        &allowed,
        Some(&MotionBias::new("三档子句：nod, smile".to_string(), false)),
    );
    for (label, prompt) in [("none", &none), ("bias", &bias)] {
        assert!(prompt.starts_with("你是台词清洗器。"), "{label}：开头不变");
        assert!(
            prompt.contains("display：这一句原文里要显示给用户看的连续片段"),
            "{label}：display 句不变"
        );
        assert!(
            prompt.contains("speech：拿去语音合成的文本"),
            "{label}：speech 句不变"
        );
        assert!(
            prompt.ends_with("不要新增、删改或翻译台词内容，不要输出 Markdown 代码块。"),
            "{label}：结尾不变"
        );
    }
    assert!(bias.contains("cues：三档子句：nod, smile。"), "{bias}");
    assert!(!bias.contains("只根据用户给你的这一句原文"), "{bias}");
}

/// `empty_cues_fall_back` 转给 bias 的 `fallback_on_empty`（只有主动档 true）。
#[test]
fn motion_bias_empty_cues_fall_back_follows_the_bias() {
    let cleaner = |fallback: bool| {
        SentenceCleaner::new(Box::new(StubCleaner { reply: None }), CLEAN_TIMEOUT_MS).with_motion(
            vec!["nod".to_string()],
            MotionBias::new("子句".to_string(), fallback),
        )
    };
    assert!(
        !cleaner(false).empty_cues_fall_back(),
        "克制 / 平常：空数组不回退"
    );
    assert!(cleaner(true).empty_cues_fall_back(), "主动：空数组回退");
}

// ---- 长数字段只改送 TTS 的那份（T1，2026-10-13 执行单）------------------------

/// 31 位的连续数字在送 TTS 的稿里被整段换成占位句；**原文一字不动**
/// （上屏用的是另一份：引擎把原文交给 display，本函数只作用在 TTS 那一份）。
#[test]
fn long_digit_run_of_thirty_one_is_replaced_in_tts_text_only() {
    let digits = "7182818284590452353602874713527";
    assert_eq!(digits.len(), 31);
    let sentence = format!("圆周率从这里开始 {digits} 后面还有。");
    let speech = speech_for_tts(&sentence);
    assert!(
        !speech.contains(digits),
        "送 TTS 的那份不得再含这 31 位：{speech}"
    );
    assert!(
        speech.contains(LONG_DIGIT_PLACEHOLDER),
        "必须换成占位句：{speech}"
    );
    assert_eq!(
        speech,
        format!("圆周率从这里开始 {LONG_DIGIT_PLACEHOLDER} 后面还有。")
    );
    // display 侧拿的是**原文引用**，不经本函数——这里如实断言原文没被改。
    assert!(sentence.contains(digits), "上屏那份必须仍是原文");
}

/// 边界恰好是 17：16 位不动、17 位换；两段之间夹着别的字符就各算一段。
#[test]
fn long_digit_boundary_is_seventeen_and_segments_split_on_non_digits() {
    assert_eq!(speech_for_tts("1234567890123456"), "1234567890123456");
    assert_eq!(speech_for_tts("12345678901234567"), LONG_DIGIT_PLACEHOLDER);
    // 中文 / 空白 / 换行都算「别的字符」，把数字切回短段（不动）。
    assert_eq!(
        speech_for_tts("编号1234567890123456，期12345678901234567"),
        "编号1234567890123456，期一长串数字"
    );
    // 多字节字符不被劈开；emoji 原样保留（本函数不清洗任何非数字内容）。
    assert_eq!(
        speech_for_tts("甲🙂12345678901234567890乙"),
        "甲🙂一长串数字乙"
    );
    // 全角数字**不算**（口径只认 0-9）。
    assert_eq!(
        speech_for_tts("１２３４５６７８９０１２３４５６７８"),
        "１２３４５６７８９０１２３４５６７８"
    );
}

/// 占位句本身与纯数字串的幂等：改过一遍再改不会二次膨胀。
#[test]
fn long_digit_masking_is_idempotent() {
    let once = speech_for_tts("请记住7182818284590452353602874713527。");
    assert_eq!(once, format!("请记住{LONG_DIGIT_PLACEHOLDER}。"));
    assert_eq!(speech_for_tts(&once), once, "占位句里没有长数字段");
}
