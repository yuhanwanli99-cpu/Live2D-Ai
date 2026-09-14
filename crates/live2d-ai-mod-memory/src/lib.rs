//! live2d-ai-mod-memory（Wave 2 C 轨，2026-09-14）——**本地记忆 v0**。
//!
//! 一句话：把每一轮的用户输入记进本地 JSONL，并在下一轮把**最相关的 top-k**
//! 经 [`ModServices::apply_settings`] 注入主链 `persona.system_prompt` 的一个
//! 带标记的块里。检索是**纯函数词元重叠**——没有向量库、没有 embedding、
//! 没有网络调用（见 [`strategy`]）。
//!
//! # 注入点（钉死，不许自行改方案）
//!
//! 订阅基座新增的 [`ModEventTopic::TurnPrompt`]（payload = 本轮输入正文）。
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
//!                   ├─ 载入（窗口 max_records）+ 检索 top-k（排除刚写入的那条）
//!                   └─ 非空 → apply_settings               → 写 live2d-ai.toml + reload
//!              └► 构建本轮请求体（用的还是**旧** system_prompt）
//! 下一轮 ────► 构建请求体（这时才带上本轮注入的块）
//! ```
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
//! 在核心里埋第二个产品。要「两个都生效」得先论证仲裁规则（非目标，见
//! `docs/architecture/memory-mod-v0.md` §4）。
//!
//! # 启停（唯一真源 = Mod manifest 的 `enabled`）
//!
//! settings schema 里**没有**第二个 `enabled`——只有 `enabled_injection`
//! （「要不要往提示词里注入」，不改变「记忆照记」）。停用 Mod 时 `shutdown`
//! 会检查当前提示词有没有本 Mod 的 marker，有就剥掉写回：**不留残留**。
//!
//! # 失败纪律（与 persona 刻意不同）
//!
//! persona 是「接管主链人设」，坏配置 → `Failed`；memory 是**增量**能力：
//! 记忆写不进去、路径不可用、`apply_settings` 被拒，都只 **warn + 本轮 no-op**，
//! 绝不把一轮对话打挂、绝不 panic。坏行由 [`store`] 跳过并计数。
//!
//! # 非目标（v0，明文）
//!
//! - **不做物理 compaction**：`max_records` 只是**检索窗口**（载入时只留最新 N 条），
//!   JSONL 文件本身只增不删——删除/压缩留给 v1，且必须先有备份语义；
//! - 不做向量 / embedding / 语义检索 / 外部记忆服务；
//! - 不接管 LLM 客户端，不新增对话通道，不写 `persona` 之外的键；
//! - 不做跨会话去重 / 冲突消解 / 摘要（同一句话说两次就是两条记录）。
//!
//! [`ModServices::apply_settings`]: live2d_ai_mod_system::ModServices::apply_settings
//! [`ModServices::settings`]: live2d_ai_mod_system::ModServices::settings

pub mod store;
pub mod strategy;

pub use store::{JsonlStore, LoadOutcome};
pub use strategy::{
    DEFAULT_MAX_RECORDS, DEFAULT_STORE_FILE, DEFAULT_TOP_K, Hit, MAX_MAX_RECORDS,
    MAX_MEMORY_LINE_CHARS, MAX_TOP_K, MEMORY_MARKER_BEGIN, MEMORY_MARKER_END, MIN_MAX_RECORDS,
    MIN_TOP_K, MemoryRecord,
};

use std::path::PathBuf;

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "memory",
    name: "本地记忆",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// 本 Mod 的 namespaced 配置（缺省全部安全；**没有** `enabled` 键）。
#[derive(Debug, Clone, PartialEq)]
pub struct MemoryConfig {
    /// JSONL 路径；空 = 配置文件同目录的 [`DEFAULT_STORE_FILE`]。
    pub store_path: String,
    /// 每轮注入几条（钳在 [`MIN_TOP_K`]..=[`MAX_TOP_K`]）。
    pub top_k: usize,
    /// 检索窗口（钳在 [`MIN_MAX_RECORDS`]..=[`MAX_MAX_RECORDS`]）。
    pub max_records: usize,
    /// 是否注入（false = 只记不注入；缺省 true）。
    pub enabled_injection: bool,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            store_path: String::new(),
            top_k: DEFAULT_TOP_K,
            max_records: DEFAULT_MAX_RECORDS,
            enabled_injection: true,
        }
    }
}

impl MemoryConfig {
    /// 从 namespaced JSON 读配置：**越界只钳、类型不对只用缺省**，绝不失败。
    pub fn from_value(value: &serde_json::Value) -> Self {
        let store_path = value
            .get("store_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let number = |key: &str, default: usize, lo: usize, hi: usize| {
            value
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .filter(|f| f.is_finite())
                .map(|f| (f.round() as i64).clamp(lo as i64, hi as i64) as usize)
                .unwrap_or(default)
        };
        Self {
            store_path,
            top_k: number("top_k", DEFAULT_TOP_K, MIN_TOP_K, MAX_TOP_K),
            max_records: number(
                "max_records",
                DEFAULT_MAX_RECORDS,
                MIN_MAX_RECORDS,
                MAX_MAX_RECORDS,
            ),
            enabled_injection: value
                .get("enabled_injection")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
        }
    }

    /// 解析实际 JSONL 路径（[`strategy::resolve_store_path`] 的配置包装）。
    pub fn resolve_store_path(&self, config_path: &str) -> Option<PathBuf> {
        strategy::resolve_store_path(config_path, &self.store_path)
    }
}

/// 本 Mod 的设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
///
/// 四个字段，`enabled_injection` 是「注入开关」——**不是**「Mod 启停开关」。
pub fn memory_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::String {
                key: "store_path".to_string(),
                label: format!("记忆库路径（留空 = 配置文件同目录的 {DEFAULT_STORE_FILE}）"),
                secret: false,
            },
            ModSettingField::Number {
                key: "top_k".to_string(),
                label: format!("每轮注入条数（{MIN_TOP_K}–{MAX_TOP_K}，缺省 {DEFAULT_TOP_K}）"),
                min: MIN_TOP_K as f64,
                max: MAX_TOP_K as f64,
            },
            ModSettingField::Number {
                key: "max_records".to_string(),
                label: format!(
                    "检索窗口条数（{MIN_MAX_RECORDS}–{MAX_MAX_RECORDS}，缺省 {DEFAULT_MAX_RECORDS}）"
                ),
                min: MIN_MAX_RECORDS as f64,
                max: MAX_MAX_RECORDS as f64,
            },
            ModSettingField::Bool {
                key: "enabled_injection".to_string(),
                label: "把检索结果注入下一轮提示词（关掉则只记不注入）".to_string(),
                default: true,
            },
        ],
    }
}

/// 记忆 Mod 运行时。
pub struct MemoryRuntime {
    services: ModServices,
    config: MemoryConfig,
    registered: bool,
    /// 本 Mod 观察到的 `TurnPrompt` 次数（记忆的 `turn` 序号来源）。
    turn_seq: u64,
    remembered: u64,
    injected: u64,
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
            remembered: 0,
            injected: 0,
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

    /// 记住一条：追加 JSONL 一行。返回是否真的落盘。
    ///
    /// 失败只 warn——记忆是增量能力，写不进去不该打断这一轮（见模块头注
    /// 「失败纪律」）。
    pub fn remember(&mut self, text: &str, ts: i64, turn: u64) -> bool {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(store) = self.store() else {
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
        match store.append(&record) {
            Ok(()) => {
                self.remembered += 1;
                true
            }
            Err(e) => {
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
    pub fn retrieve_with(&self, query: &str, exclude_last: bool) -> Vec<MemoryRecord> {
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
                self.services.logger.warn(&format!(
                    "memory 读取记忆失败（{}）: {e}",
                    store.path().display()
                ));
                Vec::new()
            }
        }
    }

    /// 检索 top-k（不排除任何记录；对外的常规读取面）。
    pub fn retrieve(&self, query: &str) -> Vec<MemoryRecord> {
        self.retrieve_with(query, false)
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
            self.services
                .logger
                .warn("memory 剥离注入块失败（apply_settings 返回 false），提示词保留旧块");
        }
        applied
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
        let Some(patch) = self.injection_patch(&hits) else {
            if self.config.enabled_injection && hits.is_empty() {
                self.services
                    .logger
                    .info("memory 本轮没有相关记忆，不注入（no-op）");
            }
            return;
        };
        if self.services.apply_settings.apply(patch) {
            self.injected += 1;
            self.services.logger.info(&format!(
                "memory 已注入 {} 条记忆到 persona.system_prompt（只对下一轮生效）",
                hits.len()
            ));
        } else {
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

    /// 运行态快照（**纯内存计数**，不做磁盘 IO，符合基座对 `state_json` 的契约）。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "store_path": self
                .config
                .resolve_store_path(&self.services.config_path)
                .map(|p| p.display().to_string()),
            "top_k": self.config.top_k,
            "max_records": self.config.max_records,
            "enabled_injection": self.config.enabled_injection,
            "turns_seen": self.turn_seq,
            "remembered": self.remembered,
            "injected": self.injected,
            "last_hits": self.last_hits,
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
#[path = "strategy_tests.rs"]
mod strategy_tests;
#[cfg(test)]
mod tests;
