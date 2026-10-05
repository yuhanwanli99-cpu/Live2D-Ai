//! 角色卡解析（V1/V2 JSON + PNG `chara` 负载；自 `lib.rs` 拆出）。
//!
//! 三种来源（`card_json` / `card_path` / PNG `chara`）统一归一化成 `PersonaCard`，
//! 再由 `compose_system_prompt` 合成主链 system_prompt。本文件**只做解析**：
//! 「什么时候接管 / 什么时候还原」在 `runtime`，启停语义仍在 `lib.rs` 头注。
//!
//! `CardSource` / `AppliedPersona` 及其字段、`read_card_file` /
//! `check_card_json_size` 按 `pub(super)` 开放给兄弟模块（`runtime` / `command`），
//! 不进对外 API。

use std::path::Path;

use base64::Engine as _;

use super::*;

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

    /// **规范化**成一张 V2 JSON（导入卡落盘用；见 `src/command.rs` 头注）。
    ///
    /// 六个字段固定出现（哪怕为空）：一个键都没有的 JSON 会被
    /// [`Self::parse_json`] 判成「不是角色卡」，规范化产物必须能原样再解析。
    /// 刻意**丢掉**卡里其余字段（头像 / 扩展键 / `creator_notes`…）：
    /// 本 Mod 只用这六项，落盘的文件越接近「它到底用了什么」越好审计。
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "spec": "chara_card_v2",
            "spec_version": "2.0",
            "data": {
                "name": self.name,
                "description": self.description,
                "personality": self.personality,
                "scenario": self.scenario,
                "first_mes": self.first,
                "system_prompt": self.system_prompt,
            },
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

/// 一张卡的**来源**（`state_json.card_source` 的稳定字符串；前端按它出中文）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CardSource {
    /// 界面导入（`persona-mod-card.json`），优先级最高。
    Imported,
    /// config 的 `card_json`。
    ConfigJson,
    /// config 的 `card_path`。
    ConfigPath,
    /// 只有手工覆盖项（`name` / `description` / …）。
    Overrides,
    /// 什么都没配（合法 no-op）。
    None,
}

impl CardSource {
    /// 稳定 ASCII 值——**不要**改成中文：中文是前端 `stateLabels` 的职责，
    /// 这里换文案会让「按值反查」的界面悄悄失配。
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Imported => "imported",
            Self::ConfigJson => "config_json",
            Self::ConfigPath => "config_path",
            Self::Overrides => "overrides",
            Self::None => "none",
        }
    }
}

/// 最近一次**成功接管**主链的摘要（`state_json` 的数据源）。
///
/// 刻意缓存而不是每次读盘：`state_json` 跑在 web_api 线程上，
/// 契约是「只读快照、不阻塞」（见 `ModRuntime::state_json` 头注）。
#[derive(Debug, Clone)]
pub(super) struct AppliedPersona {
    pub(super) source: &'static str,
    pub(super) name: String,
    pub(super) format: &'static str,
    pub(super) chars: usize,
}

// ------------------------------------------------------------------ 输入上限

/// 读角色卡文件：**先量大小、再读内容**（两道闸：`metadata` + 实际读到的字节数）。
///
/// 第二道闸对应 TOCTOU：`metadata` 与 `read` 之间文件可能被换掉/追加；
/// 以**真正读到的大小**为准再判一次，才谈得上「上限」。
pub(super) fn read_card_file(path: &str) -> Result<Vec<u8>, String> {
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
pub(super) fn check_card_json_size(text: &str) -> Result<(), String> {
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

pub(super) const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

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
