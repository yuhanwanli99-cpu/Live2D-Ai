//! live2d-ai-mod-director（Wave 3 G 轨，2026-09-14）——**导演最小骨架**。
//!
//! 一句话：把每一轮的输入正文经**纯函数**推导成
//! `{emotion, intent, suggested_tts:{speed,pitch}}`，**只写日志 + state_json**。
//! 动作通道 `ModServices::action_tx` **保持休眠**（本 crate 从不调用它），
//! 也**不**经 `apply_settings` 投递任何 TTS 参数——骨架里没有任何下行通道。
//!
//! # 与 Wave 2 RFC 的关系
//!
//! Wave 2 D 轨只交了一份契约草案（[`docs/architecture/director-rfc.md`]）。
//! 本波把它推进到**可启用的最小骨架**：有 crate、有 `settings_spec`、有事件订阅、
//! 有可观察状态面。RFC §5.1 的晋升门槛（用户可见演示、≥20 条纯函数单测、
//! 端点红线可测……）**尚未**全部满足，因此本 Mod 的定位是**骨架**而不是 v1；
//! 详见 [`docs/architecture/director-mod-v0.md`]。
//!
//! # 输入（订阅两个主题，正好两个）
//!
//! | 主题 | payload | 用途 |
//! | --- | --- | --- |
//! | [`ModEventTopic::TurnPrompt`] | 本轮输入**正文** | 推导情绪 / 意图 / 建议参数 |
//! | [`ModEventTopic::TurnEnded`] | turn id | 把本轮决策**结项**（写上 turn id、置 `closed`） |
//!
//! `TurnPrompt` 不带 turn id、`TurnEnded` 不带正文，两者由 host 在同一个 worker 上
//! **顺序投递**，因此「最近一条未结项决策」就是本轮（见 [`ledger`] 头注）。
//!
//! # 不做（红线，逐条可测）
//!
//! - **不投递**：不调 `action_tx`、不调 `apply_settings`、不写 `live2d-ai.toml`
//!   （回归 `tests::action_tx_and_apply_settings_are_never_called`）；
//! - **不复活动作**：不 `use` core 动作类型、不产出任何 `channel != "none"`；
//! - **不起第二个 LLM**：推导是本地纯函数（无网络 / 无时钟 / 无随机）；
//! - **不写别人的字段**：不碰 `persona.system_prompt`、不碰壁纸偏好、不碰 `[tts]`。
//!
//! # 状态面（`ModRuntime::state_json` → `GET /api/v1/mods/director/state`）
//!
//! ```json
//! {
//!   "delivered": false,
//!   "channel": "none",
//!   "turns_seen": 3,
//!   "turns_ended": 3,
//!   "decisions": 2,
//!   "silent": 1,
//!   "errors": 0,
//!   "log_capacity": 20,
//!   "emotion_lexicon": "builtin",
//!   "latest": {
//!     "seq": 1,
//!     "turn": "1",
//!     "emotion": "happy",
//!     "intent": "greeting",
//!     "suggested_tts": { "speed": 1.08, "pitch": 1.2 },
//!     "closed": true,
//!     "delivered": false
//!   },
//!   "recent_decisions": [
//!     {
//!       "seq": 1,
//!       "turn": "1",
//!       "emotion": "happy",
//!       "intent": "greeting",
//!       "suggested_tts": { "speed": 1.08, "pitch": 1.2 },
//!       "closed": true,
//!       "delivered": false
//!     }
//!   ]
//! }
//! ```
//!
//! `delivered` 恒 `false`、`channel` 恒 `"none"`——状态面**只描述决策，不承诺动作**。
//! `recent_decisions` 只留最近 `log_capacity` 条（缺省 20，钳在 1..=200），
//! 计数（`turns_seen` / `decisions` / `errors`）不受容量影响。
//!
//! `latest` = `recent_decisions` 的**最后一条**（最近一条决策，同一个
//! `LedgerEntry::to_json` 形状），空账本 → `null`——前端不必再从数组尾部自己取。
//!
//! # 一次性命令（产品级加强波次）
//!
//! `ModRuntime::command` 只认 `"clear"`：清空决策账本（内存），返回清空前的
//! counts；其它命令 → `Err(ModError::UnsupportedCommand)`（host 回 409
//! `unsupported_command`）。命令路径**同样不触碰**任何下行通道
//! （回归 `tests::command_paths_never_touch_action_or_settings`）。
//!
//! # 日志纪律
//!
//! 日志只写**推导结果与正文长度**，**不写正文原文**（请求体含用户提示词，
//! 与 `web_api/dispatch.rs` 的「不记录请求体」同一条纪律）。错误码前缀 `director_`。
//!
//! [`docs/architecture/director-rfc.md`]: ../../../docs/architecture/director-rfc.md
//! [`docs/architecture/director-mod-v0.md`]: ../../../docs/architecture/director-mod-v0.md

pub mod decision;
pub mod ledger;

pub use decision::{
    Decision, EmotionHint, IntentHint, Lexicon, MAX_TEXT_CHARS, TtsSuggestion, derive,
};
pub use ledger::{
    DEFAULT_LOG_CAPACITY, DecisionLedger, LedgerCounts, LedgerEntry, MAX_LOG_CAPACITY,
    MIN_LOG_CAPACITY, PromptOutcome, clamp_log_capacity,
};

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。id `director` 与 RFC / 路由 / 文档同源。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "director",
    name: "导演",
    version: "0.1.0",
    // 对齐 Mod 系统 API 版本；不对齐时 host 会把本 Mod 置 Failed（主链不崩）。
    api_version: MOD_API_VERSION,
};

/// `emotion_lexicon` 缺省档位。
pub const DEFAULT_LEXICON: Lexicon = Lexicon::Builtin;

/// 本 Mod 的 namespaced 配置（**没有**第二个 `enabled`：启停唯一真源 = manifest）。
#[derive(Debug, Clone, PartialEq)]
pub struct DirectorConfig {
    /// 决策日志容量（最近 N 条）。
    pub log_capacity: usize,
    /// 情绪词表档位。
    pub emotion_lexicon: Lexicon,
}

impl Default for DirectorConfig {
    fn default() -> Self {
        Self {
            log_capacity: DEFAULT_LOG_CAPACITY,
            emotion_lexicon: DEFAULT_LEXICON,
        }
    }
}

impl DirectorConfig {
    /// 从 namespaced JSON 读配置：**越界只钳、类型不对只用缺省**，绝不失败。
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
        }
    }
}

/// 本 Mod 的设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
///
/// 两个字段，**没有** `enabled`（启停唯一真源是 Mod manifest 的 `enabled`，
/// 与 external-input / pet-desktop / memory 同口径）。
pub fn director_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 1,
        fields: vec![
            ModSettingField::Number {
                key: "log_capacity".to_string(),
                label: format!(
                    "决策日志容量（{MIN_LOG_CAPACITY}–{MAX_LOG_CAPACITY}，缺省 {DEFAULT_LOG_CAPACITY}）"
                ),
                min: MIN_LOG_CAPACITY as f64,
                max: MAX_LOG_CAPACITY as f64,
            },
            ModSettingField::Select {
                key: "emotion_lexicon".to_string(),
                label: "情绪词表".to_string(),
                options: vec![
                    SelectOption {
                        value: "builtin".to_string(),
                        label: "内置词表（缺省）".to_string(),
                    },
                    SelectOption {
                        value: "strict".to_string(),
                        label: "仅强关键词（保守）".to_string(),
                    },
                ],
            },
        ],
    }
}

/// 导演 Mod 运行时（骨架：观察 → 纯函数推导 → 写账本；无下行通道）。
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
        // 正好订阅两个主题：正文来源 + 轮末收口。
        registrar.subscribe(ModEventTopic::TurnPrompt)?;
        registrar.subscribe(ModEventTopic::TurnEnded)?;
        self.registered = true;
        self.services.logger.info(&format!(
            "director Mod 已启动（骨架：只记决策日志、不投递；lexicon={}, log_capacity={}）",
            self.config.emotion_lexicon.as_str(),
            self.ledger.capacity()
        ));
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        match topic {
            ModEventTopic::TurnPrompt => {
                let len = payload.chars().count();
                match self
                    .ledger
                    .record_prompt(payload, self.config.emotion_lexicon)
                {
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
                    } => {
                        self.services.logger.info(&format!(
                            "director 本轮决策（len={len}，未投递）: emotion={}, intent={}, suggested_tts={{speed:{speed}, pitch:{pitch}}}",
                            emotion.as_str(),
                            intent.as_str()
                        ));
                    }
                }
                Ok(())
            }
            ModEventTopic::TurnEnded => {
                if self.ledger.record_ended(payload) {
                    self.services.logger.info(&format!(
                        "director 轮 {payload} 已结项（action_tx 从未调用，零副作用）"
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
            .info("director Mod 已关闭（骨架无残留：从未写配置 / 从未投递）");
        Ok(())
    }

    /// 只读运行态快照——形状与语义见 crate 头注「状态面」。
    ///
    /// 纯内存读取：不写盘、不加锁等待、不发网络请求（host 在 web_api 线程调用）。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        let mut value = self.ledger.state_json();
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "emotion_lexicon".to_string(),
                serde_json::json!(self.config.emotion_lexicon.as_str()),
            );
        }
        Some(value)
    }

    /// **一次性命令**（产品级加强波次）：只认 `"clear"`。
    ///
    /// - `clear` → `Ok({"cleared": <清掉的条目数>, "counts": {清空前的计数}})`；
    /// - 其它命令 → `Err(ModError::UnsupportedCommand)`（host 回 409
    ///   `unsupported_command`，前端据此说「这个 Mod 没有这个动作」）。
    ///
    /// 与 `state_json` 同一条纪律：**只动内存**——不写盘、不发网络请求、不阻塞，
    /// 且**不触碰任何下行通道**（`action_tx` / `apply_settings` 零调用，
    /// 回归 `tests::command_paths_never_touch_action_or_settings`）。
    fn command(
        &mut self,
        command: &str,
        _args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        match command {
            "clear" => {
                let (cleared, before) = self.ledger.clear();
                self.services.logger.info(&format!(
                    "director 命令 clear：清空 {cleared} 条决策（清空前 counts: turns_seen={}, decisions={}, silent={}, errors={}；未投递）",
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

/// 导演 Mod 工厂。
pub struct DirectorFactory;

impl ModFactory for DirectorFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// **静态** schema：未启用也能渲染配置表单（缺省停用的 Mod 要先让用户填）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(director_settings_spec())
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(DirectorRuntime::new(
            services,
            DirectorConfig::from_value(&config),
        )))
    }
}

/// 工厂单例（**已**注册进 `AVAILABLE_MOD_FACTORIES`，当前注册面共 5 个；缺省停用）。
pub const FACTORY: DirectorFactory = DirectorFactory;

#[cfg(test)]
mod tests;
