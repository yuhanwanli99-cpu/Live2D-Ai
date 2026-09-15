//! persona 的**会话级角色卡档案**（L1 persona 会话绑定，2026-09-15）。
//!
//! # 为什么要有它
//!
//! L1 验收要求「角色卡绑定**当前会话**：换会话不串卡、回原会话仍在」。宿主的
//! SessionPromptSink（live2d-ai-mod-system::session）只活在进程内存里，
//! 进程一停就没了；「回原会话仍在」跨重启那半条必须由**本 Mod 自己落盘**。
//! 于是有了与 live2d-ai.toml 同目录的 persona-mod-cards.json。
//!
//! # 文件格式（version 1）
//!
//! {"version":1,"sessions":{"<session_id>":{"spec":"chara_card_v2","data":{…}}}}
//!
//! 每个值是**规范化后的 V2 卡**（PersonaCard::to_json），不是原始文本——
//! 「档案里存的到底是什么」因此可读、可审计，也能原样再解析。
//! 会话 id 是**宿主归一化过的**（sanitize_session_id），本文件不自己发明
//! 命名规则：将来若档案拆成 <session>.json，同一套规则直接复用。
//!
//! # 坏文件 = warn + 忽略，**不是** Mod Failed
//!
//! 档案是**派生状态**（真相是用户导入的那张卡），一份写坏的档案不该让整个
//! 角色卡 Mod 连全局人设都启不来。所以：文件坏 / 版本不认识 / 会话数超上限 →
//! 记一条 warn 后当空档案继续；单个会话的卡坏 → 只跳过那一条（其余照常）。
//! 这与「配置里的 card_json 坏了就 Failed」是**两种输入、两种结局**：
//! 那是用户显式配置的真源，这是 Mod 自己的缓存。
//!
//! # 原子写
//!
//! tmp + rename（与 host 写 mods.json / 本 Mod 写导入卡同一条纪律）：
//! 写了一半的档案比没有档案更坏——下一次 start 会读到半截 JSON。
//!
//! # 上限
//!
//! 最多 MAX_SESSION_CARDS 个会话；单文件大小沿用 MAX_CARD_FILE_BYTES
//!（卡文件口径：一张 PNG 卡本来就塞得下）。超限 → 导入时回**可读错误**
//!（不是静默丢弃最旧的一条：那会让用户发现「卡自己没了」）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use live2d_ai_mod_system::{
    ModError, ModLogger, ModServices, ModSessionPrompts, SESSION_PROMPT_OWNER_PERSONA,
    sanitize_session_id,
};

use super::{MAX_CARD_FILE_BYTES, PersonaCard, compose_system_prompt};

/// 会话角色卡档案文件名（与 live2d-ai.toml 同目录）。
pub(crate) const SESSION_CARDS_FILE: &str = "persona-mod-cards.json";

/// 最多同时绑定的会话数。
///
/// 50 与前端会话列表上限（ChatSessionStore）一致：一个用户手里最多 50 条
/// 会话，档案不该比这更大；超了说明调用方在拿会话 id 当垃圾桶。
pub(crate) const MAX_SESSION_CARDS: usize = 50;

/// 档案格式版本。将来改结构必须升这个数，否则旧版本会按旧结构读新文件。
const ARCHIVE_VERSION: u64 = 1;

/// 会话角色卡档案（内存视图 + 落盘路径）。
pub(crate) struct SessionArchive {
    path: PathBuf,
    cards: BTreeMap<String, PersonaCard>,
}

impl SessionArchive {
    /// 从磁盘载入（**永不失败**：坏文件 warn 后当空档案；见模块头注）。
    pub(crate) fn load(path: PathBuf, logger: &ModLogger) -> Self {
        let cards = read_archive(&path, logger);
        Self { path, cards }
    }

    /// 已绑定的会话 id（BTreeMap 保证**已排序**，与宿主的 sessions() 同序）。
    pub(crate) fn ids(&self) -> Vec<String> {
        self.cards.keys().cloned().collect()
    }

    /// 已绑定的会话数。
    pub(crate) fn len(&self) -> usize {
        self.cards.len()
    }

    /// 写入 / 覆盖一个会话的卡：**先落盘再改内存**（落盘失败 = 内存不变，
    /// 于是宿主会话表也没被改过——不留「界面说成功、重启就没了」的半截状态）。
    pub(crate) fn insert(&mut self, session: &str, card: PersonaCard) -> Result<(), String> {
        if !self.cards.contains_key(session) && self.cards.len() >= MAX_SESSION_CARDS {
            return Err(format!(
                "会话角色卡已达上限（{MAX_SESSION_CARDS} 个）：请先清除某个会话的角色卡，再导入新的"
            ));
        }
        let mut next = self.cards.clone();
        next.insert(session.to_string(), card);
        write_archive(&self.path, &next)?;
        self.cards = next;
        Ok(())
    }

    /// 删掉某个会话的卡；返回「本来有没有」。先落盘再改内存（同 insert）。
    pub(crate) fn remove(&mut self, session: &str) -> Result<bool, String> {
        if !self.cards.contains_key(session) {
            return Ok(false);
        }
        let mut next = self.cards.clone();
        next.remove(session);
        write_archive(&self.path, &next)?;
        self.cards = next;
        Ok(true)
    }

    /// 把档案里每个会话的卡合成提示词**灌回宿主会话表**（start 时调用）。
    ///
    /// 这是「进程重启后回到会话 A 人设还在」的落点：宿主的表是内存态，每次
    /// 启动都要由持有档案的一方重新填。单张卡合成结果为空（理论上导入时已拦）
    /// 只 warn 跳过，不让整次 start 失败。
    pub(crate) fn restore(
        &self,
        prompts: &ModSessionPrompts,
        include_discipline: bool,
        logger: &ModLogger,
    ) {
        if self.len() == 0 {
            return;
        }
        if !prompts.enabled() {
            logger.warn(&format!(
                "persona 有 {} 个会话角色卡，但本进程没有会话绑定能力（host 未注入会话表）：本次不生效",
                self.len()
            ));
            return;
        }
        let mut restored = 0usize;
        for (session, card) in &self.cards {
            let composed = compose_system_prompt(card, include_discipline);
            if composed.trim().is_empty() {
                logger.warn(&format!(
                    "persona 会话 {session} 的角色卡合成结果为空，已跳过"
                ));
                continue;
            }
            // 灌回的是 persona 自己的来源槽（停用时 persona 只清这一格）。
            prompts.set_owned(SESSION_PROMPT_OWNER_PERSONA, session, composed);
            restored += 1;
        }
        if restored > 0 {
            logger.info(&format!("persona 已恢复 {restored} 个会话的角色卡"));
        }
    }
}

/// 档案路径（与 live2d-ai.toml 同目录）；config_path 空（单测 / 无宿主）→ None。
pub(crate) fn archive_path(config_path: &str) -> Option<PathBuf> {
    let config_path = config_path.trim();
    if config_path.is_empty() {
        return None;
    }
    Path::new(config_path)
        .parent()
        .map(|parent| parent.join(SESSION_CARDS_FILE))
}

/// 载入档案进 runtime 的槽位（start 用）；没有路径就什么都不做。
pub(crate) fn load_archive(services: &ModServices, slot: &mut Option<SessionArchive>) {
    let Some(path) = archive_path(&services.config_path) else {
        return;
    };
    *slot = Some(SessionArchive::load(path, &services.logger));
}

/// 命令通道取档案：本环境不可用 → **可读错误**（不静默降级成全局）。
///
/// 两条不可用路径分开说：没有会话表（host 没注入 sink）与没有落点
/// （host 没注入 config_path）——用户的下一步不一样（前者改用全局导入，
/// 后者要检查点火方式）。第一次取时顺手载入（start 已载过则直接用内存那份）。
pub(crate) fn require_archive<'a>(
    services: &ModServices,
    slot: &'a mut Option<SessionArchive>,
) -> Result<&'a mut SessionArchive, ModError> {
    if !services.session_prompts.enabled() {
        return Err(ModError::Other(
            "本进程没有会话绑定能力（host 未注入会话表）：请改用「导入为全局人设」，或用支持会话的宿主重新点火"
                .to_string(),
        ));
    }
    if slot.is_none() {
        let Some(path) = archive_path(&services.config_path) else {
            return Err(ModError::Other(
                "无法定位会话角色卡档案路径（host 未注入 config_path）：请改用「导入为全局人设」"
                    .to_string(),
            ));
        };
        *slot = Some(SessionArchive::load(path, &services.logger));
    }
    Ok(slot.as_mut().expect("just set"))
}

/// 读档案：坏文件一律 warn + 空档案（**不 Err**，见模块头注）。
fn read_archive(path: &Path, logger: &ModLogger) -> BTreeMap<String, PersonaCard> {
    let meta = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return BTreeMap::new(),
        Err(e) => {
            warn_ignore(path, logger, &format!("读取失败 {e}"));
            return BTreeMap::new();
        }
    };
    if !meta.is_file() {
        warn_ignore(path, logger, "不是普通文件");
        return BTreeMap::new();
    }
    if meta.len() > MAX_CARD_FILE_BYTES {
        warn_ignore(
            path,
            logger,
            &format!("{} 字节超过上限 {} 字节", meta.len(), MAX_CARD_FILE_BYTES),
        );
        return BTreeMap::new();
    }
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            warn_ignore(path, logger, &format!("读取失败 {e}"));
            return BTreeMap::new();
        }
    };
    let root: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            warn_ignore(path, logger, &format!("不是合法 JSON：{e}"));
            return BTreeMap::new();
        }
    };
    if root.get("version").and_then(|v| v.as_u64()) != Some(ARCHIVE_VERSION) {
        warn_ignore(path, logger, "version 不是 1（不认识的格式）");
        return BTreeMap::new();
    }
    let Some(entries) = root.get("sessions").and_then(|v| v.as_object()) else {
        warn_ignore(path, logger, "没有 sessions 对象");
        return BTreeMap::new();
    };
    if entries.len() > MAX_SESSION_CARDS {
        warn_ignore(
            path,
            logger,
            &format!("会话数 {} 超过上限 {MAX_SESSION_CARDS}", entries.len()),
        );
        return BTreeMap::new();
    }
    let mut cards = BTreeMap::new();
    for (key, value) in entries {
        // 防御性归一化：文件是我们自己写的，但手改 / 旧版本可能塞进别的 id。
        let Some(session) = sanitize_session_id(key) else {
            logger.warn(&format!("persona 会话角色卡档案跳过非法会话 id {key:?}"));
            continue;
        };
        match PersonaCard::parse_json(&value.to_string()) {
            Some(card) => {
                cards.insert(session, card);
            }
            None => logger.warn(&format!(
                "persona 会话角色卡档案跳过会话 {session}：里面的卡解析失败"
            )),
        }
    }
    cards
}

fn warn_ignore(path: &Path, logger: &ModLogger, why: &str) {
    logger.warn(&format!(
        "persona 会话角色卡档案忽略（{}）: {why}",
        path.display()
    ));
}

/// 原子写档案（tmp + rename）。序列化 / 落盘失败 → Err（调用方决定怎么报）。
fn write_archive(path: &Path, cards: &BTreeMap<String, PersonaCard>) -> Result<(), String> {
    let mut sessions = serde_json::Map::with_capacity(cards.len());
    for (session, card) in cards {
        sessions.insert(session.clone(), card.to_json());
    }
    let doc = serde_json::json!({"version": ARCHIVE_VERSION, "sessions": sessions});
    let text =
        serde_json::to_string_pretty(&doc).map_err(|e| format!("会话角色卡档案序列化失败: {e}"))?;
    if text.len() as u64 > MAX_CARD_FILE_BYTES {
        return Err(format!(
            "会话角色卡档案过大：{} 字节，上限 {} 字节；请清除一些会话的角色卡",
            text.len(),
            MAX_CARD_FILE_BYTES
        ));
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("会话角色卡档案目录创建失败（{}）: {e}", parent.display()))?;
    }
    let tmp = path.with_file_name(format!("{SESSION_CARDS_FILE}.tmp"));
    std::fs::write(&tmp, text)
        .map_err(|e| format!("会话角色卡档案写入失败（{}）: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("会话角色卡档案落盘失败（{}）: {e}", path.display())
    })
}
