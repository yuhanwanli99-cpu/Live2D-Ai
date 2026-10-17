//! live2d-ai-mod-director —— **已退休的导演 Mod**（2026-10-17，rc4）。
//!
//! # 现状（不要再按旧文档理解）
//!
//! 按句第二路（清洗 + 按这一句原文出动作）与规则表已**迁入核心**：
//! - 规则表 / 情绪意图纯函数：`live2d_ai_runtime::performance::rules`；
//! - 按句清洗与动作 cue：`live2d_ai_runtime::conversation`（每句恰一次，
//!   动作恒开、**不设关闭开关**）。
//!
//! 本 crate **仍在 workspace 里、仍可编译可测**，但：
//! - **不再注册进 `AVAILABLE_MOD_FACTORIES`**（工厂数由 desktop 的断言守住）；
//! - **不再投递 `action_cue`**（`ModServices.cues` 零调用）；不接 `action_tx`；
//! - **不再订阅 `SentenceReady`**；staging / takeover / 异步第二路 LLM 已删除；
//! - 只保留最小运行时：`TurnPrompt` → 决策日志、`TurnEnded` → 结项、
//!   `state_json` 只读（不再有 staging / plan 子面）。
//!
//! # 兼容层（旧路径仍可用）
//!
//! - [`decision`]：`pub use` runtime `performance::rules::decision::*`；
//! - [`presets`]：`pub use` runtime `performance::rules::presets::*`；
//! - [`motion_bias`]：保留（runtime `cleaning::MotionBias` 的三档文案真源；
//!   host 只读 mods.json 既有 `mods.director.config.motion_bias` 键，不新增键 / UI）。
//!
//! # 红线（逐条可测）
//!
//! - 不调 `action_tx` / `apply_settings`、不写 `live2d-ai.toml`；
//! - 不产任何 cue、不发 HTTP（没有第二路 LLM）；
//! - **不重新注册**（禁止挂回；恢复需要维护者新裁决）；
//! - 日志只写推导结果与正文长度，**不写正文原文**。

pub mod decision;
pub mod ledger;
pub mod motion_bias;
pub mod presets;

pub use decision::{
    Decision, EmotionHint, IntentHint, Lexicon, MAX_TEXT_CHARS, TtsSuggestion, derive,
};
pub use ledger::{
    DEFAULT_LOG_CAPACITY, DecisionLedger, LedgerCounts, LedgerEntry, MAX_LOG_CAPACITY,
    MIN_LOG_CAPACITY, PromptOutcome, clamp_log_capacity,
};
pub use presets::{PRESET_IDS, PRESET_NONE, PresetTable};

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。**保留**，但本 Mod 已退出工厂注册表。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "director",
    name: "导演（已退休）",
    version: "0.2.0",
    // 对齐 Mod 系统 API 版本；不对齐时 host 会把本 Mod 置 Failed（主链不崩）。
    api_version: MOD_API_VERSION,
};

/// `emotion_lexicon` 缺省档位。
pub const DEFAULT_LEXICON: Lexicon = Lexicon::Builtin;

/// 本 Mod 的 namespaced 配置（**没有**第二个 `enabled`：启停唯一真源 = manifest）。
///
/// rc4 起 `staging_*` 五个键**已删除**：第二路并入核心后本 crate 不再发 HTTP。
/// `motion_bias` 保留（mods.json 既有键，host 读它投影给 runtime 的按句清洗器）。
#[derive(Debug, Clone, PartialEq)]
pub struct DirectorConfig {
    /// 决策日志容量（最近 N 条）。
    pub log_capacity: usize,
    /// 情绪词表档位。
    pub emotion_lexicon: Lexicon,
    /// **动作预设表**（emotion|intent → preset_id；规则实现已迁入 runtime rules）。
    pub presets: PresetTable,
    /// 表演倾向（2026-10-13）：克制 / 平常 / 主动；缺省平常。
    ///
    /// 只影响主链按句清洗的提示词与空数组回退（由 host 投影给 runtime），
    /// 不影响本 Mod 自己的决策日志。
    pub motion_bias: motion_bias::MotionBias,
}

impl Default for DirectorConfig {
    fn default() -> Self {
        Self {
            log_capacity: DEFAULT_LOG_CAPACITY,
            emotion_lexicon: DEFAULT_LEXICON,
            presets: PresetTable::default(),
            motion_bias: motion_bias::MotionBias::Normal,
        }
    }
}

impl DirectorConfig {
    /// 从 namespaced JSON 读配置：**越界只钳、类型不对只用缺省**，绝不失败。
    ///
    /// `staging_*` 键即使仍留在 mods.json 里也**被忽略**（退休后不再读取）。
    pub fn from_value(value: &serde_json::Value) -> Self {
        let log_capacity = value
            .get("log_capacity")
            .and_then(serde_json::Value::as_f64)
            .filter(|f| f.is_finite())
            .map(|f| clamp_log_capacity((f.round() as i64).max(0) as usize))
            .unwrap_or(DEFAULT_LOG_CAPACITY);
        let emotion_lexicon = Lexicon::parse(
            value
                .get("emotion_lexicon")
                .and_then(serde_json::Value::as_str),
        );
        Self {
            log_capacity,
            emotion_lexicon,
            presets: PresetTable::from_value(value),
            motion_bias: motion_bias::MotionBias::parse(
                value.get("motion_bias").and_then(serde_json::Value::as_str),
            ),
        }
    }
}

/// 本 Mod 的设置 schema（**静态**：未启用也拿得到）。
///
/// rc4 起只剩 3 个预设 Select + 表演倾向；`staging_*` 五项已删除。
/// **没有** `enabled`（启停唯一真源是 Mod manifest）。
pub fn director_settings_spec() -> ModSettingsSpec {
    // 缺省映射表就是这张表单的默认值来源（单一真源：PresetTable::default）。
    let d = PresetTable::default();
    let mut fields = vec![
        preset_field("preset_happy", "开心 → 动作", &d.happy),
        preset_field("preset_sad", "难过 → 动作", &d.sad),
        preset_field("preset_greeting", "打招呼 → 动作", &d.greeting),
    ];
    // 表演倾向（2026-10-13）：字段本体在 motion_bias.rs，这里只 push 一次。
    fields.push(motion_bias::MotionBias::setting_field());
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: "导演".to_string(),
        version: 2,
        fields,
    }
}

/// 一条 Select 预设字段：选项来自 presets::PRESET_IDS（单一真源），
/// default = 内置映射表的缺省（不是「第一个选项」——第一个是 none）。
fn preset_field(key: &str, label: &str, default: &str) -> ModSettingField {
    ModSettingField::Select {
        key: key.to_string(),
        label: label.to_string(),
        options: presets::PRESET_IDS
            .iter()
            .map(|id| SelectOption {
                value: (*id).to_string(),
                label: preset_label(id),
            })
            .collect(),
        default: Some(default.to_string()),
    }
}

/// 嵌入的标签表：`assets/actions/preset_labels.json`（**展示名唯一真源**）。
///
/// **不在 Rust 里再手写一套中文标签**（调研
/// `docs/research/preset-label-map-2026-09.md`）：Flutter 调试面板、本 Select、
/// 文档读的是同一份表。表是**编译期**嵌入的（展示名不需要热改）；
/// 预设**行为**仍在运行期可改的 `assets/actions/presets.json` 里。
const PRESET_LABELS_JSON: &str = include_str!("../../../assets/actions/preset_labels.json");

/// 一条标签：中文名 + 通道。
struct PresetLabelRow {
    zh: String,
    channel: String,
}

/// 解析一次的标签表（进程内缓存）。
fn preset_label_map() -> &'static std::collections::HashMap<String, PresetLabelRow> {
    static MAP: std::sync::OnceLock<std::collections::HashMap<String, PresetLabelRow>> =
        std::sync::OnceLock::new();
    MAP.get_or_init(|| {
        let mut out = std::collections::HashMap::new();
        let Ok(value) = serde_json::from_str::<serde_json::Value>(PRESET_LABELS_JSON) else {
            return out;
        };
        let Some(labels) = value.get("labels").and_then(|v| v.as_object()) else {
            return out;
        };
        for (id, entry) in labels {
            let zh = entry
                .get("zh")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if zh.is_empty() {
                continue;
            }
            let channel = entry
                .get("channel")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            out.insert(
                id.clone(),
                PresetLabelRow {
                    zh: zh.to_string(),
                    channel,
                },
            );
        }
        out
    })
}

/// 预设 id → 面板展示名（读嵌入的标签表；未知 id 原样显示、**不隐藏**）。
///
/// `none` 在**导演 Select** 里的语义是「本轮不投递」（不是中性情绪），
/// 与标签表里 `none → 中性` 的渲染面撤销语义不同，所以这里单独给。
fn preset_label(id: &str) -> String {
    if id == PRESET_NONE {
        return "不投递 (none)".to_string();
    }
    let Some(row) = preset_label_map().get(id) else {
        return id.to_string();
    };
    let channel = match row.channel.as_str() {
        "expression" => "（表情）",
        "motion" => "（短动作）",
        _ => "",
    };
    format!("{}{} ({id})", row.zh, channel)
}

/// 最小运行时：观察一轮输入 → 纯函数决策 → 写内存账本 → 只读状态面。
///
/// **零下行通道**：不投 cue、不写配置、不发 HTTP。
pub struct DirectorRuntime {
    services: ModServices,
    config: DirectorConfig,
    ledger: DecisionLedger,
    /// `start` 是否已注册 schema（单测断言用）。
    registered: bool,
}

impl DirectorRuntime {
    /// 用注入的 host services + 配置构造（不注册、不碰盘）。
    pub fn new(services: ModServices, config: DirectorConfig) -> Self {
        let ledger = DecisionLedger::new(config.log_capacity);
        Self {
            services,
            config,
            ledger,
            registered: false,
        }
    }

    /// settings schema 是否已注册。
    pub fn is_registered(&self) -> bool {
        self.registered
    }

    /// 当前生效配置。
    pub fn config(&self) -> &DirectorConfig {
        &self.config
    }

    /// 决策账本（只读）。
    pub fn ledger(&self) -> &DecisionLedger {
        &self.ledger
    }
}

impl ModRuntime for DirectorRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 与 DirectorFactory::settings_spec() 同一份（单测钉住不分叉）。
        registrar.register_settings(director_settings_spec())?;
        // 正好订阅两个主题：本轮正文 + 轮末收口。**不再订阅 SentenceReady**。
        registrar.subscribe(ModEventTopic::TurnPrompt)?;
        registrar.subscribe(ModEventTopic::TurnEnded)?;
        self.registered = true;
        // 日志必须反映现状：本 Mod 已退休，按句第二路在核心主链。
        self.services.logger.info(&format!(
            "director Mod 已启动（已退休的最小运行时：只记决策日志 / state_json；按句清洗与动作 cue 由核心主链负责；不投递 cue、不发 HTTP；lexicon={}, log_capacity={}）",
            self.config.emotion_lexicon.as_str(),
            self.ledger.capacity()
        ));
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        match topic {
            ModEventTopic::TurnPrompt => {
                let len = payload.chars().count();
                match self.ledger.record_prompt(
                    payload,
                    self.config.emotion_lexicon,
                    &self.config.presets,
                ) {
                    PromptOutcome::Silent => {
                        self.services.logger.info(&format!(
                            "director 收到空正文（len={len}），本轮静默：无决策、零副作用"
                        ));
                    }
                    PromptOutcome::Decided {
                        emotion,
                        intent,
                        speed,
                        pitch,
                        preset_id,
                    } => {
                        // 只写日志 / 账本；动作由核心主链按句产出，不从这里投递。
                        self.services.logger.info(&format!(
                            "director 本轮决策（len={len}）: emotion={}, intent={}, suggested_tts={{speed:{speed}, pitch:{pitch}}}, preset={}",
                            emotion.as_str(),
                            intent.as_str(),
                            preset_id.as_deref().unwrap_or("none")
                        ));
                    }
                }
                Ok(())
            }
            ModEventTopic::TurnEnded => {
                if self.ledger.record_ended(payload) {
                    self.services.logger.info(&format!(
                        "director 轮 {payload} 已结项（action_tx / cue 通道从未调用，零副作用）"
                    ));
                } else {
                    self.services.logger.warn(&format!(
                        "director_orphan_turn_ended: 收到无在飞轮的 TurnEnded（turn={payload}），已计入 errors（主链不受影响）"
                    ));
                }
                Ok(())
            }
            other => {
                self.services
                    .logger
                    .info(&format!("director 忽略 {} 事件（未订阅）", other.as_str()));
                Ok(())
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.registered = false;
        self.services
            .logger
            .info("director Mod 已关闭（从未写配置；决策账本随运行时释放）");
        Ok(())
    }

    /// 只读运行态快照：账本 + 生效词表 / 映射表。
    ///
    /// 纯内存读取：不写盘、不加锁等待、不发网络请求（host 在 web_api 线程调用）。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        let mut value = self.ledger.state_json();
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "emotion_lexicon".to_string(),
                serde_json::json!(self.config.emotion_lexicon.as_str()),
            );
            // 生效的映射表：面板据此显示「现在会映射到哪条」。
            object.insert("presets".to_string(), self.config.presets.to_json());
            // 退休标记：没有 cue 通道、没有第二路 LLM；消费者不必再找 staging/plan。
            object.insert("retired".to_string(), serde_json::json!(true));
        }
        Some(value)
    }

    /// **一次性命令**：只认 `"clear"`（清空决策账本，内存 only）。
    ///
    /// 其它命令 → `Err(ModError::UnsupportedCommand)`（host 回 409）。
    /// 与 `state_json` 同一条纪律：**不触碰任何下行通道**。
    fn command(
        &mut self,
        command: &str,
        _args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        match command {
            "clear" => {
                let (cleared, before) = self.ledger.clear();
                self.services.logger.info(&format!(
                    "director 命令 clear：清空 {cleared} 条决策（清空前 counts: turns_seen={}, decisions={}, silent={}, errors={}）",
                    before.turns_seen, before.decisions, before.silent, before.errors
                ));
                Ok(serde_json::json!({
                    "cleared": cleared,
                    "counts": before.to_json(),
                }))
            }
            other => Err(ModError::UnsupportedCommand {
                command: other.to_string(),
            }),
        }
    }
}

/// 导演 Mod 工厂（**保留**；rc4 起**不再注册**进 `AVAILABLE_MOD_FACTORIES`）。
pub struct DirectorFactory;

impl ModFactory for DirectorFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// **静态** schema：即使不注册也仍可单独构造 / 测试。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(director_settings_spec())
    }

    /// 最小运行时装配：**没有**第二路客户端、没有 cue 通道。
    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        let config = DirectorConfig::from_value(&config);
        Ok(Box::new(DirectorRuntime::new(services, config)))
    }
}

/// 工厂单例（**已退出** `AVAILABLE_MOD_FACTORIES`，禁止挂回）。
pub const FACTORY: DirectorFactory = DirectorFactory;

#[cfg(test)]
mod tests;
// T9（2026-10-07）问句补丁的回归（经兼容层仍测 runtime 的真表）。
#[cfg(test)]
mod tests_question;
