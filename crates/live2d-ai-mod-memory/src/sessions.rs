//! 会话分桶与「按会话注入」的口径（L1 产品级，2026-09-15）。
//!
//! # 为什么拆出来
//!
//! 会话感知把「一个 JSONL + 一份全局 system_prompt」变成「会话桶 + 每会话
//! 提示词覆盖」。这段规则有清晰的边界（谁能进文件名、回落给谁），单列一文件
//! 让 `lib.rs` 只留生命周期与被动时序。
//!
//! # 三条口径（写死，测试守）
//!
//! 1. **有会话 → 会话桶**：`<老路径同目录>/sessions/<session>.memory.jsonl`；
//!    无会话 → **老路径**（`memory.jsonl`）。老记忆必须还能读到（兼容红线）。
//! 2. **注入按会话**：有会话时把命中内容写进
//!    [`ModServices::session_prompts`] 的**本 Mod 来源槽**
//!    （owner = `SESSION_PROMPT_OWNER_MEMORY`），**不** apply_settings 写全局
//!    `persona.system_prompt`——否则 A 会话的记忆会串到 B。**不**再把 base
//!    拼进来：persona 的卡在自己的槽里，宿主读时按固定顺序组合。
//! 3. **无会话 → 老路径**：裸 HTTP / 终端壳没有会话，退回全局
//!    `apply_settings`。这是**降级**，不是等价能力——面板与文档都写明。
//!
//! # 会话 id 进文件名
//!
//! 一律过 [`sanitize_session_id`]（宿主归一化后再投递；命令参数是用户输入，
//! 更要自己再验一遍）。非法 id 不会成为第二个桶，只会变成一条可读错误。
//!
//! [`ModServices::session_prompts`]: live2d_ai_mod_system::ModServices::session_prompts

use live2d_ai_mod_system::{ModError, sanitize_session_id};
use serde_json::Value;

use crate::MemoryRuntime;
use crate::store::JsonlStore;
use crate::strategy;

impl MemoryRuntime {
    /// 解析出的**全局（老路径）**存储句柄；`None` = 路径未配置（调用方 no-op）。
    ///
    /// 名字保留为 `store`：老调用方/测试把它当「记忆库」；
    /// 会话感知后它的准确含义是**无会话时的桶**，见 [Self::store_for]。
    pub fn store(&self) -> Option<JsonlStore> {
        self.store_for(None)
    }

    /// 指定会话的存储句柄；`session = None` → 老路径全局桶。
    ///
    /// 会话 id 非法（路径穿越闸）→ `None`：调用方必须报可读错误或按无会话处理，
    /// **绝不**拿用户输入直接拼路径。
    pub fn store_for(&self, session: Option<&str>) -> Option<JsonlStore> {
        let base = self.config.resolve_store_path(&self.services.config_path)?;
        match session {
            Some(id) => strategy::resolve_session_store_path(&base, id).map(JsonlStore::new),
            None => Some(JsonlStore::new(base)),
        }
    }

    /// 当前生效的桶 = 最近一次 `TurnPrompt` 的会话桶；没有会话 → 老路径。
    ///
    /// 被动路径（remember / retrieve / state_json）用它，保证「记进哪个桶、
    /// 从哪个桶检索、面板看到的条数」三者同源。
    pub(crate) fn effective_store(&self) -> Option<JsonlStore> {
        self.store_for(self.current_session.as_deref())
    }

    /// 命令参数里的 `session_id`：**给了就必须合法**，没给 → 全局桶。
    ///
    /// 与面板口径逐字一致（`mod_panel.dart` 的 `activeSessionId`）：面板在
    /// `activeSessionId == null` 时**不传** `session_id`，此时落老路径全局桶
    /// ——这正是 UI 上写的降级，而不是「悄悄找上一个会话」（那会让面板说一套、
    /// 实际做另一套）。
    pub(crate) fn command_session(&self, args: &Value) -> Result<Option<String>, ModError> {
        let raw = args
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        match raw {
            None => Ok(None),
            Some(id) => sanitize_session_id(id).map(Some).ok_or_else(|| {
                ModError::Other(format!(
                    "会话 id 不合法：{id}（只允许 ASCII 字母数字与 - _ . :，且不超过 128 字符）"
                ))
            }),
        }
    }

    /// 命令用存储句柄；路径不可解析 → 可读错误 + `errors`。
    pub(crate) fn require_store(&mut self, session: Option<&str>) -> Result<JsonlStore, ModError> {
        match self.store_for(session) {
            Some(store) => Ok(store),
            None => {
                self.errors += 1;
                Err(ModError::Other(
                    "memory 存储路径未配置（store_path 空且 host 未注入 config_path），命令无法执行"
                        .to_string(),
                ))
            }
        }
    }

    /// 提示词里是否还有本 Mod 的注入块（`clear` 的 `residue` 用）。
    ///
    /// 有会话就看**该会话**的覆盖，无会话才看全局——保证面板提示的那份提示词
    /// 与它操作的桶同源。
    pub(crate) fn prompt_has_residue(&self, session: Option<&str>) -> bool {
        match session {
            Some(id) => self
                .services
                .session_prompts
                .get(id)
                .is_some_and(|prompt| strategy::contains_memory_block(&prompt)),
            None => strategy::contains_memory_block(&self.current_main_prompt()),
        }
    }
}
