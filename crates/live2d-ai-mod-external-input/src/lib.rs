//! live2d-ai-mod-external-input（节点 E E5）。
//!
//! 第一个真实 Mod：接收**主 UI 之外**的外部对话输入（本地程序 / 消息回调等），
//! 通过 `ModServices.say_tx` 注入同一 LLM/TTS 主链路（人设 / TTS / 口型一致）。
//!
//! 作为 Mod 系统的验证示例，它演示：
//! - [`ModFactory`] + [`ModRuntime`] 实现（`descriptor` / `create` / `start`）。
//! - `start` 注册 settings schema（暴露给前端统一渲染）。
//! - 订阅事件（`ModEventTopic`，有界隔离）。
//! - 外部文本经 `say()` 进主链路。
//!
//! # 外部 HTTP 端点
//!
//! v1（E5）Mod 本身不内置 HTTP server——外部请求由主仓库 web_api 提供
//! `/api/v1/external/chat` 端点代理，handler 调本 Mod 的 `say_external`。
//! 这样控制平面（web_api）与 Mod 逻辑分离，Mod 只提供能力与配置。

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "external-input",
    name: "外部事件接入",
    version: "0.1.0",
    api_version: 1,
};

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
        // 注册设置 schema（前端统一渲染 "外部接入" 面板）。
        let spec = ModSettingsSpec {
            mod_id: "external-input".to_string(),
            title: "外部事件接入".to_string(),
            version: 1,
            fields: vec![
                ModSettingField::Bool {
                    key: "enabled".to_string(),
                    label: "启用外部接入".to_string(),
                    default: true,
                },
                ModSettingField::Number {
                    key: "listen_port".to_string(),
                    label: "监听端口".to_string(),
                    min: 1024.0,
                    max: 65535.0,
                },
                ModSettingField::String {
                    key: "token".to_string(),
                    label: "访问令牌（空=仅本机）".to_string(),
                    secret: true,
                },
            ],
        };
        registrar.register_settings(spec)?;
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
