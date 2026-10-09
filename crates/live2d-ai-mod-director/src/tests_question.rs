//! **T9 问句补丁**在 director 侧的两条回归（从 presets.rs 的 tests 拆出：
//! AGENTS 源码 ≤500 行 / 测试文件 ≤800 行）。
//!
//! 一条钉规则层「正文有问号时只补空着的槽」，一条钉「表情槽划分与 runtime 的
//! expression 能力集逐项相等」（跨 crate 的漂移护栏）。

use crate::presets::{
    PRESET_IDS, PRESET_NONE, PresetSlot, is_known_preset, preset_slot, rule_cues_for_text,
};

/// **T9（2026-10-07）**：正文含问号时给**空着的槽**补默认值；已有的一律留着。
#[test]
fn question_marks_fill_only_the_empty_slots() {
    let ids = |text: &str| -> Vec<String> {
        rule_cues_for_text(text)
            .iter()
            .map(|c| c.preset_id.clone())
            .collect()
    };
    // 中性 + 问句：两个槽都空 → tilt_left（手势）+ thinking（表情）。
    assert_eq!(ids("你在想什么？"), vec!["tilt_left", "thinking"]);
    // 半角 ? 与全角 ？ 等价。
    assert_eq!(ids("really?"), vec!["tilt_left", "thinking"]);
    // 情绪脸已有 → 只补手势，不挤掉表情。
    assert_eq!(ids("你好呀，今天真开心？"), vec!["smile", "tilt_left"]);
    // 没有问号 → 一个字不补（「吗 / 呢」不是问句标记）。
    assert!(ids("嗯").is_empty());
    assert!(ids("这样吗。").is_empty());
    // 问候（无问号）行为逐字不变：nod + smile。
    assert_eq!(ids("你好呀，今天真开心！"), vec!["nod", "smile"]);
    // 产出的 id 必须都在可接受集合内，且都锚第 1 句。
    for cue in rule_cues_for_text("你在想什么？") {
        assert!(is_known_preset(cue.preset_id.as_str()));
        assert_eq!(cue.sentence_seq, 1);
        assert_eq!(cue.ttl_ms, 2_000);
        assert_eq!(cue.intensity, 1);
    }
    // 同一输入恒等输出。
    assert_eq!(ids("你在想什么？"), ids("你在想什么？"));
}

/// **T9**：表情槽划分与 runtime 的 expression 能力集必须**逐项相等**（不得漂移）。
///
/// 这是两处 id 划分的跨 crate 护栏：director 的 preset_slot 决定规则层补哪个槽，
/// runtime 的 EXPRESSION_PRESET_IDS 决定表演层收哪种 id——漂了就会出现
/// 「补出来的表情在表演层被判成手势 id 丢掉」。
#[test]
fn expression_slot_matches_the_performance_field_map() {
    let face: Vec<&str> = PRESET_IDS
        .iter()
        .copied()
        .filter(|id| *id != PRESET_NONE && preset_slot(id) == PresetSlot::Face)
        .collect();
    assert_eq!(
        face,
        live2d_ai_runtime::performance::EXPRESSION_PRESET_IDS.to_vec(),
        "表情槽集合必须与 runtime 的 expression 能力集逐项相等"
    );
    assert_eq!(
        preset_slot("thinking"),
        PresetSlot::Face,
        "thinking 归表情槽"
    );
    for id in ["look_up", "look_down", "nod", "tilt_left"] {
        assert_eq!(preset_slot(id), PresetSlot::Gesture, "{id} 归手势槽");
    }
    // 问句补丁的表情 id 就在这份集合里。
    assert!(is_known_preset(
        live2d_ai_runtime::performance::QUESTION_EXPRESSION_ID
    ));
    assert!(
        live2d_ai_runtime::performance::is_expression_preset(
            live2d_ai_runtime::performance::QUESTION_EXPRESSION_ID
        ),
        "问句补丁的 thinking 必须是表情槽 id"
    );
}
