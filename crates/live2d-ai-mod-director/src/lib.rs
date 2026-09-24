//! live2d-ai-mod-director（Wave 3 G 轨，2026-09-14；**动作预设投递** 2026-09-15）——**导演**。
//!
//! 一句话：把每一轮的输入正文经**纯函数**推导成
//! `{emotion, intent, suggested_tts:{speed,pitch}, preset_id}`，写日志 + state_json，
//! 其中 **preset_id 就是本轮要演的动作预设**（表情 / 短动作），经**只读状态面**
//! `latest.preset_id` 交给前端，由前端转发给渲染面（协议 v1 `preset` 消息）。
//! 动作通道 `ModServices::action_tx` **仍然休眠**（本 crate 从不调用它），
//! 也**不**经 `apply_settings` 投递任何 TTS 参数——下行只有「状态面 + 前端拉取」这一条。
//!
//! # 与 Wave 2 RFC 的关系
//!
//! Wave 2 D 轨只交了一份契约草案（[`docs/architecture/director-rfc.md`]）。
//! 本波把它推进到**可启用的最小骨架**：有 crate、有 `settings_spec`、有事件订阅、
//! 有可观察状态面。RFC §5.1 的晋升门槛（用户可见演示、≥20 条纯函数单测、
//! 端点红线可测……）**尚未**全部满足，因此本 Mod 的定位是**骨架**而不是 v1；
//! 详见 [`docs/architecture/director-mod-v0.md`]。
//!
//! # 行数（AGENTS.md「源码 ≤500 行」，豁免到 ≤1000）
//!
//! 本文件是 **Mod 集成面**：配置解析 + settings schema + 状态面 + 事件分发 +
//! 工厂装配（真实二路客户端注入）。拆开会把「同一份配置的读写」散到多处，
//! 反而更难保证不分叉；有头注理由的 ≤1000 豁免适用。
//!
//! # 输入（订阅三个主题）
//!
//! | 主题 | payload | 用途 |
//! | --- | --- | --- |
//! | [`ModEventTopic::TurnPrompt`] | 本轮输入**正文** | 推导情绪 / 意图 / 建议参数 |
//! | [`ModEventTopic::TurnEnded`] | turn id | 把本轮决策**结项**（写上 turn id、置 `closed`） |
//! | [`ModEventTopic::SentenceReady`] | JSON {epoch, ts_ms, sentence_seq, text} | 按句锚点：规则 cue + 异步二路（默认关） |
//!
//! `TurnPrompt` 不带 turn id、`TurnEnded` 不带正文，两者由 host 在同一个 worker 上
//! **顺序投递**，因此「最近一条未结项决策」就是本轮（见 [`ledger`] 头注）。
//!
//! # 不做（红线，逐条可测）
//!
//! - **不自己投递**：不调 `action_tx`、不调 `apply_settings`、不写 `live2d-ai.toml`
//!   （回归 `tests::action_tx_and_apply_settings_are_never_called`）。
//!   「投递」只发生在**前端**：它读 `latest.preset_id` 再经协议 v1 `preset` 消息
//!   交给渲染面——host 下行通道零调用这条不变；
//! - **不复活动作**：不 `use` core 动作类型、不产出任何 `channel != "none"`；
//! - **异步第二路 LLM 默认关，规则常开兜底**（P1-3，2026-09-16；真实客户端
//!   P1-4，2026-09-19）：规则推导仍是本地纯函数（无网络 / 无时钟 / 无随机）；
//!   异步第二路由 `staging_http::OpenAiStagingClient` 发**真实** HTTP
//!   （非流式 /chat/completions，独立 base_url/model/timeout/api_key_env）。
//!   **默认关**：没配端点 / `staging_enabled=false` → 仍 `DisabledStaging`，
//!   行为与「默认关」逐字一致（降级原因进 `state_json.staging.degraded/note`）。
//!   失败 / 超时 / 坏 JSON → 静默回退规则。它只产出按句 cue，**不改**送 TTS 的
//!   文本（导演是备注，不是誊写员）；
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
//!   ],
//!   "staging": {
//!     "enabled": false,
//!     "client": "disabled",
//!     "degraded": false,
//!     "note": null,
//!     "base_url": "",
//!     "model": "",
//!     "api_key_env": "",
//!     "api_key_set": false,
//!     "timeout_ms": 1500,
//!     "min_interval_ms": 1200,
//!     "max_per_turn": 3,
//!     "fires_this_turn": 0,
//!     "async_plans": 0,
//!     "async_failures": 0
//!   }
//! }
//! ```
//!
//! `staging` = 异步二路（P1-4）的可观察面：`client` ∈
//! `disabled` / `openai` / `injected`；`degraded=true` = 开了闸但没配端点/模型
//! （仍走规则层，`note` 给原因）；`async_failures` = 失败/超时/坏 JSON 次数。
//! 密钥**只回布尔** `api_key_set`，永不回值。
//!
//! `channel` 恒 `"preset"`：下行通道是「状态面 + 前端拉取」；
//! `delivered` = 最近一轮**是否选出了一条预设**（空账本 → `false`）。
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

pub mod arbiter;
pub mod decision;
pub mod ledger;
pub mod plan;
pub mod presets;
pub mod staging;
pub mod staging_http;

pub use arbiter::Arbiter;
pub use decision::{
    Decision, EmotionHint, IntentHint, Lexicon, MAX_TEXT_CHARS, TtsSuggestion, derive,
};
pub use ledger::{
    DEFAULT_LOG_CAPACITY, DecisionLedger, LedgerCounts, LedgerEntry, MAX_LOG_CAPACITY,
    MIN_LOG_CAPACITY, PromptOutcome, clamp_log_capacity,
};
pub use plan::{
    Cue, DirectorPlan, MAX_CUES, MAX_INTENSITY, MAX_TTL_MS, PRIORITY_ASYNC, PRIORITY_LABEL,
    PRIORITY_RULE, parse_plan,
};
pub use presets::{PRESET_IDS, PRESET_NONE, PresetTable};
pub use staging::{DisabledStaging, StagingClient, StagingSetup};

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
    /// **动作预设表**（emotion|intent → preset_id，2026-09-15）。
    pub presets: PresetTable,
    /// 异步第二路 LLM 总闸（缺省 false；P1-3）。
    ///
    /// **必须同时配齐** [Self::staging_base_url] 与 [Self::staging_model] 才会
    /// 真的发 HTTP；只要开闸没端点 → 仍 Disabled + state_json.staging.degraded
    /// （见 staging::assemble）。密钥走 [Self::staging_api_key_env]。
    pub staging_enabled: bool,
    /// 二路端点 base_url（OpenAI 兼容，独立于 `[llm]`；空 = 未配）。
    pub staging_base_url: String,
    /// 二路模型名（空 = 未配）。
    pub staging_model: String,
    /// 二路密钥的**环境变量名**（.env / 进程环境；空 = 不鉴权）。
    ///
    /// 与 `[llm].api_key_env` 同一条口径：配置只持**变量名**，值只住 `.env`。
    pub staging_api_key_env: String,
    /// 异步调用超时（毫秒；缺省 1500；钳 100..=5000）。
    pub staging_timeout_ms: u64,
    /// 两次异步触发的最小间隔（毫秒；缺省 1200）。
    pub staging_min_interval_ms: u64,
    /// 每轮异步触发上限（缺省 3）。
    pub staging_max_per_turn: usize,
}

impl Default for DirectorConfig {
    fn default() -> Self {
        Self {
            log_capacity: DEFAULT_LOG_CAPACITY,
            emotion_lexicon: DEFAULT_LEXICON,
            presets: PresetTable::default(),
            staging_enabled: false,
            staging_base_url: String::new(),
            staging_model: String::new(),
            staging_api_key_env: String::new(),
            staging_timeout_ms: 1_500,
            staging_min_interval_ms: 1_200,
            staging_max_per_turn: 3,
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
        let presets = PresetTable::from_value(value);
        let millis = |key: &str, default: u64, lo: u64, hi: u64| {
            value
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .filter(|f| f.is_finite())
                .map(|f| (f.round() as i64).clamp(lo as i64, hi as i64) as u64)
                .unwrap_or(default)
        };
        // 字符串配置：类型不对 / 缺省 → 空串（**不失败**，与其余字段同口径）。
        let text = |key: &str| {
            value
                .get(key)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string()
        };
        Self {
            log_capacity,
            emotion_lexicon,
            presets,
            staging_enabled: value
                .get("staging_enabled")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            staging_base_url: text("staging_base_url"),
            staging_model: text("staging_model"),
            staging_api_key_env: text("staging_api_key_env"),
            staging_timeout_ms: millis("staging_timeout_ms", 1_500, 100, 5_000),
            staging_min_interval_ms: millis("staging_min_interval_ms", 1_200, 0, 10_000),
            staging_max_per_turn: millis("staging_max_per_turn", 3, 0, 10) as usize,
        }
    }
}

/// 本 Mod 的设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
///
/// 两个字段，**没有** `enabled`（启停唯一真源是 Mod manifest 的 `enabled`，
/// 与 external-input / pet-desktop / memory 同口径）。
pub fn director_settings_spec() -> ModSettingsSpec {
    // 缺省映射表就是这张表单的默认值来源（单一真源：PresetTable::default）。
    let d = PresetTable::default();
    ModSettingsSpec {
        mod_id: DESCRIPTOR.id.to_string(),
        title: DESCRIPTOR.name.to_string(),
        version: 2,
        // **只留 3 个常用档**（2026-09-15 用户裁决：可操控项过多要收）。
        // 其余 5 档（亲昵 / 生气 / 惊讶 / 焦虑 / 告别）不再上表单，一律走
        // PresetTable::default() 的内置映射；log_capacity / emotion_lexicon
        // 同样退出表单（缺省 20 / builtin），配置里显式写了仍然生效。
        fields: vec![
            preset_field("preset_happy", "开心 → 动作", &d.happy),
            preset_field("preset_sad", "难过 → 动作", &d.sad),
            preset_field("preset_greeting", "打招呼 → 动作", &d.greeting),
            // P1-4（2026-09-19）：**要哪些键才能真开二路**（面板可直接填）。
            // 只开了 staging_enabled 还不够——必须再配 base_url + model，
            // 否则仍走规则层（state_json.staging.degraded 为 true，可观察）。
            //
            // 2026-09-22：表演层（`[performance]`，runtime 主链）已是**表演主路由**；
            // 这 5 个 `staging_*` 是**遗留回退旁路**。label 一律带「【遗留】」——
            // 不然表单看起来像「要开表演得先配这里」，用户会以为两套大脑在抢 cue。
            ModSettingField::Bool {
                key: "staging_enabled".to_string(),
                label: "【遗留】二路 LLM（日常用表演层；未配端点则仅规则）".to_string(),
                default: false,
            },
            ModSettingField::String {
                key: "staging_base_url".to_string(),
                label: "【遗留】二路端点 base_url（如 http://127.0.0.1:11434/v1；空=仅规则）"
                    .to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "staging_model".to_string(),
                label: "【遗留】二路模型名（空=仅规则）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "staging_api_key_env".to_string(),
                label: "【遗留】二路密钥变量名（.env 里的名字；空=不鉴权）".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::Number {
                key: "staging_timeout_ms".to_string(),
                label: "【遗留】二路超时（毫秒，100~5000）".to_string(),
                min: 100.0,
                max: 5_000.0,
            },
        ],
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

/// 导演 Mod 运行时（观察 → 纯函数推导 → 写账本 → 产出动作预设经状态面交付）。
pub struct DirectorRuntime {
    services: ModServices,
    config: DirectorConfig,
    ledger: DecisionLedger,
    /// `start` 是否已注册 schema（单测断言用）。
    registered: bool,
    /// 异步第二路 LLM 客户端（缺省 Disabled；host 装配时按配置注入）。
    staging: Box<dyn StagingClient>,
    /// 「开了二路但没接上」的原因（缺端点 / 缺模型 / 密钥变量查不到）；
    /// 只写日志与 state_json（**不含密钥值**）。
    staging_note: Option<String>,
    /// 当前 epoch 的按句仲裁表。
    arbiter: Arbiter,
    /// 当前 epoch（SentenceReady 的 epoch）。
    current_epoch: u64,
    /// 最近一次异步触发的 ts_ms（本轮相对毫秒）。
    last_fire_ms: Option<u64>,
    /// 本轮已触发异步次数。
    fires_this_turn: u32,
    /// 最近一轮的用户正文（异步 prompt 的输入；不写日志）。
    last_user_text: String,
    /// 统计。
    sentences_seen: u64,
    rule_cues: u64,
    async_plans: u64,
    async_failures: u64,
    cues_emitted: u64,
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
            staging: Box::new(DisabledStaging),
            staging_note: None,
            arbiter: Arbiter::default(),
            current_epoch: 0,
            last_fire_ms: None,
            fires_this_turn: 0,
            last_user_text: String::new(),
            sentences_seen: 0,
            rule_cues: 0,
            async_plans: 0,
            async_failures: 0,
            cues_emitted: 0,
        }
    }

    /// 注入异步第二路 LLM 客户端（缺省 Disabled；测试替身 / 显式注入用）。
    pub fn with_staging(mut self, staging: Box<dyn StagingClient>) -> Self {
        self.staging = staging;
        self.staging_note = None;
        self
    }

    /// 注入 **host 装配**（[`staging::assemble`]）的结果：客户端 + degraded 原因。
    ///
    /// 生产路径唯一入口（`DirectorFactory::create` 调它）；缺配置时 client 仍是
    /// `DisabledStaging`，行为与「默认关」逐字一致。
    pub fn with_staging_setup(mut self, setup: StagingSetup) -> Self {
        self.staging = setup.client;
        self.staging_note = setup.degraded_note;
        self
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

    /// SentenceReady 的一轮处理（P1-3）：规则常开兜底 + 异步覆盖（默认关）。
    fn handle_sentence_ready(&mut self, payload: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
            self.services
                .logger
                .warn("director_sentence_ready_bad_payload: 不是合法 JSON，忽略本条");
            return;
        };
        let epoch = value
            .get("epoch")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let ts_ms = value
            .get("ts_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let sentence_seq = value
            .get("sentence_seq")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let text = value
            .get("text")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        // **epoch=0 是合法的首轮**（core 只在 stop 时推进 epoch；实测真实链路
        // 普通轮次恒为 0）。旧守卫「epoch == 0 就丢」会让规则层与二路在
        // **所有普通轮次**都不工作——这正是活服务自测抓到的缺陷。
        if sentence_seq == 0 {
            return;
        }
        if epoch != self.current_epoch {
            self.arbiter.reset(epoch);
            self.current_epoch = epoch;
            self.last_fire_ms = None;
            self.fires_this_turn = 0;
        }
        self.sentences_seen += 1;

        // 规则兜底（常开）：本轮规则决策的 preset 绑到第一句。
        if sentence_seq == 1 {
            let preset = self.ledger.latest().and_then(|e| e.preset_id.clone());
            let rule = DirectorPlan::rule(epoch, preset.as_deref(), 1, 2_000);
            if !rule.cues.is_empty() && self.arbiter.apply(&rule) {
                self.rule_cues += 1;
            }
        }

        // 异步覆盖（默认关）：首句触发 + 每轮上限 + 最小间隔。
        if self.should_fire_async(ts_ms, sentence_seq) {
            self.fire_async(epoch, ts_ms, text);
        }

        self.emit_cues(epoch);
    }

    /// 是否该触发异步第二路（节流：首句 + 间隔 + 每轮上限）。
    fn should_fire_async(&self, ts_ms: u64, sentence_seq: u64) -> bool {
        if !self.config.staging_enabled || !self.staging.enabled() {
            return false;
        }
        if self.fires_this_turn as usize >= self.config.staging_max_per_turn {
            return false;
        }
        if self.fires_this_turn == 0 {
            return true;
        }
        if sentence_seq <= self.arbiter.covers_upto_seq() {
            return false;
        }
        match self.last_fire_ms {
            Some(last) => ts_ms.saturating_sub(last) >= self.config.staging_min_interval_ms,
            None => true,
        }
    }

    /// 触发一次异步第二路；失败 / 超时 / JSON 坏一律静默回退规则。
    ///
    /// epoch 必须进 prompt：模型的 plan 要原样回填它，否则 arbiter 的 epoch
    /// 硬闸会把整份 plan 丢掉（见 staging::build_user_prompt）。
    fn fire_async(&mut self, epoch: u64, ts_ms: u64, text: &str) {
        self.fires_this_turn += 1;
        self.last_fire_ms = Some(ts_ms);
        let prompt =
            staging::build_user_prompt(epoch, &self.last_user_text, text, presets::PRESET_IDS);
        match self.staging.complete(
            staging::STAGING_SYSTEM,
            &prompt,
            self.config.staging_timeout_ms,
        ) {
            Some(raw) => match plan::parse_plan(&raw, presets::PRESET_IDS) {
                Ok((plan, warnings)) => {
                    if !warnings.is_empty() {
                        self.services.logger.warn(&format!(
                            "director_staging_plan_warnings: {}",
                            warnings.join("；")
                        ));
                    }
                    if self.arbiter.apply(&plan) {
                        self.async_plans += 1;
                    }
                }
                Err(e) => {
                    self.async_failures += 1;
                    self.services.logger.warn(&format!(
                        "director_staging_plan_invalid: {e}（静默回退规则）"
                    ));
                }
            },
            None => {
                self.async_failures += 1;
                self.services
                    .logger
                    .info("director_staging_failed_or_disabled: 静默回退规则层");
            }
        }
    }

    /// 把当前仲裁表的 plan 经 host 的 cue 通道推给前端（无通道 / 空表 -> no-op）。
    fn emit_cues(&mut self, epoch: u64) {
        if !self.services.cues.enabled() || self.arbiter.is_empty() {
            return;
        }
        let cues: Vec<serde_json::Value> = self.arbiter.cues().map(Cue::to_json).collect();
        let payload = serde_json::json!({
            "epoch": epoch,
            "covers_upto_seq": self.arbiter.covers_upto_seq(),
            "cues": cues,
        });
        if self.services.cues.send(payload) {
            self.cues_emitted += 1;
            // 排障锚点：这条日志 = host 真的收到了**这一轮这一句**的 plan
            // （只写 epoch / 覆盖范围 / 条数，不写正文）。
            self.services.logger.info(&format!(
                "director action_cue：epoch={epoch}, covers_upto_seq={}, cues={}",
                self.arbiter.covers_upto_seq(),
                self.arbiter.len()
            ));
        }
    }
}

impl ModRuntime for DirectorRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 与 DirectorFactory::settings_spec() 同一份（单测钉住不分叉）。
        registrar.register_settings(director_settings_spec())?;
        // 正好订阅两个主题：正文来源 + 轮末收口。
        registrar.subscribe(ModEventTopic::TurnPrompt)?;
        registrar.subscribe(ModEventTopic::TurnEnded)?;
        // P1-3：句子交给 TTS 之前的锚点（异步导演按句对齐）。
        registrar.subscribe(ModEventTopic::SentenceReady)?;
        self.registered = true;
        // 日志必须反映**现状**：动作预设经只读状态面 latest.preset_id 交给前端，
        // 再由前端转发给渲染面（表情 / 短动作）。不要再写「只记决策、不投递」。
        self.services.logger.info(&format!(
            "director Mod 已启动（按情绪/意图选动作预设：expression|motion 经状态面 latest.preset_id 交给渲染面；lexicon={}, log_capacity={}）",
            self.config.emotion_lexicon.as_str(),
            self.ledger.capacity()
        ));
        // P1-4：把二路的**真实状态**说清楚——「开了」不等于「会发 HTTP」。
        if self.config.staging_enabled {
            match self.staging_note.as_deref() {
                Some(note) => self.services.logger.warn(&format!(
                    "director_staging_degraded: {note}——本轮起仅规则层（state_json.staging.degraded=true）"
                )),
                None if self.staging.enabled() => self.services.logger.info(&format!(
                    // 注意：不要把「密钥是否已解析」写成 api_key=...——日志脱敏器
                    // 会把等号后的内容整体打成 [REDACTED]（那是给真密钥的护栏，
                    // 但会把这句非密钥的状态说明也吃掉）。用一句普通说明代替。
                    "director 二路 LLM 已接线（client={}, base_url={}, model={}, timeout_ms={}；{}）",
                    self.staging.kind(),
                    self.config.staging_base_url,
                    self.config.staging_model,
                    self.config.staging_timeout_ms,
                    if self.staging.has_api_key() {
                        "密钥已从 .env/环境解析"
                    } else {
                        "未设置密钥（请求不带 Authorization）"
                    }
                )),
                None => self.services.logger.warn(
                    "director_staging_enabled_but_disabled: staging_enabled=true 却没有可用客户端——仅规则层",
                ),
            }
        } else {
            self.services
                .logger
                .info("director 二路 LLM 未开启（staging_enabled=false）：仅规则层");
        }
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        match topic {
            ModEventTopic::TurnPrompt => {
                self.last_user_text = payload.to_string();
                // **每轮换一张干净的仲裁表**：epoch 只在 stop 时推进，普通轮次
                // 之间它不变（真实链路实测首轮 epoch=0，且不会自动 +1），所以
                // 「换轮」的信号是 TurnPrompt，不是 epoch 变化。不在这里 reset
                // 会把上一轮的 cue 残留到本轮（upsert 同级保留先到的 → 旧 cue 赢）。
                self.fires_this_turn = 0;
                self.last_fire_ms = None;
                self.arbiter.reset(self.current_epoch);
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
                        // 预设是**本轮**的结论；面板/前端从 state_json 读它并投给
                        // 渲染面。日志里写清「选了哪条」——排障要看的就是这一句。
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
            ModEventTopic::SentenceReady => {
                self.handle_sentence_ready(payload);
                Ok(())
            }
            ModEventTopic::TurnEnded => {
                // 下一轮的节流从零开始。
                self.fires_this_turn = 0;
                self.last_fire_ms = None;
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
            .info("director Mod 已关闭（从未写配置；决策账本随运行时释放）");
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
            // 生效的映射表：面板据此显示「现在会映射到哪条」。
            object.insert("presets".to_string(), self.config.presets.to_json());
            // P1-3：异步第二路 / 按句 plan 的可观察状态。
            object.insert(
                "staging".to_string(),
                serde_json::json!({
                    "enabled": self.config.staging_enabled && self.staging.enabled(),
                    "client": self.staging.kind(),
                    // degraded = 开了闸但没接上（缺端点/缺模型/非法 URL）→ 仅规则。
                    // 面板与排障据此**不用猜**「为什么没发 HTTP」。
                    "degraded": self.config.staging_enabled && !self.staging.enabled(),
                    "note": self.staging_note,
                    "base_url": self.config.staging_base_url,
                    "model": self.config.staging_model,
                    "api_key_env": self.config.staging_api_key_env,
                    "api_key_set": self.staging.has_api_key(),
                    "timeout_ms": self.config.staging_timeout_ms,
                    "min_interval_ms": self.config.staging_min_interval_ms,
                    "max_per_turn": self.config.staging_max_per_turn,
                    "fires_this_turn": self.fires_this_turn,
                    "async_plans": self.async_plans,
                    "async_failures": self.async_failures,
                }),
            );
            object.insert(
                "plan".to_string(),
                serde_json::json!({
                    "epoch": self.arbiter.epoch(),
                    "covers_upto_seq": self.arbiter.covers_upto_seq(),
                    "cues": self.arbiter.cues().map(Cue::to_json).collect::<Vec<_>>(),
                    "cues_emitted": self.cues_emitted,
                    "rule_cues": self.rule_cues,
                    "sentences_seen": self.sentences_seen,
                    "cue_sink_enabled": self.services.cues.enabled(),
                }),
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

    /// **host 装配点**（P1-4）：按 namespaced config 构造真实二路客户端并注入。
    ///
    /// - staging_enabled=false → 仍 DisabledStaging（与「默认关」逐字一致）；
    /// - 开闸但缺 staging_base_url / staging_model / URL 非法 → 仍 Disabled，
    ///   degraded 原因写进日志与 state_json.staging.note（**不是**静默假装接上）；
    /// - 配齐 → OpenAiStagingClient（非流式 /chat/completions，密钥走
    ///   secrets::lookup）。
    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        let config = DirectorConfig::from_value(&config);
        let setup = staging::assemble(&config);
        Ok(Box::new(
            DirectorRuntime::new(services, config).with_staging_setup(setup),
        ))
    }
}

/// 工厂单例（**已**注册进 `AVAILABLE_MOD_FACTORIES`，当前注册面共 5 个；缺省停用）。
pub const FACTORY: DirectorFactory = DirectorFactory;

#[cfg(test)]
mod tests;
// P1-3 / P1-4 的句子锚点 + 二路回归单独成文件（AGENTS「测试文件 ≤800 行」）。
#[cfg(test)]
mod tests_staging;
