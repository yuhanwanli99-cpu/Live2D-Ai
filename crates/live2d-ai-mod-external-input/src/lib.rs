//! live2d-ai-mod-external-input（节点 E E5；0.2.0-rc.1 加强）。
//!
//! 第一个真实 Mod：接收**主 UI 之外**的外部事件（本地程序 / 直播弹幕 sidecar /
//! 消息回调等），通过 `ModServices.say_tx` 注入同一 LLM/TTS 主链路
//! （人设 / TTS / 口型与聊天框完全一致）。
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
//! | `token` | 可选访问令牌（secret）；与 env `EXTERNAL_INPUT_TOKEN` 二选一，env 优先 |
//! | `text_template` | 可选文本模板，`{text}` = 清洗后的外部文本；空 = 原样 |
//! | `prefix` | 可选前缀，拼在模板结果之前（如 `[弹幕] `） |
//!
//! **不要**在本 Mod 内实现 B 站协议 / blivedm / WSS——协议抓取住在
//! Windows sidecar（见 `docs/examples/bilibili-sidecar/`），主仓只收已清洗文本。

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "external-input",
    name: "外部事件接入",
    version: "0.2.0",
    api_version: 1,
};

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
                label: "监听端口（提示；实际端口见 live2d-ai.toml [web].port）".to_string(),
                min: 1024.0,
                max: 65535.0,
            },
            ModSettingField::String {
                key: "token".to_string(),
                label: "访问令牌（空 = 回落到 env EXTERNAL_INPUT_TOKEN；都空 = 仅本机不鉴权）"
                    .to_string(),
                secret: true,
            },
            ModSettingField::String {
                key: "text_template".to_string(),
                label: "文本模板（{text} = 外部文本；空 = 原样）".to_string(),
                secret: false,
            },
            ModSettingField::String {
                key: "prefix".to_string(),
                label: "前缀（拼在模板结果之前，如「[弹幕] 」）".to_string(),
                secret: false,
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
