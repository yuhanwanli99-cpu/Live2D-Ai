//! `PersonaRuntime`：主链接管 / 还原的应用面（自 `lib.rs` 拆出）。
//!
//! 输入有两个：卡解析（`card`）与静态 settings schema（`factory`）。
//! 「**没真的接管，就不许留下痕迹**」这条不变量体现在 `apply_card` 与基线快照里。
//!
//! `command` / `factory` 要用运行态的私有字段、方法与 `PersonaConfig`，故按
//! `pub(super)` 开放；对外仍只有 `PersonaRuntime`（`pub`）与 `ModRuntime` 实现。

use std::path::{Path, PathBuf};

use super::card::{
    AppliedPersona, CardSource, PersonaCard, check_card_json_size, compose_system_prompt,
    read_card_file,
};
use super::factory::persona_settings_spec;
use super::*;

// ------------------------------------------------------------------ Runtime

/// 本 Mod 的 namespaced 配置（缺省全部安全）。
///
/// **刻意不读 `enabled`**：Mod 的启停只由 manifest 表达（见模块头注）。
/// 混进 config 的 `enabled` 键是**惰性**的，不参与任何判断。
#[derive(Debug, Clone)]
pub(super) struct PersonaConfig {
    pub(super) card_path: String,
    pub(super) card_json: String,
    pub(super) include_discipline: bool,
    pub(super) say_first_mes: bool,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) personality: String,
    pub(super) scenario: String,
}

impl PersonaConfig {
    pub(super) fn from_value(v: &serde_json::Value) -> Self {
        let s = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let b = |k: &str, d: bool| v.get(k).and_then(|x| x.as_bool()).unwrap_or(d);
        Self {
            card_path: s("card_path"),
            card_json: s("card_json"),
            include_discipline: b("include_discipline", true),
            say_first_mes: b("say_first_mes", false),
            name: s("name"),
            description: s("description"),
            personality: s("personality"),
            scenario: s("scenario"),
        }
    }

    /// 手工覆盖是否至少有一项非空。
    pub(super) fn has_overrides(&self) -> bool {
        [
            &self.name,
            &self.description,
            &self.personality,
            &self.scenario,
        ]
        .iter()
        .any(|s| !s.is_empty())
    }
}

/// 角色卡 Mod 运行时。
pub struct PersonaRuntime {
    pub(super) services: ModServices,
    pub(super) config: PersonaConfig,
    registered: bool,
    /// 主链原本的 `system_prompt`（停用时写回；见模块头注）。
    pub(super) base_prompt: Option<String>,
    /// 最近一次成功接管的摘要（`state_json` 的**零 IO** 数据源）。
    pub(super) applied: Option<AppliedPersona>,
    /// **会话级角色卡档案**（L1 会话绑定）。`None` = 还没载入 / 本环境没有
    /// `config_path`（此时会话绑定不可用，带 `session_id` 的 `import_card` 会
    /// 明确报错，**不静默**退回全局）。载入时机见 [`Self::load_sessions`]。
    pub(super) session_cards: Option<sessions::SessionArchive>,
}

impl PersonaRuntime {
    pub(super) fn new(services: ModServices, config: PersonaConfig) -> Self {
        Self {
            services,
            config,
            registered: false,
            base_prompt: None,
            applied: None,
            session_cards: None,
        }
    }

    /// `start` 时载入会话档案（坏文件只 warn 忽略，不让整个 Mod Failed；
    /// 见 `src/sessions.rs` 头注）。
    fn load_sessions(&mut self) {
        sessions::load_archive(&self.services, &mut self.session_cards);
    }

    /// 命令通道取档案：不可用 → **可读错误**（不静默降级成全局）；两条不可用
    /// 路径与首次载入的语义见 [`sessions::require_archive`]。
    pub(crate) fn require_session_archive(
        &mut self,
    ) -> Result<&mut sessions::SessionArchive, ModError> {
        sessions::require_archive(&self.services, &mut self.session_cards)
    }

    /// 导入卡文件路径（与 `live2d-ai.toml` 同目录）。
    ///
    /// host 没注入 `config_path`（单测 / 无 supervisor 环境）→ `None`：
    /// 此时**没有**导入卡这条来源，一切按 config 的老路径走。
    pub(super) fn imported_card_path(&self) -> Option<PathBuf> {
        let config_path = self.services.config_path.trim();
        if config_path.is_empty() {
            return None;
        }
        Path::new(config_path)
            .parent()
            .map(|parent| parent.join(IMPORTED_CARD_FILE))
    }

    /// 载入卡：**导入卡优先** > `card_json` > `card_path`；都空 → `Ok(None)`。
    ///
    /// 第二项是来源（进 `state_json.card_source` 与日志）。
    /// `Err` = **用户配了东西但它是坏的**（导入卡损坏 / 读不到 / 不是卡 / 超大）；
    /// `Ok(None)` = 用户根本没配 —— 两者结局不同，绝不能混成一个「空卡」。
    pub(super) fn load_card(&self) -> Result<Option<(PersonaCard, CardSource)>, String> {
        if let Some(path) = self.imported_card_path() {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    if bytes.len() as u64 > MAX_CARD_FILE_BYTES {
                        return Err(format!(
                            "导入卡文件过大（{}）：{} 字节，上限 {} 字节；请清除导入卡后重新导入",
                            path.display(),
                            bytes.len(),
                            MAX_CARD_FILE_BYTES
                        ));
                    }
                    let card = PersonaCard::parse_bytes(&bytes).map_err(|e| {
                        format!(
                            "导入卡文件已损坏（{}）：{e}；请重新导入或点「清除导入卡」",
                            path.display()
                        )
                    })?;
                    return Ok(Some((card, CardSource::Imported)));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(format!("读取导入卡文件失败（{}）: {e}", path.display()));
                }
            }
        }
        let json = self.config.card_json.trim();
        if !json.is_empty() {
            check_card_json_size(json)?;
            let card = PersonaCard::parse_json(json).ok_or_else(|| {
                "card_json 不是可识别的角色卡 JSON（需要 V1 扁平对象，或 V2 带 `data` 的对象）"
                    .to_string()
            })?;
            return Ok(Some((card, CardSource::ConfigJson)));
        }
        let path = self.config.card_path.trim();
        if !path.is_empty() {
            let bytes = read_card_file(path)?;
            let card = PersonaCard::parse_bytes(&bytes)?;
            return Ok(Some((card, CardSource::ConfigPath)));
        }
        Ok(None)
    }

    /// 手工覆盖：非空配置项优先于卡字段。
    pub(super) fn with_overrides(&self, mut card: PersonaCard) -> PersonaCard {
        for (dst, src) in [
            (&mut card.name, &self.config.name),
            (&mut card.description, &self.config.description),
            (&mut card.personality, &self.config.personality),
            (&mut card.scenario, &self.config.scenario),
        ] {
            if !src.is_empty() {
                *dst = src.clone();
            }
        }
        card
    }

    /// 基线快照路径（与 `live2d-ai.toml` 同目录）。
    fn base_state_path(&self) -> Option<PathBuf> {
        let config_path = self.services.config_path.trim();
        if config_path.is_empty() {
            return None;
        }
        let parent = Path::new(config_path).parent()?;
        Some(parent.join(BASE_STATE_FILE))
    }

    /// 当前主链 `system_prompt`（脱敏设置快照里的那一份）。
    fn current_main_prompt(&self) -> String {
        self.services
            .settings
            .read()
            .get("persona")
            .and_then(|p| p.get("system_prompt"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// 取得基线：内存里已有 → 直接用；磁盘上有快照 → **以快照为准**
    /// （多次启停都回到同一条基线）；都没有 → 拿当前主链提示词当基线。
    ///
    /// 这里**只读不写**：落盘交给 [`Self::persist_base_prompt`]，且必须在
    /// `apply_settings` 成功之后 —— 没真的接管主链就不许在磁盘上留基线。
    pub(super) fn ensure_base_prompt(&mut self) -> String {
        if let Some(base) = &self.base_prompt {
            return base.clone();
        }
        if let Some(path) = self.base_state_path() {
            match std::fs::read_to_string(&path) {
                Ok(saved) => {
                    self.base_prompt = Some(saved.clone());
                    return saved;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => self.services.logger.warn(&format!(
                    "persona 基线快照读取失败（{}）: {e}；本次以当前主链提示词为基线",
                    path.display()
                )),
            }
        }
        let current = self.current_main_prompt();
        self.base_prompt = Some(current.clone());
        current
    }

    /// 把基线落到 Mod 状态文件（**只在成功写回主链之后**调用；已有快照不覆盖）。
    fn persist_base_prompt(&self) {
        let Some(base) = &self.base_prompt else {
            return;
        };
        let Some(path) = self.base_state_path() else {
            return;
        };
        if path.exists() {
            return; // 已有基线：启停以它为锚，不被本次产物污染。
        }
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            self.services
                .logger
                .warn(&format!("persona 基线快照目录创建失败: {e}"));
            return;
        }
        if let Err(e) = std::fs::write(&path, base) {
            self.services
                .logger
                .warn(&format!("persona 基线快照写入失败: {e}"));
        }
    }

    /// 把一张卡合成 `system_prompt` 并写回主链，记录接管摘要。
    ///
    /// - `Ok(())`：已写入；
    /// - `Err(e)`：合成结果为空 / 写盘被拒。调用方（[`ModRuntime::start`]）把它
    ///   变成 `ModStatus::Failed`，导入路径（`src/command.rs`）则**回滚导入卡**：
    ///   两条路都不许出现「界面说好了、主链其实没动」。
    ///
    /// 写盘在**所有校验之后**：失败时主链 `system_prompt` 一个字都不动。
    pub(super) fn apply_card(
        &mut self,
        card: &PersonaCard,
        source: CardSource,
    ) -> Result<(), String> {
        let composed = compose_system_prompt(card, self.config.include_discipline);
        if composed.trim().is_empty() {
            return Err(
                "角色卡没有产生任何可写入的人设文本（字段全空，且未启用对话纪律模板）".to_string(),
            );
        }
        let base = self.ensure_base_prompt();
        let ok = self
            .services
            .apply_settings
            .apply(serde_json::json!({"persona": {"system_prompt": composed}}));
        if !ok {
            return Err(format!(
                "apply_settings 写回主链失败（{} 不可写？），system_prompt 未改变；原基线 {base:?}",
                self.services.config_path.trim()
            ));
        }
        self.persist_base_prompt();
        let chars = composed.chars().count();
        self.applied = Some(AppliedPersona {
            source: source.as_str(),
            name: card.name.clone(),
            // 只有覆盖项、没有卡时 `format` 是空串 —— 报 `manual` 而不是空值，
            // 「（空）」在界面上读起来像坏了。
            format: if card.format.is_empty() {
                "manual"
            } else {
                card.format
            },
            chars,
        });
        self.services.logger.info(&format!(
            "persona 已写入主链 system_prompt（来源 {}，{} 字）",
            source.as_str(),
            chars
        ));
        if self.config.say_first_mes && !card.first.is_empty() {
            self.services.say_tx.say(card.first.clone());
        }
        Ok(())
    }

    /// 按 config（含导入卡）决定要接管哪张卡，并写回主链。
    ///
    /// - `Ok(())`：已写入，或**本来就没配卡**（合法 no-op：先启用、后填卡）；
    /// - `Err(e)`：**配置坏了**（读不到 / 不是卡 / 超大 / 合成结果为空 / 写盘被拒）。
    fn apply_from_config(&mut self) -> Result<(), String> {
        let (card, source) = match self.load_card()? {
            Some(pair) => pair,
            None if self.config.has_overrides() => (PersonaCard::default(), CardSource::Overrides),
            None => {
                self.services.logger.info(
                    "persona 未配置角色卡（card_path / card_json / 导入卡均空、也无覆盖项），保持主链现有提示词",
                );
                self.applied = None;
                return Ok(());
            }
        };
        let card = self.with_overrides(card);
        self.apply_card(&card, source)
    }
}

impl ModRuntime for PersonaRuntime {
    /// 启动 = 校验配置 → 写回主链 → 注册 schema。
    ///
    /// 顺序是有意的：**坏配置不得留下半个副作用**（不注册、不写盘、不落基线快照）。
    /// 坏配置 → `Err(ModError::Init)` → host 置 `ModStatus::Failed` 并停用本实例
    /// （失败隔离见 `docs/architecture/mod-product-chain.md` §7）；**不要**退化成
    /// 「记一行 error 然后 Ok」——那会让界面显示「运行中」而主链其实没接管（自检说谎）。
    /// 静态 schema 仍由 `factory.settings_spec()` 提供，所以失败后前端照样拿得到
    /// 表单去修配置。
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        if let Err(e) = self.apply_from_config() {
            self.services
                .logger
                .error(&format!("persona 配置无效：{e}"));
            return Err(ModError::Init {
                mod_id: DESCRIPTOR.id.to_string(),
                message: e,
            });
        }
        // 会话档案：坏文件只 warn 忽略（见 `src/sessions.rs` 头注）——它不该让
        // 整个 Mod Failed，更不该带走已经成功的全局接管。载入后把每个会话的卡
        // **灌回宿主会话表**：宿主那张表是内存态，「重启后回到会话 A 人设还在」
        // 靠的就是这一步。
        self.load_sessions();
        if let Some(archive) = &self.session_cards {
            archive.restore(
                &self.services.session_prompts,
                self.config.include_discipline,
                &self.services.logger,
            );
        }
        if let Err(e) = registrar.register_settings(persona_settings_spec()) {
            // 注册失败也要**回滚**：已经写回主链的提示词不能留在那儿，
            // 否则一个 Failed 的 Mod 却在主链上留了痕——半个副作用比不接管更坏。
            let _ = self.shutdown();
            return Err(e);
        }
        self.registered = true;
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        self.services
            .logger
            .info(&format!("persona 收到 {}: {payload}", topic.as_str()));
        Ok(())
    }

    /// **一次性命令**（产品级加强波次）：转发给 `src/command.rs` 的
    /// [`command::dispatch`]（两条命令与失败语义见那里的头注）。
    fn command(
        &mut self,
        command: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        command::dispatch(self, command, args)
    }

    /// **只读运行态**（persona-polish 有意升级：`GET /mods/persona/state`
    /// 从恒 503 变 200）。快照零 IO，字段语义见 [`command::snapshot`]。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(command::snapshot(self))
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        // 「停用即还原」有两层：**会话覆盖**（persona 的来源槽一条不剩）与
        // **全局主链**（写回基线快照）。前者不落盘（档案留着，下次启用会重新
        // 灌回），后者是老的还原逻辑，一行不动。
        //
        // 只清自己的 owner：会话表是共享的（memory 也写它），clear_all() 会把
        // 别人在同一会话里的贡献一起清掉——那正是 L1 修掉的跨 Mod 干扰。
        if self.services.session_prompts.enabled() {
            self.services
                .session_prompts
                .clear_owner(SESSION_PROMPT_OWNER_PERSONA);
            self.services
                .logger
                .info("persona 已清除自己的会话角色卡槽（停用即还原；不动别的来源）");
        }
        // 把基线写回主链——「禁用 Mod → 回到仅主链 system_prompt」。
        if let Some(base) = self.base_prompt.take() {
            let ok = self
                .services
                .apply_settings
                .apply(serde_json::json!({"persona": {"system_prompt": base}}));
            if ok {
                self.services
                    .logger
                    .info("persona 已还原主链原 system_prompt");
            } else {
                self.services
                    .logger
                    .error("persona 还原主链 system_prompt 失败（apply_settings 返回 false）");
            }
        }
        self.registered = false;
        self.services.logger.info("persona Mod 已关闭");
        Ok(())
    }
}
