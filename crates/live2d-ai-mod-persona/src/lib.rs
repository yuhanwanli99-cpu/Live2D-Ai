//! live2d-ai-mod-persona（rc.4 M5 立，persona-polish 打磨）——**酒馆角色卡标准 Mod**。
//!
//! # 它解决什么
//!
//! rc.4 起主链的 `[persona]` **只剩** `system_prompt` + `max_history_pairs`：
//! 角色扮演的「人设」不再内嵌进核心配置，而是这条 Mod 的职责。它把一张
//! SillyTavern 卡（V1 扁平 JSON / V2 嵌套 JSON / **PNG 内嵌 `chara` 负载**）
//! 合成一段 `system_prompt`，经**一等** `ModServices.apply_settings` 写回
//! 主链（写盘 + `supervisor.reload()`）。
//!
//! # 为什么写回主链而不是自己造一条对话通道
//!
//! 主链的 system 只有一处入口（`live2d-ai-runtime` 的 `ConversationConfig`）。
//! 让 Mod 在运行时另开一条人设通道，就等于在核心里偷偷埋第二个 system 来源——
//! 那是「第二个产品」的雏形。rc.4 的口径是：**Mod 可以加，主链只留一个入口**。
//!
//! # 启停语义（唯一真源 = Mod manifest 的 `enabled`）
//!
//! - **启用**：`mods.json` 里 `mods.persona.enabled = true`（前端「Mod 管理」
//!   或 `POST /api/v1/mods/persona/enable`）。host 才 `create` 本 runtime 并调
//!   [`ModRuntime::start`]：载入角色卡 → 合成 → 写回主链 `system_prompt`。
//! - **停用**：`disable`（或改 `mods.json` 后重启）。host 调 `shutdown`，
//!   主链 `system_prompt` **还原成启用前的基线**。
//! - 本 Mod 的 config / settings schema 里**没有**第二个 `enabled` 字段：
//!   「Mod 开关」只能有一处真相（与 `external-input` 同一条纪律，见
//!   `docs/architecture/mod-product-chain.md` §2）。config 里若混进 `enabled`
//!   键，它**不参与任何判断**（`PersonaConfig` 不读它）。
//!
//! # 坏输入 = 显式失败（不 panic、不静默、不动主链）
//!
//! 一条纪律：**没真的接管主链，就不许留下「接管了」的痕迹**。
//!
//! | 输入 | 结局 |
//! |---|---|
//! | `card_json` 不是 JSON / 不是角色卡 | `start` 返回 `Err` → `ModStatus::Failed`，主链提示词一字不动 |
//! | `card_path` 不存在 / 不是普通文件 / 读失败 | 同上，错误里带**路径**与**系统错误** |
//! | 卡文件 > [`MAX_CARD_FILE_BYTES`] | 同上；**读盘之前**就拒（`metadata` 先量），不做「读到 OOM 再报错」 |
//! | `card_json` > [`MAX_CARD_JSON_BYTES`] | 同上 |
//! | PNG 截断 / chunk 长度越界 / 无 `chara` / 压缩 iTXt | 同上（`tEXt`/未压缩 `iTXt` 之外一律明确报错） |
//! | `apply_settings` 写盘被拒 | 同上；基线快照**不落盘**（没接管就不记基线） |
//! | 卡没配（`card_path` / `card_json` 全空、也无覆盖） | **合法 no-op**：`Running`，主链提示词保持不变 |
//!
//! 「失败」用 host 的失败隔离（`mod-product-chain.md` §7）：本 Mod `Failed`，
//! 主链继续跑。**不要**改成「只写一行 warn 然后报 Running」——那会让界面显示
//! 「运行中」而实际什么都没发生（自检说谎）。
//!
//! # 禁用时怎么「回到仅主链 system_prompt」
//!
//! `apply_settings` 会真的改写 `live2d-ai.toml`，所以 Mod 必须能还回去。
//! 启用时它把**当前** `persona.system_prompt` 快照到 `persona-mod-base.txt`
//! （与 `live2d-ai.toml` 同目录，Mod 自己的状态文件），停用时写回该快照。
//! 快照只在**首次成功接管**后落盘，之后的启停都以它为基线，不会被合成产物污染。
//!
//! # 配置（住 `mods.json`，不回流主链）
//!
//! - `card_path`：角色卡文件路径（`.json` 或内嵌 `chara` 的 `.png`）；
//! - `card_json`：角色卡 JSON 文本（与路径二选一，**优先**）；
//! - `include_discipline`：是否附加对话纪律模板（缺省 true）；
//! - `say_first_mes`：启用时是否朗读卡里的开场白（缺省 false）；
//! - `name` / `description` / `personality` / `scenario`：手工覆盖（非空优先于卡）。
//!
//! # 与 memory 共存（last-writer-wins，无仲裁）
//!
//! `persona` 与 `memory`（`live2d-ai-mod-memory`）都可能写
//! `persona.system_prompt`。两侧共用同一段契约文字，与
//! `docs/architecture/memory-mod-v0.md` §5.1 **逐字一致**：
//!
//! > 两者都可能写 `persona.system_prompt`，规则是 **last-writer-wins，没有仲裁**：
//! >
//! > | 事件顺序 | 结果 |
//! > | --- | --- |
//! > | `persona` 后写 | 它的合成结果覆盖整个 `system_prompt`，**记忆块被冲掉** |
//! > | memory 后写（下一轮） | 它在 persona 的合成结果之上重新拼上记忆块 |
//! > | memory 停用 | `shutdown` 检查 marker，有就剥掉写回——**不留残留** |
//! >
//! > 这是**已知取舍**，不是 bug：主链只有一个 system 入口，加一套优先级表就是
//! > 在核心里埋第二个产品。要「两个都生效」必须先论证仲裁规则（见 §8 非目标）。
//!
//! 本 Mod 侧的直接结论：`start` 是**整段替换**（不做拼接、不解析 memory 的
//! marker），所以 persona 后写必然冲掉记忆块；`shutdown` 写回的是启用那一刻的
//! **整段**快照，若当时含记忆块就原样还回（memory 下一轮会幂等重拼或按需剥离）。
//! 回归在 `src/tests_e2e.rs`，用 memory crate 的**真实**
//! `strategy::compose_injection` / `strip_memory_block`（不是本地复刻）。
//!
//! # 文件大小
//!
//! 源码 > 500 行（< 1000）：卡解析与主链写回共用同一份不变量（V2 判定 +
//! `system_prompt` 合成口径），拆成两文件只会让读者来回跳；测试已拆到
//! `src/tests.rs`，源码保持**一个文件顺序读完**。
//!
//! `src/tests.rs` 略超「测试文件 ≤ 800 行」：超出的部分是启停循环三条入口
//! （`card_json` / `card_path` JSON / `card_path` PNG）各自的端到端回归——
//! 合并成参数化用例会把「哪条入口坏了」这个信息藏起来。
//!
//! Wave 3 轨 E 新增的坏卡 E2E 与共存契约**另起** `src/tests_e2e.rs`（318 行），
//! 不再往 `tests.rs` 里加——两个文件各自守一组契约，也守住单文件行数纪律。

use std::path::{Path, PathBuf};

use base64::Engine as _;
use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
///
/// `version` 随行为变化走：persona-polish 起坏配置不再「假装 Running」，
/// 而是显式 `Failed`（见模块头注「坏输入 = 显式失败」）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "persona",
    name: "角色卡",
    version: "0.2.0",
    api_version: MOD_API_VERSION,
};

/// 基线快照文件名（与 `live2d-ai.toml` 同目录）。
const BASE_STATE_FILE: &str = "persona-mod-base.txt";

/// 角色卡**文件**大小上限（字节）。
///
/// 必须**在读盘之前**判断：`fs::read` 会先吃掉整份文件，一个误指向的 GB 级
/// 文件（或管道/设备文件）足以把常驻内存打穿——「坏输入」的正确结局是一条
/// 可读的错误，不是 OOM。
///
/// 为什么是 16 MiB：PNG 卡把整张立绘和 base64 的 JSON 塞进同一个文件，
/// 实测正常卡 < 2 MiB；16 MiB 余量给足，同时仍是一条能对用户解释清楚的界。
pub const MAX_CARD_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// `card_json` **文本**大小上限（字节）。
///
/// `card_json` 住在 `mods.json` 里；典型 V2 卡 2–8 KiB，1 MiB 已远超任何
/// 真实角色卡，超过它基本等于「把文件内容贴错了地方」。
pub const MAX_CARD_JSON_BYTES: usize = 1024 * 1024;

/// 对话纪律模板（可关；配合「一句一单元」的语音契约：句数少 ⇒ TTS 请求少）。
const DISCIPLINE_TEMPLATE: &str = "【对话纪律】\n\
1. 每次回复只写 1-5 句，句子要短，像即时通讯。\n\
2. 每句话以真实句读结尾（。！？），不要用逗号把多层意思串成长句。\n\
3. 不要写 Markdown 标题/列表/加粗，不要替用户说话。";

/// 归一化后的角色卡（本 Mod 内部命名，与酒馆字段解耦）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonaCard {
    pub name: String,
    pub description: String,
    pub personality: String,
    pub scenario: String,
    /// 开场白（酒馆 `first_mes`）。
    pub first: String,
    /// 卡自带 system 提示（酒馆 V2 `data.system_prompt`）。
    pub system_prompt: String,
    /// `v1` / `v2`（诊断/日志用）。
    pub format: &'static str,
}

/// 参与识别的键：一个都没有 → 这不是角色卡（返回 `None`）。
const MAPPED_KEYS: &[&str] = &[
    "name",
    "description",
    "personality",
    "scenario",
    "first_mes",
    "system_prompt",
    "creator_notes",
    "creatorcomment",
    "tags",
];

impl PersonaCard {
    /// 解析 JSON 文本（V1 扁平 / V2 嵌套）。**永不 panic**：坏 JSON / 非卡 → `None`。
    pub fn parse_json(text: &str) -> Option<PersonaCard> {
        let root: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
        let root = root.as_object()?;
        // V2：字段在 `data` 里（spec 缺失但 data 齐全的卡也认）。
        let data = root.get("data").and_then(|v| v.as_object());
        let looks_v2 = data.is_some_and(|d| {
            root.get("spec")
                .and_then(|s| s.as_str())
                .is_some_and(|s| s.starts_with("chara_card_v2"))
                || d.contains_key("first_mes")
                || d.contains_key("name")
        });
        let fields = if looks_v2 {
            data.expect("checked")
        } else {
            root
        };
        if !fields.keys().any(|k| MAPPED_KEYS.contains(&k.as_str())) {
            return None;
        }
        Some(PersonaCard {
            name: str_field(fields, "name"),
            description: str_field(fields, "description"),
            personality: str_field(fields, "personality"),
            scenario: str_field(fields, "scenario"),
            first: str_field(fields, "first_mes"),
            system_prompt: str_field(fields, "system_prompt"),
            format: if looks_v2 { "v2" } else { "v1" },
        })
    }

    /// 从任意字节解析：PNG 走 `chara` 负载，否则当 UTF-8 JSON 文本。
    pub fn parse_bytes(bytes: &[u8]) -> Result<PersonaCard, String> {
        if bytes.starts_with(&PNG_SIGNATURE) {
            let payload = extract_png_chara(bytes)?;
            return parse_chara_payload(&payload);
        }
        let text =
            std::str::from_utf8(bytes).map_err(|e| format!("不是 PNG 也不是 UTF-8 文本: {e}"))?;
        PersonaCard::parse_json(text).ok_or_else(|| {
            "不是可识别的角色卡 JSON（需要 V1 扁平对象，或 V2 带 `data` 的对象）".to_string()
        })
    }
}

fn str_field(fields: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

/// 合成 `system_prompt`：卡字段 → 可选纪律模板（空段跳过，段间空行分隔）。
pub fn compose_system_prompt(card: &PersonaCard, include_discipline: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !card.name.is_empty() {
        parts.push(format!("[角色] {}", card.name));
    }
    if !card.description.is_empty() {
        parts.push(card.description.clone());
    }
    if !card.personality.is_empty() {
        parts.push(format!("性格：{}", card.personality));
    }
    if !card.scenario.is_empty() {
        parts.push(format!("场景：{}", card.scenario));
    }
    if !card.system_prompt.is_empty() {
        parts.push(card.system_prompt.clone());
    }
    if include_discipline {
        parts.push(DISCIPLINE_TEMPLATE.to_string());
    }
    parts.join("\n\n")
}

// ------------------------------------------------------------------ 输入上限

/// 读角色卡文件：**先量大小、再读内容**（两道闸：`metadata` + 实际读到的字节数）。
///
/// 第二道闸对应 TOCTOU：`metadata` 与 `read` 之间文件可能被换掉/追加；
/// 以**真正读到的大小**为准再判一次，才谈得上「上限」。
fn read_card_file(path: &str) -> Result<Vec<u8>, String> {
    let p = Path::new(path);
    let meta = std::fs::metadata(p).map_err(|e| format!("读取角色卡文件失败（{path}）: {e}"))?;
    if !meta.is_file() {
        return Err(format!(
            "角色卡路径不是普通文件（{path}）；请指向 .json 或内嵌 chara 的 .png"
        ));
    }
    if meta.len() > MAX_CARD_FILE_BYTES {
        return Err(format!(
            "角色卡文件过大（{path}）：{} 字节，上限 {} 字节（{} MiB）",
            meta.len(),
            MAX_CARD_FILE_BYTES,
            MAX_CARD_FILE_BYTES / (1024 * 1024)
        ));
    }
    let bytes = std::fs::read(p).map_err(|e| format!("读取角色卡文件失败（{path}）: {e}"))?;
    if bytes.len() as u64 > MAX_CARD_FILE_BYTES {
        return Err(format!(
            "角色卡文件过大（{path}）：实际读到 {} 字节，上限 {} 字节",
            bytes.len(),
            MAX_CARD_FILE_BYTES
        ));
    }
    Ok(bytes)
}

/// 校验 `card_json` 大小（**解析之前**，不先让 `serde_json` 吃掉整段文本）。
fn check_card_json_size(text: &str) -> Result<(), String> {
    if text.len() > MAX_CARD_JSON_BYTES {
        return Err(format!(
            "card_json 过大：{} 字节，上限 {} 字节（{} KiB）；请改用 card_path 指向文件",
            text.len(),
            MAX_CARD_JSON_BYTES,
            MAX_CARD_JSON_BYTES / 1024
        ));
    }
    Ok(())
}

// ------------------------------------------------------------------ PNG chara

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// 取出 PNG 里关键字为 `chara` 的文本负载（通常是 base64）。
///
/// 只读 `tEXt` 与**未压缩**的 `iTXt`；压缩 `iTXt` 需要 zlib，明确报错
/// 而不是返回空卡（与前端旧实现同一条教训）。
///
/// 全程只做「先量再取」的切片：块长度越界 → `Err`，不会 panic。
fn extract_png_chara(bytes: &[u8]) -> Result<String, String> {
    let mut offset = PNG_SIGNATURE.len();
    while offset + 8 <= bytes.len() {
        let length = read_u32(bytes, offset) as usize;
        let kind = &bytes[offset + 4..offset + 8];
        let data_start = offset + 8;
        let data_end = data_start + length;
        if data_end + 4 > bytes.len() {
            return Err(format!(
                "PNG 数据不完整（块长度 {length} 超出文件，剩余 {} 字节）",
                bytes.len().saturating_sub(data_start)
            ));
        }
        if kind == b"tEXt" {
            if let Some(text) = read_text_chunk(&bytes[data_start..data_end]) {
                return Ok(text);
            }
        } else if kind == b"iTXt"
            && let Some((text, compressed)) = read_itxt_chunk(&bytes[data_start..data_end])
        {
            if compressed {
                return Err(
                    "这张 PNG 的角色卡是压缩的 iTXt（zlib），当前不支持；请用 tEXt 卡或直接导入 JSON"
                        .to_string(),
                );
            }
            return Ok(text);
        }
        if kind == b"IEND" {
            break;
        }
        offset = data_end + 4; // 跳过 CRC
    }
    Err("这张 PNG 里没有角色卡数据（没有 chara 块）".to_string())
}

/// `tEXt`：`keyword` + 0x00 + `text`（Latin-1）。
fn read_text_chunk(data: &[u8]) -> Option<String> {
    let nul = data.iter().position(|b| *b == 0)?;
    if &data[..nul] != b"chara" {
        return None;
    }
    Some(latin1(&data[nul + 1..]))
}

/// `iTXt` 最小结构：keyword\0 flag method language\0 translated\0 text。
fn read_itxt_chunk(data: &[u8]) -> Option<(String, bool)> {
    let nul = data.iter().position(|b| *b == 0)?;
    if &data[..nul] != b"chara" {
        return None;
    }
    let mut p = nul + 1;
    if p + 2 > data.len() {
        return None;
    }
    let compressed = data[p] == 1;
    p += 2;
    let lang_end = data[p..].iter().position(|b| *b == 0)? + p;
    let translated_end = data[lang_end + 1..].iter().position(|b| *b == 0)? + lang_end + 1;
    let text = String::from_utf8_lossy(&data[translated_end + 1..]).into_owned();
    Some((text, compressed))
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|b| *b as char).collect()
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// `chara` 负载：先按 base64 解，失败则当未编码的 JSON（少见但存在）。
fn parse_chara_payload(payload: &str) -> Result<PersonaCard, String> {
    let trimmed = payload.trim();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(trimmed)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(trimmed));
    let text = match decoded {
        Ok(bytes) => {
            String::from_utf8(bytes).map_err(|e| format!("PNG 里的 chara 数据不是 UTF-8: {e}"))?
        }
        Err(_) if trimmed.starts_with('{') => trimmed.to_string(),
        Err(_) => return Err("PNG 里的 chara 数据既不是 base64 也不是 JSON".to_string()),
    };
    PersonaCard::parse_json(&text).ok_or_else(|| "PNG 里的角色卡 JSON 解析失败".to_string())
}

// ------------------------------------------------------------------ Runtime

/// 本 Mod 的 namespaced 配置（缺省全部安全）。
///
/// **刻意不读 `enabled`**：Mod 的启停只由 manifest 表达（见模块头注）。
/// 混进 config 的 `enabled` 键是**惰性**的，不参与任何判断。
#[derive(Debug, Clone)]
struct PersonaConfig {
    card_path: String,
    card_json: String,
    include_discipline: bool,
    say_first_mes: bool,
    name: String,
    description: String,
    personality: String,
    scenario: String,
}

impl PersonaConfig {
    fn from_value(v: &serde_json::Value) -> Self {
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
    fn has_overrides(&self) -> bool {
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
    services: ModServices,
    config: PersonaConfig,
    registered: bool,
    /// 主链原本的 `system_prompt`（停用时写回；见模块头注）。
    base_prompt: Option<String>,
}

impl PersonaRuntime {
    fn new(services: ModServices, config: PersonaConfig) -> Self {
        Self {
            services,
            config,
            registered: false,
            base_prompt: None,
        }
    }

    /// 载入卡：`card_json` 优先于 `card_path`；两者皆空 → `Ok(None)`（只有手工覆盖）。
    ///
    /// `Err` = **用户配了东西但它是坏的**（读不到 / 不是卡 / 超大）；
    /// `Ok(None)` = 用户根本没配 —— 两者结局不同，绝不能混成一个「空卡」。
    fn load_card(&self) -> Result<Option<PersonaCard>, String> {
        let json = self.config.card_json.trim();
        if !json.is_empty() {
            check_card_json_size(json)?;
            return PersonaCard::parse_json(json).map(Some).ok_or_else(|| {
                "card_json 不是可识别的角色卡 JSON（需要 V1 扁平对象，或 V2 带 `data` 的对象）"
                    .to_string()
            });
        }
        let path = self.config.card_path.trim();
        if !path.is_empty() {
            let bytes = read_card_file(path)?;
            return PersonaCard::parse_bytes(&bytes).map(Some);
        }
        Ok(None)
    }

    /// 手工覆盖：非空配置项优先于卡字段。
    fn with_overrides(&self, mut card: PersonaCard) -> PersonaCard {
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
    fn ensure_base_prompt(&mut self) -> String {
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

    /// 把卡合成 `system_prompt` 并写回主链。
    ///
    /// - `Ok(())`：已写入，或**本来就没配卡**（合法 no-op：先启用、后填卡）；
    /// - `Err(e)`：**配置坏了**（读不到 / 不是卡 / 超大 / 合成结果为空 / 写盘被拒）。
    ///   调用方（[`ModRuntime::start`]）把它变成 `ModStatus::Failed`：界面立刻
    ///   看得到失败，而主链 `system_prompt` **一个字都不动**（写盘在所有校验之后）。
    fn apply_from_config(&mut self) -> Result<(), String> {
        let card = match self.load_card()? {
            Some(card) => self.with_overrides(card),
            None if self.config.has_overrides() => self.with_overrides(PersonaCard::default()),
            None => {
                self.services.logger.info(
                    "persona 未配置角色卡（card_path / card_json 均空、也无覆盖项），保持主链现有提示词",
                );
                return Ok(());
            }
        };
        let composed = compose_system_prompt(&card, self.config.include_discipline);
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
        self.services.logger.info(&format!(
            "persona 已写入主链 system_prompt（来源 {}，{} 字）",
            card.format,
            composed.chars().count()
        ));
        if self.config.say_first_mes && !card.first.is_empty() {
            self.services.say_tx.say(card.first.clone());
        }
        Ok(())
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

    fn shutdown(&mut self) -> Result<(), ModError> {
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

/// 角色卡 Mod 的 settings schema（**静态**；factory 与 runtime.start 共用）。
///
/// **不含 `enabled`**：启停只由 Mod manifest 表达（模块头注「启停语义」）。
fn persona_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::String {
                key: "card_path".to_string(),
                label: "角色卡文件路径（.json / 内嵌 chara 的 .png）".to_string(),
                secret: false,
            },
            ModSettingField::String {
                key: "card_json".to_string(),
                label: "角色卡 JSON 文本（与路径二选一，优先）".to_string(),
                secret: false,
            },
            ModSettingField::Bool {
                key: "include_discipline".to_string(),
                label: "附加对话纪律模板".to_string(),
                default: true,
            },
            ModSettingField::Bool {
                key: "say_first_mes".to_string(),
                label: "启用时朗读开场白".to_string(),
                default: false,
            },
            ModSettingField::String {
                key: "name".to_string(),
                label: "覆盖：名称".to_string(),
                secret: false,
            },
            ModSettingField::String {
                key: "description".to_string(),
                label: "覆盖：描述".to_string(),
                secret: false,
            },
            ModSettingField::String {
                key: "personality".to_string(),
                label: "覆盖：性格".to_string(),
                secret: false,
            },
            ModSettingField::String {
                key: "scenario".to_string(),
                label: "覆盖：场景".to_string(),
                secret: false,
            },
        ],
    }
}

/// 静态工厂。
pub struct PersonaFactory;

impl ModFactory for PersonaFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// M2：未启用也能拿到 schema（前端先填卡、再启用）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(persona_settings_spec())
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(PersonaRuntime::new(
            services,
            PersonaConfig::from_value(&config),
        )))
    }
}

/// 工厂单例。
pub const FACTORY: PersonaFactory = PersonaFactory;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_e2e;
