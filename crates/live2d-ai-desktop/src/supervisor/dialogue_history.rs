//! 进程内对话历史分桶。
//!
//! 引擎同时只装一份 `history`（请求组装不用改）。每轮开始前按会话换上：
//! 无会话 = 全局桶，有会话 = 该 `session_id` 自己的桶。热重载成功时整表
//! 清空，与「新引擎从空历史开始」一致。不落盘：重启后模型忘记对话，
//! 前端本地气泡还在。

use std::collections::{HashMap, VecDeque};

use live2d_ai_mod_system::sanitize_session_id;
use live2d_ai_runtime::ConversationEngine;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Key {
    Global,
    Session(String),
}

/// 宿主侧的历史桶。`clear` 不碰引擎：调用方要先换成空历史的新引擎。
pub(super) struct DialogueHistories {
    loaded: Key,
    buckets: HashMap<Key, VecDeque<(String, String)>>,
}

impl Default for DialogueHistories {
    fn default() -> Self {
        Self {
            loaded: Key::Global,
            buckets: HashMap::new(),
        }
    }
}

impl DialogueHistories {
    /// 让引擎装上 `session` 的历史。同一键连着调用是空操作。
    pub(super) fn install(&mut self, engine: &mut ConversationEngine, session: Option<&str>) {
        let key = session
            .and_then(sanitize_session_id)
            .map(Key::Session)
            .unwrap_or(Key::Global);
        if key == self.loaded {
            return;
        }
        let previous = engine.take_history();
        self.buckets.insert(self.loaded.clone(), previous);
        self.loaded = key.clone();
        let next = self.buckets.remove(&key).unwrap_or_default();
        engine.replace_history(next);
    }

    /// 丢掉全部分桶，并把「当前装入的键」复位成全局。
    ///
    /// 只在热重载**成功**、引擎已被换成空历史的新实例之后调用。
    pub(super) fn clear(&mut self) {
        self.buckets.clear();
        self.loaded = Key::Global;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use live2d_ai_runtime::{ConversationConfig, LlmConfig, OpenAiClient, TtsConfig};

    fn engine(max_history_pairs: usize) -> ConversationEngine {
        let client = OpenAiClient::new(
            LlmConfig::new("http://127.0.0.1:9/v1", "m"),
            TtsConfig::new("http://127.0.0.1:9/v1", "alloy"),
        )
        .expect("client");
        ConversationEngine::new(
            client,
            ConversationConfig {
                max_history_pairs,
                ..ConversationConfig::new("人设")
            },
        )
    }

    #[test]
    fn sessions_do_not_share_history_and_global_bucket_stays() {
        let mut engine = engine(4);
        let mut histories = DialogueHistories::default();

        engine.commit_completed_turn("全局一", "答一");
        histories.install(&mut engine, None);
        assert_eq!(engine.history_len(), 1, "同一全局键不该把历史倒掉");

        histories.install(&mut engine, Some("session-a"));
        assert_eq!(engine.history_len(), 0, "新会话从空历史开始");
        engine.commit_completed_turn("甲", "答甲");

        histories.install(&mut engine, Some("session-b"));
        assert_eq!(engine.history_len(), 0);
        engine.commit_completed_turn("乙", "答乙");

        histories.install(&mut engine, Some("session-a"));
        assert_eq!(engine.history_len(), 1, "回到 A 时应只剩 A 的一对");

        histories.install(&mut engine, None);
        assert_eq!(engine.history_len(), 1, "全局桶不含会话轮次");
    }

    #[test]
    fn zero_cap_drops_installed_history() {
        let mut engine = engine(0);
        let mut histories = DialogueHistories::default();
        histories.install(&mut engine, Some("session-a"));
        engine.commit_completed_turn("甲", "答甲");
        assert_eq!(engine.history_len(), 0, "上限 0 时提交本身就是空操作");
        histories.install(&mut engine, None);
        histories.install(&mut engine, Some("session-a"));
        assert_eq!(engine.history_len(), 0);
    }
}
