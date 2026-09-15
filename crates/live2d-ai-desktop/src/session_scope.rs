//! 会话作用域（L1 产品级基座，2026-09-15）。
//!
//! 宿主侧的**会话级 system_prompt 覆盖层**：一张「会话 id × 来源槽（owner）
//! → 文本」的内存表 + 一个「当前活动会话」游标。能力契约在
//! live2d_ai_mod_system::SessionPromptSink（这里给出宿主实现）。
//!
//! # 它在链路里的位置（一句话）
//!
//! POST /api/v1/chat {text, session_id} → supervisor 记下活动会话 →
//! **每轮开始前** engine.set_system_prompt_override(表.get(session_id)) →
//! 本轮 LLM 请求的 system 就是该会话各来源槽的组合；表里没有 → 回落全局
//! persona.system_prompt。
//!
//! 因此它**不写 live2d-ai.toml、不触发 reload**，两个会话的人设天然互不干扰。
//!
//! # 为什么是「多来源槽」而不是「一个会话一个字符串」
//!
//! 旧实现每个会话只存一个字符串（last-writer-wins）。persona 导入卡写进会话 A
//! 的整段人设后，memory 的检索注入读「当前值」当 base、拼上记忆块再整段写回；
//! memory 停用时 clear_all() 就把 persona 在会话 A 的卡一起清掉，而 persona
//! 不会自动重写 → 该会话静默回落全局人设。改成 owner 槽后：写入只覆盖自己的槽、
//! 清除只清自己的槽、读取按固定顺序组合（见 [SessionPromptSink::get] 与
//! live2d_ai_mod_system::owner_merge_rank）。组合顺序固定为
//! 匿名槽 → persona → memory → 其余 owner（字典序）。
//!
//! # 为什么是 BTreeMap 而不是 HashMap
//!
//! sessions() 要回给面板展示、给测试断言。BTreeMap 的迭代顺序天然稳定，
//! 「同一个状态两次读出来顺序不同」这种噪声从一开始就不存在。
//!
//! # 容量
//!
//! 表是**进程内存**，必须有界：[MAX_SESSION_SCOPES] 个会话、
//! **每条贡献** [MAX_SESSION_PROMPT_CHARS] 字符。写入超限时**拒绝并返回**，
//! 绝不静默淘汰——「我明明导入过」而实际被别人的会话挤掉，是最难查的一类缺陷。
//! 空串写入 = **撤销该槽的贡献**（不留一个「空会话」条目，sessions() 才干净）。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use live2d_ai_mod_system::{
    ModSessionPrompts, SESSION_PROMPT_OWNER_DEFAULT, SessionPromptSink, compose_session_prompt,
    owner_merge_rank, sanitize_session_id,
};

/// 会话表容量上限（条）。
///
/// 取值理由：Flutter 侧 ChatSessionStore.maxSessions = 50，这里给 4 倍余量
/// 以容纳「多客户端 / 多标签页」的同进程叠加，同时保证内存与快照大小有界。
pub const MAX_SESSION_SCOPES: usize = 200;

/// 单条贡献长度上限（**字符**）。
///
/// 角色卡合成结果实测在 1 Ki 字符量级（persona 的 131 字卡约 300 字提示词）；
/// 64 Ki 是所有真实卡都摸不到、但足以挡住「误把整本书塞进来」的界。
/// 判定是**按来源槽逐条**做的：persona 的卡与 memory 的块各有各的额度。
pub const MAX_SESSION_PROMPT_CHARS: usize = 64 * 1024;

/// 某会话的槽表：owner → 文本。
type Slots = BTreeMap<String, String>;

#[derive(Default)]
struct Inner {
    /// 会话 id → (来源槽 id → 文本)（**已归一化**的 id 才可能在表里）。
    prompts: BTreeMap<String, Slots>,
    /// 当前活动会话（最近一次对话用的那个）。
    active: Option<String>,
}

/// 会话作用域存储（Clone 出的是同一张表的句柄）。
#[derive(Clone, Default)]
pub struct SessionScopeStore {
    inner: Arc<Mutex<Inner>>,
}

/// 某会话各槽的组合结果；全空 / 无槽 → None。
fn compose_slots(slots: &Slots) -> Option<String> {
    compose_session_prompt(
        slots
            .iter()
            .map(|(owner, text)| (owner.as_str(), text.as_str())),
    )
}

/// 删掉一个槽；会话因此空了就整条删掉（sessions() / len() 才干净）。
fn remove_slot(inner: &mut Inner, id: &str, owner: &str) -> bool {
    let mut removed = false;
    let mut drop_session = false;
    if let Some(slots) = inner.prompts.get_mut(id) {
        removed = slots.remove(owner).is_some();
        drop_session = slots.is_empty();
    }
    if drop_session {
        inner.prompts.remove(id);
    }
    removed
}

impl SessionScopeStore {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 取锁；**锁中毒时不 panic**，直接拿回内部数据。
    ///
    /// 会话表在每轮对话的热路径上被读；一次 Mod 线程 panic 不该让整个对话链路
    /// 从此 panic（那会把「一个 Mod 崩了」升级成「产品不能说话」）。
    fn guard(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 包装成注入给 Mod 的能力句柄。
    pub fn as_mod_prompts(&self) -> ModSessionPrompts {
        ModSessionPrompts::new(self.clone())
    }

    /// 当前会话数（**有任一槽**的会话数；测试 / 状态展示用）。
    pub fn len(&self) -> usize {
        self.guard().prompts.len()
    }

    /// 归一化 + 取某会话的**组合结果**；非法 id / 没有 → None。
    pub fn prompt_for(&self, session: Option<&str>) -> Option<String> {
        let id = session.and_then(sanitize_session_id)?;
        let inner = self.guard();
        compose_slots(inner.prompts.get(&id)?)
    }

    // ---- 下面两个是 [SessionPromptSink] 的**固有方法**转发 ----
    //
    // 为什么只转发这两个：supervisor 的热路径（say / say_scoped）要用它们，
    // 而调用点不该为了一个方法去 import 一个 trait。其余（get / set / sessions）
    // 只经 ModSessionPrompts 句柄被 Mod 使用，或只被测试使用——trait 实现本身
    // 不产生 dead_code。

    /// 见 [SessionPromptSink::active]。
    pub fn active(&self) -> Option<String> {
        SessionPromptSink::active(self)
    }

    /// 见 [SessionPromptSink::set_active]。
    pub fn set_active(&self, session: Option<&str>) {
        SessionPromptSink::set_active(self, session)
    }
}

impl SessionPromptSink for SessionScopeStore {
    fn get(&self, session: &str) -> Option<String> {
        let id = sanitize_session_id(session)?;
        let inner = self.guard();
        compose_slots(inner.prompts.get(&id)?)
    }

    fn set_owned(&self, owner: &str, session: &str, prompt: &str) {
        // session id 只校验一次（与旧 set 一致）。
        let Some(id) = sanitize_session_id(session) else {
            return;
        };
        // 长度按**每条贡献**判。
        if prompt.chars().count() > MAX_SESSION_PROMPT_CHARS {
            return;
        }
        let mut inner = self.guard();
        if prompt.is_empty() {
            // 空串 = 撤销该槽（不留空会话条目）。
            remove_slot(&mut inner, &id, owner);
            return;
        }
        // 已有会话 = 覆盖（不受容量限制）；新会话才检查容量。
        if !inner.prompts.contains_key(&id) && inner.prompts.len() >= MAX_SESSION_SCOPES {
            return;
        }
        inner
            .prompts
            .entry(id)
            .or_default()
            .insert(owner.to_string(), prompt.to_string());
    }

    fn clear_owned(&self, owner: &str, session: &str) -> bool {
        let Some(id) = sanitize_session_id(session) else {
            return false;
        };
        let mut inner = self.guard();
        remove_slot(&mut inner, &id, owner)
    }

    fn clear_owner(&self, owner: &str) {
        let mut inner = self.guard();
        let ids: Vec<String> = inner.prompts.keys().cloned().collect();
        for id in ids {
            remove_slot(&mut inner, &id, owner);
        }
    }

    fn contributions(&self, session: &str) -> Vec<(String, String)> {
        let Some(id) = sanitize_session_id(session) else {
            return Vec::new();
        };
        let inner = self.guard();
        let Some(slots) = inner.prompts.get(&id) else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = slots
            .iter()
            .filter(|(_, text)| !text.is_empty())
            .map(|(owner, text)| (owner.clone(), text.clone()))
            .collect();
        out.sort_by(|a, b| owner_merge_rank(&a.0).cmp(&owner_merge_rank(&b.0)));
        out
    }

    fn set(&self, session: &str, prompt: &str) {
        // 遗留入口 = 写匿名槽。
        self.set_owned(SESSION_PROMPT_OWNER_DEFAULT, session, prompt);
    }

    fn clear(&self, session: &str) -> bool {
        let Some(id) = sanitize_session_id(session) else {
            return false;
        };
        // 清掉该会话**所有**槽。
        self.guard().prompts.remove(&id).is_some()
    }

    fn clear_all(&self) {
        self.guard().prompts.clear();
    }

    fn sessions(&self) -> Vec<String> {
        self.guard().prompts.keys().cloned().collect()
    }

    fn active(&self) -> Option<String> {
        self.guard().active.clone()
    }

    fn set_active(&self, session: Option<&str>) {
        let normalized = session.and_then(sanitize_session_id);
        self.guard().active = normalized;
    }
}

impl std::fmt::Debug for SessionScopeStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = self.guard();
        f.debug_struct("SessionScopeStore")
            .field("sessions", &inner.prompts.len())
            .field("active", &inner.active)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use live2d_ai_mod_system::{SESSION_PROMPT_OWNER_MEMORY, SESSION_PROMPT_OWNER_PERSONA};

    #[test]
    fn set_get_clear_round_trip() {
        let s = SessionScopeStore::new();
        assert_eq!(s.len(), 0);
        assert_eq!(s.prompt_for(Some("a")), None);
        s.set("a", "A 的人设");
        s.set("b", "B 的人设");
        assert_eq!(s.get("a").as_deref(), Some("A 的人设"));
        assert_eq!(s.get("b").as_deref(), Some("B 的人设"));
        // 稳定顺序（BTreeMap）：面板与测试都不该被迭代顺序咬。
        assert_eq!(s.sessions(), vec!["a".to_string(), "b".to_string()]);
        assert!(s.prompt_for(Some("a")).is_some());
        assert!(s.clear("a"));
        assert!(!s.clear("a"), "重复清空返回 false");
        assert_eq!(s.prompt_for(Some("a")), None);
        assert_eq!(s.prompt_for(Some("b")).as_deref(), Some("B 的人设"));
    }

    #[test]
    fn sessions_do_not_leak_into_each_other() {
        // L1 验收的核心断言：A 的卡不会出现在 B 上。
        let s = SessionScopeStore::new();
        s.set("session-a", "星梦");
        s.set("session-b", "另一个角色");
        assert_ne!(s.get("session-a"), s.get("session-b"));
        s.clear("session-b");
        assert_eq!(s.get("session-b"), None);
        assert_eq!(s.get("session-a").as_deref(), Some("星梦"), "清 B 不该动 A");
    }

    #[test]
    fn clear_all_removes_every_scope_but_keeps_active_cursor() {
        let s = SessionScopeStore::new();
        s.set("a", "x");
        s.set("b", "y");
        s.set_active(Some("a"));
        s.clear_all();
        assert_eq!(s.len(), 0);
        assert_eq!(s.sessions(), Vec::<String>::new());
        // 游标是「当前在看哪个会话」，不是覆盖内容，刻意不跟着清。
        assert_eq!(s.active().as_deref(), Some("a"));
    }

    #[test]
    fn invalid_ids_are_ignored_not_stored() {
        let s = SessionScopeStore::new();
        s.set("../etc/passwd", "坏人设");
        s.set("", "空 id");
        s.set("a b", "带空格");
        s.set_owned(SESSION_PROMPT_OWNER_PERSONA, "../etc/passwd", "坏人设");
        assert_eq!(s.len(), 0, "非法会话 id 不得进表");
        assert_eq!(s.get("../etc/passwd"), None);
        // active 也走同一道归一化闸。
        s.set_active(Some("../etc"));
        assert_eq!(s.active(), None);
        s.set_active(Some("  good-id  "));
        assert_eq!(s.active().as_deref(), Some("good-id"), "归一化后应被接受");
    }

    #[test]
    fn empty_prompt_revokes_the_slot_and_overlong_is_rejected() {
        let s = SessionScopeStore::new();
        // 空串 = 撤销该槽的贡献（不留空会话条目）。
        s.set("a", "");
        assert_eq!(s.get("a"), None, "空贡献不产生组合结果");
        assert_eq!(s.len(), 0, "空写入不得留下会话条目");
        assert_eq!(s.sessions(), Vec::<String>::new());
        // 单槽超长按**每条贡献**判，不得静默截断进表。
        s.set_owned("other", "b", &"字".repeat(MAX_SESSION_PROMPT_CHARS + 1));
        assert_eq!(s.get("b"), None, "超长提示词不得静默截断进表");
        s.set_owned("other", "c", &"字".repeat(MAX_SESSION_PROMPT_CHARS));
        assert!(s.get("c").is_some(), "正好到上限要能进");
    }

    #[test]
    fn capacity_is_bounded_and_never_evicts_silently() {
        let s = SessionScopeStore::new();
        for i in 0..MAX_SESSION_SCOPES {
            s.set(&format!("s{i}"), "p");
        }
        assert_eq!(s.len(), MAX_SESSION_SCOPES);
        s.set("overflow", "p");
        assert_eq!(s.len(), MAX_SESSION_SCOPES, "超容量不得挤掉别人");
        assert_eq!(s.get("overflow"), None);
        // 已有条目仍可覆盖（容量只在「新条目」上判定）。
        s.set("s0", "更新后");
        assert_eq!(s.get("s0").as_deref(), Some("更新后"));
        // 已有会话的**新 owner** 也不算新会话：仍可写。
        s.set_owned(SESSION_PROMPT_OWNER_PERSONA, "s0", "人设");
        assert_eq!(
            s.contributions("s0")
                .iter()
                .map(|(o, _)| o.as_str())
                .collect::<Vec<_>>(),
            vec![SESSION_PROMPT_OWNER_DEFAULT, SESSION_PROMPT_OWNER_PERSONA],
            "匿名槽在前，persona 在后"
        );
    }

    #[test]
    fn mod_prompts_handle_shares_the_same_table() {
        let s = SessionScopeStore::new();
        let handle = s.as_mod_prompts();
        assert!(handle.enabled(), "宿主实现必须自报可用");
        handle.set("a", "来自 Mod");
        assert_eq!(s.get("a").as_deref(), Some("来自 Mod"));
        s.set("b", "来自宿主");
        assert_eq!(handle.get("b").as_deref(), Some("来自宿主"));
        handle.clear_all();
        assert_eq!(s.len(), 0);
    }

    // ------------------------------------------------ 多来源槽（本次修复）

    #[test]
    fn composition_order_is_anonymous_then_persona_then_memory() {
        let s = SessionScopeStore::new();
        s.set_owned(SESSION_PROMPT_OWNER_MEMORY, "A", "记忆块");
        s.set_owned(SESSION_PROMPT_OWNER_DEFAULT, "A", "匿名");
        s.set_owned(SESSION_PROMPT_OWNER_PERSONA, "A", "人设");
        assert_eq!(
            s.get("A").as_deref(),
            Some("匿名\n\n人设\n\n记忆块"),
            "顺序固定：匿名 → persona → memory"
        );
        assert_eq!(
            s.prompt_for(Some("A")),
            s.get("A"),
            "supervisor 的读点与 get 同语义"
        );
    }

    /// 本次修复的动机：清掉一个来源的槽，不得误伤另一个来源。
    #[test]
    fn clearing_one_owner_leaves_the_other_owners_contribution() {
        let s = SessionScopeStore::new();
        s.set_owned(SESSION_PROMPT_OWNER_PERSONA, "A", "人格卡：猫娘小灰");
        s.set_owned(SESSION_PROMPT_OWNER_MEMORY, "A", "用户喜欢薄荷");

        assert!(s.clear_owned(SESSION_PROMPT_OWNER_MEMORY, "A"));
        let kept = s.get("A").expect("persona 的贡献必须还在");
        assert_eq!(kept, "人格卡：猫娘小灰", "memory 停用不得清掉 persona");
        assert_eq!(s.sessions(), vec!["A".to_string()], "会话条目仍在");

        // 反向同样成立：persona 的槽被清后，memory 的贡献还在。
        let s = SessionScopeStore::new();
        s.set_owned(SESSION_PROMPT_OWNER_PERSONA, "A", "人格卡");
        s.set_owned(SESSION_PROMPT_OWNER_MEMORY, "A", "记忆块");
        assert!(s.clear_owned(SESSION_PROMPT_OWNER_PERSONA, "A"));
        assert_eq!(s.get("A").as_deref(), Some("记忆块"));
    }

    #[test]
    fn clear_owner_prunes_sessions_that_become_empty() {
        let s = SessionScopeStore::new();
        s.set_owned(SESSION_PROMPT_OWNER_MEMORY, "A", "只有记忆");
        s.set_owned(SESSION_PROMPT_OWNER_MEMORY, "B", "只有记忆");
        s.set_owned(SESSION_PROMPT_OWNER_PERSONA, "B", "还有人设");
        s.clear_owner(SESSION_PROMPT_OWNER_MEMORY);
        assert_eq!(s.len(), 1, "A 已无槽，条目必须一起删");
        assert_eq!(s.sessions(), vec!["B".to_string()]);
        assert_eq!(s.get("A"), None);
        assert_eq!(s.get("B").as_deref(), Some("还有人设"));
        // 幂等：再清一次不动 B。
        s.clear_owner(SESSION_PROMPT_OWNER_MEMORY);
        assert_eq!(s.get("B").as_deref(), Some("还有人设"));
    }

    #[test]
    fn contributions_snapshot_is_ordered_and_skips_empty() {
        let s = SessionScopeStore::new();
        s.set_owned(SESSION_PROMPT_OWNER_MEMORY, "A", "记忆");
        s.set_owned(SESSION_PROMPT_OWNER_PERSONA, "A", "人设");
        s.set_owned("zzz", "A", "Z");
        assert_eq!(
            s.contributions("A"),
            vec![
                (SESSION_PROMPT_OWNER_PERSONA.to_string(), "人设".to_string()),
                (SESSION_PROMPT_OWNER_MEMORY.to_string(), "记忆".to_string()),
                ("zzz".to_string(), "Z".to_string()),
            ]
        );
        assert!(s.contributions("missing").is_empty());
        assert!(s.contributions("../x").is_empty(), "非法 id 不得读槽");
    }

    #[test]
    fn owned_writes_still_enforce_the_session_cap() {
        let s = SessionScopeStore::new();
        for i in 0..MAX_SESSION_SCOPES {
            s.set_owned(SESSION_PROMPT_OWNER_PERSONA, &format!("s{i}"), "p");
        }
        s.set_owned(SESSION_PROMPT_OWNER_MEMORY, "overflow", "p");
        assert_eq!(s.len(), MAX_SESSION_SCOPES, "超容量不得挤掉别人");
        assert_eq!(s.get("overflow"), None);
        // 非法 id 的 clear_owned 也不得删到东西。
        assert!(!s.clear_owned(SESSION_PROMPT_OWNER_PERSONA, "a b"));
    }
}
