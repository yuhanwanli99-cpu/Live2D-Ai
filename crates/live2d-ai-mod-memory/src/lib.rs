//! live2d-ai-mod-memory（Wave 2 C 轨 2026-09-14 起草；Wave 3 C 轨 2026-09-14
//! 补质量基线 + 条数上限物理淘汰 + 可观察计数）——**本地记忆 v0**。
//!
//! 一句话：把每一轮的用户输入记进本地 JSONL，并在下一轮把**最相关的 top-k**
//! 经 [`ModServices::apply_settings`] 注入主链 `persona.system_prompt` 的一个
//! 带标记的块里。检索是**纯函数词元重叠**——没有向量库、没有 embedding、
//! 没有网络调用（见 [`strategy`]）。
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
//!              └► supervisor 发 TurnPrompt（本轮正文）   ◄── memory 在这里干活
//!                   ├─ remember(正文, now, turn)          → 追加 memory.jsonl 一行
//!                   │    └─ 超过 max_records → 物理淘汰最旧（Wave 3）
//!                   ├─ 载入（窗口 max_records）+ 检索 top-k（排除刚写入的那条）
//!                   └─ 非空 → apply_settings               → 写 live2d-ai.toml + reload
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
//! # 与 `persona` Mod 的边界（last-writer-wins，无仲裁）
//!
//! `persona` 与本 Mod 都可能写 `persona.system_prompt`。本 Mod 的规则是：
//! **每次注入前先用固定 marker 剥掉旧块，再从 [`ModServices::settings`] 读到的
//! base 重拼**（[`strategy::compose_injection`]，幂等）。两者同时启用时：
//!
//! - `persona` 后写 → 它的合成结果覆盖整个 `system_prompt`，**记忆块被冲掉**；
//! - memory 后写（下一轮）→ 它在 persona 的合成结果之上重新拼上记忆块；
//! - 谁最后写谁赢，**没有仲裁、没有合并语义**。
//!
//! 这是**已知取舍**，不是 bug：主链只有一个 system 入口，加一套优先级表就是
//! 在核心里埋第二个产品。规则钉死在 `docs/architecture/memory-mod-v0.md` §5.1
//! 与 `docs/plans/parallel-mods/REGISTER-memory-v0.md` §3.2，并有两向对称回归
//! （`tests.rs` 的 `last_writer_wins_*`）。
//!
//! # 启停（唯一真源 = Mod manifest 的 `enabled`）
//!
//! settings schema 里**没有**第二个 `enabled`——只有 `enabled_injection`
//! （「要不要往提示词里注入」，不改变「记忆照记」）。停用 Mod 时 `shutdown`
//! 会检查当前提示词有没有本 Mod 的 marker，有就剥掉写回：**不留残留**。
//!
//! # 产品级加强波次（版本仍 0.2.0-rc.3）
//!
//! - `state_json` 新增 `records`：记忆库**当前实际条数**（只读一次全量读，
//!   见 [`MemoryRuntime::record_count`]）；
//! - 实现 [`ModRuntime::command`] 的 `clear`：`JsonlStore::clear` **原子重写**
//!   JSONL 为空（`.tmp` + `rename`，不是 `remove_file`），返回
//!   `{records, cleared, removed, residue}`；**不写** `persona.system_prompt`
//!   （残留按既有 `strip_residue` 生命周期处理，见文档 §5.2 / §9.1）。
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

pub use config::{MemoryConfig, memory_settings_spec};
pub use store::{AppendOutcome, JsonlStore, LoadOutcome};
pub use strategy::{
    DEFAULT_MAX_RECORDS, DEFAULT_STORE_FILE, DEFAULT_TOP_K, Hit, MAX_MAX_RECORDS,
    MAX_MEMORY_LINE_CHARS, MAX_TOP_K, MEMORY_MARKER_BEGIN, MEMORY_MARKER_END, MIN_MAX_RECORDS,
    MIN_TOP_K, MemoryRecord,
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

    /// 解析出的存储句柄；`None` = 路径未配置（调用方 no-op）。
    pub fn store(&self) -> Option<JsonlStore> {
        self.config
            .resolve_store_path(&self.services.config_path)
            .map(JsonlStore::new)
    }

    /// 记住一条：追加 JSONL 一行；超过 `max_records` 时**物理淘汰**最旧。
    ///
    /// 失败只 warn + 计入 `errors`——记忆是增量能力，写不进去不该打断这一轮
    /// （见模块头注「失败纪律」）。
    pub fn remember(&mut self, text: &str, ts: i64, turn: u64) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(store) = self.store() else {
            self.errors += 1;
            self.services.logger.warn(
                "memory 存储路径未配置（store_path 空且 host 未注入 config_path），本轮记忆被跳过",
            );
            return false;
        };
        let record = MemoryRecord {
            text: trimmed.to_string(),
            ts,
            turn,
        };
        match store.append_capped(&record, self.config.max_records) {
            Ok(outcome) => {
                self.writes += 1;
                if outcome.evicted > 0 {
                    self.evicted += outcome.evicted as u64;
                    self.services.logger.info(&format!(
                        "memory 超过条数上限 {}，物理淘汰最旧 {} 条（现保留 {} 条）",
                        self.config.max_records, outcome.evicted, outcome.kept
                    ));
                    if outcome.bad_lines > 0 {
                        self.services.logger.warn(&format!(
                            "memory 物理淘汰重写顺带清掉 {} 条坏记录",
                            outcome.bad_lines
                        ));
                    }
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
        let Some(store) = self.store() else {
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
        let store = self.store()?;
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

    /// `command("clear")` 的实现：**原子重写** JSONL 为空，并报告可观察结果。
    ///
    /// 语义边界（文档 §9.1）：
    /// - 只清 **JSONL 记忆库**，**不写** `persona.system_prompt`——注入残留按
    ///   crate 既有 `strip_residue` 生命周期处理（停用 Mod 时剥离；下一轮有非空
    ///   命中时 `compose_injection` 也会先剥旧再拼新），本命令**不**额外动手；
    /// - `records` 是**清空后的现存条数**（恒 0，与 `state_json.records` 同口径），
    ///   `removed` 是本次清掉的条数，`residue` 表示提示词里当前**是否还有**注入块
    ///   （只读探测，供面板如实提示）；
    /// - 累计计数 `writes` / `hits` / `injects` / `evicted` **不重置**：它们是
    ///   生命周期计数，`records` 才是「现在库里有多少」。
    fn clear_store(&mut self) -> Result<serde_json::Value, ModError> {
        let Some(store) = self.store() else {
            self.errors += 1;
            return Err(ModError::Other(
                "memory 存储路径未配置，无法清空（store_path 空且 host 未注入 config_path）"
                    .to_string(),
            ));
        };
        let removed = match store.load(0) {
            Ok(outcome) => outcome.records.len(),
            Err(e) => {
                self.errors += 1;
                return Err(ModError::Other(format!(
                    "清空前读取记忆库失败（{}）: {e}",
                    store.path().display()
                )));
            }
        };
        if let Err(e) = store.clear() {
            self.errors += 1;
            return Err(ModError::Other(format!(
                "清空记忆库失败（{}）: {e}",
                store.path().display()
            )));
        }
        let residue = strategy::contains_memory_block(&self.current_main_prompt());
        self.services.logger.info(&format!(
            "memory 已清空记忆库（清掉 {removed} 条，文件原子重写为空；提示词注入块{}）",
            if residue {
                "仍在（按既有 strip_residue 语义处理）"
            } else {
                "本就不存在"
            }
        ));
        Ok(serde_json::json!({
            "records": 0,
            "cleared": true,
            "removed": removed,
            "residue": residue,
        }))
    }

    /// `TurnPrompt` 的一轮完整处理（见模块头注时序图）。
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
        let Some(patch) = self.injection_patch(&hits) else {
            if self.config.enabled_injection && hits.is_empty() {
                self.services
                    .logger
                    .info("memory 本轮没有相关记忆，不注入（no-op）");
            }
            return;
        };
        if self.services.apply_settings.apply(patch) {
            self.injects += 1;
            self.services.logger.info(&format!(
                "memory 已注入 {} 条记忆到 persona.system_prompt（只对下一轮生效）",
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

    /// 一次性命令（产品级加强波次）：当前只认识 `clear`（清空记忆库）。
    ///
    /// `args` 目前**不解释**（保留给将来带条件的清空）。不认识命令一律回
    /// [`ModError::UnsupportedCommand`] → host 409 `unsupported_command`；执行
    /// 失败用 [`ModError::Other`] → host 409 `command_failed`；未启用 / worker
    /// 正持锁由 host 的 runtime 锁挡下 → 503 `command_unavailable`（可重试）。
    fn command(
        &mut self,
        command: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        let _ = args;
        match command {
            "clear" => self.clear_store(),
            other => Err(ModError::UnsupportedCommand {
                command: other.to_string(),
            }),
        }
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        // 不留残留：启用期间注入的块必须随停用一起消失（否则它会一直留在
        // live2d-ai.toml 里，连 Mod 都被禁用了还在影响提示词）。
        let cleaned = self.strip_residue();
        self.registered = false;
        self.services.logger.info(&format!(
            "memory Mod 已关闭（注入块{}）",
            if cleaned {
                "已剥离"
            } else {
                "本就不存在"
            }
        ));
        Ok(())
    }

    /// 运行态快照（**只读、不写盘**；产品级加强波次新增 `records`）。
    ///
    /// `records` 需要一次本地全量读（见 [`Self::record_count`]），其余键仍是
    /// 纯内存计数。Wave 3 起必含四个计数键 `writes` / `hits` / `injects` /
    /// `errors`；`remembered` 与 `injected` 是 Wave 2 的兼容别名（与 `writes` /
    /// `injects` 恒同值，由同一字段派生，不会漂移）。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        let records = self.record_count();
        Some(serde_json::json!({
            "store_path": self
                .config
                .resolve_store_path(&self.services.config_path)
                .map(|p| p.display().to_string()),
            // 记忆库里**当前实际条数**：只读探测，读不到为 null（不是 0）。
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
