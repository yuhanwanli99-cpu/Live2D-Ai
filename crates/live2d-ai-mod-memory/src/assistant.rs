//! 助手侧记忆（Wave 3，2026-09-21）——AssistantReplied → 按 conversation 分桶追加。
//!
//! 拆出来的理由：lib.rs 已接近「源码 ≤1000 行（有头注豁免）」的上限；助手侧这条
//! 链路有自己的入参（JSON payload + 轮末会话）与失败纪律，单列一文件后 lib.rs 只留
//! 「一轮发生了什么」的主线（与 summary_flow.rs 同款拆分）。
//!
//! 写入实现只有一处（[MemoryRuntime::append_record_in]）：用户侧 remember 与助手侧
//! remember_assistant 共用它，因此「物理淘汰 / 计数 / 失败 warn」的口径不会两边漂移。

use live2d_ai_mod_system::sanitize_session_id;

use crate::MemoryRuntime;
use crate::strategy::{self, MemoryRecord};

impl MemoryRuntime {
    /// **助手侧记录**（Wave 3，2026-09-21）：把「助手已说出 / 已上屏的正文」
    /// 追加进**本轮 conversation 的桶**（session 由 host 在 AssistantReplied
    /// 投递时给出）。
    ///
    /// 口径与用户侧**完全对称**：
    /// - 有 conversation → 会话桶；无 conversation → 老路径全局桶（只记）；
    /// - 无论哪种，**都不注入**、不写全局 persona.system_prompt、不单独触发摘要；
    /// - 写失败只 warn（记忆是增量能力，写不进去不该打断主链）。
    ///
    /// 正文已经是**清洗后的上屏口径**（supervisor 侧做过 clean_for_tts），
    /// 本函数不再二次加工。
    pub fn remember_assistant(
        &mut self,
        text: &str,
        ts: i64,
        turn: u64,
        session: Option<&str>,
    ) -> bool {
        let record = MemoryRecord::assistant(text.trim(), ts, turn);
        self.append_record_in(session, record)
    }

    /// 追加一条记录到指定桶（用户 / 助手共用；**唯一**写入实现）。
    ///
    /// session = None → 老路径全局桶（与既有「无会话仍落老桶」口径一致）。
    pub(crate) fn append_record_in(&mut self, session: Option<&str>, record: MemoryRecord) -> bool {
        if record.text.is_empty() {
            return false;
        }
        let Some(store) = self.store_for(session) else {
            self.errors += 1;
            self.services.logger.warn(
                "memory 存储路径未配置（store_path 空且 host 未注入 config_path），本轮记忆被跳过",
            );
            return false;
        };
        match store.append_capped(&record, self.config.max_records) {
            Ok(outcome) => {
                self.writes += 1;
                if record.role == strategy::MemoryRole::Assistant {
                    self.assistant_writes += 1;
                }
                self.note_eviction(outcome.evicted, outcome.kept);
                if outcome.evicted > 0 && outcome.bad_lines > 0 {
                    self.services.logger.warn(&format!(
                        "memory 物理淘汰重写顺带清掉 {} 条坏记录",
                        outcome.bad_lines
                    ));
                }
                true
            }
            Err(e) => {
                self.errors += 1;
                self.services.logger.warn(&format!(
                    "memory 追加记忆失败（{}）: {e}",
                    store.path().display()
                ));
                false
            }
        }
    }

    /// AssistantReplied 事件（Wave 3，2026-09-21）：把助手侧正文追加进桶。
    ///
    /// payload 契约（host 侧唯一生产者，见 topics.rs）：
    /// {"turn":N,"role":"assistant","text":"...","interrupted":bool}。
    ///
    /// 纪律：
    /// - payload 坏 / 缺 text / text 空白 → warn + 本轮跳过（**不 panic**）；
    /// - 事件里的 role 只作 provenance 校验：本主题契约恒 assistant，非
    ///   assistant 只 warn 仍按助手侧入桶；
    /// - 轮次：优先 payload.turn，缺失回落 self.turn_seq（同轮用户记录的值）——
    ///   同轮共用一个 turn 是摘要「保留最近 K 轮」按 turn 分组的前提；
    /// - **不阻塞主链**：只做一次本地追加（与 TurnPrompt 的 remember 同一个
    ///   append_capped），没有网络、没有锁等待；失败只 warn。
    pub(crate) fn handle_assistant_replied(&mut self, payload: &str, session: Option<&str>) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
            self.services
                .logger
                .warn("memory 收到无法解析的 AssistantReplied payload，本轮助手侧记忆跳过");
            return;
        };
        let text = value
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            return;
        }
        if let Some(role) = value.get("role").and_then(serde_json::Value::as_str)
            && role != "assistant"
        {
            self.services.logger.warn(&format!(
                "memory AssistantReplied 的 role={role} 非 assistant（契约异常），仍按助手侧入桶"
            ));
        }
        let turn = value
            .get("turn")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(self.turn_seq);
        // 会话来源：优先事件带的（host 每轮都带，与 TurnPrompt 同一个 id）；
        // 没带就回落到本轮的 current_session（unscoped 投递路径）。
        let raw_session = session
            .map(str::to_string)
            .or_else(|| self.current_session.clone());
        let normalized = raw_session.as_deref().and_then(sanitize_session_id);
        if raw_session.is_some() && normalized.is_none() {
            self.services
                .logger
                .warn("memory 助手侧收到非法会话 id，退回全局桶（不拿它拼路径）");
        }
        if self.remember_assistant(text, strategy::unix_now(), turn, normalized.as_deref()) {
            self.services.logger.info(&format!(
                "memory 已记录助手侧正文 {} 字（turn={turn}）",
                text.chars().count()
            ));
        }
    }
}
