//! live2d-ai-mod-persona（rc.4 M5）——**酒馆角色卡标准 Mod**。
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
//! # 禁用时怎么「回到仅主链 system_prompt」
//!
//! `apply_settings` 会真的改写 `live2d-ai.toml`，所以 Mod 必须能还回去。
//! 启动时它把**当前** `persona.system_prompt` 快照到 `persona-mod-base.txt`
//! （与 `live2d-ai.toml` 同目录，Mod 自己的状态文件），关闭时写回该快照。
//! 快照只在**首次启用**时落盘，之后的启停都以它为基线，不会被合成产物污染。
//!
//! # 配置（住 `mods.json`，不回流主链）
//!
//! - `card_path`：角色卡文件路径（`.json` 或内嵌 `chara` 的 `.png`）；
//! - `card_json`：角色卡 JSON 文本（与路径二选一，**优先**）；
//! - `include_discipline`：是否附加对话纪律模板（缺省 true）；
//! - `say_first_mes`：启用时是否朗读卡里的开场白（缺省 false）；
//! - `name` / `description` / `personality` / `scenario`：手工覆盖（非空优先于卡）。

use std::path::{Path, PathBuf};

use base64::Engine as _;
use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "persona",
    name: "角色卡",
    version: "0.1.0",
    api_version: MOD_API_VERSION,
};

/// 基线快照文件名（与 `live2d-ai.toml` 同目录）。
const BASE_STATE_FILE: &str = "persona-mod-base.txt";

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
        PersonaCard::parse_json(text).ok_or_else(|| "不是可识别的角色卡 JSON".to_string())
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

// ------------------------------------------------------------------ PNG chara

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// 取出 PNG 里关键字为 `chara` 的文本负载（通常是 base64）。
///
/// 只读 `tEXt` 与**未压缩**的 `iTXt`；压缩 `iTXt` 需要 zlib，明确报错
/// 而不是返回空卡（与前端旧实现同一条教训）。
fn extract_png_chara(bytes: &[u8]) -> Result<String, String> {
    let mut offset = PNG_SIGNATURE.len();
    while offset + 8 <= bytes.len() {
        let length = read_u32(bytes, offset) as usize;
        let kind = &bytes[offset + 4..offset + 8];
        let data_start = offset + 8;
        let data_end = data_start + length;
        if data_end + 4 > bytes.len() {
            return Err("PNG 数据不完整（块长度超出文件）".to_string());
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
}

/// 角色卡 Mod 运行时。
pub struct PersonaRuntime {
    services: ModServices,
    config: PersonaConfig,
    registered: bool,
    /// 主链原本的 `system_prompt`（关闭时写回；见模块头注）。
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

    /// 载入卡：`card_json` 优先于 `card_path`；两者皆空 → 只有手工覆盖。
    fn load_card(&self) -> Result<Option<PersonaCard>, String> {
        if !self.config.card_json.trim().is_empty() {
            return PersonaCard::parse_json(&self.config.card_json)
                .map(Some)
                .ok_or_else(|| "card_json 不是可识别的角色卡 JSON".to_string());
        }
        if !self.config.card_path.trim().is_empty() {
            let bytes = std::fs::read(Path::new(&self.config.card_path))
                .map_err(|e| format!("读取角色卡文件失败（{}）: {e}", self.config.card_path))?;
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

    /// 确保基线快照存在（首次启用时把当前主链 system_prompt 写进 Mod 状态文件）。
    fn ensure_base_prompt(&mut self) {
        if self.base_prompt.is_some() {
            return;
        }
        let current = self
            .services
            .settings
            .read()
            .get("persona")
            .and_then(|p| p.get("system_prompt"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        match self.base_state_path() {
            Some(path) => {
                if let Ok(saved) = std::fs::read_to_string(&path) {
                    self.base_prompt = Some(saved);
                    return;
                }
                if let Some(parent) = path.parent()
                    && let Err(e) = std::fs::create_dir_all(parent)
                {
                    self.services
                        .logger
                        .warn(&format!("persona 基线快照目录创建失败: {e}"));
                }
                if let Err(e) = std::fs::write(&path, &current) {
                    self.services
                        .logger
                        .warn(&format!("persona 基线快照写入失败: {e}"));
                }
                self.base_prompt = Some(current);
            }
            None => self.base_prompt = Some(current),
        }
    }

    /// 把卡合成 `system_prompt` 并写回主链。
    fn apply_from_config(&mut self) {
        let card = match self.load_card() {
            Ok(Some(card)) => self.with_overrides(card),
            Ok(None) => self.with_overrides(PersonaCard::default()),
            Err(e) => {
                self.services
                    .logger
                    .warn(&format!("persona 载入角色卡失败，保留主链现有提示词: {e}"));
                return;
            }
        };
        let composed = compose_system_prompt(&card, self.config.include_discipline);
        if composed.is_empty() {
            self.services
                .logger
                .info("persona 没有可写入的人设（卡为空且无覆盖），保留主链现有提示词");
            return;
        }
        self.ensure_base_prompt();
        let ok = self
            .services
            .apply_settings
            .apply(serde_json::json!({"persona": {"system_prompt": composed}}));
        if ok {
            self.services.logger.info(&format!(
                "persona 已写入主链 system_prompt（来源 {}，{} 字）",
                card.format,
                composed.chars().count()
            ));
        } else {
            self.services
                .logger
                .warn("persona apply_settings 失败：主链 system_prompt 未改变");
        }
        if self.config.say_first_mes && !card.first.is_empty() {
            self.services.say_tx.say(card.first.clone());
        }
    }
}

impl ModRuntime for PersonaRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 静态 schema（factory.settings_spec 同源）：未启用也能渲染表单。
        registrar.register_settings(persona_settings_spec())?;
        self.registered = true;
        self.apply_from_config();
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
                    .warn("persona 还原主链 system_prompt 失败（apply_settings 返回 false）");
            }
        }
        self.registered = false;
        self.services.logger.info("persona Mod 已关闭");
        Ok(())
    }
}

/// 角色卡 Mod 的 settings schema（**静态**；factory 与 runtime.start 共用）。
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
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// 记录 register_settings 的测试注册器。
    #[derive(Default)]
    struct MockRegistrar {
        specs: Vec<ModSettingsSpec>,
    }

    impl ModRegistrar for MockRegistrar {
        fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
            self.specs.push(spec);
            Ok(())
        }
        fn subscribe(&mut self, _topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
            Ok(SubscriptionId(1))
        }
        fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
            Ok(())
        }
    }

    fn noop_services() -> ModServices {
        ModServices::new(
            ModActionSender::new(|_| false),
            SaySender::new(|_| true),
            ModEventSender::new(|_, _| true),
            ModLogger::new(|_, _| {}),
        )
    }

    /// 带捕获 apply + 固定 settings 快照 + config 路径的 services。
    fn capturing_services(
        calls: Arc<Mutex<Vec<serde_json::Value>>>,
        base_prompt: &'static str,
        config_path: String,
    ) -> ModServices {
        noop_services()
            .with_apply_settings(ModSettingsApplier::new(move |patch| {
                calls.lock().unwrap().push(patch);
                true
            }))
            .with_settings_reader(ModSettingsReader::new(
                move || serde_json::json!({"persona": {"system_prompt": base_prompt}}),
            ))
            .with_config_path(config_path)
    }

    #[test]
    fn descriptor_is_static_and_api_compatible() {
        assert_eq!(DESCRIPTOR.id, "persona");
        assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
    }

    #[test]
    fn parse_v1_flat_json() {
        let card = PersonaCard::parse_json(
            r#"{"name":"NEKO","description":"猫娘","personality":"傲娇","scenario":"咖啡馆","first_mes":"你好"}"#,
        )
        .expect("V1 卡应解析");
        assert_eq!(card.name, "NEKO");
        assert_eq!(card.first, "你好");
        assert_eq!(card.format, "v1");
    }

    #[test]
    fn parse_v2_nested_json() {
        let card = PersonaCard::parse_json(
            r#"{"spec":"chara_card_v2","spec_version":"2.0","data":{"name":"NEKO","description":"猫娘","system_prompt":"用短句。"}}"#,
        )
        .expect("V2 卡应解析");
        assert_eq!(card.name, "NEKO");
        assert_eq!(card.system_prompt, "用短句。");
        assert_eq!(card.format, "v2");
    }

    #[test]
    fn reject_non_card_json() {
        assert!(PersonaCard::parse_json(r#"{"foo":1}"#).is_none());
        assert!(PersonaCard::parse_json("not json").is_none());
    }

    fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        out.extend_from_slice(&[0, 0, 0, 0]); // CRC（本 Mod 不校验）
    }

    fn png_with_chara(payload: &str) -> Vec<u8> {
        let mut out = PNG_SIGNATURE.to_vec();
        push_chunk(&mut out, b"tEXt", format!("chara\0{payload}").as_bytes());
        push_chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn parse_png_with_base64_chara() {
        let card_json =
            r#"{"spec":"chara_card_v2","data":{"name":"PNG猫","description":"来自 PNG"}}"#;
        let b64 = base64::engine::general_purpose::STANDARD.encode(card_json.as_bytes());
        let png = png_with_chara(&b64);
        assert!(png.starts_with(&PNG_SIGNATURE));
        let card = PersonaCard::parse_bytes(&png).expect("PNG 卡应解析");
        assert_eq!(card.name, "PNG猫");
        assert_eq!(card.description, "来自 PNG");
        assert_eq!(card.format, "v2");
    }

    #[test]
    fn png_without_chara_reports_honestly() {
        let mut out = PNG_SIGNATURE.to_vec();
        push_chunk(&mut out, b"IEND", &[]);
        let err = PersonaCard::parse_bytes(&out).expect_err("无 chara 应报错");
        assert!(err.contains("没有角色卡数据"), "err = {err}");
    }

    #[test]
    fn compose_skips_empty_and_includes_discipline() {
        let card = PersonaCard {
            name: "NEKO".into(),
            description: "猫娘".into(),
            ..Default::default()
        };
        let with = compose_system_prompt(&card, true);
        assert!(with.contains("[角色] NEKO"));
        assert!(with.contains("猫娘"));
        assert!(with.contains("【对话纪律】"));
        let without = compose_system_prompt(&card, false);
        assert!(!without.contains("【对话纪律】"));
        assert_eq!(compose_system_prompt(&PersonaCard::default(), false), "");
    }

    #[test]
    fn start_applies_and_shutdown_restores_base() {
        let dir = std::env::temp_dir().join(format!("l2d-persona-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("live2d-ai.toml").display().to_string();

        let calls: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let card = r#"{"spec":"chara_card_v2","data":{"name":"NEKO","description":"猫娘","first_mes":"你好"}}"#;
        let mut rt = PersonaRuntime::new(
            capturing_services(calls.clone(), "BASE", config_path.clone()),
            PersonaConfig::from_value(&serde_json::json!({
                "card_json": card,
                "include_discipline": true,
            })),
        );
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).expect("start");
        assert_eq!(mock.specs.len(), 1, "应注册 settings schema");
        assert_eq!(mock.specs[0].fields.len(), 8);

        {
            let got = calls.lock().unwrap();
            assert_eq!(got.len(), 1, "start 应写一次 system_prompt");
            let prompt = got[0]["persona"]["system_prompt"].as_str().unwrap();
            assert!(prompt.contains("[角色] NEKO"), "prompt = {prompt}");
            assert!(prompt.contains("猫娘"));
            assert!(prompt.contains("【对话纪律】"));
        }

        // 基线快照落盘（与 live2d-ai.toml 同目录）。
        let saved = std::fs::read_to_string(dir.join(BASE_STATE_FILE)).expect("基线快照应存在");
        assert_eq!(saved, "BASE");

        rt.shutdown().expect("shutdown");
        let got = calls.lock().unwrap();
        assert_eq!(got.len(), 2, "shutdown 应还原一次");
        assert_eq!(got[1]["persona"]["system_prompt"], "BASE");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 手工覆盖优先于卡字段；card_path 不存在时如实报错且不改主链。
    #[test]
    fn overrides_win_and_bad_path_keeps_main_prompt() {
        let calls: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let mut rt = PersonaRuntime::new(
            capturing_services(calls.clone(), "BASE", String::new()),
            PersonaConfig::from_value(&serde_json::json!({
                "card_json": r#"{"name":"CARD","description":"卡描述"}"#,
                "name": "OVERRIDE",
            })),
        );
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        {
            let got = calls.lock().unwrap();
            let prompt = got[0]["persona"]["system_prompt"].as_str().unwrap();
            assert!(prompt.contains("OVERRIDE"), "覆盖应生效: {prompt}");
            assert!(!prompt.contains("CARD"), "卡里的 name 应被覆盖: {prompt}");
        }

        let calls2: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
        let mut bad = PersonaRuntime::new(
            capturing_services(calls2.clone(), "BASE", String::new()),
            PersonaConfig::from_value(&serde_json::json!({
                "card_path": "/definitely/not/here.png",
            })),
        );
        bad.start(&mut MockRegistrar::default()).unwrap();
        assert!(calls2.lock().unwrap().is_empty(), "读卡失败不得改主链");
    }
}
