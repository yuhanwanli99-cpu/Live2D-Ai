//! live2d-ai-mod-director（动作编序 Mod，节点 E E7）。
//!
//! 第一个 performance-sequence 演示 Mod：在 turn 开始时按 `config.sequence`
//! 顺序驱动一组预设 Live2D 动作，演示 Mod 驱动主链路动作的编序能力。
//!
//! 作为 Mod 系统的验证示例，它演示：
//! - [`ModFactory`] + [`ModRuntime`] 实现（`descriptor` / `create` / `start`）。
//! - `start` 注册 settings schema（暴露给前端统一渲染）。
//! - 订阅事件（`TurnStarted` / `ActionFinished`）。
//! - 通过 `action_tx.request()` 驱动主链路动作。
//! - 配置驱动动作序列 + 白名单校验。

use live2d_ai_mod_system::services::ActionRequest;
use live2d_ai_mod_system::*;

/// 动作名白名单（6 个基础动作；名字与 `live2d_ai_core::ActionId::name()`
/// 严格一致，host 侧 `parse_action_id` 按 `name()` 严格匹配）。
///
/// 注意：摇头是 `"shake_no"`（不是 `"shake"`）——与 core 协议名对齐，
/// 否则 host `parse_action_id("shake")` 返回 `None` 导致摇头永远不触发。
const ACTIONS_WHITELIST: &[&str] = &[
    "nod",
    "shake_no",
    "tilt",
    "look_around",
    "listen",
    "surprise",
];

/// Mod 描述符（静态身份）。
const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: "director",
    name: "动作编排",
    version: "0.1.0",
    api_version: 1,
};

/// Director Mod 的运行时状态。
pub struct DirectorRuntime {
    services: ModServices,
    config: serde_json::Value,
    /// 已过滤为白名单合法的动作序列。
    sequence: Vec<&'static str>,
    /// 非法动作（从 config.sequence 中筛出）。
    invalid_actions: Vec<String>,
    /// 是否开启自动编序。
    auto_sequence_on_turn: bool,
    /// 编序间隔 ms（当前仅记录，不真正计时）。
    interval_ms: u64,
    /// 最近一次成功请求的动作名。
    last_sequence: Vec<&'static str>,
    /// 累计请求失败次数。
    last_error: u32,
    /// settings schema 是否已注册（start 成功标志）。
    registered: bool,
}

impl Default for DirectorRuntime {
    fn default() -> Self {
        Self {
            services: ModServices::new(
                ModActionSender::new(|_| true),
                SaySender::new(|_| true),
                ModEventSender::new(|_topic, _payload| true),
                ModLogger::new(|_lvl, _msg| {}),
            ),
            config: serde_json::Value::Object(Default::default()),
            sequence: Vec::new(),
            invalid_actions: Vec::new(),
            auto_sequence_on_turn: true,
            interval_ms: 600,
            last_sequence: Vec::new(),
            last_error: 0,
            registered: false,
        }
    }
}

/// Director 工厂。
pub struct DirectorFactory;

impl ModFactory for DirectorFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    fn create(
        &self,
        services: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        let mut rt = DirectorRuntime {
            services,
            config,
            sequence: Vec::new(),
            invalid_actions: Vec::new(),
            auto_sequence_on_turn: true,
            interval_ms: 600,
            last_sequence: Vec::new(),
            last_error: 0,
            registered: false,
        };
        rt.parse_config();
        Ok(Box::new(rt))
    }
}

impl DirectorRuntime {
    /// 从 config JSON 解析 sequence（已白名单过滤）与其它字段。
    fn parse_config(&mut self) {
        let cfg = &self.config;
        self.auto_sequence_on_turn = cfg
            .get("auto_sequence_on_turn")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        self.interval_ms = cfg
            .get("interval_ms")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(600);

        // 解析 sequence，默认 ["nod", "shake_no"]（摇头用协议名 "shake_no"，与
        // core `ActionId::name()` 一致；"shake" 是历史错误名，host 侧会拒绝）。
        let raw_seq: Vec<String> = cfg
            .get("sequence")
            .and_then(serde_json::Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| vec!["nod".to_string(), "shake_no".to_string()]);

        self.sequence.clear();
        self.invalid_actions.clear();
        for item in raw_seq {
            if let Some(valid) = ACTIONS_WHITELIST.iter().find(|a| **a == item) {
                self.sequence.push(valid);
            } else {
                self.invalid_actions.push(item);
            }
        }

        if !self.invalid_actions.is_empty() {
            self.services.logger.warn(&format!(
                "director: 跳过非法动作 {:?}",
                self.invalid_actions
            ));
        }
    }
}

impl ModRuntime for DirectorRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        let spec = ModSettingsSpec {
            mod_id: "director".to_string(),
            title: "动作编排".to_string(),
            version: 1,
            fields: vec![
                ModSettingField::Bool {
                    key: "auto_sequence_on_turn".to_string(),
                    label: "自动编序".to_string(),
                    default: true,
                },
                ModSettingField::Number {
                    key: "interval_ms".to_string(),
                    label: "编序间隔（ms）".to_string(),
                    min: 100.0,
                    max: 5000.0,
                },
                ModSettingField::String {
                    key: "greeting".to_string(),
                    label: "问候语".to_string(),
                    secret: false,
                },
            ],
        };
        registrar.register_settings(spec)?;

        registrar.subscribe(ModEventTopic::TurnStarted)?;
        registrar.subscribe(ModEventTopic::ActionFinished)?;

        self.registered = true;
        self.services.logger.info("director Mod 已启动");
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, _payload: &str) -> Result<(), ModError> {
        match topic {
            ModEventTopic::TurnStarted => {
                if !self.auto_sequence_on_turn {
                    return Ok(());
                }
                self.last_sequence.clear();
                for &action in &self.sequence {
                    let req = ActionRequest {
                        mod_id: "director",
                        action,
                        strength: 1,
                    };
                    if self.services.action_tx.request(req) {
                        self.last_sequence.push(action);
                    } else {
                        self.last_error += 1;
                    }
                }
            }
            ModEventTopic::ActionFinished => {
                self.services.logger.info("编织推进");
            }
            _ => {}
        }
        Ok(())
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        self.sequence.clear();
        self.last_sequence.clear();
        self.invalid_actions.clear();
        self.last_error = 0;
        self.registered = false;
        self.services.logger.info("director Mod 已关闭");
        Ok(())
    }
}

/// 供 host 装配 `AVAILABLE_MOD_FACTORIES` 的工厂单例。
pub static FACTORY: DirectorFactory = DirectorFactory;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[test]
    fn descriptor_is_static() {
        let d = &DESCRIPTOR;
        assert_eq!(d.id, "director");
        assert_eq!(d.name, "动作编排");
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
        let mut rt = DirectorFactory
            .create(services, serde_json::json!({}))
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();
        assert_eq!(mock.specs.len(), 1);
        assert_eq!(mock.subs.len(), 2); // TurnStarted + ActionFinished
    }

    #[test]
    fn turn_started_triggers_sequence_with_whitelist_filter() {
        // config.sequence 里放了一个非法动作，验证被跳过。
        let received: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let services = ModServices::new(
            ModActionSender::new(move |req: ActionRequest| {
                received_clone.lock().unwrap().push(req.action);
                true
            }),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = DirectorFactory
            .create(
                services,
                serde_json::json!({
                    "sequence": ["nod", "invalid_action", "tilt", "bogus"],
                    "auto_sequence_on_turn": true
                }),
            )
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();

        rt.on_event(ModEventTopic::TurnStarted, "").unwrap();

        // 非法动作被过滤，仅收到白名单合法项，顺序 preserved。
        let actions = received.lock().unwrap();
        assert_eq!(*actions, vec!["nod", "tilt"]);
    }

    #[test]
    fn default_sequence_uses_protocol_action_names() {
        // 空 config → 默认序列必须用 core `ActionId::name()` 协议名，
        // 尤其是摇头必须是 "shake_no" 而非 "shake"（后者 host parse 会拒绝）。
        let received: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let services = ModServices::new(
            ModActionSender::new(move |req: ActionRequest| {
                received_clone.lock().unwrap().push(req.action);
                true
            }),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = DirectorFactory
            .create(services, serde_json::json!({}))
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();

        // 默认序列 ["nod","shake_no"]：摇头以协议名（非 "shake"）通过白名单发送。
        rt.on_event(ModEventTopic::TurnStarted, "").unwrap();
        let actions = received.lock().unwrap();
        assert_eq!(*actions, vec!["nod", "shake_no"]);
    }

    #[test]
    fn whitelist_excludes_legacy_shake_misspelling() {
        // 用户配了历史错误名 "shake"，应被过滤（host 会拒绝它）；
        // 配 "shake_no" 则被接受。行为经 received 通道验证。
        let received: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let services = ModServices::new(
            ModActionSender::new(move |req: ActionRequest| {
                received_clone.lock().unwrap().push(req.action);
                true
            }),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = DirectorFactory
            .create(
                services,
                serde_json::json!({ "sequence": ["nod", "shake", "shake_no", "tilt"] }),
            )
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();

        rt.on_event(ModEventTopic::TurnStarted, "").unwrap();
        let actions = received.lock().unwrap();
        // "shake" 被过滤（host 不认），"shake_no" 与 "tilt" 被接受。
        assert_eq!(*actions, vec!["nod", "shake_no", "tilt"]);
    }

    #[test]
    fn action_finished_does_not_trigger_requests() {
        let received: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let received_clone = received.clone();
        let services = ModServices::new(
            ModActionSender::new(move |req: ActionRequest| {
                received_clone.lock().unwrap().push(req.action);
                true
            }),
            SaySender::new(|_| true),
            ModEventSender::new(|_t, _p| true),
            ModLogger::new(|_l, _m| {}),
        );
        let mut rt = DirectorFactory
            .create(services, serde_json::json!({}))
            .unwrap();
        let mut mock = MockRegistrar::default();
        rt.start(&mut mock).unwrap();

        rt.on_event(ModEventTopic::ActionFinished, "").unwrap();

        let actions = received.lock().unwrap();
        assert!(actions.is_empty());
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
