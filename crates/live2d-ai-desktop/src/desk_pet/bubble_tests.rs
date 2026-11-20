use super::*;
use crate::desk_pet::hit::HitMap;

fn frame_of(width: u32, height: u32) -> RgbaFrame {
    RgbaFrame {
        width,
        height,
        pixels: vec![0; (width * height * 4) as usize],
    }
}

#[test]
fn empty_and_whitespace_sentences_are_not_drawn() {
    assert_eq!(truncate(""), "");
    assert_eq!(truncate("   \n\t "), "");
    let now = Instant::now();
    let mut state = BubbleState::default();
    state.on_sentence("", now, false);
    assert_eq!(state.text(), None);
    assert!(!state.visible());
    state.on_sentence("   ", now, false);
    assert_eq!(state.text(), None);
}

#[test]
fn only_abnormally_long_sentence_is_truncated() {
    let ordinary_long = "中".repeat(60);
    assert_eq!(
        truncate(&ordinary_long),
        ordinary_long,
        "60 字不再截成 48 字"
    );
    let long = "中".repeat(MAX_CHARS + 10);
    let shown = truncate(&long);
    assert_eq!(shown.chars().count(), MAX_CHARS + 1);
    assert!(shown.ends_with('…'));
    assert_eq!(shown.chars().take(MAX_CHARS).count(), MAX_CHARS);
    let exact = "好".repeat(MAX_CHARS);
    assert_eq!(truncate(&exact), exact, "正好到上限不加省略号");
}

#[test]
fn a_new_sentence_replaces_the_old_one_as_a_whole_block() {
    let now = Instant::now();
    let mut state = BubbleState::default();
    state.on_sentence("第一句上台词", now, false);
    assert_eq!(state.text(), Some("第一句上台词"));
    state.on_sentence("第二句完全不一样", now + Duration::from_millis(10), false);
    assert_eq!(state.text(), Some("第二句完全不一样"));
    assert!(
        !state.text().unwrap().contains("第一句"),
        "整块替换，不能追加成两段"
    );
}

#[test]
fn text_only_bubble_disappears_after_four_seconds() {
    let now = Instant::now();
    let mut state = BubbleState::default();
    state.on_sentence("只出文字", now, false);
    state.tick(now + Duration::from_millis(3_999));
    assert!(state.visible());
    state.tick(now + TEXT_ONLY_TTL);
    assert!(!state.visible(), "没有 TTS 时 4 秒后必须消失");
}

#[test]
fn voice_started_switches_to_playing_and_waits_for_the_end() {
    let now = Instant::now();
    let mut state = BubbleState::default();
    state.on_sentence("这句有声音", now, false);
    state.tick(now + Duration::from_millis(3_900));
    state.on_voice_started();
    state.tick(now + Duration::from_secs(9));
    assert!(state.visible(), "正在播，4 秒兜底不该把气泡撤掉");
    state.on_voice_ended();
    assert!(!state.visible(), "播完/被打断立刻消失");
}

#[test]
fn sentence_arriving_while_playing_is_not_treated_as_text_only() {
    let now = Instant::now();
    let mut state = BubbleState::default();
    state.on_sentence("VoiceStarted 早到了", now, true);
    state.tick(now + Duration::from_secs(5));
    assert!(state.visible(), "VoiceStarted 先到的情况也要等 VoiceEnded");
}

#[test]
fn notice_has_its_own_ttl_and_survives_a_stop() {
    let now = Instant::now();
    let mut state = BubbleState::default();
    state.on_notice("没听清", Duration::from_secs(2), now);
    state.on_voice_ended(); // /stop 不该把提示一起抹掉
    assert_eq!(state.text(), Some("没听清"));
    state.tick(now + Duration::from_secs(2));
    assert!(!state.visible());
}

/// 气泡像素算进命中：写在 frame 上 → refresh_map 后 alpha ≥ 16。
#[test]
fn bubble_pixels_are_part_of_the_hit_map() {
    let mut frame = frame_of(64, 32);
    let rect = BubbleRect {
        x: 2,
        y: 2,
        w: 40,
        h: 20,
    };
    paint_background(&mut frame, rect, 14);
    let mut map = HitMap::new();
    map.refresh(&frame);
    assert!(map.hits_at(20, 12), "点在气泡字上等于点在角色上");
    assert!(map.hits_at(2 + 20, 2 + 10));
    assert!(!map.hits_at(0, 0), "圆角外仍是透明像素，照旧穿透");
    assert!(!map.hits_at(60, 30), "气泡之外照旧穿透");
    // 拖拽规则不变：verdict 只看同一张 alpha 表。
    assert_eq!(
        crate::desk_pet::hit::verdict(
            crate::desk_pet::config::HitMode::Opaque,
            Some(&map),
            20,
            12,
            false
        ),
        crate::desk_pet::hit::Verdict::Client
    );
}
