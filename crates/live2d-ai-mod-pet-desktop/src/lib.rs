//! live2d-ai-mod-pet-desktop（节点 E E7）。
//!
//! 桌宠窗口 Mod——v1 骨架。
//!
//! # v1 范围声明
//!
//! 桌宠悬浮窗 / 点击穿透 / 托盘图标 是**原生能力**（由 `live2d-ai-desktop`
//! 的 winit 窗口与 tray 实现接管），v1 骨架**不引入** `winit` / `egui` / `wgpu`
//! 等 GUI 依赖。本 Mod 仅负责：
//!
//! - 注册 settings schema（暴露给前端统一渲染『桌宠窗口』面板）。
//! - 订阅 `VoiceStarted` / `VoiceEnded` 事件，记录口型联动占位日志并翻转
//!   `voice_active` 状态（供未来原生渲染窗口读取）。
//! - 窗口生命周期接线**后置**至 E7-T3 之后或节点 F，v1 不创建任何窗口。
//!
//! # 配置字段
//!
//! - `always_on_top`（bool，缺省 `true`）：窗口总在最前。
//! - `click_through`（bool，缺省 `false`）：鼠标穿透（点击穿透）。
//! - `opacity`（f64，`0.1..1.0`，缺省 `0.95`）：窗口不透明度。
//!
//! # 版本
//!
//! `api_version` 声明为 1（对齐 `live2d-ai-mod-system` API 版本）。

use live2d_ai_mod_system::*;

/// Mod 描述符（静态身份）。
const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "pet-desktop",
    name: "桌宠窗口",
    version: "0.1.0",
    api_version: 1,
};

/// 桌宠窗口 Mod 运行时状态。
pub struct PetDesktopRuntime {
    services: ModServices,
    /// settings schema 是否已注册（start 成功标志）。
    registered: bool,
    /// 语音是否活跃（VoiceStarted=true / VoiceEnded=false）。
    /// 供未来原生窗口渲染读取口型。
    voice_active: bool,
}

impl Default for PetDesktopRuntime {
    fn default() -> Self {
        Self {
            services: ModServices::new(
                ModActionSender::new(|_| true),
                SaySender::new(|_| true),
                ModEventSender::new(|_topic, _payload| true),
                ModLogger::new(|_lvl, _msg| {}),
            ),
            registered: false,
            voice_active: false,
        }
    }
}

/// 桌宠窗口 Mod 工厂。
pub struct PetDesktopFactory;

impl ModFactory for PetDesktopFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    fn create(
        &self,
        services: ModServices,
        _config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(PetDesktopRuntime {
            services,
            registered: false,
            voice_active: false,
        }))
    }
}

impl ModRuntime for PetDesktopRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 注册设置 schema（前端统一渲染 『桌宠窗口』 面板）。
        let spec = ModSettingsSpec {
            mod_id: "pet-desktop".to_string(),
            title: "桌宠窗口".to_string(),
            version: 1,
            fields: vec![
                ModSettingField::Bool {
                    key: "always_on_top".to_string(),
                    label: "总在最前".to_string(),
                    default: true,
                },
                ModSettingField::Bool {
                    key: "click_through".to_string(),
                    label: "点击穿透".to_string(),
                    default: false,
                },
                ModSettingField::Number {
                    key: "opacity".to_string(),
                    label: "窗口不透明度".to_string(),
                    min: 0.1,
                    max: 1.0,
                },
            ],
        };
        registrar.register_settings(spec)?;
        // 订阅语音事件（v1 用于未来口型联动占位）。
        registrar.subscribe(ModEventTopic::VoiceStarted)?;
        registrar.subscribe(ModEventTopic::VoiceEnded)?;
        self.registered = true;
        self.services.logger.info("pet-desktop Mod 已启动");
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, payload: &str) -> Result<(), ModError> {
        match topic {
            ModEventTopic::VoiceStarted => {
                self.voice_active = true;
                self.services
                    .logger
                    .info("桌宠口型联动占位（VoiceStarted）");
            }
            ModEventTopic::VoiceEnded => {
                self.voice_active = false;
                self.services.logger.info("桌宠口型联动占位（VoiceEnded）");
            }
            _ => {}
        }
        let _ = payload;
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.registered = false;
        self.voice_active = false;
        self.services.logger.info("pet-desktop Mod 已关闭");
        Ok(())
    }
}

impl PetDesktopRuntime {
    /// 是否已启动（start 成功）。
    pub fn is_ready(&self) -> bool {
        self.registered
    }

    /// 语音是否活跃（供未来窗口渲染读取口型）。
    pub fn voice_active(&self) -> bool {
        self.voice_active
    }
}

/// 供 host 装配 `AVAILABLE_MOD_FACTORIES` 的工厂单例。
pub static FACTORY: PetDesktopFactory = PetDesktopFactory;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_is_static() {
        let d = FACTORY.descriptor();
        assert_eq!(d.id, "pet-desktop");
        assert_eq!(d.name, "桌宠窗口");
        assert_eq!(d.version, "0.1.0");
        assert_eq!(d.api_version, 1);
    }

    #[test]
    fn start_registers_settings_and_subscriptions() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = FACTORY.create(services, serde_json::json!({})).unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        // 1 个 settings spec。
        assert_eq!(mock.specs.len(), 1);
        let spec = &mock.specs[0];
        assert_eq!(spec.mod_id, "pet-desktop");
        assert_eq!(spec.fields.len(), 3);
        assert_eq!(spec.fields[0].key(), "always_on_top");
        assert_eq!(spec.fields[1].key(), "click_through");
        assert_eq!(spec.fields[2].key(), "opacity");
        // 2 个订阅：VoiceStarted + VoiceEnded。
        assert_eq!(mock.subs.len(), 2);
    }

    #[test]
    fn voice_events_flip_voice_active() {
        let services = ModServices::new(
            ModActionSender::new(|_| true),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        // 直接构造具体类型（create 返回 Box<dyn ModRuntime>，在测试中构造实例更便于断言）。
        let mut rt = PetDesktopRuntime {
            services,
            registered: false,
            voice_active: false,
        };
        // VoiceStarted 后 voice_active 应为 true。
        rt.on_event(ModEventTopic::VoiceStarted, "").unwrap();
        assert!(rt.voice_active());
        // VoiceEnded 后 voice_active 应为 false。
        rt.on_event(ModEventTopic::VoiceEnded, "").unwrap();
        assert!(!rt.voice_active());
        // 其它事件不影响状态。
        rt.on_event(ModEventTopic::TurnStarted, "").unwrap();
        assert!(!rt.voice_active());
        // shutdown 复位。
        rt.shutdown().unwrap();
        assert!(!rt.voice_active());
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
