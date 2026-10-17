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

#[test]
fn empty_system_prompt_is_omitted_from_wire_messages() {
    let client = OpenAiClient::new(
        LlmConfig::new("http://a/v1", "m"),
        TtsConfig::new("http://b/v1", "v"),
    )
    .expect("client");
    let engine = ConversationEngine::new(client, ConversationConfig::new(""));
    let messages = engine.build_messages("hi");
    assert_eq!(messages, vec![crate::llm::ChatMessage::user("hi")]);
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
    assert_eq!(
        engine.build_messages("弹幕消息：你好"),
        vec![
            crate::llm::ChatMessage::system(super::engine::DANMAKU_PREFACE),
            crate::llm::ChatMessage::user("弹幕消息：你好"),
        ],
        "出厂空人设：这一轮只有弹幕解说这一句 system，正文原样"
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
    let engine = ConversationEngine::new(client, ConversationConfig::new(""));
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
