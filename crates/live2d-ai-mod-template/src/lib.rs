//! live2d-ai-mod-template（rc.4 M3）——**新 Mod 的复制起点**。
//!
//! 本 crate 是一个**能编译、有单测的最小 Mod**，同时是本仓库的活文档：
//! 复制它、按 id 改名、再走 [`docs/architecture/mod-product-chain.md`] §3 的勾选表
//! （根 `Cargo.toml` → desktop 依赖 → `AVAILABLE_MOD_FACTORIES` → 数量/id 断言 →
//! 文档一行）即可。**模板 crate 自身不注册**进 `AVAILABLE_MOD_FACTORIES`——
//! 注册表里只放真实能力（见 `main.rs` 的 `mod_count_is_*` 断言）。
//!
//! # 最小骨架包含什么
//!
//! - `DESCRIPTOR`：静态身份；`api_version` **必须**取 `MOD_API_VERSION`；
//! - `FACTORY`：`ModFactory` 的单例；
//! - `TemplateRuntime`：`start` 注册 settings schema + 订阅事件；
//!   `on_event` 记录日志；`shutdown` 复位；
//! - settings 是**纯数据 schema**（Bool/String/Number/Select）——**禁止**注入 HTML/JS。
//!
//! # 不要做什么
//!
//! - 不接动作通道：`ModServices.action_tx` 自 rc.2 起休眠，请求会被 host 丢弃并返回
//!   `false`（见 `docs/architecture/core-chain-baseline.md` §3.3）；
//! - 不直接读进程环境/密钥：配置写回走 `ModServices.apply_settings`（一等 API）。
//!
//! [`docs/architecture/mod-product-chain.md`]: ../../../docs/architecture/mod-product-chain.md

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。改 `id` 时同步 `main.rs` 的 id 断言与文档表格。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "template",
    name: "模板 Mod",
    version: "0.1.0",
    // 对齐 Mod 系统 API 版本；不对齐时 host 会把本 Mod 置 Failed（主链不崩）。
    api_version: MOD_API_VERSION,
};

/// 模板 Mod 运行时。
pub struct TemplateRuntime {
    services: ModServices,
    /// `start` 是否已注册 schema（单测断言用）。
    registered: bool,
}

impl TemplateRuntime {
    /// 用注入的 host services 构造。
    pub fn new(services: ModServices) -> Self {
        Self {
            services,
            registered: false,
        }
    }

    /// settings schema 是否已注册。
    pub fn is_registered(&self) -> bool {
        self.registered
    }
}

impl ModRuntime for TemplateRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 纯数据 schema：前端按字段类型渲染，Mod 不碰 DOM。
        let spec = ModSettingsSpec {
            mod_id: DESCRIPTOR.id.to_string(),
            title: DESCRIPTOR.name.to_string(),
            version: 1,
            fields: vec![
                ModSettingField::Bool {
                    key: "enabled".to_string(),
                    label: "启用模板能力".to_string(),
                    default: false,
                },
                ModSettingField::String {
                    key: "note".to_string(),
                    label: "备注".to_string(),
                    secret: false,
                },
            ],
        };
        registrar.register_settings(spec)?;
        // 只订阅真正需要的事件主题。
        registrar.subscribe(ModEventTopic::TurnStarted)?;
        self.registered = true;
        self.services.logger.info("template Mod 已启动");
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        self.services
            .logger
            .info(&format!("template 收到 {}: {payload}", topic.as_str()));
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.registered = false;
        self.services.logger.info("template Mod 已关闭");
        Ok(())
    }
}

/// 静态工厂（注册进 `AVAILABLE_MOD_FACTORIES` 时用 `&FACTORY`）。
pub struct TemplateFactory;

impl ModFactory for TemplateFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    fn create(
        &self,
        services: ModServices,
        _config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(TemplateRuntime::new(services)))
    }
}

/// 工厂单例。
pub const FACTORY: TemplateFactory = TemplateFactory;

#[cfg(test)]
mod tests {
    use super::*;

    /// 记录 register_settings / subscribe 调用的测试注册器。
    #[derive(Default)]
    struct RecordingRegistrar {
        specs: Vec<ModSettingsSpec>,
        topics: Vec<ModEventTopic>,
    }

    impl ModRegistrar for RecordingRegistrar {
        fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
            self.specs.push(spec);
            Ok(())
        }
        fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
            self.topics.push(topic);
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

    #[test]
    fn descriptor_declares_current_api_version() {
        assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
        assert_eq!(DESCRIPTOR.id, "template");
    }

    #[test]
    fn start_registers_settings_and_topic() {
        let mut rt = TemplateRuntime::new(noop_services());
        let mut reg = RecordingRegistrar::default();
        rt.start(&mut reg).expect("start 应成功");
        assert!(rt.is_registered());
        assert_eq!(reg.specs.len(), 1, "应注册一份 settings schema");
        assert_eq!(reg.specs[0].mod_id, "template");
        assert!(reg.specs[0].validate().is_ok(), "字段 key 必须唯一");
        assert_eq!(reg.topics, vec![ModEventTopic::TurnStarted]);
        rt.shutdown().unwrap();
        assert!(!rt.is_registered());
    }

    #[test]
    fn factory_creates_runtime() {
        // 类型断言：create 返回可持有的 `Box<dyn ModRuntime>` 即可。
        let rt: Box<dyn ModRuntime> = FACTORY
            .create(noop_services(), serde_json::json!({}))
            .expect("create 应成功");
        drop(rt);
    }
}
