//! P1.5（0.7.0 收口）：Mod 事件订阅路由回归（验收项 4）。
//!
//! - `HostRegistrar::subscribe` 不再丢弃 topic：worker 只向订阅了该 topic 的
//!   Mod 投递（不同 Mod 各收各的，没订阅的收不到）；
//! - `unsubscribe` 撤销最后一个引用后，该 topic 不再送达；
//! - 回执照旧等所有 Mod 处理完（`dispatch_event_and_flush` 的时序不变）。
//!
//! 单列文件：`tests_lifecycle.rs` 已 477 行（500 行棘轮），挂法照
//! `tests_start_cause` 的 `#[path]` 先例（见 `mod_registry.rs` 末尾）。

use super::events::HostRegistrar;
use super::*;

fn seen(m: &Mutex<Vec<String>>) -> Vec<String> {
    m.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

// ------------------------------- 1) 只投递「订阅了该 topic」的 Mod

static PROMPT_SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());
static TEXT_SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());
static NO_SUB_SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn clear_seen() {
    for m in [&PROMPT_SEEN, &TEXT_SEEN, &NO_SUB_SEEN] {
        m.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}

/// 只订阅一个 topic 的桩：`start` 只 subscribe `$topic`，`on_event` 记到 `$seen`。
macro_rules! single_topic_mod {
    ($mod_ty:ident, $rt_ty:ident, $id:literal, $topic:expr, $seen:ident) => {
        struct $mod_ty;

        impl ModFactory for $mod_ty {
            fn descriptor(&self) -> &'static ModDescriptor {
                static D: ModDescriptor = ModDescriptor {
                    id: $id,
                    name: $id,
                    version: "0.1.0",
                    api_version: live2d_ai_mod_system::MOD_API_VERSION,
                };
                &D
            }

            fn create(
                &self,
                _: ModServices,
                _: serde_json::Value,
            ) -> Result<Box<dyn ModRuntime>, ModError> {
                Ok(Box::new($rt_ty))
            }
        }

        struct $rt_ty;

        impl ModRuntime for $rt_ty {
            fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
                registrar.subscribe($topic)?;
                Ok(())
            }

            fn on_event(&mut self, topic: ModEventTopic, _: &str) -> Result<(), ModError> {
                $seen
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(topic.as_str().to_string());
                Ok(())
            }
        }
    };
}

single_topic_mod!(
    PromptOnlyMod,
    PromptOnlyRuntime,
    "prompt_only",
    ModEventTopic::TurnPrompt,
    PROMPT_SEEN
);
single_topic_mod!(
    TextOnlyMod,
    TextOnlyRuntime,
    "text_only",
    ModEventTopic::TextDelta,
    TEXT_SEEN
);

/// 一个 topic 都不订阅的桩：按 P1.5 契约它不该收到任何事件。
struct NoSubMod;

impl ModFactory for NoSubMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "no_sub",
            name: "NoSub",
            version: "0.1.0",
            api_version: live2d_ai_mod_system::MOD_API_VERSION,
        };
        &D
    }

    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(NoSubRuntime))
    }
}

struct NoSubRuntime;

impl ModRuntime for NoSubRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 刻意不订阅：没订阅 = 收不到（这是契约，不是故障）。
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, _: &str) -> Result<(), ModError> {
        NO_SUB_SEEN
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(topic.as_str().to_string());
        Ok(())
    }
}

#[test]
fn only_subscribed_topic_is_delivered() {
    clear_seen();
    // 同一个 registry 里放三个工厂：worker 的路由按 Mod 各自分流。
    static BOTH: &[&dyn ModFactory] = &[&PromptOnlyMod, &TextOnlyMod, &NoSubMod];
    let mut reg = ModRegistry::new(BOTH, &serde_json::json!({}));
    reg.enable("prompt_only").expect("启用 prompt_only");
    reg.enable("text_only").expect("启用 text_only");
    reg.enable("no_sub").expect("启用 no_sub");

    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnPrompt, "本轮正文", Some("s1")));
    assert_eq!(
        seen(&PROMPT_SEEN),
        vec!["turn_prompt"],
        "订阅了 TurnPrompt 的 Mod 应收到"
    );
    assert!(
        seen(&TEXT_SEEN).is_empty(),
        "未订阅 TurnPrompt 的 Mod 不得收到"
    );
    assert!(
        seen(&NO_SUB_SEEN).is_empty(),
        "没订阅任何 topic 的 Mod 不得收到"
    );

    assert!(reg.dispatch_event_and_flush(ModEventTopic::TextDelta, "增量", None));
    assert_eq!(
        seen(&PROMPT_SEEN),
        vec!["turn_prompt"],
        "只订阅 TurnPrompt 的 Mod 不得收到 TextDelta"
    );
    assert_eq!(seen(&TEXT_SEEN), vec!["text_delta"]);

    // 两个 Mod 都没订阅 VoiceStarted → 谁都不该收到。
    assert!(reg.dispatch_event_and_flush(ModEventTopic::VoiceStarted, "{}", None));
    assert_eq!(seen(&PROMPT_SEEN).len(), 1);
    assert_eq!(seen(&TEXT_SEEN).len(), 1);
    assert!(seen(&NO_SUB_SEEN).is_empty(), "没订阅的 Mod 永远收不到事件");
}

// ------------------------------- 2) unsubscribe 之后不再收到

static UNSUB_ID: Mutex<Option<SubscriptionId>> = Mutex::new(None);
static UNSUB_SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct UnsubMod;

impl ModFactory for UnsubMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "unsub",
            name: "Unsub",
            version: "0.1.0",
            api_version: live2d_ai_mod_system::MOD_API_VERSION,
        };
        &D
    }

    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(UnsubRuntime))
    }
}

struct UnsubRuntime;

impl ModRuntime for UnsubRuntime {
    fn start(&mut self, registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        let id = registrar.subscribe(ModEventTopic::TurnPrompt)?;
        *UNSUB_ID.lock().unwrap_or_else(|e| e.into_inner()) = Some(id);
        Ok(())
    }

    fn on_event(&mut self, topic: ModEventTopic, _: &str) -> Result<(), ModError> {
        UNSUB_SEEN
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(topic.as_str().to_string());
        Ok(())
    }
}

static UNSUB_FACTORIES: &[&dyn ModFactory] = &[&UnsubMod];

#[test]
fn unsubscribe_stops_delivery() {
    *UNSUB_ID.lock().unwrap_or_else(|e| e.into_inner()) = None;
    UNSUB_SEEN.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let mut reg = ModRegistry::new(UNSUB_FACTORIES, &serde_json::json!({}));
    reg.enable("unsub").expect("启用应成功");
    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnPrompt, "第一轮", Some("s1")));
    assert_eq!(seen(&UNSUB_SEEN), vec!["turn_prompt"]);

    let id = UNSUB_ID
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .expect("start 应记下 TurnPrompt 的订阅 id");

    // 运行期取消订阅：与 Mod 在 start 里拿到的是同一份 registrar 契约。
    {
        let mut registrar = HostRegistrar {
            mod_id: "unsub",
            settings_specs: &mut reg.settings_specs,
            subscriptions: &mut reg.subscriptions,
            routes: &reg.subscription_routes,
            next_id: &reg.next_sub_id,
        };
        registrar.unsubscribe(id).expect("unsubscribe 应成功");
    }
    assert!(
        reg.subscription_routes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get("unsub")
            .is_none(),
        "撤销最后一个订阅后路由表里不应再有该 Mod"
    );

    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnPrompt, "第二轮", Some("s1")));
    assert_eq!(
        seen(&UNSUB_SEEN),
        vec!["turn_prompt"],
        "unsubscribe 之后不得再收到该 topic"
    );
}
