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
//!
//! # baseline：prompt 的**兄弟字段**（V10 / O8）
//!
//! 「角色 baseline」= 该会话多角色扮演本身的**基础状态值**（开心 / 难过 /
//! 思考这类）。停止键 / 新用户消息 → 清动作 + 表情 + TTS 待播后**回该会话
//! 的 baseline**（协议 §9.3）。
//!
//! 存储口径（O8 冻结）：baseline 住在**同一张会话表**（同一个 [SessionScopeStore]、
//! 同一个 [Inner]、同一个会话键空间），与 [SessionScopeStore::prompt_for] 逐字
//! 共用同一道 [sanitize_session_id] 归一化闸。**不是**第二套会话表、**不加**新机制。
//!
//! - **缺省为空 = 待机**：没有写入过（或写了空串撤销）= 回待机，不伪造一个基准；
//! - **写入方 = 宿主 API**：POST /api/v1/chat/session 随会话设置一起写
//!   （见 web_api::chat_routes），本轮**不做** per-character profile 文件；
//! - baseline 是**不透明字符串**（与 prompts 同口径）：宿主只存 / 只回，
//!   「它是不是一个合法的表演 cue」由写入方负责。
//!
//! 为什么是兄弟**字段**而不是第二张表：会话 id 的归一化闸只有一道（路径穿越
//! 风险只在落盘方，而闸在 id 上），两张表就有两条 id 归一化路径、两条容量路径、
//! 两个「表里到底有没有这个会话」的答案。放在同一个 [Inner] 里，这些答案天然只有一个。

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

/// 单会话 baseline 的长度上限（**字符**；序列化后的 JSON 文本）。
///
/// 取值理由：baseline 是「基础状态值」——几条三字段 cue 的 JSON 在几十到几百
/// 字符量级；4000 与协议 O2 的 MAX_SEGMENT_CHARS 同量级，足以表达一个完整
/// 基准姿态，又能挡住「误把整份表演计划塞进 baseline」。超限**拒绝**（不静默
/// 截断——与 prompts 每条贡献超限的处理逐字一致）。
pub const MAX_SESSION_BASELINE_CHARS: usize = 4_000;

/// 某会话的槽表：owner → 文本。
type Slots = BTreeMap<String, String>;

#[derive(Default)]
struct Inner {
    /// 会话 id → (来源槽 id → 文本)（**已归一化**的 id 才可能在表里）。
    prompts: BTreeMap<String, Slots>,
    /// 会话 id → baseline（V10/O8 的**兄弟字段**：同一个键空间、同一道归一化闸）。
    ///
    /// 独立于 prompts 的**存在性**：一个会话可以只有人设没有 baseline，也可以
    /// 只有 baseline 没有人设；两者都用同一个 [sanitize_session_id] 归一化后的 id
    /// 当键。空串 = 撤销（缺省为空 = 待机），不留「空 baseline」条目。
    baselines: BTreeMap<String, String>,
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

    /// 当前会话数（**有任一 prompt 槽**的会话数；测试 / 状态展示用）。
    ///
    /// 注意：只数 **prompt 槽**——baseline 是兄弟字段，它的条数由
    /// [Self::baseline_count] 单独报（两半各自有界，合并计数会让「人设会话数」
    /// 这个面板数字随 baseline 写入而漂移）。
    pub fn len(&self) -> usize {
        self.guard().prompts.len()
    }

    /// 归一化 + 取某会话的**组合结果**；非法 id / 没有 → None。
    pub fn prompt_for(&self, session: Option<&str>) -> Option<String> {
        let id = session.and_then(sanitize_session_id)?;
        let inner = self.guard();
        compose_slots(inner.prompts.get(&id)?)
    }

    // ---------------------------------------------------------- baseline（V10/O8）
    //
    // 下面是 [prompt_for] 的**兄弟入口**：同一张表、同一个会话键、同一道
    // [sanitize_session_id] 闸、同一条「空串 = 撤销」纪律。不要在这里另起一套
    // 归一化或容量判定——那正是「第二套会话表」的开端。

    /// 写入 / 撤销某会话的 baseline（宿主 API 的唯一写入口，O8）。
    ///
    /// 返回是否被接受：
    /// - 非法会话 id → `false`（**与 prompts 同一道闸**，不特事特办）；
    /// - 超长（> [MAX_SESSION_BASELINE_CHARS]）→ `false`（**不静默截断**）；
    /// - 空串 → 撤销该会话的 baseline（缺省为空 = 待机），返回 `true`；
    /// - 新会话且 baseline 表已达 [MAX_SESSION_SCOPES] → `false`（不挤掉别人）。
    pub fn set_baseline(&self, session: &str, baseline: &str) -> bool {
        let Some(id) = sanitize_session_id(session) else {
            return false;
        };
        if baseline.chars().count() > MAX_SESSION_BASELINE_CHARS {
            return false;
        }
        let mut inner = self.guard();
        if baseline.is_empty() {
            // 空串 = 撤销（缺省为空 = 待机）；不留空条目，baseline_sessions() 才干净。
            inner.baselines.remove(&id);
            return true;
        }
        // 已有条目 = 覆盖（不受容量限制）；新条目才判容量（与 set_owned 同口径）。
        if !inner.baselines.contains_key(&id) && inner.baselines.len() >= MAX_SESSION_SCOPES {
            return false;
        }
        inner.baselines.insert(id, baseline.to_string());
        true
    }

    /// 取某会话的 baseline；**非法 id / 没有 → None**——与 [Self::prompt_for]
    /// 逐字同构（同一道归一化闸、同一个键空间）。
    pub fn baseline_for(&self, session: Option<&str>) -> Option<String> {
        let id = session.and_then(sanitize_session_id)?;
        self.guard().baselines.get(&id).cloned()
    }

    /// 撤销某会话的 baseline；返回是否真的删掉了（幂等）。
    pub fn clear_baseline(&self, session: &str) -> bool {
        let Some(id) = sanitize_session_id(session) else {
            return false;
        };
        self.guard().baselines.remove(&id).is_some()
    }

    /// 有 baseline 的会话数（状态面 / 测试；prompts 的计数另有 [Self::len]）。
    pub fn baseline_count(&self) -> usize {
        self.guard().baselines.len()
    }

    /// 有 baseline 的会话 id（BTreeMap 迭代顺序天然稳定）。
    pub fn baseline_sessions(&self) -> Vec<String> {
        self.guard().baselines.keys().cloned().collect()
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
        // 「清空全部会话作用域」= prompts 与 baseline 一起清（两半都是会话作用域，
        // 只清一半会留下「表看着空了、停止仍回旧基准」的鬼状态）。
        let mut inner = self.guard();
        inner.prompts.clear();
        inner.baselines.clear();
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
            .field("baselines", &inner.baselines.len())
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

    // ------------------------------------------------ baseline（V10/O8 兄弟字段）

    /// **E1（阶段4e 判据）**：baseline 绑**会话**，不是全局。
    ///
    /// 语义（协议 §9.1 / O8）：会话 A / B 各持自己的 baseline；没写入过的会话
    /// 回 None（= 待机）；写 A 不动 B；清 B 不动 A。同一道 sanitize 闸拒绝非法 id。
    #[test]
    fn baseline_is_bound_per_session_and_not_global() {
        let s = SessionScopeStore::new();
        // 缺省 = 待机（不是某个隐式全局 baseline）。
        assert_eq!(s.baseline_for(Some("A")), None);
        assert_eq!(s.baseline_for(None), None);

        let a = r#"{"field":"expression","id":"smile","intensity":1,"at":"now","hold":true}"#;
        let b = r#"{"field":"body","x":-0.2,"y":0.1,"intensity":2,"at":"now","hold":true}"#;
        assert!(s.set_baseline("A", a));
        assert!(s.set_baseline("B", b));

        // 各回各的：**不是**全局（写 B 不许改 A 的值）。
        assert_eq!(s.baseline_for(Some("A")).as_deref(), Some(a));
        assert_eq!(s.baseline_for(Some("B")).as_deref(), Some(b));
        assert_ne!(s.baseline_for(Some("A")), s.baseline_for(Some("B")));
        // 没写入过的会话仍是待机，绝不回落到「别人的 baseline」。
        assert_eq!(s.baseline_for(Some("C")), None);
        // None 桶（裸 HTTP / 不带会话）与任何具名会话都不共享。
        assert_eq!(s.baseline_for(None), None);

        // 清 B 不动 A。
        assert!(s.clear_baseline("B"));
        assert_eq!(s.baseline_for(Some("B")), None);
        assert_eq!(s.baseline_for(Some("A")).as_deref(), Some(a));

        // 同一道归一化闸：非法 id 写入被拒、读取不得到东西。
        assert!(!s.set_baseline("../etc/passwd", a));
        assert_eq!(s.baseline_for(Some("../etc/passwd")), None);
        assert!(!s.set_baseline("a b", a));
        // 归一化后合法（前后空白被 trim）——与 prompt_for 同一条闸。
        assert!(s.set_baseline("  spaced-id  ", a));
        assert_eq!(s.baseline_for(Some("spaced-id")).as_deref(), Some(a));

        // baseline 是**兄弟字段**：会话数（prompt 槽计数）不因 baseline 漂移。
        assert_eq!(s.len(), 0, "只写 baseline 不得伪造出 prompt 会话");
        assert_eq!(s.baseline_count(), 2, "A + spaced-id");
        assert_eq!(
            s.baseline_sessions(),
            vec!["A".to_string(), "spaced-id".to_string()],
            "稳定排序"
        );
    }

    /// baseline 的容量 / 超长 / 空串撤销三条边界（与 prompts 同纪律）。
    #[test]
    fn baseline_write_gate_rejects_overlong_and_empty_revokes() {
        let s = SessionScopeStore::new();
        // 超长 = 拒绝（不静默截断）。
        assert!(!s.set_baseline("A", &"x".repeat(MAX_SESSION_BASELINE_CHARS + 1)));
        assert_eq!(s.baseline_for(Some("A")), None);
        // 正好到上限要能进。
        assert!(s.set_baseline("A", &"x".repeat(MAX_SESSION_BASELINE_CHARS)));
        assert!(s.baseline_for(Some("A")).is_some());
        // 空串 = 撤销（缺省为空 = 待机），不留空条目。
        assert!(s.set_baseline("A", ""));
        assert_eq!(s.baseline_for(Some("A")), None);
        assert_eq!(s.baseline_count(), 0);
        // 重复撤销是幂等 no-op（仍返回 true：写请求被接受）。
        assert!(s.set_baseline("A", ""));
        // 容量有界、不挤掉别人。
        for i in 0..MAX_SESSION_SCOPES {
            assert!(s.set_baseline(&format!("s{i}"), "b"));
        }
        assert_eq!(s.baseline_count(), MAX_SESSION_SCOPES);
        assert!(!s.set_baseline("overflow", "b"));
        assert_eq!(s.baseline_count(), MAX_SESSION_SCOPES);
        // 已有条目仍可覆盖。
        assert!(s.set_baseline("s0", "更新后"));
        assert_eq!(s.baseline_for(Some("s0")).as_deref(), Some("更新后"));
    }

    /// clear_all 把两半（prompts + baseline）一起清；active 游标不跟着清。
    #[test]
    fn clear_all_also_clears_baselines() {
        let s = SessionScopeStore::new();
        s.set("A", "人设");
        s.set_baseline("A", "基准");
        s.set_active(Some("A"));
        s.clear_all();
        assert_eq!(s.len(), 0);
        assert_eq!(s.baseline_count(), 0);
        assert_eq!(s.baseline_for(Some("A")), None, "清空后不许再回旧基准");
        assert_eq!(s.active().as_deref(), Some("A"), "游标是位置，不是内容");
    }

    /// baseline 与 prompt 同表但互不污染：清人设不动 baseline，反之亦然。
    #[test]
    fn baseline_and_prompt_do_not_overwrite_each_other() {
        let s = SessionScopeStore::new();
        s.set("A", "人设");
        s.set_baseline("A", "基准");
        assert!(s.clear("A"), "清 prompt 槽");
        assert_eq!(s.get("A"), None);
        assert_eq!(s.baseline_for(Some("A")).as_deref(), Some("基准"));
        s.set("A", "又写回来");
        assert_eq!(
            s.baseline_for(Some("A")).as_deref(),
            Some("基准"),
            "写人设不动基准"
        );
        assert!(s.clear_baseline("A"));
        assert_eq!(s.get("A").as_deref(), Some("又写回来"), "清基准不动人设");
        // 只有 baseline 的会话同样计入 baseline_sessions（同一个键空间）。
        s.set("B", "人设B");
        s.set_baseline("B", "基准B");
        assert_eq!(s.sessions(), vec!["A".to_string(), "B".to_string()]);
    }
}
