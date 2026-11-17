//! 窗线程每帧一次的编排：抽事件 → 推进按住说话 → 气泡到期 → 画进同一帧。
//!
//! 全部状态住在这里，`window.rs` 只多两行（`new` 与 `tick`），避免把那扇窗
//! 撑过 500 行。构造时才 `bridge::open_events()`——**窗没开不分配通道**；
//! `Drop` 时撤掉，之后 supervisor 的事件重新变回 no-op。

use std::sync::mpsc;
use std::time::Instant;

use l2d::renderer::RgbaFrame;

use super::bridge::{self, PetEvent};
use super::bubble::{self, BubbleRect, BubbleState, TextCache};
use super::menu_state;
use super::talk;

pub struct Companion {
    events: mpsc::Receiver<PetEvent>,
    bubble: BubbleState,
    cache: TextCache,
    /// 用户句自己的文字栅格缓存，与 AI 句分开。
    user_cache: TextCache,
    /// `bubble::layout` 要扫整帧 bbox，窗尺寸固定 ⇒ 只算一次。
    base: Option<BubbleRect>,
}

impl Companion {
    /// 建窗成功后调；此刻才有接收端，也才有麦克风/气泡这条旁路。
    pub fn new() -> Self {
        Self {
            events: bridge::open_events(),
            bubble: BubbleState::default(),
            cache: TextCache::new(),
            user_cache: TextCache::new(),
            base: None,
        }
    }

    /// 在 `renderer.render_frame()` 之后、`hit::refresh_map` / `present` 之前调。
    pub fn tick(&mut self, frame: &mut RgbaFrame) {
        let now = Instant::now();

        // 1) 抽干 supervisor 送来的事实（顺序即发送顺序）。
        while let Ok(event) = self.events.try_recv() {
            match event {
                PetEvent::Sentence(text) => {
                    self.bubble.on_sentence(&text, now, bridge::is_playing());
                    super::chat::push("AI", &text);
                }
                PetEvent::UserLine(text) => {
                    self.bubble.on_user_line(&text, now);
                    super::chat::push("我", &text);
                }
                PetEvent::VoiceStarted => self.bubble.on_voice_started(),
                PetEvent::VoiceEnded => self.bubble.on_voice_ended(),
            }
        }

        // 2) 按住说话的 300 ms / 120 秒判定与「没配钥匙 / 没听清」提示。
        for notice in talk::poll() {
            self.bubble.on_notice(notice.text, notice.ttl, now);
        }

        // 3) 到期（只出文字 4 秒、notice 自己的 ttl、播放保险丝）。
        self.bubble.tick(now);

        // 4) 简易会话框按菜单状态开 / 关 / 置顶 / 重绘（独立于角色窗）。
        super::chat::poll();

        // 5) 画进这一帧：两块气泡各自受菜单开关控制，像素同时进命中表。
        let state = menu_state::get();
        let (show_user, show_ai) = menu_state::bubble_visibility(&state);
        let base = *self.base.get_or_insert_with(|| bubble::layout(frame, 0));
        bubble::overlay_pair(
            frame,
            &self.bubble,
            &mut self.cache,
            &mut self.user_cache,
            show_user,
            show_ai,
            base,
        );
    }
}

impl Default for Companion {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Companion {
    fn drop(&mut self) {
        // 宠物窗关了，那块小窗不能留在屏幕上（它的消息循环也一起没了）。
        super::chat::close();
        bridge::close_events();
    }
}
