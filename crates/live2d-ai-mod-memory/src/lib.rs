//! live2d-ai-mod-memory（Wave 2 C 轨 2026-09-14 起草；Wave 3 C 轨 2026-09-14
//! 补质量基线 + 条数上限物理淘汰 + 可观察计数；L1 产品级 2026-09-15 补主动管理
//! 与会话分桶）——**本地记忆 v0**。
//!
//! # 文件长度（>500 行，理由）
//!
//! 本文件是 Mod 的**入口**：descriptor、运行时字段、被动一轮时序、两种注入落点、
//! 工厂与 `ModRuntime` 实现都在这里。命令面（`list/import/update/delete/clear`）
//! 已拆到 `commands.rs`、会话口径拆到 `sessions.rs`；再往下拆只会让「一轮发生
//! 了什么」跨三个文件才能读完——可读性优先，故按 ≤1000 行口径保留，理由在此。
//!
//! 一句话：把每一轮的用户输入记进本地 JSONL，并把**最相关的 top-k**
//! 注入 system_prompt 的一个带标记的块里。检索是**纯函数词元重叠**——没有向量库、
//! 没有 embedding、没有网络调用（见 [`strategy`]）。
//!
//! # 注入落到哪里（L1 产品级；P1-5 起删掉全局降级）
//!
//! - **有 conversation**（`on_scoped_event` 带了 session）：写
//!   [`ModServices::session_prompts`] 的**本 Mod 来源槽**
//!   （owner = `SESSION_PROMPT_OWNER_MEMORY`），**不碰**全局
//!   `persona.system_prompt`，也**不碰** persona 在会话里的角色卡
//!   ——否则 A 会话的记忆会串到 B 会话，或把别人的槽覆盖掉；
//! - **无 conversation**（裸 HTTP / 终端壳）：记忆仍落**老桶**
//!   `memory.jsonl`，但**不注入**——**绝不**再写全局
//!   `persona.system_prompt`（那是「两份真相」的来源，还会污染
//!   所有会话）。面板与日志如实说「无 conversation，本轮不注入」。
//!
//! 有 conversation 时在 **TurnPrompt（本轮请求体构建之前）** 写完，且 host 对该
//! 话题走同步投递（等 Mod worker 回执）——所以注入**本轮就带上**。
//!
//! # 注入预算与摘要钩子（P1-5）
//!
//! 注入块挂**字符预算**（`injection_budget_chars`，缺省 1600）：
//! - 占用 > `trim_ratio`（缺省 0.60）→ 按分数从**最低分**逐条裁
//!   （真裁条数）；
//! - **桶内原文总字符** > 注入预算 × `summary_ratio`（缺省 0.75）→
//!   **触发真摘要**（P1-5）：过冷却（`summary_cooldown_turns`）＋待压原文
//!   ≥ 2 条 ＋ 客户端可用 → **后台线程**跑一次 LLM（主链本轮不等它）；
//!   产出落旁车文件（`summary_store.rs`）并作为「摘要」进注入块；
//! - 摘要输入 = 该桶**尚未被摘要覆盖**的原文，且**保留最近
//!   `summary_keep_recent_turns`（缺省 4）轮原文**；
//! - 失败 ≡ 无摘要（旁车一字不改，只记 `state_json.summary.last_error`）；
//!   回滚 = 丢弃当前生效版、被它覆盖的原文重新可检索（`summary rollback` 命令）。
//!
//! # 注入点（钉死，不许自行改方案）
//!
//! 订阅**两个**基座主题：
//!
//! - [`ModEventTopic::TurnPrompt`]（payload = 本轮输入正文）：用户侧「记 +
//!   检索 + 注入」。该事件在 supervisor 提交 turn 时、**请求体构建之前**发出
//!   （见 `supervisor.rs` 的提交点），且 host 对它走**同步投递**——因此本
//!   Mod 在这里写的会话注入槽（MEMORY 块）**本轮请求体就带上**（2026-09-15 起）。
//!   历史：旧实现把它排在 supervisor 决议 system_prompt **之后**，注入只能等
//!   下一轮；现在改成「先投递、再决议」并用回执钉死时序。文档、REGISTER 与头注
//!   **不得**再写「只对下一轮生效」——那是假承诺：本轮请求体已经带着旧
//!   system_prompt 上路了。
//! - [`ModEventTopic::AssistantReplied`]（payload = JSON，见该主题头注）：
//!   **助手侧记**——轮末把「助手已说出 / 已上屏的正文」按 conversation 分桶
//!   追加。这一条是**非阻塞投递**（host 只对 TurnPrompt 走同步投递），慢 /
//!   失败的 Mod 不影响主链，处理失败只 warn。
//!
//! # 一轮的时序（实际发生的事）
//!
//! ```text
//! 用户输入 ──► supervisor 发 TurnStarted（turn id）
//!              └► supervisor 发 TurnPrompt（本轮正文, session）  ◄── memory 在这里干活
//!                   ├─ remember(正文, now, turn)
//!                   │    → 追加 <桶> 一行（有会话: sessions/<id>.memory.jsonl；
//!                   │      无会话: 老路径 memory.jsonl）
//!                   │    └─ 超过 max_records → 物理淘汰最旧（Wave 3）
//!                   ├─ 载入（窗口 max_records）+ 检索 top-k（排除刚写入的那条）
//!                   └─ 非空 → 有会话: session_prompts.set_owned(MEMORY, session, 本 Mod 的块)
//!                             无会话: 不注入（**绝不**写全局 persona.system_prompt）
//!              └► 构建本轮请求体（用的还是**旧** system_prompt）
//!              └► supervisor 决议 system_prompt（读到刚写的注入）→ 本轮请求体带上它
//! 助手说完 ──► supervisor 发 AssistantReplied（turn, 清洗后正文, session）
//!              └─ remember_assistant(正文, now, turn, session)
//!                   → 追加同一桶（role=assistant，与同轮用户记录共用 turn）
//! ```
//!
//! **无 conversation 时**：用户侧与助手侧都仍落老路径 `memory.jsonl`（只记），
//! 但都不注入、不写全局 persona、不摘要——这条对两侧完全对称。
//!
//! # 质量基线（Wave 3 C 轨，`src/quality_tests.rs` + `tests/fixtures/quality_corpus.json`）
//!
//! 固定语料 + 「查询 → 期望命中」表，至少覆盖：中文 bigram 命中、**词面不重叠
//! → 不命中 → 不注入**、同义改写不命中（v0 明文边界）、ASCII 大小写不敏感。
//! 质量口径与实现同源：断言直接打 [`strategy::rank_top_k`] 与运行时
//! `state_json.last_hits`，不靠手感。
//!
//! # 条数上限 = 物理淘汰（Wave 3 起，取舍见文档 §6.1）
//!
//! [`MemoryConfig::max_records`] 不再只是检索窗口：写入超过上限时
//! [`store::JsonlStore::append_capped`] 会**物理重写** JSONL，只留最新 N 条
//! （写同目录临时文件 + `rename` 原子替换）。**没有备份语义**——被淘汰的记录
//! 不可恢复；换取的是文件有界、检索/注入不随历史无限变慢。默认 200 条即上限，
//! 想要更长的历史就把 `max_records` 调大（最大 10000）。
//!
//! # 与 `persona` Mod 的边界（各写各的来源槽，读时组合）
//!
//! 会话表现在是「会话 id × 来源槽（owner）」，persona 与 memory 各占一格
//! （`SESSION_PROMPT_OWNER_PERSONA` / `SESSION_PROMPT_OWNER_MEMORY`），
//! 宿主按固定顺序把非空槽拼成最终 system_prompt（匿名 → persona → memory）。
//! 因此：
//!
//! - **全局（无会话）**：仍然都写 `persona.system_prompt`，谁后写谁覆盖
//!   ——这是既有口径，未改；
//! - **会话（有会话）**：persona 写角色卡、memory 写记忆块，**互不覆盖**；
//!   memory 每轮只重拼**自己的块**（先剥旧 marker 再拼，幂等），
//!   persona 的卡一个字不动；
//! - **停用各清自己的槽**：memory 走 `clear_owner(MEMORY)`，persona 走
//!   `clear_owner(PERSONA)`。**谁都不许用 `clear_all()`**——那会误伤对方
//!   （旧实现正是这样静默清掉了 persona 的会话卡）；
//! - **跨会话不串**：A 会话的写入只进 A 的槽，B 读不到。
//!
//! 全局路径的已知取舍（system 只有一个入口、没有优先级表）钉死在
//! `docs/architecture/memory-mod-v0.md` §5.1 与
//! `docs/plans/parallel-mods/REGISTER-memory-v0.md` §3.2，并有两向对称回归
//! （`tests.rs` 的 `last_writer_wins_*`）。
//!
//! # 启停（唯一真源 = Mod manifest 的 `enabled`）
//!
//! settings schema 里**没有**第二个 `enabled`——只有 `enabled_injection`
//! （「要不要往提示词里注入」，不改变「记忆照记」）。停用 Mod 时 `shutdown`
//! 剥掉全局注入块**并** `clear_owner(SESSION_PROMPT_OWNER_MEMORY)`：
//! 只清自己的槽，**不留残留也不误伤 persona**。
//!
//! # 会话分桶（L1 产品级，2026-09-15）
//!
//! - 被动路径用**最近一轮 `TurnPrompt` 的会话**选桶：
//!   `<老路径同目录>/sessions/<session>.memory.jsonl`；无会话 → **老路径**
//!   `memory.jsonl`（老记忆必须还能读到，这是兼容红线）；
//! - 命令（list/import/update/delete/clear）按自己的 `args.session_id` 选桶；
//!   没给 → 老路径全局桶（与面板 `activeSessionId == null` 的降级文案一致）；
//! - `state_json` 新增 `session_scoped` / `active_session` / `bucket_path`。
//!
//! 详细口径见 `sessions.rs` 头注与 `docs/architecture/memory-mod-v0.md` §13。
//!
//! # 产品级加强波次（版本仍 0.2.0-rc.3）
//!
//! - `state_json` 新增 `records`：记忆库**当前实际条数**（只读一次全量读，
//!   见 [`MemoryRuntime::record_count`]）；
//! - [`ModRuntime::command`] 的**主动管理面**（实现见 `commands.rs`）：
//!   `list`（最新在前、limit 钳位）/ `import` / `update` / `delete` / `clear`；
//!   每条记忆有稳定 `id`（[`MemoryRecord::make_id`]，旧行按同一公式派生）；
//!   `clear` 仍是 `JsonlStore::clear` **原子重写**（`.tmp` + `rename`），
//!   **不写** `persona.system_prompt`（残留按既有 `strip_residue` 生命周期处理）。
//!
//! # 失败纪律（与 persona 刻意不同）
//!
//! persona 是「接管主链人设」，坏配置 → `Failed`；memory 是**增量**能力：
//! 记忆写不进去、路径不可用、`apply_settings` 被拒，都只 **warn + 本轮 no-op**，
//! 绝不把一轮对话打挂、绝不 panic。坏行由 [`store`] 跳过并计数。Wave 3 起每条
//! 失败路径都进 `state_json.errors`，让「静默变少」可观察。
//!
//! # 非目标（v0/v1，明文）
//!
//! - **没有备份 / 撤销语义**：条数上限淘汰是物理删除，删了就没了（取舍见文档
//!   §6.1）；不提供导出、不做双写、不做软删除；
//! - 不做向量 / embedding / 语义检索 / 外部记忆服务；
//! - 不接管 LLM 客户端，不新增对话通道，不写 `persona` 之外的键；
//! - 不做跨会话去重 / 冲突消解（同一句话说两次就是两条记录）；摘要见
//!   [`summary`]（P1-5 / Wave 3 起含助手侧）；
//! - 不做 `persona` / memory 的仲裁优先级（见上）。
//!
//! [`ModServices::apply_settings`]: live2d_ai_mod_system::ModServices::apply_settings
//! [`ModServices::settings`]: live2d_ai_mod_system::ModServices::settings

pub mod config;
pub mod store;
pub mod strategy;
pub mod summary;
pub mod summary_http;
pub mod summary_store;

mod assistant;
mod commands;
mod sessions;
mod summary_flow;

pub use config::{MemoryConfig, memory_settings_spec};
pub use store::{AppendOutcome, JsonlStore, LoadOutcome};
pub use strategy::{
    BudgetedInjection, DEFAULT_INJECTION_BUDGET_CHARS, DEFAULT_MAX_RECORDS, DEFAULT_STORE_FILE,
    DEFAULT_SUMMARY_RATIO, DEFAULT_TOP_K, DEFAULT_TRIM_RATIO, Hit, MAX_INJECTION_BUDGET_CHARS,
    MAX_MAX_RECORDS, MAX_MEMORY_LINE_CHARS, MAX_TOP_K, MEMORY_MARKER_BEGIN, MEMORY_MARKER_END,
    MIN_INJECTION_BUDGET_CHARS, MIN_MAX_RECORDS, MIN_TOP_K, MemoryRecord, SESSIONS_DIR,
    SUMMARY_KEEP_RECENT_TURNS, budget_injection, fnv1a_hex8, resolve_session_store_path,
};
pub use summary::{DisabledSummarizer, Summarizer, SummaryConfig, SummarySetup};
pub use summary_store::{SummaryFile, SummaryStore, SummaryVersion};

use std::collections::BTreeSet;

use live2d_ai_mod_system::*;

use summary_flow::SummaryOutcome;

/// Mod 描述符（静态身份）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "memory",
    name: "本地记忆",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// 记忆 Mod 运行时。
pub struct MemoryRuntime {
    services: ModServices,
    config: MemoryConfig,
    registered: bool,
    /// 本 Mod 观察到的 `TurnPrompt` 次数（记忆的 `turn` 序号来源）。
    turn_seq: u64,
    /// 成功落盘的记忆条数（`state_json.writes`；`remembered` 是同值兼容别名）。
    /// Wave 3 起含助手侧记录（见 [Self::assistant_writes]）。
    writes: u64,
    /// 其中**助手侧**记录的条数（Wave 3，2026-09-21）；用户侧 = writes - 它。
    assistant_writes: u64,
    /// 累计检索到的记忆条数（跨轮累加；`last_hits` = 最近一轮）。
    hits: u64,
    /// 成功注入主链提示词的次数（`state_json.injects`；`injected` 是同值别名）。
    injects: u64,
    /// 失败路径计数：路径不可用 / 追加失败 / 读取失败 / `apply_settings` 被拒。
    /// **不含**坏行（坏行有自己的 warn 日志，见 [`store`]）。
    errors: u64,
    /// 被**物理淘汰**（从 JSONL 删除）的历史条数。
    evicted: u64,
    /// 最近一轮检索到几条（`state_json.last_hits`）。
    last_hits: usize,
    /// 本轮（最近一次 `TurnPrompt`）属于哪个会话；`None` = 裸 HTTP / 终端壳。
    ///
    /// 由 [`ModRuntime::on_scoped_event`] 在投递时记下，被动路径（记 / 检索 /
    /// 注入）据此选桶。命令**不**读它——命令按自己的 `args.session_id` 解析
    /// （见 `sessions.rs::command_session`），避免切会话瞬间错位。
    current_session: Option<String>,
    /// 最近一轮注入预算占用（字符；P1-5）。
    budget_used: usize,
    /// 最近一轮注入预算占用比例（used / budget）。
    budget_ratio: f64,
    /// 最近一轮被预算裁掉的条数。
    budget_dropped: usize,
    /// 历史累计被预算裁掉的条数。
    budget_dropped_total: u64,
    /// 最近一轮的摘要触发状态（越阈值 + 过冷却；= 已排队 / 正在跑）。
    summary_pending: bool,
    /// 距允许再次触发摘要还要多少轮（达到即 0）。
    summary_ready_at: u64,
    /// 累计触发真摘要次数（含后台失败的那几次）。
    summary_marks: u64,
    /// 因无 conversation 而跳过注入的轮数累计。
    no_conversation_turns: u64,
    /// 最近一轮桶内**全部原文**的字符数（摘要判据；见 `summary` 模块头注）。
    bucket_chars: usize,
    /// 最近一轮「桶内原文 / 注入预算」的比值（摘要判据与 state_json 共用）。
    bucket_ratio: f64,
    /// 最近一轮「尚未被摘要覆盖」的原文条数。
    summary_pending_records: usize,
    /// 摘要客户端（缺省 [summary::DisabledSummarizer]；工厂按配置注入）。
    summarizer: std::sync::Arc<dyn Summarizer>,
    /// 开了总闸但没接上时的可读原因（进 `state_json.summary.note`）。
    summary_note: Option<String>,
    /// 后台摘要线程的回执通道（`try_recv` 非阻塞；主链从不 `recv` 等它）。
    summary_rx: Option<std::sync::mpsc::Receiver<SummaryOutcome>>,
    /// 最近一次成功的摘要版本号（0 = 从未成功）。
    summary_version: u64,
    /// 最近一次失败的**可读原因**（成功后清空）。
    summary_last_error: Option<String>,
    /// 当前生效摘要的正文（只读缓存；注入与面板预览共用）。
    summary_text: Option<String>,
    /// 当前生效摘要覆盖到的原文条数（0 = 无摘要）。
    summary_covers_upto: usize,
    /// 加载旁车时**被物理淘汰**的头部条数（把位点换算成有效行号）。
    summary_evicted: usize,
    /// 缓存对应的桶路径（换会话 / 换桶时重读；见 `refresh_summary_state`）。
    summary_cache_key: Option<String>,
}

impl MemoryRuntime {
    /// 用注入的 host services + namespaced config 构造（不碰盘、不注册）。
    pub fn new(services: ModServices, config: MemoryConfig) -> Self {
        Self {
            services,
            config,
            registered: false,
            turn_seq: 0,
            writes: 0,
            assistant_writes: 0,
            hits: 0,
            injects: 0,
            errors: 0,
            evicted: 0,
            last_hits: 0,
            current_session: None,
            budget_used: 0,
            budget_ratio: 0.0,
            budget_dropped: 0,
            budget_dropped_total: 0,
            summary_pending: false,
            summary_ready_at: 0,
            summary_marks: 0,
            no_conversation_turns: 0,
            bucket_chars: 0,
            bucket_ratio: 0.0,
            summary_pending_records: 0,
            summarizer: std::sync::Arc::new(DisabledSummarizer),
            summary_note: None,
            summary_rx: None,
            summary_version: 0,
            summary_last_error: None,
            summary_text: None,
            summary_covers_upto: 0,
            summary_evicted: 0,
            summary_cache_key: None,
        }
    }

    /// builder：注入摘要客户端 + 可读的降级原因（工厂装配点；测试替身同一条路）。
    ///
    /// 缺省是 [DisabledSummarizer]——**显式注入才开通**，与
    /// `apply_settings` / `session_prompts` 同一条纪律：单测里静默跑起真
    /// HTTP 是最坏的一种假通过。
    pub fn with_summarizer(
        mut self,
        client: std::sync::Arc<dyn Summarizer>,
        note: Option<String>,
    ) -> Self {
        self.summarizer = client;
        self.summary_note = note;
        self
    }

    /// `start` 是否已成功（schema 已注册）。
    pub fn is_registered(&self) -> bool {
        self.registered
    }

    /// 当前生效配置。
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }

    /// 记住一条：追加到**当前生效桶**（会话桶或老路径）；超过 `max_records`
    /// 时**物理淘汰**最旧。
    ///
    /// 失败只 warn + 计入 `errors`——记忆是增量能力，写不进去不该打断这一轮
    /// （见模块头注「失败纪律」）。
    pub fn remember(&mut self, text: &str, ts: i64, turn: u64) -> bool {
        let session = self.current_session.clone();
        let record = MemoryRecord::new(text.trim(), ts, turn);
        self.append_record_in(session.as_deref(), record)
    }

    /// 检索 top-k（`strategy::rank_top_k` 的 IO + 坏行可见化包装）。
    ///
    /// `exclude_last` = 排除**桶里最后一条**（刚被 `remember` 追加的那条，
    /// 否则它必然以 1.0 的分自命中，白白占掉一个 k 的名额）。
    ///
    /// P1-5 起还多一层过滤：**已被当前摘要覆盖**的原文不进 top-k——它们已经以
    /// 「摘要」的身份进了注入块，再整条塞一遍就等于没有任何压缩（原文出两次、
    /// token 照吃）。摘要回滚后覆盖位点退回，这批原文**重新可被检索**。
    pub fn retrieve_with(&mut self, query: &str, exclude_last: bool) -> Vec<MemoryRecord> {
        let Some((records, offset)) = self.load_bucket() else {
            return Vec::new();
        };
        let start = self.effective_summary_start(records.len(), offset);
        let last = records.len().checked_sub(1);
        let covered: BTreeSet<usize> = (offset..start).collect();
        strategy::rank_top_k(query, &records, self.config.top_k)
            .into_iter()
            .filter(|hit| !(exclude_last && Some(hit.index) == last))
            .filter(|hit| !covered.contains(&(hit.index + offset)))
            .map(|hit| records[hit.index].clone())
            .collect()
    }

    /// 检索 top-k（不排除任何记录；对外的常规读取面）。
    pub fn retrieve(&mut self, query: &str) -> Vec<MemoryRecord> {
        self.retrieve_with(query, false)
    }

    /// 读整桶原文（**不设窗口**）+ 坏行可见化。
    ///
    /// 返回值第二项是**载入偏移**：`load(0)` 返回整桶，偏移恒 0；保留这个
    /// 形状是为了不让调用方假设「records[0] == 文件第 0 行」——摘要的位点是
    /// **文件行号**，读窗口一变位点就会错。路径不可解析 / IO 错 → `None`。
    fn load_bucket(&mut self) -> Option<(Vec<MemoryRecord>, usize)> {
        let store = self.effective_store()?;
        match store.load(0) {
            Ok(outcome) => {
                if outcome.bad_lines > 0 {
                    self.services.logger.warn(&format!(
                        "memory 跳过 {} 条坏记录（{}）",
                        outcome.bad_lines,
                        store.path().display()
                    ));
                }
                Some((outcome.records, 0))
            }
            Err(e) => {
                self.errors += 1;
                self.services.logger.warn(&format!(
                    "memory 读取记忆失败（{}）: {e}",
                    store.path().display()
                ));
                None
            }
        }
    }

    /// 当前生效摘要在这个桶里的**有效起点**（有效行号 = 未被摘要覆盖的第一条）。
    ///
    /// `offset` 是载入窗口的左移量（`load(N)` 时才有）；`len` 是本窗口条数。
    fn effective_summary_start(&self, len: usize, offset: usize) -> usize {
        let covers = self.summary_covers_upto;
        if covers == 0 {
            return 0;
        }
        summary::uncovered_start(covers, self.summary_evicted, len + offset)
            .saturating_sub(offset)
            .min(len)
    }

    /// 桶内「总字符数 + 待压条数」（摘要触发判据；一次全量读）。
    ///
    /// 返回 `(chars, pending_records)`；读不到 `(0, 0)`（不触发，也不谎报）。
    pub(crate) fn summary_bucket_stats(&mut self) -> (usize, usize) {
        let Some((records, offset)) = self.load_bucket() else {
            return (0, 0);
        };
        // 计的是**注入形态**（角色前缀 + 压平截断后的行），与注入预算同口径。
        let chars: usize = records.iter().map(|r| r.line().chars().count()).sum();
        let start = self.effective_summary_start(records.len(), offset);
        // Wave 3：保留窗口按 **turn 分组**（同轮的 user + assistant 一起留），
        // 不再是「最后 K 条」——助手记录入桶后两者不再等价。
        let end =
            strategy::recent_turn_window_start(&records, self.config.summary_keep_recent_turns);
        let pending = end.saturating_sub(start);
        (chars, pending)
    }

    /// 当前 JSONL 里的**有效记录条数**（`state_json.records` 的来源）。
    ///
    /// 只读纪律：走 [`JsonlStore::load`]（逐行读、不写盘、不创建文件）。
    /// 代价是**一次本地全量读**——与每轮 `append_capped` + `retrieve_with` 的
    /// 两次全量读是同一取舍（文档 §9 / §11.2）；`state_json` 的「不阻塞」指的是
    /// 不等待网络 / 锁，本地顺序读不在其列。
    ///
    /// 读不到（路径不可解析 / IO 错）→ `None`：面板显示「—」，与 `store_path`
    /// 为 null 同口径——「读不到」不能谎报成「0 条」。坏行不计入条数（它们不是记忆）。
    pub fn record_count(&mut self) -> Option<usize> {
        let store = self.effective_store()?;
        match store.load(0) {
            Ok(outcome) => Some(outcome.records.len()),
            Err(e) => {
                self.errors += 1;
                self.services.logger.warn(&format!(
                    "memory 统计记忆条数失败（{}）: {e}",
                    store.path().display()
                ));
                None
            }
        }
    }

    /// 当前主链 `persona.system_prompt`（脱敏设置快照里的那一份）。
    pub fn current_main_prompt(&self) -> String {
        self.services
            .settings
            .read()
            .get("persona")
            .and_then(|persona| persona.get("system_prompt"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    /// 剥掉本 Mod 注入过的块（停用时清残留）。返回是否真的改了主链提示词。
    pub fn strip_residue(&mut self) -> bool {
        let base = self.current_main_prompt();
        if !strategy::contains_memory_block(&base) {
            return false;
        }
        let stripped = strategy::strip_memory_block(&base);
        let applied = self
            .services
            .apply_settings
            .apply(serde_json::json!({"persona": {"system_prompt": stripped}}));
        if applied {
            self.services
                .logger
                .info("memory 已剥离注入块，主链提示词回到 base");
        } else {
            self.errors += 1;
            self.services
                .logger
                .warn("memory 剥离注入块失败（apply_settings 返回 false），提示词保留旧块");
        }
        applied
    }

    /// `TurnPrompt` 的一轮完整处理（见模块头注时序图）。
    ///
    /// # 会话注入（L1 产品级）
    ///
    /// - **有会话**：命中只写本 Mod 的来源槽
    ///   （`set_owned(SESSION_PROMPT_OWNER_MEMORY, session, 只有记忆块)`）；
    ///   没有命中 / 注入被关掉则 `clear_owned` 清掉该槽（幂等）。**绝不**写全局
    ///   `persona.system_prompt`，也**绝不**把 persona 的卡拼进来再写回
    ///   ——A 会话的记忆串到 B、或覆盖别人的槽，就是这么发生的；
    /// - **无会话**（裸 HTTP / 终端壳）：保持老路径 `apply_settings` 全局写回；
    ///   这是文档与面板都写明的**降级**，不是等价能力；
    /// - **本轮生效**：本函数在请求体构建之前跑完，且 host 对 TurnPrompt 同步投递
    ///   system_prompt 上路（§2 时序，未变）。
    fn handle_turn_prompt(&mut self, payload: &str) {
        let text = payload.trim();
        if text.is_empty() {
            self.services
                .logger
                .info("memory 收到空正文，本轮 no-op（不记不注入）");
            return;
        }
        self.turn_seq += 1;
        // P1-5：先收上一轮后台摘要的回执、再按当前桶刷新缓存——顺序不能反：
        // 缓存按桶路径失效，而「哪个桶」正是本轮会话决定的。
        self.drain_summary_results();
        self.refresh_summary_state();
        let (chars, pending) = self.summary_bucket_stats();
        self.bucket_chars = chars;
        self.summary_pending_records = pending;
        // 比值**无条件**算：它既是触发判据，也是 state_json 的可观察项
        // （缺省「摘要未启用」时同样要能看到桶已经涨到多少）。
        self.bucket_ratio = if self.config.injection_budget_chars == 0 {
            0.0
        } else {
            chars as f64 / self.config.injection_budget_chars as f64
        };
        if self.current_session.is_none() {
            // P1-5：无 conversation 的轮次**每轮都计数**（注入只在命中时发生，
            // 但「这一轮没有 conversation」是每轮都成立的事实）。
            self.no_conversation_turns += 1;
            self.services.logger.info(
                "memory 无 conversation：记忆写入老桶 memory.jsonl，本轮不注入（不写全局 persona.system_prompt）",
            );
        }
        let stored = self.remember(text, strategy::unix_now(), self.turn_seq);
        let hits = self.retrieve_with(text, stored);
        self.last_hits = hits.len();
        self.hits += hits.len() as u64;
        let summary_text = self.summary_text.clone();
        let injectable = summary_text.is_some() || !hits.is_empty();
        if !self.config.enabled_injection || !injectable {
            // 「没东西可注入」比「留上一轮的旧块」更正确：清掉本 Mod 的槽。
            // 幂等（本来没有就是 no-op），且只清自己的 owner。
            // 注意：**摘要也算「有东西可注入」**——只有摘要在手时绝不能清槽。
            if let Some(session) = self.current_session.clone() {
                self.clear_session_injection(&session);
            }
            if self.config.enabled_injection && hits.is_empty() && summary_text.is_none() {
                self.services
                    .logger
                    .info("memory 本轮没有相关记忆，已清掉本 Mod 的注入槽（no-op）");
            }
        } else {
            match self.current_session.clone() {
                Some(session) => self.inject_into_session(&session, summary_text.as_deref(), &hits),
                None => {
                    // P1-5：**删掉全局降级**——无 conversation 绝不写 persona.system_prompt
                    // （那是「两份真相」的来源，还会污染所有会话）。计数与说明在上面已做。
                }
            }
        }
        // 摘要触发放最后：先让本轮注入落地，再决定要不要起后台（顺序不影响本轮）。
        self.maybe_spawn_summary();
    }

    /// 清掉本 Mod 在某会话的注入槽（能力不可用 / 无命中时调用）。
    ///
    /// 只碰自己的 owner：persona 在同一会话里的角色卡不受影响。
    fn clear_session_injection(&mut self, session: &str) {
        if self.services.session_prompts.enabled() {
            self.services
                .session_prompts
                .clear_owned(SESSION_PROMPT_OWNER_MEMORY, session);
        }
    }

    /// 按会话注入：**只写本 Mod 的来源槽**（persona 的卡由宿主按固定顺序另拼）。
    ///
    /// `summary` 是当前生效摘要（无 → `None`）。五段上下文里的「摘要」段就落在
    /// 这里：**摘要在最前、原文命中在后**，同一个记忆槽（不新开 owner 槽——
    /// 那会让摘要块跑到 persona 之后、历史之前，与 §0 的段序对不上）。
    ///
    /// 注入内容为空 → 清槽（幂等），绝不写一个空的「记忆块」。
    fn inject_into_session(&mut self, session: &str, summary: Option<&str>, hits: &[MemoryRecord]) {
        if !self.services.session_prompts.enabled() {
            self.errors += 1;
            self.services.logger.warn(
                "memory 宿主未注入会话提示词能力，本轮会话记忆不注入（不写全局，避免串会话）",
            );
            return;
        }
        // Wave 3：每条带**角色前缀**（[用户]/[助手]）——注入格式区分角色，模型
        // 才知道哪句是自己说过的。line() 同时负责压平 + 截断。
        let memories: Vec<String> = hits.iter().map(MemoryRecord::line).collect();
        // P1-5：注入挂**预算**——占用 >trim_ratio 从最低分逐条裁。
        // 摘要行**必须先占位**：它压缩的是更早的历史，被顶掉就等于没摘要。
        //
        // 前缀由 summary::compose_lines **一处**加（它同时负责「摘要在最前」）。
        // 这里再自己加一遍会得到 `- [摘要] [摘要] ...`——2026-09-20 活服务冒烟
        // 当场抓到过（注入请求体），回归 `true_summary_...` 断言前缀只出现一次。
        let has_summary = summary.map(str::trim).is_some_and(|s| !s.is_empty());
        let budgeted = strategy::budget_injection(
            &summary::compose_lines(summary, &memories),
            self.config.injection_budget_chars,
            self.config.trim_ratio,
            self.config.summary_ratio,
        );
        self.budget_used = budgeted.used_chars;
        self.budget_ratio = budgeted.ratio;
        self.budget_dropped = budgeted.dropped;
        self.budget_dropped_total += budgeted.dropped as u64;
        if budgeted.topk_trimmed {
            self.services.logger.warn(&format!(
                "memory 注入预算 {} 字符：按分数裁掉 {} 条（本次注入 {} 条，摘要{}）",
                budgeted.budget_chars,
                budgeted.dropped,
                budgeted.lines.len(),
                if has_summary { "已占位" } else { "无" }
            ));
        }
        let block = budgeted.block();
        if block.trim().is_empty() {
            // 理论上 hits 非空且每行非空才会走到这；保守起见清槽而不是写空块。
            self.clear_session_injection(session);
            return;
        }
        // 幂等：槽里已经是同一份块 -> 不重复计数、不重复写（同一句话连说三轮
        // 不该计三次注入）。
        let already = self
            .services
            .session_prompts
            .contributions(session)
            .into_iter()
            .any(|(owner, text)| owner == SESSION_PROMPT_OWNER_MEMORY && text == block);
        if already {
            return;
        }
        self.services
            .session_prompts
            .set_owned(SESSION_PROMPT_OWNER_MEMORY, session, block);
        self.injects += 1;
        self.services.logger.info(&format!(
            "memory 已注入 {} 条记忆到会话 {session} 的提示词（本轮请求体已带上）",
            budgeted.lines.len()
        ));
    }
}

impl ModRuntime for MemoryRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        registrar.register_settings(memory_settings_spec())?;
        // 只订阅 TurnPrompt：TurnStarted 的 payload 只是序号，拿不到正文。
        registrar.subscribe(ModEventTopic::TurnPrompt)?;
        // Wave 3（2026-09-21）：助手侧正文（已在轮末、清洗后）。
        registrar.subscribe(ModEventTopic::AssistantReplied)?;
        self.registered = true;
        let where_to = match self.store() {
            Some(store) => store.path().display().to_string(),
            None => "未配置（store_path 空且无 config_path；记忆将 no-op）".to_string(),
        };
        self.services
            .logger
            .info(&format!("memory Mod 已启动（存储: {where_to}）"));
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        match topic {
            ModEventTopic::TurnPrompt => {
                self.handle_turn_prompt(payload);
                Ok(())
            }
            // unscoped 投递路径（host 走 on_scoped_event，这两条是 API 完整性 /
            // 单测直调）：会话回落到本轮 TurnPrompt 记下的 current_session。
            ModEventTopic::AssistantReplied => {
                let session = self.current_session.clone();
                self.handle_assistant_replied(payload, session.as_deref());
                Ok(())
            }
            other => {
                self.services
                    .logger
                    .info(&format!("memory 忽略 {} 事件", other.as_str()));
                Ok(())
            }
        }
    }

    /// **带会话事件**（L1 产品级）：TurnPrompt 时先记下本轮会话，再走既有
    /// [`Self::on_event`] 逻辑（语义不变——非 TurnPrompt 只是原样转发）。
    ///
    /// 会话 id 由宿主在投递前归一化（`sanitize_session_id`）；这里**再验一遍**
    /// 是因为它要进文件名。非法 id 退回 `None`（全局桶）+ warn，绝不拿它拼路径。
    fn on_scoped_event(
        &mut self,
        topic: ModEventTopic,
        payload: &str,
        session: Option<&str>,
    ) -> Result<(), ModError> {
        if topic == ModEventTopic::TurnPrompt {
            let normalized = session.and_then(live2d_ai_mod_system::sanitize_session_id);
            if session.is_some() && normalized.is_none() {
                self.services
                    .logger
                    .warn("memory 收到非法会话 id，本轮退回全局桶（不拿它拼路径）");
            }
            self.current_session = normalized;
        } else if topic == ModEventTopic::AssistantReplied {
            // 助手侧正文带**本轮**会话（host 与 TurnPrompt 投同一个 id）：
            // 直接用它选桶，不依赖 current_session（切会话瞬间也不会错位）。
            self.handle_assistant_replied(payload, session);
            return Ok(());
        }
        self.on_event(topic, payload)
    }

    /// 一次性命令（L1 产品级）：`list` / `import` / `update` / `delete` / `clear`。
    ///
    /// `args` 是 JSON 对象；每条的 args / 返回契约与失败文案见 `commands.rs`
    /// 头注。不认识命令一律回 [`ModError::UnsupportedCommand`] → host 409
    /// `unsupported_command`；执行失败用 [`ModError::Other`] → host 409
    /// `command_failed`；未启用 / worker 正持锁由 host 的 runtime 锁挡下 →
    /// 503 `command_unavailable`（可重试）。
    fn command(
        &mut self,
        command: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        self.dispatch_command(command, args)
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        // 不留残留：
        // 1) 全局注入块 → 按 marker 剥离写回（老路径）；
        // 2) 会话注入 → **只清本 Mod 的来源槽**（clear_owner(MEMORY)）。
        //
        // 为什么不能用 clear_all()：会话表是 persona 与本 Mod 共用的。clear_all()
        // 会把 persona 在同一会话里的角色卡一起清掉，而 persona 只在 start /
        // import_card 时写、不会自动重写 → 该会话静默回落全局人设。owner 槽就是
        // 为这条边界加的：每个 Mod 只清自己的格子。
        let cleaned = self.strip_residue();
        if self.services.session_prompts.enabled() {
            self.services
                .session_prompts
                .clear_owner(SESSION_PROMPT_OWNER_MEMORY);
        }
        self.current_session = None;
        self.registered = false;
        self.services.logger.info(&format!(
            "memory Mod 已关闭（全局注入块{}；本 Mod 的会话注入槽已清）",
            if cleaned {
                "已剥离"
            } else {
                "本就不存在"
            }
        ));
        Ok(())
    }

    /// 运行态快照（**只读、不写盘**）。
    ///
    /// `records` 需要一次本地全量读（见 [`Self::record_count`]），其余键仍是
    /// 纯内存计数。Wave 3 起必含四个计数键 `writes` / `hits` / `injects` /
    /// `errors`；`remembered` 与 `injected` 是 Wave 2 的兼容别名（与 `writes` /
    /// `injects` 恒同值，由同一字段派生，不会漂移）。
    ///
    /// L1 新增三个会话键（**现有键一个未删**）：
    /// - `session_scoped`：当前生效桶是不是会话桶（= 最近一轮带没带会话）；
    /// - `active_session`：最近一轮的会话 id（`null` = 裸 HTTP / 终端壳）；
    /// - `bucket_path`：当前生效桶的路径（会话桶或老路径；不可解析 → `null`）。
    ///   `store_path` 保持**全局（老路径）**含义不变，便于排障时区分两者。
    ///
    /// P1-5 新增 `summary` 对象（**只读内存缓存**，不碰盘）：
    /// `enabled / client / has_api_key / endpoint / note / idle / pending /`
    /// `version / covers_upto / evicted / pending_records / kept_recent /`
    /// `cooldown_left / marks / bucket_chars / bucket_ratio / last_error /`
    /// `text / sidecar`。旧的 `summary_pending / summary_marks / summary_*`
    /// 平铺键**一个未删**（兼容别名，同值派生）。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        let records = self.record_count();
        let active_session = self.current_session.clone();
        let conversation_id = active_session.clone();
        let bucket_path = self
            .effective_store()
            .map(|store| store.path().display().to_string());
        let summary = self.summary_state();
        Some(serde_json::json!({
            "store_path": self
                .config
                .resolve_store_path(&self.services.config_path)
                .map(|p| p.display().to_string()),
            "bucket_path": bucket_path,
            "session_scoped": active_session.is_some(),
            "active_session": active_session,
            "conversation_id": conversation_id,
            // P1-5：注入预算 / 裁剪 / 摘要钩子 / 无 conversation 计数。
            "injection_budget_chars": self.config.injection_budget_chars,
            "budget_used": self.budget_used,
            "budget_ratio": self.budget_ratio,
            "budget_dropped": self.budget_dropped,
            "budget_dropped_total": self.budget_dropped_total,
            "summary_pending": self.summary_pending,
            "summary_marks": self.summary_marks,
            "summary_cooldown_left": self.summary_ready_at.saturating_sub(self.turn_seq),
            "summary_keep_recent_turns": self.config.summary_keep_recent_turns,
            // P1-5：真摘要状态（见本函数头注的键表）。
            "summary": summary,
            "no_conversation_turns": self.no_conversation_turns,
            // 记忆库里**当前实际条数**（当前生效桶）：只读探测，读不到为 null（不是 0）。
            "records": records,
            "top_k": self.config.top_k,
            // Wave 3：max_records 现在同时是**条数上限（物理淘汰）**与检索窗口。
            "max_records": self.config.max_records,
            "enabled_injection": self.config.enabled_injection,
            "turns_seen": self.turn_seq,
            "writes": self.writes,
            // Wave 3：用户侧 / 助手侧的拆分（writes = user_writes + assistant_writes）。
            "user_writes": self.writes.saturating_sub(self.assistant_writes),
            "assistant_writes": self.assistant_writes,
            "hits": self.hits,
            "injects": self.injects,
            "errors": self.errors,
            "evicted": self.evicted,
            "last_hits": self.last_hits,
            "remembered": self.writes,
            "injected": self.injects,
        }))
    }
}

/// 静态工厂（注册进 `AVAILABLE_MOD_FACTORIES` 时用 `&FACTORY`）。
pub struct MemoryFactory;

impl ModFactory for MemoryFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// 未启用也能拿到表单（缺省停用的 Mod 要先让用户填路径再启用）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(memory_settings_spec())
    }

    /// 装配点：解析配置 → 按 `[summary]` 段装配摘要客户端（唯一装配处）。
    ///
    /// **摘要缺省关闭**：`summary_enabled=false`（或没配端点 / 模型）→
    /// [summary::DisabledSummarizer]，行为与「没有摘要能力」逐字一致；
    /// 开了总闸但没配齐 → 仍 Disabled + 可读 `degraded_note`
    /// （进 `state_json.summary.note`，不谎报「已启用」）。
    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        let config = MemoryConfig::from_value(&config);
        let setup = summary::assemble(&config.summary_config());
        Ok(Box::new(
            MemoryRuntime::new(services, config).with_summarizer(setup.client, setup.degraded_note),
        ))
    }
}

/// 工厂单例。
pub const FACTORY: MemoryFactory = MemoryFactory;

#[cfg(test)]
#[path = "assistant_tests.rs"]
mod assistant_tests;
#[cfg(test)]
#[path = "commands_tests.rs"]
mod commands_tests;
#[cfg(test)]
#[path = "quality_tests.rs"]
mod quality_tests;
#[cfg(test)]
#[path = "strategy_tests.rs"]
mod strategy_tests;
#[cfg(test)]
#[path = "summary_tests.rs"]
mod summary_tests;
#[cfg(test)]
#[path = "test_support.rs"]
mod test_support;
#[cfg(test)]
mod tests;
