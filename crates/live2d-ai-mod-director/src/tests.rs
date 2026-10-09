//! 导演骨架回归测试（Wave 3 G 轨）。
//!
//! 覆盖四类：① 纯函数推导（命中 / 缺省 / 词表优先 / 空输入 / 边界）；
//! ② 订阅主题与静态 schema；③ state_json 形状与容量上限；
//! ④ **不投递**——action_tx 与 apply_settings 的调用次数必须为 0。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use live2d_ai_mod_system::*;

use crate::decision::{
    EMOTION_PRIORITY, EmotionHint, IntentHint, Lexicon, MAX_EMPHASIS, MAX_PITCH, MAX_SPEED,
    MAX_TEXT_CHARS, MIN_PITCH, MIN_SPEED, clean_text, count_keyword, derive, emphasis_level,
    score_emotions, suggest_tts,
};
use crate::ledger::{DEFAULT_LOG_CAPACITY, LedgerEntry, MAX_LOG_CAPACITY, MIN_LOG_CAPACITY};
use crate::{DESCRIPTOR, DirectorConfig, DirectorFactory, DirectorRuntime, director_settings_spec};

// ---- 测试替身 ----

/// 记录 register_settings / subscribe 调用的测试注册器。
///
/// `pub(crate)`：`tests_staging.rs` 也用（不复制第二份）。
#[derive(Default)]
pub(crate) struct RecordingRegistrar {
    specs: Vec<ModSettingsSpec>,
    topics: Vec<ModEventTopic>,
}

impl ModRegistrar for RecordingRegistrar {
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
        self.specs.push(spec);
        Ok(())
    }

    fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
        self.topics.push(topic);
        Ok(SubscriptionId(self.topics.len() as u64))
    }

    fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
        Ok(())
    }
}

/// 间谍：记录下行通道（动作 / 配置写回）被调用的次数——骨架必须恒为 0。
///
/// `pub(crate)`：`tests_staging.rs` 也读这两个计数（不复制第二份）。
pub(crate) struct Spies {
    pub(crate) action_calls: Arc<AtomicUsize>,
    pub(crate) apply_calls: Arc<AtomicUsize>,
}

pub(crate) fn spy_services() -> (ModServices, Spies) {
    let action_calls = Arc::new(AtomicUsize::new(0));
    let apply_calls = Arc::new(AtomicUsize::new(0));
    let action_counter = Arc::clone(&action_calls);
    let apply_counter = Arc::clone(&apply_calls);
    let services = ModServices::new(
        ModActionSender::new(move |_req| {
            action_counter.fetch_add(1, Ordering::SeqCst);
            // host 的休眠 sender 语义：永不接受。
            false
        }),
        SaySender::new(|_| true),
        ModEventSender::new(|_, _| true),
        ModLogger::new(|_, _| {}),
    )
    .with_apply_settings(ModSettingsApplier::new(move |_patch| {
        apply_counter.fetch_add(1, Ordering::SeqCst);
        false
    }));
    (
        services,
        Spies {
            action_calls,
            apply_calls,
        },
    )
}

fn started(config: serde_json::Value) -> (DirectorRuntime, RecordingRegistrar, Spies) {
    let (services, spies) = spy_services();
    let mut runtime = DirectorRuntime::new(services, DirectorConfig::from_value(&config));
    let mut registrar = RecordingRegistrar::default();
    runtime.start(&mut registrar).expect("start 应成功");
    (runtime, registrar, spies)
}

// ---- ① 纯函数推导 ----

#[test]
fn empty_and_whitespace_are_silent_neutral() {
    for text in ["", "   ", "\t\n "] {
        let decision = derive(text, Lexicon::Builtin);
        assert!(decision.is_silent(), "空输入必须静默: {text:?}");
        assert_eq!(decision.emotion, EmotionHint::Neutral);
        assert_eq!(decision.intent, IntentHint::Silence);
        assert_eq!(decision.suggested_tts.speed, 1.0);
        assert_eq!(decision.suggested_tts.pitch, 1.0);
    }
}

#[test]
fn derive_is_deterministic() {
    let first = derive("你好！今天太开心了！", Lexicon::Builtin);
    let second = derive("你好！今天太开心了！", Lexicon::Builtin);
    assert_eq!(first, second, "同一输入必须恒等输出");
}

#[test]
fn greeting_intent_hits() {
    assert_eq!(
        derive("你好呀", Lexicon::Builtin).intent,
        IntentHint::Greeting
    );
    assert_eq!(
        derive("hello there", Lexicon::Builtin).intent,
        IntentHint::Greeting
    );
}

#[test]
fn question_outranks_greeting() {
    // 「你好」是 Greeting，「？」是 Question——首个匹配者胜，Question 在前。
    assert_eq!(
        derive("你好吗？", Lexicon::Builtin).intent,
        IntentHint::Question
    );
    assert_eq!(
        derive("这是什么", Lexicon::Builtin).intent,
        IntentHint::Question
    );
}

#[test]
fn emotion_keyword_hits_happy() {
    let decision = derive("今天太开心了", Lexicon::Builtin);
    assert_eq!(decision.emotion, EmotionHint::Happy);
    assert_eq!(decision.suggested_tts.speed, 1.08);
    assert_eq!(decision.suggested_tts.pitch, 1.1);
}

#[test]
fn emotion_tie_broken_by_priority() {
    // 开心(1) 与 难过(1) 同分 → 优先级表里 Happy 在前。
    assert_eq!(
        derive("开心又难过", Lexicon::Builtin).emotion,
        EmotionHint::Happy
    );
    // 评分顺序必须与优先级表逐项一致（否则平局裁决是隐式的）。
    let hints: Vec<EmotionHint> = score_emotions("开心又难过", Lexicon::Builtin)
        .iter()
        .map(|(emotion, _)| *emotion)
        .collect();
    assert_eq!(hints, EMOTION_PRIORITY.to_vec());
}

#[test]
fn negation_suppresses_keyword() {
    assert_eq!(count_keyword("不开心", "开心"), 0);
    assert_eq!(count_keyword("开心", "开心"), 1);
    assert_eq!(
        derive("我不开心", Lexicon::Builtin).emotion,
        EmotionHint::Neutral
    );
}

#[test]
fn strict_lexicon_drops_weak_keywords() {
    assert_eq!(
        derive("有点开心", Lexicon::Builtin).emotion,
        EmotionHint::Happy
    );
    assert_eq!(
        derive("有点开心", Lexicon::Strict).emotion,
        EmotionHint::Neutral,
        "strict 只认强关键词"
    );
    // 强关键词在 strict 下仍命中。
    assert_eq!(
        derive("气死我了", Lexicon::Strict).emotion,
        EmotionHint::Angry
    );
}

#[test]
fn ascii_keywords_need_word_boundary() {
    assert_eq!(
        derive("I am happy today", Lexicon::Builtin).emotion,
        EmotionHint::Happy
    );
    assert_eq!(
        derive("I am unhappy today", Lexicon::Builtin).emotion,
        EmotionHint::Neutral,
        "unhappy 的 happy 无词边界，不得命中"
    );
}

#[test]
fn emphasis_raises_pitch_and_caps() {
    assert_eq!(derive("开心", Lexicon::Builtin).suggested_tts.pitch, 1.1);
    assert_eq!(derive("开心！", Lexicon::Builtin).suggested_tts.pitch, 1.15);
    assert_eq!(
        derive("开心！！！！", Lexicon::Builtin).suggested_tts.pitch,
        1.2,
        "强调级封顶"
    );
    assert_eq!(emphasis_level("！！！"), MAX_EMPHASIS);
}

#[test]
fn suggestion_is_clamped_and_emphasis_moves_pitch_only() {
    for (emotion, emphasis) in [
        (EmotionHint::Neutral, 0),
        (EmotionHint::Happy, 2),
        (EmotionHint::Sad, 2),
        (EmotionHint::Angry, 2),
        (EmotionHint::Surprised, 2),
        (EmotionHint::Anxious, 2),
        (EmotionHint::Affectionate, 2),
    ] {
        let suggestion = suggest_tts(emotion, emphasis);
        assert!(
            (MIN_SPEED..=MAX_SPEED).contains(&suggestion.speed),
            "{emotion:?} {suggestion:?}"
        );
        assert!(
            (MIN_PITCH..=MAX_PITCH).contains(&suggestion.pitch),
            "{emotion:?} {suggestion:?}"
        );
    }
    assert_eq!(
        suggest_tts(EmotionHint::Happy, 0).speed,
        suggest_tts(EmotionHint::Happy, 2).speed,
        "强调不抬语速"
    );
    assert!(
        suggest_tts(EmotionHint::Happy, 2).pitch > suggest_tts(EmotionHint::Happy, 0).pitch,
        "强调抬音高"
    );
}

#[test]
fn pure_punctuation_is_neutral_chat_not_silence() {
    let decision = derive("。。。", Lexicon::Builtin);
    assert!(!decision.is_silent(), "纯标点不是空输入");
    assert_eq!(decision.emotion, EmotionHint::Neutral);
    assert_eq!(decision.intent, IntentHint::Chat);
}

#[test]
fn long_text_is_truncated_at_char_boundary() {
    let long = "好".repeat(MAX_TEXT_CHARS + 50);
    assert_eq!(clean_text(&long).chars().count(), MAX_TEXT_CHARS);
    assert!(!derive(&long, Lexicon::Builtin).is_silent());
}

#[test]
fn whitespace_is_trimmed_before_derivation() {
    assert_eq!(
        derive("  你好  ", Lexicon::Builtin).intent,
        IntentHint::Greeting
    );
}

// ---- ② 身份 / 注册面 ----

#[test]
fn descriptor_declares_current_api_version_and_id() {
    assert_eq!(DESCRIPTOR.id, "director");
    assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
}

#[test]
fn start_subscribes_prompt_ended_and_sentence_ready() {
    let (_runtime, registrar, _spies) = started(serde_json::json!({}));
    // P1-3：TurnPrompt（规则决策）+ TurnEnded（结项）+ SentenceReady（按句 cue）。
    assert_eq!(
        registrar.topics,
        vec![
            ModEventTopic::TurnPrompt,
            ModEventTopic::TurnEnded,
            ModEventTopic::SentenceReady,
        ]
    );
    assert_eq!(registrar.specs.len(), 1);
    assert_eq!(registrar.specs[0].mod_id, "director");
    assert!(registrar.specs[0].validate().is_ok());
}

/// **标签唯一真源**（2026-09-20 调研产出）：导演 Select 的每个预设选项的中文名
/// 必须来自 `assets/actions/preset_labels.json`，不得在 Rust 里另写一套。
#[test]
fn preset_select_labels_come_from_the_shared_table() {
    let spec = director_settings_spec();
    let table = crate::preset_label_map();
    let mut checked = 0usize;
    for field in &spec.fields {
        let ModSettingField::Select { options, .. } = field else {
            continue;
        };
        for opt in options {
            if opt.value == crate::PRESET_NONE {
                continue;
            }
            let row = table
                .get(&opt.value)
                .unwrap_or_else(|| panic!("Select 选项 {} 不在标签表里", opt.value));
            assert!(
                opt.label.contains(row.zh.as_str()),
                "{} 的展示名没读表：{:?} vs {:?}",
                opt.value,
                opt.label,
                row.zh
            );
            checked += 1;
        }
    }
    assert!(checked >= crate::PRESET_IDS.len(), "至少要覆盖全部预设 id");
}

/// **这 8 个键仍是解析真源**（`mods.json` 里已写的值一个不丢），但**产品界面
/// 不再渲染它们**：2026-10-08 起 `DirectorPanel.hiddenKeys` 声明这 8 个 key
/// 「产品面不渲染、也不收进高级」。
///
/// 「产品卡片上找不到这些字符串」那条断言在 Flutter 侧（Rust 看不到界面）：
/// `shell/flutter/test/performance_layer_copy_test.dart` 真泵 ModsSection 并断言
/// 8 个 label 一个都不上屏。这里继续钉 **spec 本身**（键集合 + 预设选项表），
/// 那是解析与开发者面的事实。
#[test]
fn settings_spec_has_no_enabled_and_matches_static_spec() {
    let spec = director_settings_spec();
    let keys: Vec<&str> = spec.fields.iter().map(|field| field.key()).collect();
    // 2026-09-15 用户裁决：8 个预设 Select 收成 3 个常用档，
    // log_capacity / emotion_lexicon 退出表单（缺省仍然生效）。
    // 2026-09-19（P1-4）：追加 5 个二路键——**要哪些键才能真开二路**必须能从
    // 表单本身读出来（staging_enabled + base_url + model；密钥变量名/超时可省）。
    assert_eq!(
        keys,
        vec![
            "preset_happy",
            "preset_sad",
            "preset_greeting",
            "staging_enabled",
            "staging_base_url",
            "staging_model",
            "staging_api_key_env",
            "staging_timeout_ms",
        ],
        "3 个常用档 + 5 个二路键"
    );
    assert!(
        !keys.contains(&"enabled"),
        "启停唯一真源是 manifest，schema 里不得有第二个 enabled"
    );
    // 2026-10-07（T8）：`staging_*` **不再是遗留回退旁路**——导演启用后主链就用它
    // （两项都填）或对话模型（两项都空）把原文分段送 TTS。旧口径不得回潮。
    let label_of = |key: &str| -> String {
        spec.fields
            .iter()
            .find(|field| field.key() == key)
            .map(|field| match field {
                ModSettingField::Bool { label, .. }
                | ModSettingField::String { label, .. }
                | ModSettingField::Number { label, .. }
                | ModSettingField::Select { label, .. } => label.clone(),
            })
            .unwrap_or_else(|| panic!("schema 里没有 {key}"))
    };
    for field in &spec.fields {
        if !field.key().starts_with("staging_") {
            continue;
        }
        let label = label_of(field.key());
        assert!(
            !label.contains("【遗留】") && !label.contains("日常用表演层"),
            "staging 字段 {field:?} 的 label 仍是旧口径：{label}"
        );
    }
    // 口径表：「两项都空 = 复用对话模型」——地址与模型名必须把这件事写出来。
    for key in ["staging_base_url", "staging_model"] {
        let label = label_of(key);
        assert!(
            label.contains("留空") && label.contains("对话模型"),
            "{key} 的 label 必须写明「留空则用对话模型」：{label}"
        );
    }
    assert!(
        label_of("staging_api_key_env").contains("对话模型"),
        "密钥变量名也要说明留空时用对话模型的密钥"
    );
    // 每个预设字段都要带**真缺省**（不是第一个选项 none），否则表单会用
    // none 覆盖掉内置映射表（「点一次保存就没动作了」）。
    let expected = crate::presets::PresetTable::default();
    let defaults: Vec<&str> = spec
        .fields
        .iter()
        .filter_map(|f| match f {
            ModSettingField::Select {
                default: Some(d), ..
            } => Some(d.as_str()),
            ModSettingField::Select { default: None, .. } => {
                panic!("预设字段必须是带缺省的 Select: {f:?}")
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        defaults,
        vec![
            expected.happy.as_str(),
            expected.sad.as_str(),
            expected.greeting.as_str()
        ],
        "表单缺省必须等于内置映射表的缺省"
    );
    // 二路总闸的 Bool 字段缺省必须是 false（出厂即「仅规则」）。
    let staging_bool = spec
        .fields
        .iter()
        .find_map(|f| match f {
            ModSettingField::Bool { key, default, .. } if key == "staging_enabled" => {
                Some(*default)
            }
            _ => None,
        })
        .expect("必须有 staging_enabled");
    assert!(!staging_bool, "二路必须缺省关");
    // 静态 schema 与 start 注册的必须是同一份（否则前端表单与运行时分叉）。
    let factory_spec = DirectorFactory
        .settings_spec()
        .expect("骨架必须提供静态 schema");
    assert_eq!(factory_spec, spec);
}

#[test]
fn factory_creates_runtime_with_config() {
    let (services, _spies) = spy_services();
    let mut runtime = DirectorFactory
        .create(
            services,
            serde_json::json!({"log_capacity": 7, "emotion_lexicon": "strict"}),
        )
        .expect("create 应成功");
    let mut registrar = RecordingRegistrar::default();
    runtime.start(&mut registrar).expect("start 应成功");
    runtime
        .on_event(ModEventTopic::TurnPrompt, "有点开心")
        .expect("on_event 不应失败");
    // 经 trait object 的公开面验证配置确实生效（不依赖具体类型的方法）。
    let state = runtime.state_json().expect("应能取状态面");
    assert_eq!(state["log_capacity"].as_u64(), Some(7));
    assert_eq!(state["emotion_lexicon"], serde_json::json!("strict"));
    // 非空正文仍会产生一条决策（Neutral 也是决策），但 strict 词表不吃弱关键词。
    assert_eq!(
        state["recent_decisions"][0]["emotion"],
        serde_json::json!("neutral"),
        "strict 词表不吃弱关键词"
    );
}

#[test]
fn shutdown_resets_registered() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    assert!(runtime.is_registered());
    runtime.shutdown().expect("shutdown 应成功");
    assert!(!runtime.is_registered());
}

// ---- ③ state_json 形状与容量 ----

#[test]
fn state_json_shape_and_preset_delivery() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    runtime
        .on_event(ModEventTopic::TurnPrompt, "你好！今天太开心了！")
        .unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();

    let state = runtime.state_json().expect("骨架必须有状态面");
    // **2026-09-15 起**：「零投递」的定义变了——本 Mod 仍然不调用 action_tx /
    // apply_settings（零调用），但它**产出**一条动作预设：驱动舞台走按句
    // action_cue（ModServices.cues → WS）；只读状态面的 latest.preset_id
    // **仅供面板展示**（前端拉取驱动已退役，D12）。
    // 「你好！今天太开心了！」= Greeting + Happy → greeting 优先 = nod。
    assert_eq!(state["channel"], serde_json::json!("preset"));
    assert_eq!(state["delivered"], serde_json::json!(true));
    assert_eq!(state["presets_chosen"].as_u64(), Some(1));
    assert_eq!(state["latest"]["preset_id"], serde_json::json!("nod"));
    assert_eq!(state["turns_seen"].as_u64(), Some(1));
    assert_eq!(state["turns_ended"].as_u64(), Some(1));
    assert_eq!(state["decisions"].as_u64(), Some(1));
    assert_eq!(state["silent"].as_u64(), Some(0));
    assert_eq!(state["errors"].as_u64(), Some(0));
    assert_eq!(state["log_capacity"].as_u64(), Some(20));
    assert_eq!(state["emotion_lexicon"], serde_json::json!("builtin"));

    let recent = state["recent_decisions"].as_array().expect("应为数组");
    assert_eq!(recent.len(), 1);
    assert_eq!(recent[0]["turn"], serde_json::json!("1"));
    assert_eq!(recent[0]["emotion"], serde_json::json!("happy"));
    assert_eq!(recent[0]["intent"], serde_json::json!("greeting"));
    assert_eq!(recent[0]["suggested_tts"]["speed"].as_f64(), Some(1.08));
    assert_eq!(recent[0]["suggested_tts"]["pitch"].as_f64(), Some(1.2));
    assert_eq!(recent[0]["closed"], serde_json::json!(true));
    assert_eq!(recent[0]["delivered"], serde_json::json!(true));
    assert_eq!(recent[0]["preset_id"], serde_json::json!("nod"));
    // 映射表随状态面下发，面板据此显示「现在会映射到哪条」。
    assert_eq!(state["presets"]["greeting"], serde_json::json!("nod"));
    assert_eq!(state["presets"]["anxious"], serde_json::json!("none"));
}

#[test]
fn suggested_tts_exposes_only_speed_and_pitch() {
    let decision = derive("气死我了！", Lexicon::Builtin);
    let entry = LedgerEntry {
        preset_id: None,
        seq: 1,
        turn: None,
        emotion: decision.emotion,
        intent: decision.intent,
        speed: decision.suggested_tts.speed,
        pitch: decision.suggested_tts.pitch,
        closed: false,
    };
    let value = entry.to_json();
    let mut keys: Vec<String> = value["suggested_tts"]
        .as_object()
        .expect("suggested_tts 应为对象")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(keys, vec!["pitch".to_string(), "speed".to_string()]);
}

#[test]
fn state_json_capacity_keeps_last_n() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({"log_capacity": 3}));
    for turn in 1..=5 {
        runtime.on_event(ModEventTopic::TurnPrompt, "开心").unwrap();
        runtime
            .on_event(ModEventTopic::TurnEnded, &turn.to_string())
            .unwrap();
    }
    assert_eq!(runtime.ledger().turns_seen(), 5);
    assert_eq!(runtime.ledger().decisions(), 5, "计数不受容量影响");
    assert_eq!(runtime.ledger().recent().len(), 3);

    let state = runtime.state_json().unwrap();
    let recent = state["recent_decisions"].as_array().unwrap();
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0]["seq"].as_u64(), Some(3));
    assert_eq!(recent[2]["seq"].as_u64(), Some(5));
    assert_eq!(recent[2]["turn"], serde_json::json!("5"));
}

#[test]
fn state_json_latest_tracks_the_newest_decision() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    assert!(
        runtime.state_json().unwrap()["latest"].is_null(),
        "空账本 latest 必须是 null（不是空对象）"
    );

    runtime
        .on_event(ModEventTopic::TurnPrompt, "你好！今天太开心了！")
        .unwrap();
    // 还没收到 TurnEnded：latest 已经在飞（closed=false、turn=null）。
    let open = runtime.state_json().unwrap();
    assert_eq!(open["latest"]["emotion"], serde_json::json!("happy"));
    assert_eq!(open["latest"]["closed"], serde_json::json!(false));
    assert!(open["latest"]["turn"].is_null(), "turn id 结项时才写");

    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();
    runtime.on_event(ModEventTopic::TurnPrompt, "再见").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "2").unwrap();

    let state = runtime.state_json().unwrap();
    let recent = state["recent_decisions"].as_array().expect("应为数组");
    assert_eq!(
        state["latest"], recent[1],
        "latest 必须与 recent_decisions 的最后一条**逐字段相同**"
    );
    assert_eq!(state["latest"]["seq"].as_u64(), Some(2));
    assert_eq!(state["latest"]["turn"], serde_json::json!("2"));
    assert_eq!(state["latest"]["intent"], serde_json::json!("farewell"));
    assert_eq!(state["latest"]["closed"], serde_json::json!(true));
    assert_eq!(state["latest"]["delivered"], serde_json::json!(false));
    assert_eq!(
        state["latest"]["suggested_tts"]["speed"].as_f64(),
        Some(1.0)
    );
}

#[test]
fn recent_decisions_shape_is_unchanged_with_latest_added() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    runtime
        .on_event(ModEventTopic::TurnPrompt, "你好！今天太开心了！")
        .unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();

    let state = runtime.state_json().unwrap();
    assert!(
        state.get("recent_decisions").is_some(),
        "recent_decisions 必须保留（向后兼容）"
    );
    let mut keys: Vec<String> = state["recent_decisions"][0]
        .as_object()
        .expect("单条应为对象")
        .keys()
        .cloned()
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "closed".to_string(),
            "delivered".to_string(),
            "emotion".to_string(),
            "intent".to_string(),
            "preset_id".to_string(),
            "seq".to_string(),
            "suggested_tts".to_string(),
            "turn".to_string(),
        ],
        "2026-09-15 只新增 preset_id 一个键（其余形状一行未改）"
    );
}

#[test]
fn clear_command_empties_ledger_and_returns_before_counts() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({"log_capacity": 5}));
    for turn in 1..=3 {
        runtime.on_event(ModEventTopic::TurnPrompt, "开心").unwrap();
        runtime
            .on_event(ModEventTopic::TurnEnded, &turn.to_string())
            .unwrap();
    }
    runtime.on_event(ModEventTopic::TurnPrompt, "   ").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "4").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "4").unwrap();

    let result = runtime
        .command("clear", &serde_json::json!({}))
        .expect("clear 应成功");
    // 回执是**清空前**的计数（用户想知道自己刚清掉了什么）。
    assert_eq!(result["cleared"].as_u64(), Some(3), "清掉 3 条决策条目");
    assert_eq!(result["counts"]["decisions"].as_u64(), Some(3));
    assert_eq!(result["counts"]["turns_seen"].as_u64(), Some(4));
    assert_eq!(result["counts"]["turns_ended"].as_u64(), Some(4));
    assert_eq!(result["counts"]["silent"].as_u64(), Some(1));
    assert_eq!(result["counts"]["errors"].as_u64(), Some(1));

    let state = runtime.state_json().unwrap();
    assert_eq!(state["decisions"].as_u64(), Some(0));
    assert_eq!(state["turns_seen"].as_u64(), Some(0));
    assert_eq!(state["turns_ended"].as_u64(), Some(0));
    assert_eq!(state["errors"].as_u64(), Some(0));
    assert!(state["recent_decisions"].as_array().unwrap().is_empty());
    assert!(state["latest"].is_null(), "清空后 latest 回到 null");
    assert_eq!(state["channel"], serde_json::json!("preset"));
    // 空账本 = 没有「最近一轮」→ delivered=false（而不是「历史投递过」）。
    assert_eq!(state["delivered"], serde_json::json!(false));
    assert_eq!(state["presets_chosen"].as_u64(), Some(0));

    // 清空**不是**把 Mod 关掉：还能继续记账。
    runtime.on_event(ModEventTopic::TurnPrompt, "你好").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "5").unwrap();
    let state = runtime.state_json().unwrap();
    assert_eq!(state["decisions"].as_u64(), Some(1));
    assert_eq!(state["latest"]["turn"], serde_json::json!("5"));
    assert_eq!(state["errors"].as_u64(), Some(0));
}

#[test]
fn unknown_command_is_unsupported_and_leaves_state_untouched() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    runtime.on_event(ModEventTopic::TurnPrompt, "开心").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();
    let before = runtime.state_json().unwrap();

    let error = runtime
        .command("bogus", &serde_json::json!({"x": 1}))
        .expect_err("不认识的命令必须报 UnsupportedCommand");
    assert_eq!(
        error,
        ModError::UnsupportedCommand {
            command: "bogus".to_string()
        }
    );
    assert_eq!(
        runtime.state_json().unwrap(),
        before,
        "未知命令不得改状态（含账本与计数）"
    );
}

#[test]
fn log_capacity_is_clamped_and_bad_values_fall_back() {
    let config = |value: serde_json::Value| DirectorConfig::from_value(&value);
    assert_eq!(
        config(serde_json::json!({"log_capacity": 0})).log_capacity,
        MIN_LOG_CAPACITY
    );
    assert_eq!(
        config(serde_json::json!({"log_capacity": 10_000})).log_capacity,
        MAX_LOG_CAPACITY
    );
    assert_eq!(
        config(serde_json::json!({"log_capacity": "x"})).log_capacity,
        DEFAULT_LOG_CAPACITY
    );
    assert_eq!(
        config(serde_json::json!({})).emotion_lexicon,
        Lexicon::Builtin
    );
    assert_eq!(
        config(serde_json::json!({"emotion_lexicon": "bogus"})).emotion_lexicon,
        Lexicon::Builtin
    );
    // P1-4：二路端点的四个字符串 / 数值键——trim、坏类型回落空串（绝不失败）。
    let staging = config(serde_json::json!({
        "staging_enabled": true,
        "staging_base_url": "  http://127.0.0.1:11434/v1  ",
        "staging_model": " qwen ",
        "staging_api_key_env": " DEEPSEEK_API_KEY ",
        "staging_timeout_ms": 999_999,
    }));
    assert!(staging.staging_enabled);
    assert_eq!(staging.staging_base_url, "http://127.0.0.1:11434/v1");
    assert_eq!(staging.staging_model, "qwen");
    assert_eq!(staging.staging_api_key_env, "DEEPSEEK_API_KEY");
    assert_eq!(staging.staging_timeout_ms, 5_000, "超时必须钳到上限");
    let bad = config(serde_json::json!({"staging_base_url": 5, "staging_model": []}));
    assert_eq!(bad.staging_base_url, "");
    assert_eq!(bad.staging_model, "");
    assert!(!bad.staging_enabled, "缺 staging_enabled 必须回落 false");
}

// ---- ④ 轮末结项 + 不投递 ----

#[test]
fn turn_ended_closes_the_decision_and_writes_turn_id() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    runtime.on_event(ModEventTopic::TurnPrompt, "抱抱").unwrap();
    assert!(!runtime.ledger().recent()[0].closed, "未结项");
    runtime.on_event(ModEventTopic::TurnEnded, "9").unwrap();

    let entry = &runtime.ledger().recent()[0];
    assert!(entry.closed, "TurnEnded 必须结项");
    assert_eq!(entry.turn.as_deref(), Some("9"));
    assert_eq!(runtime.ledger().turns_ended(), 1);
    assert_eq!(runtime.ledger().errors(), 0);
}

#[test]
fn orphan_turn_ended_counts_error_and_does_not_panic() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();
    assert_eq!(runtime.ledger().errors(), 2);
    assert_eq!(runtime.ledger().turns_ended(), 0);
    assert_eq!(runtime.state_json().unwrap()["errors"].as_u64(), Some(2));
}

#[test]
fn blank_prompt_is_silent_not_a_decision() {
    let (mut runtime, _registrar, _spies) = started(serde_json::json!({}));
    runtime.on_event(ModEventTopic::TurnPrompt, "   ").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();
    assert_eq!(runtime.ledger().silent(), 1);
    assert_eq!(runtime.ledger().decisions(), 0);
    assert!(runtime.ledger().recent().is_empty());
    // 静默轮**不是**错误：TurnEnded 仍正常结项。
    assert_eq!(runtime.ledger().turns_ended(), 1);
    assert_eq!(runtime.ledger().errors(), 0);
}

#[test]
fn action_tx_and_apply_settings_are_never_called() {
    let (mut runtime, _registrar, spies) = started(serde_json::json!({}));
    for turn in 1..=3 {
        runtime
            .on_event(ModEventTopic::TurnPrompt, "你好！今天太开心了！")
            .unwrap();
        runtime
            .on_event(ModEventTopic::TurnEnded, &turn.to_string())
            .unwrap();
    }
    runtime.on_event(ModEventTopic::TurnPrompt, "").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "4").unwrap();
    // 产品级加强波次新增的命令通道也走一遍：认识与不认识两条路径都零投递。
    runtime
        .command("clear", &serde_json::json!({}))
        .expect("clear 应成功");
    assert!(runtime.command("bogus", &serde_json::json!({})).is_err());
    runtime.shutdown().unwrap();

    assert_eq!(
        spies.action_calls.load(Ordering::SeqCst),
        0,
        "骨架不得投递任何动作（action_tx 恒休眠）"
    );
    assert_eq!(
        spies.apply_calls.load(Ordering::SeqCst),
        0,
        "骨架不得写任何配置（含 TTS 建议参数）"
    );
    let state = runtime.state_json().unwrap();
    // host 下行通道仍然**零调用**（上面两条断言）；预设走的是状态面 + 前端。
    assert_eq!(state["channel"], serde_json::json!("preset"));
    // 本测试最后调过 clear：账本已空 → 没有「最近一轮」→ delivered=false。
    assert_eq!(state["delivered"], serde_json::json!(false));
}

// 零投递加强（产品级加强波次）：命令路径单独再钉一遍。
//
// 与上一条的分工：上一条覆盖「事件 + 命令混合」的总账；这一条只盯命令通道，
// 把每条命令分支（clear 成功 / clear 带多余 args / 未知命令报错）都走一遍。
// 将来往 command 里加新分支只要投递了，这两条断言立刻红。
#[test]
fn command_paths_never_touch_action_or_settings() {
    let (mut runtime, _registrar, spies) = started(serde_json::json!({}));
    runtime.on_event(ModEventTopic::TurnPrompt, "抱抱").unwrap();
    runtime.on_event(ModEventTopic::TurnEnded, "1").unwrap();

    runtime
        .command("clear", &serde_json::json!({}))
        .expect("clear 应成功");
    assert!(
        runtime
            .command("clear", &serde_json::json!({"unexpected": [1, 2]}))
            .is_ok(),
        "clear 忽略 args，不得因多余参数失败"
    );
    assert!(runtime.command("bogus", &serde_json::json!({})).is_err());
    let _ = runtime.state_json();

    runtime.shutdown().unwrap();
    assert_eq!(
        spies.action_calls.load(Ordering::SeqCst),
        0,
        "命令通道不得投递动作"
    );
    assert_eq!(
        spies.apply_calls.load(Ordering::SeqCst),
        0,
        "命令通道不得写配置（含 TTS 建议参数）"
    );
}

#[test]
fn unsubscribed_events_are_ignored_without_side_effects() {
    let (mut runtime, _registrar, spies) = started(serde_json::json!({}));
    runtime
        .on_event(ModEventTopic::TextDelta, "随便一句话")
        .unwrap();
    runtime.on_event(ModEventTopic::VoiceStarted, "1").unwrap();
    assert_eq!(runtime.ledger().turns_seen(), 0);
    assert_eq!(runtime.ledger().decisions(), 0);
    assert_eq!(spies.action_calls.load(Ordering::SeqCst), 0);
    assert_eq!(spies.apply_calls.load(Ordering::SeqCst), 0);
}
