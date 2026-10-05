//! `persona` 的**产品级命令通道**与**运行态快照**（persona-polish 波次）。
//!
//! 从 `lib.rs` 拆出（那边已经 800+ 行，再加这两个面会越过「源码 ≤1000 行」的
//! 口径）：本文件只放 `ModRuntime::command` / `ModRuntime::state_json` 两条新面，
//! 加上它们专属的导入卡落盘与入参解析。**卡解析与主链写回的不变量仍在 `lib.rs`**
//!（同一份 `compose_system_prompt` + `PersonaRuntime::apply_card`），这里不复制第二份。
//!
//! # 为什么导入卡落成 Mod 自己的文件，而不是回写 config
//!
//! config（`mods.json` 里的 namespaced 段）由 host 持有：原子写回的唯一入口在
//! host 侧（`mod_registry::persist_manifest`），runtime **没有**写自己 config 的
//! 通道。硬去改 `mods.json` 就是在 host 之外开第二个真源，而「Mod 启停/配置的
//! 唯一真源」正是 Mod 契约的第一条纪律（`docs/architecture/mod-product-chain.md`）。
//!
//! 于是导入卡落在与 `live2d-ai.toml` 同目录的 [`IMPORTED_CARD_FILE`]，并且
//! **优先级高于** config 的 `card_json` / `card_path`（[`PersonaRuntime::load_card`]）：
//! 界面上的「导入」是用户的一次显式动作，它必须能压过配置文件里写的卡；
//! 想回到配置里的卡就用 `clear_import`（面板上是「清除导入卡」）。
//! 这也让「导入 → 打开开关 → 人设变」在**重启之后**依然成立：config 没动，
//! 但导入卡还在。
//!
//! # 两条命令（各带两个作用域）
//!
//! | 命令 | 入参 | 成功 | 失败 |
//! |---|---|---|---|
//! | `import_card` | `card_json` 或 `data_base64`（都给时 `card_json` 优先）+ 可选 `session_id` | 规范化 JSON 落盘 → **立刻**生效 → 脱敏摘要 | `Err(ModError::Other)` → 409 `command_failed`（可读原因） |
//! | `clear_import` | 可选 `session_id` | 有 `session_id`：只清那个会话；无：删导入卡 → 按 config 重新生效 / 还原基线 | 同上 |
//!
//! **作用域由 `session_id` 决定**（L1 会话绑定）：
//! - 合法 id → **会话作用域**：卡进 `persona-mod-cards.json` + 宿主会话表
//!   （[`PersonaRuntime::import_card_for_session`]），**绝不** `apply_settings`；
//! - 缺省 / 空串 → **全局作用域**：老语义一字不改；
//! - 非空但非法 → 可读错误，**不写任何地方**（不偷偷走全局）。
//!
//! 返回体因此多两个键：`scope`（`"session"` / `"global"`）与 `session_id`
//!（全局导入时是 `null`）。会话作用域的 `card_name` / `card_format` /
//! `applied_chars` 描述的是**那个会话**的人设，不是全局主链——全局主链的接管
//! 摘要（`state_json.ready` 等）不受会话导入影响。
//!
//! 不认识的命令 → `Err(ModError::UnsupportedCommand)` → 409 `unsupported_command`
//!（host 契约：`crates/live2d-ai-desktop/src/web_api/mods_routes.rs`）。
//!
//! `data_base64` 允许直接传浏览器的 `data:image/png;base64,…`（前缀会被剥掉）：
//! 面板用 `pickImageDataUrl()` 选 PNG 卡，拿到什么就传什么，不在前端复制一份解析。
//!
//! # 失败不留下半个副作用
//!
//! 顺序是「先解析 → 先落盘 → 再写主链」：写主链失败就把导入卡**回滚**成上一次的
//! 内容（没有上一次就删掉）。反过来（先写主链、后落盘）会留下「人设已经换了、
//! 重启就没了」的哑巴状态——与「自检说谎」是同一类。
//!
//! # `state_json`（本轨的**有意升级**）
//!
//! 本 Mod 之前没实现它，`GET /api/v1/mods/persona/state` **恒 503**
//!（`state_unavailable`）。现在返回 200 + 一份**零 IO** 的脱敏摘要：
//! 快照在 `start` / `import_card` / `clear_import` 时就地更新，`state_json`
//! 只读内存（它跑在 web_api 线程上，不许读盘 / 阻塞）。
//! **仍会 503 的只剩一种**：Mod 未启用 / 已 Failed —— 那时没有 runtime 实例，
//! 命令通道同理（面板据此把「导入」按钮与那句提示一起换掉）。

use std::path::Path;

use base64::Engine as _;
use live2d_ai_mod_system::{
    MAX_SESSION_ID_CHARS, SESSION_PROMPT_OWNER_PERSONA, sanitize_session_id,
};

use crate::card::{CardSource, check_card_json_size};

use super::*;

impl PersonaRuntime {
    /// `import_card`：解析入参里的卡 → 规范化落盘 → 立刻生效。
    ///
    /// 作用域由入参的 `session_id` 决定（见 [parse_session_arg]）：
    /// 有合法 id → 只写宿主会话表（**不碰**全局主链）；没有 / 空串 → 老的全局语义。
    fn import_card(&mut self, args: &serde_json::Value) -> Result<serde_json::Value, ModError> {
        let session = parse_session_arg(args)?;
        let card = parse_card_args(args).map_err(ModError::Other)?;
        match session {
            Some(session) => self.import_card_for_session(&session, card),
            None => self.import_card_globally(card),
        }
    }

    /// 会话作用域导入：**只**写会话表与档案，全局主链一字不动。
    ///
    /// 顺序 = 先落档案（失败则内存与宿主表都不动）→ 再写宿主会话表；与全局导入
    /// 「先落盘再写主链」同一条纪律，不留「界面说成功、重启就没了」的半截状态。
    fn import_card_for_session(
        &mut self,
        session: &str,
        card: PersonaCard,
    ) -> Result<serde_json::Value, ModError> {
        let composed = compose_system_prompt(&card, self.config.include_discipline);
        if composed.trim().is_empty() {
            return Err(ModError::Other(
                "角色卡没有产生任何可写入的人设文本（字段全空，且未启用对话纪律模板）".to_string(),
            ));
        }
        let chars = composed.chars().count();
        let name = card.name.clone();
        let format = if card.format.is_empty() {
            "manual"
        } else {
            card.format
        };
        let bytes = serde_json::to_string_pretty(&card.to_json())
            .map_err(|e| ModError::Other(format!("会话角色卡序列化失败: {e}")))?
            .len();
        self.require_session_archive()?
            .insert(session, card)
            .map_err(ModError::Other)?;
        self.services
            .session_prompts
            .set_owned(SESSION_PROMPT_OWNER_PERSONA, session, composed);
        self.services.logger.info(&format!(
            "persona 已把角色卡绑定到会话 {session}（{chars} 字，全局主链未改）"
        ));
        Ok(serde_json::json!({
            "ok": true,
            "scope": "session",
            "session_id": session,
            "card_source": CardSource::Imported.as_str(),
            "card_name": name,
            "card_format": format,
            "applied_chars": chars,
            "bytes": bytes,
        }))
    }

    /// 全局作用域导入（老语义，一字不改：导入卡 + `with_overrides` + 写回主链）。
    fn import_card_globally(&mut self, card: PersonaCard) -> Result<serde_json::Value, ModError> {
        let Some(path) = self.imported_card_path() else {
            return Err(ModError::Other(
                "无法定位导入卡存储路径（host 未注入 config_path）；请改用配置里的 card_json"
                    .to_string(),
            ));
        };
        let text = serde_json::to_string_pretty(&card.to_json())
            .map_err(|e| ModError::Other(format!("导入卡序列化失败: {e}")))?;
        // 先落盘（失败可回滚），再写主链：写主链失败不留「导入成功」的痕迹。
        let previous = std::fs::read(&path).ok();
        write_imported_card(&path, text.as_bytes()).map_err(ModError::Other)?;
        let card = self.with_overrides(card);
        let source = CardSource::Imported;
        if let Err(e) = self.apply_card(&card, source) {
            restore_imported_card(&path, previous.as_deref());
            return Err(ModError::Other(e));
        }
        Ok(serde_json::json!({
            "ok": true,
            "scope": "global",
            "card_source": source.as_str(),
            "card_name": card.name,
            "card_format": if card.format.is_empty() { "manual" } else { card.format },
            "applied_chars": self.applied.as_ref().map_or(0, |a| a.chars),
            "bytes": text.len(),
        }))
    }

    /// `clear_import`：`session_id` 给了 → 只清那个会话；没给 → 老的全局语义。
    fn clear_import(&mut self, args: &serde_json::Value) -> Result<serde_json::Value, ModError> {
        match parse_session_arg(args)? {
            Some(session) => self.clear_session_import(&session),
            None => self.clear_global_import(),
        }
    }

    /// 会话作用域清除：档案里删 + 宿主表里 clear；**其余会话与全局主链不受影响**。
    ///
    /// 幂等：本来就没绑也回 `Ok(cleared=false)`（与全局清除同一条口径）。
    fn clear_session_import(&mut self, session: &str) -> Result<serde_json::Value, ModError> {
        let in_archive = self
            .require_session_archive()?
            .remove(session)
            .map_err(ModError::Other)?;
        // 只清**自己**的来源槽：memory 在同一个会话里的记忆块不受影响（L1 修复）。
        let in_host = self
            .services
            .session_prompts
            .clear_owned(SESSION_PROMPT_OWNER_PERSONA, session);
        let cleared = in_archive || in_host;
        if cleared {
            self.services.logger.info(&format!(
                "persona 已清除会话 {session} 的角色卡（其余会话不受影响）"
            ));
        }
        Ok(serde_json::json!({
            "ok": true,
            "scope": "session",
            "session_id": session,
            "cleared": cleared,
        }))
    }

    /// 全局作用域清除（老语义）：删掉导入卡 → 按 config 重新决定接管谁。
    ///
    /// 幂等：本来就没有导入卡也回 `Ok(cleared=false)`——用户连点两下不该报错。
    fn clear_global_import(&mut self) -> Result<serde_json::Value, ModError> {
        let Some(path) = self.imported_card_path() else {
            return Err(ModError::Other(
                "无法定位导入卡存储路径（host 未注入 config_path）".to_string(),
            ));
        };
        let removed = match std::fs::remove_file(&path) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => {
                return Err(ModError::Other(format!(
                    "删除导入卡失败（{}）: {e}",
                    path.display()
                )));
            }
        };
        let source = self.reapply_without_import().map_err(ModError::Other)?;
        let name = match &self.applied {
            Some(a) => a.name.clone(),
            None => String::new(),
        };
        Ok(serde_json::json!({
            "ok": true,
            "scope": "global",
            "cleared": removed,
            "card_source": source,
            "card_name": name,
        }))
    }

    /// 导入卡没了之后重新决定接管谁：config 的卡 / 覆盖 → 写回；
    /// 都没有 → **把主链还原成基线**（Mod 还在跑，但人设已撤销）。
    fn reapply_without_import(&mut self) -> Result<&'static str, String> {
        let (card, source) = match self.load_card()? {
            Some(pair) => pair,
            None if self.config.has_overrides() => (PersonaCard::default(), CardSource::Overrides),
            None => {
                let base = self.ensure_base_prompt();
                if !self
                    .services
                    .apply_settings
                    .apply(serde_json::json!({"persona": {"system_prompt": base}}))
                {
                    return Err(format!(
                        "清除导入卡后还原主链 system_prompt 失败（{} 不可写？）",
                        self.services.config_path.trim()
                    ));
                }
                self.applied = None;
                self.services
                    .logger
                    .info("persona 已清除导入卡，主链 system_prompt 还原基线");
                return Ok(CardSource::None.as_str());
            }
        };
        let card = self.with_overrides(card);
        self.apply_card(&card, source)?;
        Ok(source.as_str())
    }
}

// ---------------------------------------------------------------- 命令分派

/// 两条命令的分派入口（`lib.rs` 的 `ModRuntime::command` 只做转发）。
///
/// 从 `lib.rs` 出发而不是把整个 `impl ModRuntime` 搬过来：`start` / `shutdown`
/// 是**主链接管生命周期**，属于 `lib.rs`；这里只放「用户按了一次按钮」那两条。
/// 不认识的命令 → `UnsupportedCommand`（host 回 409 `unsupported_command`）。
pub(crate) fn dispatch(
    runtime: &mut PersonaRuntime,
    command: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, ModError> {
    match command {
        "import_card" => runtime.import_card(args),
        "clear_import" => runtime.clear_import(args),
        other => Err(ModError::UnsupportedCommand {
            command: other.to_string(),
        }),
    }
}

/// **零 IO** 的只读运行态（全部脱敏；面板按 key 渲染，不解释语义）。
///
/// 字段语义（前端 `persona_panel.dart` 的 `stateLabels` 是中文标签的唯一来源）：
/// - `ready`：本 Mod 当前是否已把一张可用人设写进主链（没有卡 = `false`）；
/// - `card_source`：`imported` / `config_json` / `config_path` / `overrides` / `none`；
/// - `card_name` / `card_format`：生效人设的名称与格式（`v1` / `v2` / `manual`）；
/// - `applied_chars`：写入主链的人设字数（**不含卡正文**，只是一个长度）；
/// - `has_base_snapshot`：停用还原用的基线是否已锚定（内存中有）；
/// - `include_discipline` / `say_first_mes`：生效的开关值（配置项的真相）；
/// - `config_has_card`：config 里是否写了 `card_json` / `card_path`。
///
/// L1 会话绑定新增四个字段（既有键一个不少、语义不变：它们是**全局主链**的
/// 接管摘要）：
/// - `session_bound`：本进程是否有会话绑定能力（= `session_prompts.enabled()`）；
/// - `sessions`：**persona 自己绑定过**的会话 id（**已排序**）——宿主表可用时
///   以宿主表里 **persona 那个来源槽**为准（宿主表是共享的，memory 也往里写；
///   直接取 `sessions()` 会把「只有 memory 绑定过的会话」也算成「persona 已绑定」，
///   那是界面在说谎），否则退回档案里的 id（让界面看得见「存了但没生效」）；
/// - `active_session`：宿主记录的当前活动会话（没有 = `null`）；
/// - `scope`：**当前人设作用域**——活动会话有绑定时是 `"session"`，否则 `"global"`。
///
/// **刻意不含**卡正文、`first_mes`、覆盖项正文与任何密钥——返回体会被界面
/// 渲染、也会进日志。
pub(crate) fn snapshot(runtime: &mut PersonaRuntime) -> serde_json::Value {
    let (ready, source, name, format, chars) = match &runtime.applied {
        Some(a) => (true, a.source, a.name.clone(), a.format, a.chars),
        None => (false, CardSource::None.as_str(), String::new(), "", 0),
    };
    let session_bound = runtime.services.session_prompts.enabled();
    let sessions: Vec<String> = if session_bound {
        runtime
            .services
            .session_prompts
            .sessions()
            .into_iter()
            .filter(|s| {
                runtime
                    .services
                    .session_prompts
                    .contributions(s)
                    .iter()
                    .any(|(owner, text)| {
                        owner.as_str() == SESSION_PROMPT_OWNER_PERSONA && !text.trim().is_empty()
                    })
            })
            .collect()
    } else {
        runtime
            .session_cards
            .as_ref()
            .map(super::sessions::SessionArchive::ids)
            .unwrap_or_default()
    };
    let active_session = runtime.services.session_prompts.active();
    let scope = if active_session
        .as_deref()
        .is_some_and(|s| sessions.iter().any(|x| x == s))
    {
        "session"
    } else {
        "global"
    };
    serde_json::json!({
        "ready": ready,
        "card_source": source,
        "card_name": name,
        "card_format": format,
        "applied_chars": chars,
        "has_base_snapshot": runtime.base_prompt.is_some(),
        "include_discipline": runtime.config.include_discipline,
        "say_first_mes": runtime.config.say_first_mes,
        "config_has_card": !runtime.config.card_json.trim().is_empty()
            || !runtime.config.card_path.trim().is_empty(),
        "session_bound": session_bound,
        "sessions": sessions,
        "active_session": active_session,
        "scope": scope,
    })
}

// ---------------------------------------------------------------- 入参与落盘

/// 解析 `session_id`（可选）：缺省 / 空串 = 全局语义；非空则**必须合法**。
///
/// **非法 id 明确报错，不回落全局**：一个写错的会话 id 若被当成「没给」，用户的
/// 角色卡就会悄悄写到所有会话上——正是这一波次要根除的串人设。老测试从不带
/// `session_id`，所以老语义一字未改。
fn parse_session_arg(args: &serde_json::Value) -> Result<Option<String>, ModError> {
    let Some(raw) = args.get("session_id").and_then(|v| v.as_str()) else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    sanitize_session_id(raw).map(Some).ok_or_else(|| {
        ModError::Other(format!(
            "session_id 不合法（{raw:?}）：只允许 ASCII 字母数字与 - _ . :，长度不超过 {MAX_SESSION_ID_CHARS}；请从会话列表重新选择，或改用「导入为全局人设」"
        ))
    })
}

/// 解析 `import_card` 的入参：`card_json`（JSON 文本）或 `data_base64`（字节）。
///
/// 两者都给时 `card_json` 优先（与 config 里 `card_json` 优先于 `card_path` 同一条口径）。
/// 一个都没有 / 解不出卡 → `Err`：**不得**静默成功成「导入了一张空卡」。
fn parse_card_args(args: &serde_json::Value) -> Result<PersonaCard, String> {
    if let Some(text) = args.get("card_json").and_then(|v| v.as_str()) {
        let text = text.trim();
        if text.is_empty() {
            return Err("card_json 是空的；请粘贴一张角色卡的 JSON 文本".to_string());
        }
        check_card_json_size(text)?;
        return PersonaCard::parse_json(text).ok_or_else(|| {
            "card_json 不是可识别的角色卡 JSON（需要 V1 扁平对象，或 V2 带 `data` 的对象）"
                .to_string()
        });
    }
    if let Some(raw) = args.get("data_base64").and_then(|v| v.as_str()) {
        let raw = strip_data_url_prefix(raw.trim());
        if raw.is_empty() {
            return Err("data_base64 是空的；请重新选择一张角色卡文件".to_string());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(raw))
            .map_err(|e| format!("data_base64 不是合法 base64: {e}"))?;
        if bytes.len() as u64 > MAX_CARD_FILE_BYTES {
            return Err(format!(
                "角色卡数据过大：{} 字节，上限 {} 字节（{} MiB）；请改用配置里的 card_path 指向文件",
                bytes.len(),
                MAX_CARD_FILE_BYTES,
                MAX_CARD_FILE_BYTES / (1024 * 1024)
            ));
        }
        return PersonaCard::parse_bytes(&bytes);
    }
    Err(
        "import_card 需要 card_json（JSON 文本）或 data_base64（PNG/JSON 字节的 base64）之一"
            .to_string(),
    )
}

/// 剥掉 `data:image/png;base64,` 这类前缀（浏览器 `readAsDataURL` 的产物）。
///
/// 直接贴 dataURL 也能用——面板因此不必在前端再实现一遍「切前缀」。
fn strip_data_url_prefix(raw: &str) -> &str {
    if raw.starts_with("data:")
        && let Some(at) = raw.find(";base64,")
    {
        return &raw[at + ";base64,".len()..];
    }
    raw
}

/// 原子写导入卡（tmp + rename）。
///
/// 写了一半的卡文件会让下一次 `start` 直接 `Failed`——「导入」这个动作自己
/// 制造出一个坏配置是最坏的结果。tmp + rename 与 host 写 `mods.json` 同一条纪律。
fn write_imported_card(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("导入卡目录创建失败（{}）: {e}", parent.display()))?;
    }
    let tmp = path.with_file_name(format!("{IMPORTED_CARD_FILE}.tmp"));
    std::fs::write(&tmp, bytes).map_err(|e| format!("导入卡写入失败（{}）: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("导入卡落盘失败（{}）: {e}", path.display())
    })
}

/// 写主链失败时把导入卡恢复原状（有旧的写回旧的，没有就删掉）。
fn restore_imported_card(path: &Path, previous: Option<&[u8]>) {
    match previous {
        Some(bytes) => {
            let _ = std::fs::write(path, bytes);
        }
        None => {
            let _ = std::fs::remove_file(path);
        }
    }
}
