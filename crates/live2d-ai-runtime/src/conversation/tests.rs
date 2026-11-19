//! `conversation` 模块内联单测（自 `mod.rs` 机械移出）。

use crate::config::{LlmConfig, TtsConfig};

use super::*;
use crate::OpenAiClient;

// ------------------------------------------------------------ 配置/历史

#[test]
fn default_config_matches_documented_values() {
    let config = ConversationConfig::default();
    assert_eq!(config.system_prompt, "");
    assert_eq!(config.max_history_pairs, 0);
    // 2026-09-10：48 → 200（「一句一单元」契约，见
    // `SentenceAssembler::DEFAULT_MAX_CHARS` 的说明）。
    // 本测试锁的是「默认值 == 文档值」，所以跟着文档一起改，
    // 而不是放宽断言。
    assert_eq!(
        config.sentence_max_chars,
        crate::dialogue::SentenceAssembler::DEFAULT_MAX_CHARS
    );
    assert_eq!(config.sentence_max_chars, 200);
    assert_eq!(
        config.tts_queue_capacity,
        ConversationConfig::DEFAULT_TTS_QUEUE_CAPACITY
    );
    assert_eq!(
        config.audio_chunk_samples,
        ConversationConfig::DEFAULT_AUDIO_CHUNK_SAMPLES
    );
    assert_eq!(config.queue_push_timeout, None);
    // ConversationConfig::new 只覆盖 system_prompt。
    assert_eq!(ConversationConfig::new("你好").system_prompt, "你好");
    assert_eq!(
        ConversationConfig::new("你好").max_history_pairs,
        config.max_history_pairs
    );
}

#[test]
fn build_messages_includes_system_history_and_current_input() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let mut engine = ConversationEngine::new(
        client,
        ConversationConfig {
            system_prompt: "人设".into(),
            max_history_pairs: 8,
            ..ConversationConfig::default()
        },
    );
    engine.commit_completed_turn("早", "早呀");
    let messages = engine.build_messages("吃饭了吗");
    assert_eq!(
        messages,
        vec![
            crate::llm::ChatMessage::system("人设"),
            crate::llm::ChatMessage::user("早"),
            crate::llm::ChatMessage::assistant("早呀"),
            crate::llm::ChatMessage::user("吃饭了吗"),
        ]
    );
    assert_eq!(engine.history_len(), 1);
}

/// **空 system 仍然不上 wire**（wire 层不变量，2026-11-16 起仍成立）。
///
/// 注意它与 [`factory_sentence_replaces_the_empty_default`] 不是同一件事：
/// 出厂的「没写人设」现在由 [`ConversationConfig::new`] 换成出厂系统句；
/// 而**真的**构造出一个空 system 的配置（`Default` / 显式空串字段）时，
/// 请求体里依然不该出现一条空 system 消息。
#[test]
fn empty_system_prompt_is_omitted_from_wire_messages() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(
        client,
        ConversationConfig {
            system_prompt: String::new(),
            ..ConversationConfig::default()
        },
    );
    let messages = engine.build_messages("hi");
    assert_eq!(messages, vec![crate::llm::ChatMessage::user("hi")]);
}

/// **出厂不设人设**（2026-11-19 0.7.0 收口）：`ConversationConfig::new("")`
/// 不产出任何 system 消息（2026-11-16 M2 曾塞一句固定陪伴句，该裁决已撤）；
/// 用户存过的**非空**句子原样使用。
#[test]
fn empty_system_prompt_means_no_system_message() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(client, ConversationConfig::new(""));
    assert_eq!(
        engine.build_messages("hi"),
        vec![crate::llm::ChatMessage::user("hi")],
        "出厂不设人设：这一轮只有 user，没有 system"
    );

    // 用户存过的非空句原样使用（不拼接、不覆盖）。
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(client, ConversationConfig::new("你是猫娘。"));
    assert_eq!(engine.effective_system_prompt(), "你是猫娘。");
}

/// **弹幕解说**（2026-10-14 执行单 §3）：只有 `弹幕消息：` 开头的本轮才多
/// 带一句系统说明；正文逐字不改（也是**历史里那一句**，不再加用户名/括号）。
///
/// 三条判据：① 出厂空人设 → 这一轮**只有**这一句 system；② 已生效的人设 →
/// 解说接在**它后面**而不是取代它；③ 正文中段出现同样字样不算弹幕。
#[test]
fn danmaku_preface_only_for_prefixed_turn_and_keeps_user_content() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(client, ConversationConfig::new(""));
    // 2026-11-19（0.7.0）：出厂不设人设 → 这一轮只有弹幕解说这一句 system。
    assert_eq!(
        engine.build_messages("弹幕消息：你好"),
        vec![
            crate::llm::ChatMessage::system(super::engine::DANMAKU_PREFACE),
            crate::llm::ChatMessage::user("弹幕消息：你好"),
        ],
        "出厂空人设 + 弹幕解说，正文原样"
    );
    // 显式空 system 配置：同一语义。
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(
        client,
        ConversationConfig {
            system_prompt: String::new(),
            ..ConversationConfig::default()
        },
    );
    assert_eq!(
        engine.build_messages("弹幕消息：你好"),
        vec![
            crate::llm::ChatMessage::system(super::engine::DANMAKU_PREFACE),
            crate::llm::ChatMessage::user("弹幕消息：你好"),
        ],
        "空 system：这一轮只有弹幕解说这一句 system，正文原样"
    );

    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(
        client,
        ConversationConfig {
            system_prompt: "人设".into(),
            ..ConversationConfig::default()
        },
    );
    let messages = engine.build_messages("弹幕消息：你好");
    assert_eq!(
        messages[0],
        crate::llm::ChatMessage::system(format!("人设\n{}", super::engine::DANMAKU_PREFACE)),
        "有已生效人设时：解说接在它后面，不覆盖"
    );
    assert_eq!(
        messages.last(),
        Some(&crate::llm::ChatMessage::user("弹幕消息：你好"))
    );

    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(
        client,
        ConversationConfig {
            system_prompt: String::new(),
            ..ConversationConfig::default()
        },
    );
    assert_eq!(
        engine.build_messages("你好，弹幕消息：不是开头"),
        vec![crate::llm::ChatMessage::user("你好，弹幕消息：不是开头")],
        "只看开头：中段出现同样字样不加解说"
    );
}

#[test]
fn zero_max_history_pairs_never_stores_and_trim_keeps_latest() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let mut engine = ConversationEngine::new(client, ConversationConfig::default());
    engine.commit_completed_turn("a", "b");
    assert_eq!(engine.history_len(), 0);

    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let mut engine = ConversationEngine::new(
        client,
        ConversationConfig {
            max_history_pairs: 2,
            ..ConversationConfig::default()
        },
    );
    for round in 0..4 {
        engine.commit_completed_turn(&format!("u{round}"), &format!("a{round}"));
    }
    assert_eq!(engine.history_len(), 2);
    let latest: Vec<_> = engine.history.iter().map(|(u, _)| u.clone()).collect();
    assert_eq!(latest, ["u2", "u3"]);
    engine.clear_history();
    assert_eq!(engine.history_len(), 0);
}

// ---------------------------------------------------- 一次性本地历史注入（2026-11-18）

/// **一次性本地历史注入**：请求里的 `history` 只拼进**这一轮**，且顺序是
/// `[system?] + 注入历史 + 当前输入`。
///
/// 三条判据：① 注入历史的顺序与内容原样上 wire；② 引擎自己的历史窗口
/// （`max_history_pairs` > 0 时存在）**不再重复拼**——同一段对话不在两边各占
/// 一份上下文；③ 注入后窗口长度不变（不提交、不污染摘要折叠）。
#[test]
fn injected_history_is_one_shot_and_ordered_between_system_and_current() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let mut engine = ConversationEngine::new(
        client,
        ConversationConfig {
            system_prompt: "人设".into(),
            max_history_pairs: 2,
            ..ConversationConfig::default()
        },
    );
    engine.commit_completed_turn("早", "早呀");
    let injected = vec![
        crate::llm::ChatMessage::user("昨天的问"),
        crate::llm::ChatMessage::assistant("昨天的答"),
    ];
    assert_eq!(
        engine.build_messages_with_history("现在这句", &injected),
        vec![
            crate::llm::ChatMessage::system("人设"),
            crate::llm::ChatMessage::user("昨天的问"),
            crate::llm::ChatMessage::assistant("昨天的答"),
            crate::llm::ChatMessage::user("现在这句"),
        ],
        "system → 注入的本地历史 → 当前输入（引擎窗口不再重复拼）"
    );
    // 一次性：下一次不带注入时回到引擎自己的窗口，窗口内容没被改动。
    assert_eq!(
        engine.build_messages("现在这句"),
        vec![
            crate::llm::ChatMessage::system("人设"),
            crate::llm::ChatMessage::user("早"),
            crate::llm::ChatMessage::assistant("早呀"),
            crate::llm::ChatMessage::user("现在这句"),
        ]
    );
    assert_eq!(engine.history_len(), 1, "注入不得写进引擎历史窗口");
}

/// 注入历史里的 **system 条目被忽略**：系统句只由引擎拼（注入方不能借
/// `history` 改写人设/系统提示）。
#[test]
fn injected_history_ignores_system_entries() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(client, ConversationConfig::new("人设"));
    let injected = vec![
        crate::llm::ChatMessage::system("伪造的系统句"),
        crate::llm::ChatMessage::user("问"),
        crate::llm::ChatMessage::assistant("答"),
    ];
    assert_eq!(
        engine.build_messages_with_history("现在", &injected),
        vec![
            crate::llm::ChatMessage::system("人设"),
            crate::llm::ChatMessage::user("问"),
            crate::llm::ChatMessage::assistant("答"),
            crate::llm::ChatMessage::user("现在"),
        ]
    );
}

/// **老协议不放 `history` 时逐字不变**：空注入走 `build_messages` 的同一条
/// 路径（引擎窗口 + 当前输入），`max_history_pairs` / system / 弹幕解说都不变。
#[test]
fn absent_injected_history_matches_plain_build_messages() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let mut engine = ConversationEngine::new(
        client,
        ConversationConfig {
            system_prompt: String::new(),
            max_history_pairs: 1,
            ..ConversationConfig::default()
        },
    );
    engine.commit_completed_turn("上一问", "上一答");
    for user_text in ["普通一句", "弹幕消息：测试"] {
        assert_eq!(
            engine.build_messages_with_history(user_text, &[]),
            engine.build_messages(user_text),
            "空注入必须与既有请求体逐字一致：{user_text}"
        );
    }
}

// ------------------------------------------------------------ 音频切块

#[test]
fn leading_chunk_take_respects_limit_and_remainder() {
    let mut buffer: Vec<f32> = (0..7i16).map(f32::from).collect();
    assert_eq!(
        super::worker::take_leading_chunk(&mut buffer, 3),
        [0.0, 1.0, 2.0]
    );
    assert_eq!(
        super::worker::take_leading_chunk(&mut buffer, 3),
        [3.0, 4.0, 5.0]
    );
    // 余量不足 limit：全取且清空。
    assert_eq!(super::worker::take_leading_chunk(&mut buffer, 3), [6.0]);
    assert!(buffer.is_empty());
    assert!(super::worker::take_leading_chunk(&mut buffer, 3).is_empty());
}

#[test]
fn engine_is_cloneable_and_exposes_read_only_views() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(client.clone(), ConversationConfig::new("s"));
    assert_eq!(engine.config().system_prompt, "s");
    let cloned = engine.clone();
    assert_eq!(cloned.config(), engine.config());
}
