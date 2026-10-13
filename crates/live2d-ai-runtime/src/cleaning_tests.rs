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
    .with_motion(vec!["nod".to_string(), "smile".to_string()]);
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
    .with_motion(allowed);
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
    .with_motion(allowed.clone());
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
