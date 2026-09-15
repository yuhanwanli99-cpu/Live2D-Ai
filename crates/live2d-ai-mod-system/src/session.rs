//! 会话级作用域（L1 产品级基座，2026-09-15）。
//!
//! # 为什么需要它
//!
//! L1 验收要求 **persona 绑定当前会话**（会话 A/B 不串卡、停用还原），
//! memory 也要**尽量**按会话分桶。而在此之前，产品的「人设」只有**一处**全局
//! 入口：live2d-ai.toml 的 [persona] system_prompt（进程级、单例）。
//! persona Mod 只能整段覆写它 —— 那就是「全局人设」，不是「会话人设」。
//!
//! 本模块补的是**最小宿主能力**：一张「会话 id × 来源槽（owner）→ system_prompt」
//! 的表，外加「当前活动会话」这一个游标。它**不落 live2d-ai.toml**、不触发
//! supervisor.reload()，因此不会与既有「last-writer-wins」的全局写回打架。
//!
//! # 为什么是「多来源槽」而不是「一个会话一个字符串」
//!
//! 最初这张表是「会话 id → 一个字符串」：读时原样返回，写时整段覆盖
//! （last-writer-wins）。多个来源同时用它就会互相误伤——L1 实测到的缺陷是：
//!
//! - persona 导入卡 → 把整段人设写进会话 A；
//! - memory 检索注入 → 读「当前值」当 base、拼上记忆块，**再整段写回**；
//! - memory 停用 → 调用 clear_all() → **把 persona 在会话 A 的卡一起清掉**。
//!   persona 只在 start / import_card 时写，不会自动重写
//!   → 该会话静默回落到全局人设（用户看到「导入过的卡自己没了」）。
//!
//! 根因是「一张表只有一个归属」。现在每个来源（owner）有**自己的槽**：
//! 写入只覆盖自己的槽，清除只清自己的槽，读时按固定顺序把非空槽拼起来。
//! 于是「memory 停用」不再误伤 persona；persona 重写也不会永久冲掉记忆块
//! （记忆块下一轮由 memory 自己刷新）。
//!
//! # 组合顺序（固定，不许各写各的）
//!
//! 匿名槽（空串，遗留 [SessionPromptSink::set] 用）→ persona → memory
//! → 其余 owner 按字典序。顺序写死在 [owner_merge_rank]，宿主实现与测试
//! 替身共用同一份，用两个换行连接。全空 / 无槽 → None（调用方回落全局）。
//! 顺序是契约的一部分：persona 是「你是谁」、memory 是「刚才聊过什么」，
//! 人设必须排在记忆之前，否则记忆块会把角色卡挤出模型注意力。
//!
//! # 一条纪律：谁清谁自己的槽
//!
//! 每个 Mod **只允许清自己的 owner**（persona → [SESSION_PROMPT_OWNER_PERSONA]，
//! memory → [SESSION_PROMPT_OWNER_MEMORY]）。clear_all() 是遗留 / 测试用的
//! 「清掉一切」；一个 Mod 在生产路径上用它，就是去清别人的槽——本次修复的缺陷
//! 正是它。停用语义因此是 clear_owner(自己的 owner)，不是 clear_all()。
//!
//! # 边界（说清楚不做什么）
//!
//! - **不持久化**：本表只活在进程内存里。谁要「跨重启还在」，谁自己把内容落盘
//!   （persona Mod 就是这么做的：persona-mod-cards.json），启动时再灌回来。
//!   宿主只保证「同一进程内、按会话取到不同的 system_prompt」。
//! - **不做内容合并**：宿主只按来源槽拼接，不解释优先级、不比对内容。同一槽内
//!   仍是 last-writer-wins（一个来源在同一个会话里只能有一份文本）。宿主**不**
//!   提供跨槽的优先级表——那等于在核心里埋第二个人设来源。
//! - **不解释内容**：宿主只存字符串。合法性（比如「是不是一张角色卡」）由写入方
//!   （Mod）负责。
//!
//! # 降级语义（当前产品只有一个会话时）
//!
//! 前端（Flutter）本来就有多会话（ChatSessionStore，上限 50），所以「会话 id」
//! 是既存事实。但如果调用方**不带**会话 id（裸 POST /api/v1/chat、终端壳、
//! 未接会话的第三方客户端），宿主按 **None 桶**处理：走全局
//! persona.system_prompt，一条会话覆盖都不生效。这个降级必须在 UI 里写明
//! （见 persona 面板），不能假装「已经绑好了」。
//!
//! # 存储键的可扩展性
//!
//! 会话 id 的**归一化规则**钉在 [sanitize_session_id]：调用方给出的 id 必须
//! 通过它才能进表。这是一条**未来兼容闸**——将来若把会话表落盘成
//! <session>.jsonl / <session>.json（memory Mod 已经这么做），id 里出现
//! / 或 .. 或控制字符就会变成路径穿越。规则现在就定死，落盘方直接复用。

use std::sync::Arc;

/// 会话 id 的最大字符数（**字符**，不是字节）。
///
/// 取值理由：Flutter 的会话 id 是「微秒时间戳-序号」（约 20 字符）；这里给
/// 128 是为了容纳将来「外部集成方自带 id」的场景，同时仍是一条能对用户解释的界。
pub const MAX_SESSION_ID_CHARS: usize = 128;

/// 来源槽 id：persona 的角色卡。
pub const SESSION_PROMPT_OWNER_PERSONA: &str = "persona";
/// 来源槽 id：memory 的检索注入块。
pub const SESSION_PROMPT_OWNER_MEMORY: &str = "memory";
/// 匿名槽（未声明 owner 的写入，遗留 [SessionPromptSink::set] 用）。
/// 组合时排在最前。
pub const SESSION_PROMPT_OWNER_DEFAULT: &str = "";

/// 归一化一个会话 id；**不合法返回 None**（调用方必须当「没有会话」处理）。
///
/// 规则（刻意从紧）：
/// - trim 之后非空、长度不超过 [MAX_SESSION_ID_CHARS]（按字符数）；
/// - 只允许 ASCII 字母数字与减号 / 下划线 / 点 / 冒号。
///
/// 为什么不容忍其它字符：会话 id 会被**用作文件名 / 存储键的一部分**
/// （memory 的 sessions/<id>.memory.jsonl）。等真的落盘了再补闸，
/// 就是在已经存在路径穿越风险的代码上事后打补丁。
pub fn sanitize_session_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_SESSION_ID_CHARS {
        return None;
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        return None;
    }
    Some(trimmed.to_string())
}

/// 组合顺序的排序键：匿名槽最前，persona 次之，memory 再次，
/// 其余 owner 归为一组并按其字典序排列。
///
/// 宿主实现与测试替身**都必须**用它，否则「宿主里一个顺序、单测里另一个顺序」
/// 会让组合结果变成两份真相。
pub fn owner_merge_rank(owner: &str) -> (u8, &str) {
    match owner {
        SESSION_PROMPT_OWNER_DEFAULT => (0, ""),
        SESSION_PROMPT_OWNER_PERSONA => (1, ""),
        SESSION_PROMPT_OWNER_MEMORY => (2, ""),
        other => (3, other),
    }
}

/// 把「(owner, 文本)」列表按 [owner_merge_rank] 组合成最终 system_prompt。
///
/// 空文本的槽被跳过；**全空 / 无槽 → None**（调用方回落全局提示词）。
/// 连接符固定为两个换行。
pub fn compose_session_prompt<'a, I>(contributions: I) -> Option<String>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let mut parts: Vec<(&'a str, (u8, &'a str))> = contributions
        .into_iter()
        .filter(|(_, text)| !text.is_empty())
        .map(|(owner, text)| (text, owner_merge_rank(owner)))
        .collect();
    if parts.is_empty() {
        return None;
    }
    parts.sort_by(|a, b| a.1.cmp(&b.1));
    Some(
        parts
            .iter()
            .map(|(text, _)| *text)
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

/// 宿主侧的**会话级 system_prompt 覆盖**能力（注入给 Mod）。
///
/// Mod 拿到它之后就能做「按会话写人设」，而不必去写全局
/// persona.system_prompt（那会把 A 会话的卡泄漏到 B 会话）。
///
/// 所有方法都必须**非阻塞**：写方在 Mod worker 线程上，读方在 supervisor 线程
/// / web_api 线程上。实现里只允许一把短锁 + BTreeMap 操作，不得做 IO。
///
/// # 来源槽（owner）
///
/// [SessionPromptSink::set_owned] / [SessionPromptSink::clear_owned] /
/// [SessionPromptSink::clear_owner] 是**产品路径**（persona / memory 各自只碰
/// 自己的槽）。[SessionPromptSink::set] / [SessionPromptSink::clear] /
/// [SessionPromptSink::clear_all] 是**遗留 / 测试用**：
/// set 写匿名槽，clear 清该会话所有槽，clear_all 清一切。
pub trait SessionPromptSink: Send + Sync {
    /// 该实现是否真的可用（默认 true）。
    ///
    /// 宿主没有注入实现时用 [NoSessionPrompts]（返回 false）。Mod 据此
    /// 决定要不要退回全局写回 / 在 UI 上说「本环境不支持会话绑定」。
    fn enabled(&self) -> bool {
        true
    }

    /// 取某会话的**组合结果**：各槽非空文本按 [owner_merge_rank] 顺序用两个
    /// 换行连接；全空 / 无槽 → None（调用方回落到全局）。
    fn get(&self, session: &str) -> Option<String>;

    /// 按**来源槽**写入 / 覆盖某会话的提示词（owner 为空 = 匿名槽）。
    ///
    /// 默认实现退回 [SessionPromptSink::set]（老 fake 因此不必改）：
    /// set 在 trait 里**没有**默认实现，所以 set_owned → set 是单向的，
    /// 两者不会互相递归。
    fn set_owned(&self, owner: &str, session: &str, prompt: &str) {
        let _ = owner;
        self.set(session, prompt);
    }

    /// 只清掉**某个来源槽**在某会话里的贡献；返回是否真的删掉了东西。
    ///
    /// 默认 false（老 fake 没有槽的概念）。
    fn clear_owned(&self, _owner: &str, _session: &str) -> bool {
        false
    }

    /// 清掉某来源槽在**所有会话**里的贡献。
    ///
    /// 默认 no-op（老 fake 没有槽的概念）。
    fn clear_owner(&self, _owner: &str) {}

    /// 某会话里各来源槽的快照（按组合顺序；诊断 / 状态用）。
    ///
    /// 默认实现把 [SessionPromptSink::get] 当作匿名槽一条返回。
    fn contributions(&self, session: &str) -> Vec<(String, String)> {
        self.get(session)
            .map(|text| vec![(String::new(), text)])
            .unwrap_or_default()
    }

    /// **遗留**：写入 / 覆盖某会话的匿名槽。session 必须已通过
    /// [sanitize_session_id]；非法 id 由实现**静默忽略**（调用方应自己先归一化
    /// 并在返回值里体现）。
    fn set(&self, session: &str, prompt: &str);

    /// **遗留**：清掉某会话的**所有**槽；返回是否真的删掉了东西。
    fn clear(&self, session: &str) -> bool;

    /// **遗留 / 测试用**：清掉**全部**会话覆盖（一切 owner）。
    ///
    /// 生产路径不要用它——一个 Mod 停用时应该只清
    /// [SessionPromptSink::clear_owner] 自己的槽。
    fn clear_all(&self);

    /// 当前有覆盖的会话 id 列表（**已排序**，便于测试与展示稳定）。
    fn sessions(&self) -> Vec<String>;

    /// 当前活动会话（宿主记录「最近一次对话用的是哪个会话」）。
    ///
    /// 面板用它显示「这张卡将绑定到哪个会话」；Mod 也用它给 import_card
    /// 定默认目标。
    fn active(&self) -> Option<String>;

    /// 设置当前活动会话（None = 清掉）。
    fn set_active(&self, session: Option<&str>);
}

/// 「没有会话能力」的空实现（单测 / 无 supervisor 环境）。
///
/// 与 [crate::ModSettingsApplier] 的默认实现同一条纪律：
/// **默认拒绝 / 默认不可用**，显式注入才开通。这样「单测里静默写进了一张
/// 不存在的会话表」这种假通过不会发生。
pub struct NoSessionPrompts;

impl SessionPromptSink for NoSessionPrompts {
    fn enabled(&self) -> bool {
        false
    }
    fn get(&self, _session: &str) -> Option<String> {
        None
    }
    fn set(&self, _session: &str, _prompt: &str) {}
    fn clear(&self, _session: &str) -> bool {
        false
    }
    fn clear_all(&self) {}
    fn sessions(&self) -> Vec<String> {
        Vec::new()
    }
    fn active(&self) -> Option<String> {
        None
    }
    fn set_active(&self, _session: Option<&str>) {}
}

/// 注入给 Mod 的会话能力句柄（薄包装，便于 ModServices 克隆）。
#[derive(Clone)]
pub struct ModSessionPrompts {
    inner: Arc<dyn SessionPromptSink>,
}

impl ModSessionPrompts {
    /// 用宿主实现构造。
    pub fn new(sink: impl SessionPromptSink + 'static) -> Self {
        Self {
            inner: Arc::new(sink),
        }
    }

    /// 空实现（默认）——见 [NoSessionPrompts]。
    pub fn disabled() -> Self {
        Self {
            inner: Arc::new(NoSessionPrompts),
        }
    }

    /// 见 [SessionPromptSink::enabled]。
    pub fn enabled(&self) -> bool {
        self.inner.enabled()
    }
    /// 见 [SessionPromptSink::get]。
    pub fn get(&self, session: &str) -> Option<String> {
        self.inner.get(session)
    }
    /// 见 [SessionPromptSink::set_owned]。
    pub fn set_owned(&self, owner: &str, session: &str, prompt: impl Into<String>) {
        self.inner.set_owned(owner, session, &prompt.into());
    }
    /// 见 [SessionPromptSink::clear_owned]。
    pub fn clear_owned(&self, owner: &str, session: &str) -> bool {
        self.inner.clear_owned(owner, session)
    }
    /// 见 [SessionPromptSink::clear_owner]。
    pub fn clear_owner(&self, owner: &str) {
        self.inner.clear_owner(owner);
    }
    /// 见 [SessionPromptSink::contributions]。
    pub fn contributions(&self, session: &str) -> Vec<(String, String)> {
        self.inner.contributions(session)
    }
    /// 见 [SessionPromptSink::set]。
    pub fn set(&self, session: &str, prompt: impl Into<String>) {
        self.inner.set(session, &prompt.into());
    }
    /// 见 [SessionPromptSink::clear]。
    pub fn clear(&self, session: &str) -> bool {
        self.inner.clear(session)
    }
    /// 见 [SessionPromptSink::clear_all]。
    pub fn clear_all(&self) {
        self.inner.clear_all();
    }
    /// 见 [SessionPromptSink::sessions]。
    pub fn sessions(&self) -> Vec<String> {
        self.inner.sessions()
    }
    /// 见 [SessionPromptSink::active]。
    pub fn active(&self) -> Option<String> {
        self.inner.active()
    }
    /// 见 [SessionPromptSink::set_active]。
    pub fn set_active(&self, session: Option<&str>) {
        self.inner.set_active(session);
    }
}

impl std::fmt::Debug for ModSessionPrompts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModSessionPrompts")
            .field("enabled", &self.enabled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[test]
    fn sanitize_accepts_the_frontend_id_shape() {
        // Flutter 的 id：<微秒时间戳>-<同刻序号>
        assert_eq!(
            sanitize_session_id("1757890123456789-3").as_deref(),
            Some("1757890123456789-3")
        );
        assert_eq!(
            sanitize_session_id("  abc.DEF_1:x ").as_deref(),
            Some("abc.DEF_1:x")
        );
    }

    #[test]
    fn sanitize_rejects_empty_overlong_and_path_like_ids() {
        assert_eq!(sanitize_session_id(""), None);
        assert_eq!(sanitize_session_id("   "), None);
        assert_eq!(
            sanitize_session_id(&"a".repeat(MAX_SESSION_ID_CHARS + 1)),
            None
        );
        // 路径穿越 / 分隔符一律拒——它们将来会变成文件名。
        for bad in ["a/b", "../x", "a b", "会话", "a.b/../c"] {
            assert_eq!(sanitize_session_id(bad), None, "{bad:?} 必须被拒");
        }
        assert!(sanitize_session_id(&"a".repeat(MAX_SESSION_ID_CHARS)).is_some());
    }

    #[test]
    fn disabled_sink_is_inert() {
        let p = ModSessionPrompts::disabled();
        assert!(!p.enabled());
        p.set("s1", "人设");
        assert_eq!(p.get("s1"), None);
        assert!(p.sessions().is_empty());
        assert!(!p.clear("s1"));
        p.set_owned(SESSION_PROMPT_OWNER_PERSONA, "s1", "人设");
        assert!(!p.clear_owned(SESSION_PROMPT_OWNER_PERSONA, "s1"));
        p.clear_owner(SESSION_PROMPT_OWNER_PERSONA);
        assert!(p.contributions("s1").is_empty());
        p.set_active(Some("s1"));
        assert_eq!(p.active(), None);
    }

    #[test]
    fn merge_order_is_anonymous_then_persona_then_memory() {
        let out = compose_session_prompt([
            (SESSION_PROMPT_OWNER_MEMORY, "记忆"),
            (SESSION_PROMPT_OWNER_DEFAULT, "匿名"),
            (SESSION_PROMPT_OWNER_PERSONA, "人设"),
        ])
        .expect("三条非空贡献必须有组合结果");
        assert_eq!(out, "匿名\n\n人设\n\n记忆");
        // 未知 owner 排在固定三个之后，且彼此按字典序。
        let out = compose_session_prompt([
            ("zzz", "Z"),
            ("aaa", "A"),
            (SESSION_PROMPT_OWNER_PERSONA, "人设"),
        ])
        .unwrap();
        assert_eq!(out, "人设\n\nA\n\nZ");
    }

    #[test]
    fn compose_skips_empty_slots_and_returns_none_when_all_empty() {
        assert_eq!(compose_session_prompt(Vec::<(&str, &str)>::new()), None);
        assert_eq!(
            compose_session_prompt([(SESSION_PROMPT_OWNER_PERSONA, "")]),
            None
        );
        assert_eq!(
            compose_session_prompt([
                (SESSION_PROMPT_OWNER_PERSONA, ""),
                (SESSION_PROMPT_OWNER_MEMORY, "记忆"),
            ])
            .as_deref(),
            Some("记忆")
        );
    }

    /// 老 fake 只实现必需方法；set_owned 默认退回 set，不得递归。
    #[derive(Default)]
    struct LegacySink {
        map: Mutex<BTreeMap<String, String>>,
    }

    impl SessionPromptSink for LegacySink {
        fn get(&self, session: &str) -> Option<String> {
            self.map.lock().unwrap().get(session).cloned()
        }
        fn set(&self, session: &str, prompt: &str) {
            self.map
                .lock()
                .unwrap()
                .insert(session.to_string(), prompt.to_string());
        }
        fn clear(&self, session: &str) -> bool {
            self.map.lock().unwrap().remove(session).is_some()
        }
        fn clear_all(&self) {
            self.map.lock().unwrap().clear();
        }
        fn sessions(&self) -> Vec<String> {
            self.map.lock().unwrap().keys().cloned().collect()
        }
        fn active(&self) -> Option<String> {
            None
        }
        fn set_active(&self, _session: Option<&str>) {}
    }

    #[test]
    fn legacy_sink_defaults_keep_working() {
        let sink = LegacySink::default();
        // 默认 set_owned → set。
        sink.set_owned(SESSION_PROMPT_OWNER_PERSONA, "A", "人设");
        assert_eq!(sink.get("A").as_deref(), Some("人设"));
        // 默认 clear_owned / clear_owner 是保守的 no-op。
        assert!(!sink.clear_owned(SESSION_PROMPT_OWNER_PERSONA, "A"));
        sink.clear_owner(SESSION_PROMPT_OWNER_PERSONA);
        assert_eq!(sink.get("A").as_deref(), Some("人设"));
        // 默认 contributions 把 get 当匿名槽一条。
        assert_eq!(
            sink.contributions("A"),
            vec![(String::new(), "人设".to_string())]
        );
    }
}
