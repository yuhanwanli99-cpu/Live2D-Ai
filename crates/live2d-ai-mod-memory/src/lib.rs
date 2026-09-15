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
//! 一句话：把每一轮的用户输入记进本地 JSONL，并在下一轮把**最相关的 top-k**
//! 注入 system_prompt 的一个带标记的块里。检索是**纯函数词元重叠**——没有向量库、
//! 没有 embedding、没有网络调用（见 [`strategy`]）。
//!
//! # 注入落到哪里（L1 产品级，2026-09-15）
//!
//! - **有会话**（`on_scoped_event` 带了 session）：写
//!   [`ModServices::session_prompts`] 的**本 Mod 来源槽**
//!   （owner = `SESSION_PROMPT_OWNER_MEMORY`），**不碰**全局
//!   `persona.system_prompt`，也**不碰** persona 在会话里的角色卡
//!   ——否则 A 会话的记忆会串到 B 会话，或把别人的槽覆盖掉；
//! - **无会话**（裸 HTTP / 终端壳）：退回既有 [`ModServices::apply_settings`]
//!   写全局 `persona.system_prompt`。这是**降级**，面板与文档都写明。
//!
//! 两种落点**都只对下一轮生效**（时序见下），时序一个字没改。
//!
//! # 注入点（钉死，不许自行改方案）
//!
//! 订阅基座的 [`ModEventTopic::TurnPrompt`]（payload = 本轮输入正文）。
//! 该事件在 supervisor 提交 turn 时、**请求体构建之前**发出（见
//! `supervisor.rs` 的提交点），因此本 Mod 在这里做的 `apply_settings`
//! **只对下一轮生效**。本文档、REGISTER 与头注**不得**写「当轮生效」——
//! 那是假承诺：本轮请求体已经带着旧 system_prompt 上路了。
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
//!                             无会话: apply_settings → 写 live2d-ai.toml + reload
//!              └► 构建本轮请求体（用的还是**旧** system_prompt）
//! 下一轮 ────► 构建请求体（这时才带上本轮注入的块）
//! ```
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
//! - 不做跨会话去重 / 冲突消解 / 摘要（同一句话说两次就是两条记录）；
//! - 不做 `persona` / memory 的仲裁优先级（见上）。
//!
//! [`ModServices::apply_settings`]: live2d_ai_mod_system::ModServices::apply_settings
//! [`ModServices::settings`]: live2d_ai_mod_system::ModServices::settings

pub mod config;
pub mod store;
pub mod strategy;

mod commands;
mod sessions;

pub use config::{MemoryConfig, memory_settings_spec};
pub use store::{AppendOutcome, JsonlStore, LoadOutcome};
pub use strategy::{
    DEFAULT_MAX_RECORDS, DEFAULT_STORE_FILE, DEFAULT_TOP_K, Hit, MAX_MAX_RECORDS,
    MAX_MEMORY_LINE_CHARS, MAX_TOP_K, MEMORY_MARKER_BEGIN, MEMORY_MARKER_END, MIN_MAX_RECORDS,
    MIN_TOP_K, MemoryRecord, SESSIONS_DIR, fnv1a_hex8, resolve_session_store_path,
};

use live2d_ai_mod_system::*;

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
    writes: u64,
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
            hits: 0,
            injects: 0,
            errors: 0,
            evicted: 0,
            last_hits: 0,
            current_session: None,
        }
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
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(store) = self.effective_store() else {
            self.errors += 1;
            self.services.logger.warn(
                "memory 存储路径未配置（store_path 空且 host 未注入 config_path），本轮记忆被跳过",
            );
            return false;
        };
        let record = MemoryRecord::new(trimmed, ts, turn);
        match store.append_capped(&record, self.config.max_records) {
            Ok(outcome) => {
                self.writes += 1;
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

    /// 检索 top-k（`strategy::rank_top_k` 的 IO + 坏行可见化包装）。
    ///
    /// `exclude_last` = 排除载入结果的最后一条（刚被 `remember` 追加的那条，
    /// 否则它必然以 1.0 的分自命中，白白占掉一个 k 的名额）。
    pub fn retrieve_with(&mut self, query: &str, exclude_last: bool) -> Vec<MemoryRecord> {
        let Some(store) = self.effective_store() else {
            return Vec::new();
        };
        match store.load(self.config.max_records) {
            Ok(outcome) => {
                if outcome.bad_lines > 0 {
                    self.services.logger.warn(&format!(
                        "memory 跳过 {} 条坏记录（{}）",
                        outcome.bad_lines,
                        store.path().display()
                    ));
                }
                let last = outcome.records.len().checked_sub(1);
                strategy::rank_top_k(query, &outcome.records, self.config.top_k)
                    .into_iter()
                    .filter(|hit| !(exclude_last && Some(hit.index) == last))
                    .map(|hit| outcome.records[hit.index].clone())
                    .collect()
            }
            Err(e) => {
                self.errors += 1;
                self.services.logger.warn(&format!(
                    "memory 读取记忆失败（{}）: {e}",
                    store.path().display()
                ));
                Vec::new()
            }
        }
    }

    /// 检索 top-k（不排除任何记录；对外的常规读取面）。
    pub fn retrieve(&mut self, query: &str) -> Vec<MemoryRecord> {
        self.retrieve_with(query, false)
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

    /// 生成本轮注入 patch；`None` = **no-op（不写配置）**。
    ///
    /// no-op 的三种情形：注入被配置关掉、`hits` 为空（没有相关记忆）、
    /// 重拼结果与当前提示词逐字相同（幂等：没必要写盘 + reload）。
    pub fn injection_patch(&self, hits: &[MemoryRecord]) -> Option<serde_json::Value> {
        if !self.config.enabled_injection || hits.is_empty() {
            return None;
        }
        let base = self.current_main_prompt();
        let memories: Vec<String> = hits.iter().map(|r| r.text.clone()).collect();
        let composed = strategy::compose_injection(&base, &memories);
        if composed == base {
            return None;
        }
        Some(serde_json::json!({"persona": {"system_prompt": composed}}))
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
    /// - 仍然**只对下一轮生效**：本函数在请求体构建之前跑完，本轮请求已带着旧
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
        let stored = self.remember(text, strategy::unix_now(), self.turn_seq);
        let hits = self.retrieve_with(text, stored);
        self.last_hits = hits.len();
        self.hits += hits.len() as u64;
        if !self.config.enabled_injection || hits.is_empty() {
            // 「没东西可注入」比「留上一轮的旧块」更正确：清掉本 Mod 的槽。
            // 幂等（本来没有就是 no-op），且只清自己的 owner。
            if let Some(session) = self.current_session.clone() {
                self.clear_session_injection(&session);
            }
            if self.config.enabled_injection && hits.is_empty() {
                self.services
                    .logger
                    .info("memory 本轮没有相关记忆，已清掉本 Mod 的注入槽（no-op）");
            }
            return;
        }
        match self.current_session.clone() {
            Some(session) => self.inject_into_session(&session, &hits),
            None => self.inject_into_global(&hits),
        }
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
    /// 命中内容为空 → 清槽（幂等），绝不写一个空的「记忆块」。
    fn inject_into_session(&mut self, session: &str, hits: &[MemoryRecord]) {
        if !self.services.session_prompts.enabled() {
            self.errors += 1;
            self.services.logger.warn(
                "memory 宿主未注入会话提示词能力，本轮会话记忆不注入（不写全局，避免串会话）",
            );
            return;
        }
        let memories: Vec<String> = hits.iter().map(|r| r.text.clone()).collect();
        let block = strategy::compose_injection("", &memories);
        let block = block.trim();
        if block.is_empty() {
            // 理论上 hits 非空且每行非空才会走到这；保守起见清槽而不是写空块。
            self.clear_session_injection(session);
            return;
        }
        self.services
            .session_prompts
            .set_owned(SESSION_PROMPT_OWNER_MEMORY, session, block);
        self.injects += 1;
        self.services.logger.info(&format!(
            "memory 已注入 {} 条记忆到会话 {session} 的提示词（只对下一轮生效）",
            hits.len()
        ));
    }

    /// 无会话时的老路径：写全局 `persona.system_prompt`（文档 §5 的降级语义）。
    fn inject_into_global(&mut self, hits: &[MemoryRecord]) {
        let Some(patch) = self.injection_patch(hits) else {
            return;
        };
        if self.services.apply_settings.apply(patch) {
            self.injects += 1;
            self.services.logger.info(&format!(
                "memory 已注入 {} 条记忆到 persona.system_prompt（无会话，全局降级；只对下一轮生效）",
                hits.len()
            ));
        } else {
            self.errors += 1;
            self.services
                .logger
                .warn("memory 注入被 apply_settings 拒绝（配置不可写？），本轮 no-op");
        }
    }
}

impl ModRuntime for MemoryRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        registrar.register_settings(memory_settings_spec())?;
        // 只订阅 TurnPrompt：TurnStarted 的 payload 只是序号，拿不到正文。
        registrar.subscribe(ModEventTopic::TurnPrompt)?;
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
    fn state_json(&mut self) -> Option<serde_json::Value> {
        let records = self.record_count();
        let active_session = self.current_session.clone();
        let bucket_path = self
            .effective_store()
            .map(|store| store.path().display().to_string());
        Some(serde_json::json!({
            "store_path": self
                .config
                .resolve_store_path(&self.services.config_path)
                .map(|p| p.display().to_string()),
            "bucket_path": bucket_path,
            "session_scoped": active_session.is_some(),
            "active_session": active_session,
            // 记忆库里**当前实际条数**（当前生效桶）：只读探测，读不到为 null（不是 0）。
            "records": records,
            "top_k": self.config.top_k,
            // Wave 3：max_records 现在同时是**条数上限（物理淘汰）**与检索窗口。
            "max_records": self.config.max_records,
            "enabled_injection": self.config.enabled_injection,
            "turns_seen": self.turn_seq,
            "writes": self.writes,
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

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(MemoryRuntime::new(
            services,
            MemoryConfig::from_value(&config),
        )))
    }
}

/// 工厂单例。
pub const FACTORY: MemoryFactory = MemoryFactory;

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
#[path = "test_support.rs"]
mod test_support;
#[cfg(test)]
mod tests;
