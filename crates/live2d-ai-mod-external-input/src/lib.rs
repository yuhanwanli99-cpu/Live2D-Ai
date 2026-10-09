//! live2d-ai-mod-external-input（节点 E E5；0.2.0-rc.1 加强）。
//!
//! 第一个真实 Mod：接收**主 UI 之外**的外部事件（本地程序 / 直播弹幕 sidecar /
//! 消息回调等），通过 `ModServices.say_tx` 注入同一 LLM/TTS 主链路
//! （人设 / TTS / 口型与聊天框完全一致）。
//!
//! # 壳内测试注入（L1 产品级波次）
//!
//! 壳内可直接「测试注入」：`ModRuntime::command("test_inject", {"text": …})`
//! （见 [`inject`]）。它与 HTTP 端点**共用**同一条渲染口径与同一个 `say_tx`，
//! 区别只是走命令通道、不需要 token。Mod id / name / 端点契约都不因此改变。
//!
//! # 外部 HTTP 端点
//!
//! 本 Mod **不内置 HTTP server**——外部请求由主仓库 web_api 提供
//! `POST /api/v1/external/chat` 端点，handler 校验后经 `say` 进主链路。
//! 这样控制平面（web_api）与 Mod 逻辑分离，Mod 只提供能力与配置。
//! 契约文档：`docs/external-input.md`。
//!
//! # 启停语义（唯一真源 = Mod manifest 的 `enabled`）
//!
//! - **启用**：`[mods.external-input].enabled = true`（`mods.json`；前端「Mod 管理」
//!   或 `POST /api/v1/mods/external-input/enable`）。启用后 host 调
//!   [`ModRuntime::start`] 注册设置 schema 并返回 `Running`；此时
//!   `POST /api/v1/external/chat` 才可用。
//! - **停用**：`disable`（或改 `mods.json` 后重启）。host 调 `shutdown`，
//!   端点返回 `403 mod_disabled`——**不静默吞掉**外部文本（发送方必须知道）。
//! - settings schema 里**没有**第二个 `enabled` 字段：避免「Mod 开关」与
//!   「配置里的开关」两处真相。schema 只描述不可由清单表达的行为参数。
//!
//! # 配置字段（settings_spec v2）
//!
//! | key | 语义 |
//! |---|---|
//! | `listen_port` | **提示值**（前端展示）；真实监听端口来自 `live2d-ai.toml` 的 `[web].port`（默认 18080），本 Mod 不开 socket |
//! | `token` | 可选访问令牌（secret）；与 env `EXTERNAL_INPUT_TOKEN` 二选一——env 侧经 `secrets::lookup` 读（**`.env` 快照 > 进程环境**），env 优先 |
//! | `text_template` | 可选文本模板，`{text}` = 清洗后的外部文本；空 = 原样 |
//! | `prefix` | 可选前缀，拼在模板结果之前（如 `[弹幕] `） |
//!
//! # 可观察计数（Wave 3，2026-09-14）
//!
//! `accepts` / `rejects` / `busy` / `v2_ignored` 经 [`counters`] 的进程级
//! `AtomicU64` 读写：handler（HTTP 线程）在分支里 `record_*`，本 runtime 的
//! `state_json` 把它们交给 `GET /api/v1/mods/external-input/state` 的 `state` 字段。
//! 每个计数对应哪条分支、`rejects` 为什么不含 400/503，钉在 `counters.rs` 头注。
//!
//! **不要**在本 Mod 内实现 B 站协议 / blivedm / WSS——协议抓取住在
//! Windows sidecar（见 `docs/examples/bilibili-sidecar/`），主仓只收已清洗文本。

use live2d_ai_mod_system::*;

pub mod counters;
pub mod inject;
pub use counters::{
    ExternalInputCounters, counters_snapshot, record_accept, record_busy, record_reject,
    record_v2_ignored,
};
pub use inject::{COMMAND_TEST_INJECT, prepare_injection};

/// Mod 描述符（静态身份）。
const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "external-input",
    name: "外部事件接入",
    version: "0.2.0",
    api_version: 1,
};

/// 一次性命令（产品级加强波次）：清零四个可观察计数，返回**清零前**的快照。
///
/// 面板「重置计数」按钮经 `POST /api/v1/mods/external-input/command` 触发它；
/// 实现对 [`counters::reset`] 的调用是唯一的清零路径。
pub const COMMAND_RESET_COUNTERS: &str = "reset_counters";

/// 外部接入设置 schema（**静态**：未启用也拿得到，前端可先填再启用）。
///
/// 见模块头注「启停语义」：这里**不**含 `enabled` 字段。
pub fn external_input_settings_spec() -> ModSettingsSpec {
    ModSettingsSpec {
        mod_id: "external-input".to_string(),
        title: "外部事件接入".to_string(),
        version: 2,
        fields: vec![
            ModSettingField::Number {
                key: "listen_port".to_string(),
                label: "端口（只作提示）".to_string(),
                min: 1024.0,
                max: 65535.0,
            },
            ModSettingField::String {
                key: "token".to_string(),
                label: "令牌".to_string(),
                secret: true,
                default: None,
            },
            ModSettingField::String {
                key: "text_template".to_string(),
                label: "弹幕怎么说给角色".to_string(),
                secret: false,
                default: None,
            },
            ModSettingField::String {
                key: "prefix".to_string(),
                label: "每条前面加的字".to_string(),
                secret: false,
                default: None,
            },
        ],
    }
}

/// 用前缀 + 模板渲染一条外部文本（纯函数）。
///
/// - `template` 为空/纯空白 → 等价 `{text}`（原样）。
/// - `template` 含 `{text}` → 替换全部占位符。
/// - `template` 非空但不含 `{text}` → 视为**字面前缀**，外部文本追加在其后
///   （宽容，不报错；调用方不会被配置错误卡死链路）。
/// - `prefix` 永远拼在最前。
pub fn render_injected_text(prefix: &str, template: &str, text: &str) -> String {
    let tpl = if template.trim().is_empty() {
        "{text}"
    } else {
        template
    };
    let body = if tpl.contains("{text}") {
        tpl.replace("{text}", text)
    } else {
        format!("{tpl}{text}")
    };
    format!("{prefix}{body}")
}

/// 从 Mod config JSON 读取 `prefix` / `text_template` 并渲染。
pub fn render_from_config(config: &serde_json::Value, text: &str) -> String {
    let prefix = config.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
    let template = config
        .get("text_template")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    render_injected_text(prefix, template, text)
}

/// 从 Mod config JSON 读取 `token`；空串/纯空白/缺省 → `None`。
pub fn token_from_config(config: &serde_json::Value) -> Option<String> {
    config
        .get("token")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// 访问令牌的环境变量名（`.env` / 进程环境共用同一个键）。
const TOKEN_ENV_VAR: &str = "EXTERNAL_INPUT_TOKEN";

/// env `EXTERNAL_INPUT_TOKEN` 是否非空（**只看存在性，绝不读取/回显明文**）。
///
/// W6：值经 `live2d_ai_runtime::secrets::lookup` 查——**`.env` 快照 > 进程环境**。
/// 直接读进程环境会绕过 `.env`，于是「界面上刚写了 key、链路还说没配置」。
/// `lookup` 抽成参数只为**可注入**（回归不碰进程环境、不用 `set_var`）。
fn env_token_is_set_with(lookup: &dyn Fn(&str) -> Option<String>) -> bool {
    lookup(TOKEN_ENV_VAR)
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}

/// 「是否已配置令牌」：Mod config 的 `token` **或** env `EXTERNAL_INPUT_TOKEN`。
///
/// 优先级链**不变**：**`.env`/env > Mod config > 不鉴权**（`token_from_config`
/// 命中即为真，env 侧由 `lookup` 决定）。这里只回 `bool`：`state_json` 会被
/// 前端渲染、被日志记录，**绝不回显明文**。
pub fn token_is_set(config: &serde_json::Value) -> bool {
    token_is_set_with(config, &live2d_ai_runtime::secrets::lookup)
}

/// [`token_is_set`] 的可注入实现（生产路径传 `secrets::lookup`）。
fn token_is_set_with(config: &serde_json::Value, lookup: &dyn Fn(&str) -> Option<String>) -> bool {
    token_from_config(config).is_some() || env_token_is_set_with(lookup)
}

/// External Input 的运行时状态。
pub struct ExternalInputRuntime {
    services: ModServices,
    config: serde_json::Value,
    /// settings schema 是否已注册（start 成功标志）。
    registered: bool,
}

impl Default for ExternalInputRuntime {
    fn default() -> Self {
        Self {
            services: ModServices::new(
                ModActionSender::new(|_| true),
                SaySender::new(|_| true),
                ModEventSender::new(|_topic, _payload| true),
                ModLogger::new(|_lvl, _msg| {}),
            ),
            config: serde_json::Value::Object(Default::default()),
            registered: false,
        }
    }
}

/// External Input 工厂。
pub struct ExternalInputFactory;

impl ModFactory for ExternalInputFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// 静态 schema：未启用也能渲染配置表单（rc.4 M2 语义）。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        Some(external_input_settings_spec())
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(ExternalInputRuntime {
            services,
            config,
            registered: false,
        }))
    }
}

impl ModRuntime for ExternalInputRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 注册设置 schema（前端统一渲染 "外部事件接入" 面板）。
        registrar.register_settings(external_input_settings_spec())?;
        // 订阅事件（示例：turn 开始 / 文本增量），验证事件隔离。
        registrar.subscribe(ModEventTopic::TurnStarted)?;
        registrar.subscribe(ModEventTopic::TextDelta)?;
        self.registered = true;
        self.services.logger.info("external-input Mod 已启动");
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        // v1：仅记录事件（日志），不做业务。验证事件经有界 worker 送达。
        self.services.logger.info(&format!(
            "external-input 收到事件 {}: {}",
            topic.as_str(),
            payload
        ));
        Ok(())
    }

    /// 一次性命令（产品级加强波次 / L1 产品级波次）。
    ///
    /// - [`COMMAND_RESET_COUNTERS`]：清零 [`counters`] 的四个可观察计数，
    ///   返回**清零前**的快照（面板据此说「清了哪些」）；
    /// - [`COMMAND_TEST_INJECT`]（= `test_inject`）：壳内测试注入，
    ///   见 `command_test_inject`；
    /// - 其余命令一律 `UnsupportedCommand`（host 回 409 `unsupported_command`）。
    ///
    /// 不做网络请求；只改进程内计数 / 把文本交给主链，立即返回。
    fn command(
        &mut self,
        command: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, ModError> {
        match command {
            COMMAND_RESET_COUNTERS => {
                let before = counters::reset();
                self.services.logger.info("external-input 计数已清零");
                Ok(serde_json::json!({"reset": true, "before": before}))
            }
            COMMAND_TEST_INJECT => self.command_test_inject(args),
            other => Err(ModError::UnsupportedCommand {
                command: other.to_string(),
            }),
        }
    }

    /// 只读运行态：接受 / 拒绝 / 忙 / v2_ignored 计数 + ready + token_set
    /// （L1 起追加「命令注入可用」一项；**既有键一个不删**）。
    ///
    /// `GET /api/v1/mods/external-input/state` 的 `state` 字段即本返回值；
    /// 契约与分支对应表见 [`counters`]。`token_set` 只报「有没有令牌」，
    /// **绝不回显明文**（见 [`token_is_set`]）。
    fn state_json(&mut self) -> Option<serde_json::Value> {
        let mut snap = counters::counters_snapshot();
        if let Some(obj) = snap.as_object_mut() {
            obj.insert(
                "ready".to_string(),
                serde_json::Value::Bool(self.registered),
            );
            // 令牌只报存在性：前端渲染 + 日志都不该看到明文。
            obj.insert(
                "token_set".to_string(),
                serde_json::Value::Bool(token_is_set(&self.config)),
            );
            // 命令通道的 `test_inject` 恒可用：本 Mod 不认识它才是不正常的。
            obj.insert(
                "inject_via_command".to_string(),
                serde_json::Value::Bool(true),
            );
        }
        Some(snap)
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.registered = false;
        self.services.logger.info("external-input Mod 已关闭");
        Ok(())
    }
}

impl ExternalInputRuntime {
    /// 外部程序调用的入口：把文本送入 LLM/TTS 主链路。
    ///
    /// 返回是否被主链路接受（忙碌 / 通道满时为 false）。
    pub fn say_external(&self, text: &str) -> bool {
        self.services.say_tx.say(text.to_string())
    }

    /// 是否已启用（start 成功）。
    pub fn is_ready(&self) -> bool {
        self.registered
    }

    /// 当前配置。
    pub fn config(&self) -> &serde_json::Value {
        &self.config
    }

    /// 按当前配置（prefix + text_template）渲染并注入。
    pub fn say_external_with_config(&self, text: &str) -> bool {
        let rendered = render_from_config(&self.config, text);
        self.say_external(&rendered)
    }

    /// `test_inject`：把一条（按 args 渲染的）文本经 `say_tx` 送进主链，
    /// 并把结果如实报回命令通道。
    ///
    /// - 取参与校验全在 [`inject::prepare_injection`]（纯函数，含长度口径）；
    /// - 主链接受 → [`counters::record_accept`]；忙碌丢弃 → [`counters::record_busy`]：
    ///   与 HTTP 端点**记同一本账**，面板看到的「已接受 N」两条入口都算；
    /// - 无论接受还是忙，命令本身都算**执行成功**（`ok:true` + `accepted:bool`）：
    ///   忙是主链的运行态，不是命令通道的失败，发送方据此决定要不要重试。
    fn command_test_inject(&self, args: &serde_json::Value) -> Result<serde_json::Value, ModError> {
        let rendered = inject::prepare_injection(&self.config, args)?;
        let accepted = self.services.say_tx.say(rendered.clone());
        if accepted {
            counters::record_accept();
        } else {
            counters::record_busy();
        }
        // 只记长度与结果，不把用户文本写进日志（与 handler「不记请求体」同一条纪律）。
        self.services.logger.info(&format!(
            "external-input test_inject：渲染后 {} 字符，主链{}",
            rendered.chars().count(),
            if accepted {
                "已接受"
            } else {
                "忙碌丢弃"
            }
        ));
        Ok(serde_json::json!({
            "ok": true,
            "injected_text": rendered,
            "accepted": accepted,
            "endpoint": "mod.command.test_inject",
            "note": inject::INJECT_NOTE,
        }))
    }
}

/// 供 host 装配 `AVAILABLE_MOD_FACTORIES` 的工厂单例。
pub static FACTORY: ExternalInputFactory = ExternalInputFactory;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_static() {
        let f = ExternalInputFactory;
        let d = f.descriptor();
        assert_eq!(d.id, "external-input");
        assert_eq!(d.api_version, 1);
    }

    #[test]
    fn create_start_registers_settings() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = ExternalInputFactory
            .create(services, serde_json::json!({}))
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        assert_eq!(mock.specs.len(), 1);
        assert_eq!(mock.subs.len(), 2); // TurnStarted + TextDelta
    }

    #[test]
    fn shutdown_clears_ready() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = ExternalInputRuntime {
            services,
            config: serde_json::json!({}),
            registered: false,
        };
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        assert!(rt.is_ready(), "start 后应 ready");
        rt.shutdown().unwrap();
        assert!(!rt.is_ready(), "shutdown 后不得再报 ready");
    }

    #[test]
    fn say_external_forwards_to_say_tx() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let c2 = called.clone();
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(move |_text| {
                c2.store(true, Ordering::SeqCst);
                true
            }),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        // 直接构造具体类型（不经 trait object），验证 say_external 转发。
        let rt = ExternalInputRuntime {
            services,
            config: serde_json::Value::Object(Default::default()),
            registered: false,
        };
        let ok = rt.say_external("提醒我喝水");
        assert!(ok);
        assert!(called.load(Ordering::SeqCst));
    }

    // ---------------------------------------------------- settings_spec v2

    /// 静态 schema 可用且字段齐全；**不含 `enabled`**（唯一开关是 manifest）。
    #[test]
    fn static_spec_has_template_fields_and_no_enabled() {
        let spec = ExternalInputFactory.settings_spec().expect("静态 schema");
        assert!(spec.validate().is_ok(), "字段 key 不得重复");
        let keys: Vec<&str> = spec.fields.iter().map(|f| f.key()).collect();
        assert_eq!(
            keys,
            vec!["listen_port", "token", "text_template", "prefix"]
        );
        assert!(
            !keys.contains(&"enabled"),
            "启停只由 Mod manifest 的 enabled 表达，schema 不再重复"
        );
        // token 必须仍是 secret（不落日志/不脱敏泄漏）。
        let token = &spec.fields[1];
        assert!(matches!(
            token,
            ModSettingField::String { secret: true, .. }
        ));
    }

    /// `start` 注册的 spec 与静态 spec 完全一致（两处不能分叉）。
    #[test]
    fn start_registers_same_spec_as_static() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = ExternalInputFactory
            .create(services, serde_json::json!({}))
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        assert_eq!(mock.specs[0], external_input_settings_spec());
    }

    // ---------------------------------------------------- 模板 / 前缀（纯函数）

    #[test]
    fn render_empty_template_passes_text_through() {
        assert_eq!(render_injected_text("", "", "你好"), "你好");
        assert_eq!(render_injected_text("", "   ", "你好"), "你好");
    }

    #[test]
    fn render_prefix_and_placeholder() {
        assert_eq!(
            render_injected_text("[弹幕] ", "{text}", "主播好"),
            "[弹幕] 主播好"
        );
        assert_eq!(
            render_injected_text("", "观众说：{text}（请回应）", "666"),
            "观众说：666（请回应）"
        );
    }

    #[test]
    fn render_template_without_placeholder_appends_text() {
        assert_eq!(
            render_injected_text("", "【外部消息】", "hi"),
            "【外部消息】hi"
        );
    }

    #[test]
    fn render_replaces_every_placeholder() {
        assert_eq!(render_injected_text("", "{text}/{text}", "x"), "x/x");
    }

    #[test]
    fn render_from_config_uses_fields() {
        let cfg = serde_json::json!({"prefix": "[礼物] ", "text_template": "{text} 感谢！"});
        assert_eq!(render_from_config(&cfg, "小心心"), "[礼物] 小心心 感谢！");
        // 缺字段 = 原样。
        assert_eq!(render_from_config(&serde_json::json!({}), "hi"), "hi");
        // 非字符串字段不得 panic。
        assert_eq!(
            render_from_config(&serde_json::json!({"prefix": 42}), "hi"),
            "hi"
        );
    }

    #[test]
    fn token_from_config_trims_and_ignores_blank() {
        assert_eq!(
            token_from_config(&serde_json::json!({"token": "  s3cret "})),
            Some("s3cret".to_string())
        );
        assert_eq!(token_from_config(&serde_json::json!({"token": ""})), None);
        assert_eq!(
            token_from_config(&serde_json::json!({"token": "   "})),
            None
        );
        assert_eq!(token_from_config(&serde_json::json!({})), None);
        assert_eq!(token_from_config(&serde_json::json!({"token": 7})), None);
    }

    /// 带配置的注入会用上 prefix + template（端到端到 say_tx 的文本）。
    #[test]
    fn say_external_with_config_renders_before_sending() {
        use std::sync::Mutex;
        let got: std::sync::Arc<Mutex<Vec<String>>> = std::sync::Arc::new(Mutex::new(Vec::new()));
        let got_c = got.clone();
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(move |t| {
                got_c.lock().unwrap().push(t);
                true
            }),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let rt = ExternalInputRuntime {
            services,
            config: serde_json::json!({"prefix": "[弹幕] ", "text_template": "{text}"}),
            registered: true,
        };
        assert!(rt.say_external_with_config("主播好"));
        assert_eq!(got.lock().unwrap().as_slice(), ["[弹幕] 主播好"]);
    }

    /// `state_json`：四个计数键 + ready；键与 `counters` 契约一致。
    #[test]
    fn state_json_exposes_counters_and_ready() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = ExternalInputFactory
            .create(services, serde_json::json!({}))
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        let state = rt.state_json().expect("external-input 必须提供 state_json");
        for k in counters::COUNTER_KEYS {
            assert!(state[k].is_u64(), "{k} 必须是数字");
        }
        assert_eq!(state["ready"], serde_json::json!(true), "已 start");
        rt.shutdown().unwrap();
        assert_eq!(
            rt.state_json().unwrap()["ready"],
            serde_json::json!(false),
            "shutdown 后 ready=false，计数仍可读"
        );
    }

    /// 无副作用的 `ModServices`（新回归共用，避免每处重复四个 sender）。
    fn noop_services() -> ModServices {
        ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        )
    }

    // ---------------------------------------------------- token_set（产品级）

    /// `token_set`：config 有令牌 → true；且 **state_json 绝不回显明文**。
    #[test]
    fn state_json_reports_token_set_without_echoing_token() {
        let mut rt = ExternalInputFactory
            .create(noop_services(), serde_json::json!({"token": "s3cret"}))
            .unwrap();
        let state = rt.state_json().expect("state_json");
        assert_eq!(state["token_set"], serde_json::json!(true));
        let rendered = state.to_string();
        assert!(
            !rendered.contains("s3cret"),
            "state_json 不得回显令牌明文：{rendered}"
        );
    }

    /// `token_set` 只看 **lookup 口径**，不看进程环境（W6 回归）：
    /// 「值只写在 `.env`、进程环境没有」时读得到令牌；两边都没有 → false。
    /// 用注入的 lookup 构造，**不**用 `std::env::set_var`（并发下不可靠）。
    #[test]
    fn token_set_follows_injected_lookup_not_process_env() {
        let none = |_name: &str| Option::<String>::None;
        assert!(
            !token_is_set_with(&serde_json::json!({}), &none),
            "config 空 + lookup 无值 → 未配置（不鉴权）"
        );
        let dotenv = |name: &str| (name == TOKEN_ENV_VAR).then(|| "dotenv-secret".to_string());
        assert!(
            token_is_set_with(&serde_json::json!({}), &dotenv),
            "值只在 `.env`（注入 lookup 有值、进程环境没有）也必须报已配置"
        );
        assert!(
            token_is_set_with(&serde_json::json!({"token": "cfg-secret"}), &none),
            "Mod config 命中同样报已配置（优先级链的 config 档不变）"
        );
    }

    // ---------------------------------------------------- command reset_counters

    /// `reset_counters`：返回清零前快照，并把进程级计数清零。
    #[test]
    fn command_reset_counters_returns_before_and_zeroes() {
        let _guard = counters::test_lock();
        counters::record_accept();
        counters::record_busy();
        let mut rt = ExternalInputRuntime {
            services: noop_services(),
            config: serde_json::json!({}),
            registered: true,
        };
        let seen = counters::counters_snapshot();
        let result = rt
            .command(COMMAND_RESET_COUNTERS, &serde_json::json!({}))
            .expect("已知命令必须 Ok");
        assert_eq!(result["reset"], serde_json::json!(true));
        assert_eq!(result["before"]["accepts"], seen["accepts"]);
        assert_eq!(result["before"]["busy"], seen["busy"]);
        let after = counters::counters_snapshot();
        for k in counters::COUNTER_KEYS {
            assert_eq!(after[k], 0, "{k} 清零");
        }
    }

    /// 不认识命令 → `UnsupportedCommand`（host 据此回 409 `unsupported_command`）。
    #[test]
    fn command_unknown_is_unsupported() {
        let mut rt = ExternalInputRuntime {
            services: noop_services(),
            config: serde_json::json!({}),
            registered: false,
        };
        let err = rt
            .command("no_such_command", &serde_json::json!({}))
            .unwrap_err();
        assert!(matches!(
            err,
            ModError::UnsupportedCommand { ref command } if command == "no_such_command"
        ));
    }

    // ---------------------------------------------------- command test_inject（L1）

    /// `test_inject` 缺省渲染与端点同口径：say_tx 收到的是**渲染后**文本，
    /// 且成功计入 `accepts`（与 HTTP 端点同一本账）。
    #[test]
    fn command_test_inject_sends_rendered_text_and_records_accept() {
        use std::sync::Mutex;
        let _guard = counters::test_lock();
        let got: std::sync::Arc<Mutex<Vec<String>>> = std::sync::Arc::new(Mutex::new(Vec::new()));
        let got_c = got.clone();
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(move |t| {
                got_c.lock().unwrap().push(t);
                true
            }),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let config = serde_json::json!({"prefix": "[弹幕] ", "text_template": "{text}"});
        let mut rt = ExternalInputRuntime {
            services,
            config: config.clone(),
            registered: true,
        };
        let before = counters::counters_snapshot();
        let result = rt
            .command(
                COMMAND_TEST_INJECT,
                &serde_json::json!({"text": " 主播好 "}),
            )
            .expect("test_inject 必须被支持");
        assert_eq!(
            got.lock().unwrap().as_slice(),
            [render_from_config(&config, "主播好")],
            "say_tx 收到的是 render_from_config 的结果（同口径，不是另一份逻辑）"
        );
        assert_eq!(result["ok"], serde_json::json!(true));
        assert_eq!(result["accepted"], serde_json::json!(true));
        assert_eq!(result["injected_text"], serde_json::json!("[弹幕] 主播好"));
        assert_eq!(
            result["endpoint"],
            serde_json::json!("mod.command.test_inject")
        );
        assert!(
            result["note"]
                .as_str()
                .unwrap_or("")
                .contains("不需要 token"),
            "note 必须点明走命令通道不需要 token"
        );
        assert_eq!(
            counters::counters_snapshot()["accepts"],
            before["accepts"].as_u64().unwrap() + 1
        );
    }

    /// say_tx 返回 false（主链忙）→ 命令仍 `ok:true`，但 `accepted:false` 且记 `busy`。
    #[test]
    fn command_test_inject_busy_records_busy_not_accept() {
        let _guard = counters::test_lock();
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| false),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = ExternalInputRuntime {
            services,
            config: serde_json::json!({}),
            registered: true,
        };
        let before = counters::counters_snapshot();
        let result = rt
            .command(COMMAND_TEST_INJECT, &serde_json::json!({"text": "hi"}))
            .unwrap();
        assert_eq!(
            result["ok"],
            serde_json::json!(true),
            "忙是主链状态，不是命令失败"
        );
        assert_eq!(result["accepted"], serde_json::json!(false));
        let after = counters::counters_snapshot();
        assert_eq!(after["busy"], before["busy"].as_u64().unwrap() + 1);
        assert_eq!(after["accepts"], before["accepts"], "忙不得冒充 accept");
    }

    /// 空 / 超长文本 → 可读中文错误，且**计数一个都不动**（没进主链就不该记账）。
    #[test]
    fn command_test_inject_invalid_text_leaves_counters_untouched() {
        let _guard = counters::test_lock();
        let mut rt = ExternalInputRuntime {
            services: noop_services(),
            config: serde_json::json!({"text_template": "x{text}"}),
            registered: true,
        };
        let before = counters::counters_snapshot();
        for args in [
            serde_json::json!({}),
            serde_json::json!({"text": "   "}),
            serde_json::json!({"text": "a".repeat(inject::MAX_INJECT_TEXT_LEN)}),
        ] {
            let err = rt.command(COMMAND_TEST_INJECT, &args).unwrap_err();
            match err {
                ModError::Other(msg) => assert!(!msg.is_empty(), "错误必须有中文可读文案"),
                other => panic!("应为 Other，实为 {other:?}"),
            }
        }
        assert_eq!(counters::counters_snapshot(), before, "非法输入不改计数");
    }

    /// `state_json` 是既有键的**超集**：L1 追加 inject_via_command。
    #[test]
    fn state_json_is_superset_with_command_flag() {
        let mut rt = ExternalInputFactory
            .create(noop_services(), serde_json::json!({}))
            .unwrap();
        let state = rt.state_json().expect("state_json");
        for k in counters::COUNTER_KEYS {
            assert!(state.get(k).is_some(), "既有计数键 {k} 不得删");
        }
        assert!(state.get("ready").is_some());
        assert!(state.get("token_set").is_some());
        assert_eq!(state["inject_via_command"], serde_json::json!(true));
    }

    /// Mod 身份是稳定契约：新命令（test_inject）不得改变 id / name。
    #[test]
    fn mod_identity_is_stable_external_input() {
        assert_eq!(FACTORY.descriptor().id, "external-input");
        assert_eq!(FACTORY.descriptor().name, "外部事件接入");
    }

    /// 测试用 ModRegistrar mock。
    #[derive(Default)]
    struct MockRegistrar {
        specs: Vec<ModSettingsSpec>,
        subs: Vec<SubscriptionId>,
    }
    impl ModRegistrar for MockRegistrar {
        fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
            self.specs.push(spec);
            Ok(())
        }
        fn subscribe(&mut self, _topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
            let id = SubscriptionId(self.subs.len() as u64 + 1);
            self.subs.push(id);
            Ok(id)
        }
        fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
            Ok(())
        }
    }
}
