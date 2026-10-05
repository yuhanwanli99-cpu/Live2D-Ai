//! `mod_registry` HostChannels 场景回归（自内联 `mod tests` 拆出）。
//!
//! 场景：action 休眠（契约断言）/ say 回路 / 一等 `apply_settings` /
//! `dispatch_event_and_flush` 回执 / 失败隔离 / config 透传。脚手架见 `tests_support`。

// 休眠断言要构造一个 ActionRequest——它仍是 Mod API 契约（`ModServices.action_tx`
// 的类型），只是 host 不再有驱动方。
use live2d_ai_mod_system::services::ActionRequest;

use super::tests_support::*;
use super::*;

// ------------------------------------------------------- HostChannels tests

thread_local! {
    /// TestActionRuntime 把创建时的 ModServices 存进来，供测试直接调用。
    static CAPTURED_SERVICES: std::cell::RefCell<Option<ModServices>> =
        const { std::cell::RefCell::new(None) };
}

/// TestActionMod：捕获 ModServices 给测试直调 action_tx / say_tx。
#[rustfmt::skip]
struct TestActionMod;
impl ModFactory for TestActionMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "action_test",
            name: "ActionTest",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        services: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        CAPTURED_SERVICES.with(|c| *c.borrow_mut() = Some(services));
        Ok(Box::new(TestActionRuntime))
    }
}
struct TestActionRuntime;
impl ModRuntime for TestActionRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        Ok(())
    }
}
static ACTION_FACTORIES: &[&dyn ModFactory] = &[&TestActionMod];

/// **同轮生效的时序保证**（2026-09-15）：`dispatch_event_and_flush` 返回时，
/// worker **已经**把这条事件交给 Running Mod——不需要轮询、不需要 sleep。
///
/// 反向断言（去掉回执等待即红）：`dispatch_event` 是 `try_send`，返回只代表
/// 「入队成功」，此刻 Mod 的记录**大概率还是空的**——supervisor 若在此之后
/// 立刻读 session_prompts，读到的就是旧值（旧的「只对下一轮生效」缺陷）。
///
/// 用**本测试专属**的工厂与计数（不碰 `RECEIVED`）：并行跑测试时，别的用例
/// 共享那个 static，会把条数断言染红——时序断言必须只看自己的事件流。
#[rustfmt::skip]
#[test]
fn dispatch_event_and_flush_means_processed_not_queued() {
    struct FlushMod;
    impl ModFactory for FlushMod {
        fn descriptor(&self) -> &'static ModDescriptor {
            static D: ModDescriptor = ModDescriptor {
                id: "flushmod",
                name: "Flush",
                version: "0.1.0",
                api_version: 1,
            };
            &D
        }
        fn create(&self, _: ModServices, _: serde_json::Value) -> Result<Box<dyn ModRuntime>, ModError> {
            Ok(Box::new(FlushRuntime))
        }
    }
    struct FlushRuntime;
    impl ModRuntime for FlushRuntime {
        fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> { Ok(()) }
        fn on_event(&mut self, topic: ModEventTopic, _: &str) -> Result<(), ModError> {
            FLUSH_RECEIVED.lock().unwrap().push(topic.as_str().to_string());
            Ok(())
        }
    }
    static FLUSH_RECEIVED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static FLUSH_FACTORIES: &[&dyn ModFactory] = &[&FlushMod];

    FLUSH_RECEIVED.lock().unwrap().clear();
    let mut reg = ModRegistry::new(FLUSH_FACTORIES, &serde_json::json!({"mods":{"flushmod":{"enabled":true}}}));
    reg.start_all();
    // 先制造积压：worker 必须先把这些处理完，回执才有意义。
    for i in 0..50 {
        assert!(reg.dispatch_event(ModEventTopic::TextDelta, &format!("d{i}"), None));
    }
    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnPrompt, "本轮正文", Some("sess-1")));
    let seen = FLUSH_RECEIVED.lock().unwrap().clone();
    assert_eq!(
        seen.last().map(String::as_str),
        Some("turn_prompt"),
        "flush 返回时 TurnPrompt 必须已被处理（且排在积压之后）: {} 条",
        seen.len()
    );
    assert_eq!(seen.len(), 51, "积压的 50 条也要处理完才回执");
}

/// 事件送达测试：enable TestMod，start_all 后 dispatch_event，
/// **阻塞等 on_event 发来的送达信号**（确定性同步，无墙钟轮询）。
///
/// 10s 只是**防挂死兜底**（worker 线程死了才会触发），不参与通过/失败判定：
/// 断言读的是 `RECEIVED` 里真的出现了本条事件，而信号保证了「已被 worker 处理」。
#[rustfmt::skip]
#[test]
fn event_delivers_to_running_mod() {
    use std::sync::mpsc::RecvTimeoutError;
    const HANG_GUARD: std::time::Duration = std::time::Duration::from_secs(10);
    RECEIVED.lock().unwrap().clear();
    let delivered = watch_delivery();
    let mut reg = ModRegistry::new(FACTORIES, &serde_json::json!({"mods":{"test":{"enabled":true}}}));
    reg.start_all();
    let slot = reg.runtimes.get("test").expect("runtime 槽位存在");
    assert!(slot.lock().unwrap().is_some(), "启用后 runtime 应已装配");
    assert!(reg.dispatch_event(ModEventTopic::TurnStarted, "{}", None));
    let topic = match delivered.recv_timeout(HANG_GUARD) {
        Ok(topic) => topic,
        Err(RecvTimeoutError::Timeout) => panic!(
            "10s 内没有收到 on_event 的送达信号：worker 疑似挂死（这是防挂死兜底，不是准时性判据）"
        ),
        Err(RecvTimeoutError::Disconnected) => panic!("送达信号通道断开（测试自身错误）"),
    };
    assert_eq!(topic, "turn_started", "送达信号应携带本次投递的话题");
    assert!(
        RECEIVED.lock().unwrap().iter().any(|t| t == "turn_started"),
        "worker 应将 event 投递到 running Mod 的 on_event"
    );
}

#[rustfmt::skip]
struct FailMod;
impl ModFactory for FailMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "failmod",
            name: "Fail",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        _: ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(FailRuntime))
    }
}
struct FailRuntime;
impl ModRuntime for FailRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        Ok(())
    }
    fn on_event(&mut self, _: ModEventTopic, _: &str) -> Result<(), ModError> {
        Err(ModError::Other("on_event 失败".into()))
    }
}
static FAIL_FACTORIES: &[&dyn ModFactory] = &[&FailMod];

/// `on_event` 失败 → worker 清空该 Mod 的 runtime 槽位（失败隔离）。
///
/// **确定性同步**（D1 同族修复）：不再「50ms×10 轮询槽位变空」，改用
/// `dispatch_event_and_flush`——worker 在**所有 Mod 都处理完之后**才回执，
/// 回执返回时失败隔离必然已落地（且 worker 已放下 runtime 锁）。
/// 断言内容不变：槽位空 + 二次 dispatch 不 panic。
#[rustfmt::skip]
#[test]
fn event_failure_isolates_mod() {
    let mut reg = ModRegistry::new(FAIL_FACTORIES, &serde_json::json!({"mods":{"failmod":{"enabled":true}}}));
    reg.start_all();
    assert!(reg.dispatch_event_and_flush(ModEventTopic::TurnStarted, "{}", None));
    let slot = reg.runtimes.get("failmod").expect("runtime 槽位存在");
    assert!(slot.lock().unwrap().is_none(), "on_event 失败后 runtime 槽位应被清空");
    assert!(reg.dispatch_event(ModEventTopic::TurnStarted, "{}", None)); // 二次 dispatch 不 panic。
}

// ------------------------------------------------------- enable_with_config test

thread_local! {
    /// ConfigCaptureMod 把 factory.create 收到的 config 写到此处。
    static CAPTURED_CONFIG: std::cell::RefCell<Option<serde_json::Value>> =
        const { std::cell::RefCell::new(None) };
}

#[rustfmt::skip]
struct ConfigCaptureMod;
impl ModFactory for ConfigCaptureMod {
    fn descriptor(&self) -> &'static ModDescriptor {
        static D: ModDescriptor = ModDescriptor {
            id: "config_capture",
            name: "ConfigCapture",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        _: ModServices,
        config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        CAPTURED_CONFIG.with(|c| *c.borrow_mut() = Some(config));
        Ok(Box::new(ConfigCaptureRuntime))
    }
}
struct ConfigCaptureRuntime;
impl ModRuntime for ConfigCaptureRuntime {
    fn start(&mut self, _: &mut dyn ModRegistrar) -> Result<(), ModError> {
        Ok(())
    }
}
static CONFIG_FACTORIES: &[&dyn ModFactory] = &[&ConfigCaptureMod];

/// enable_with_config 应在 start_one 之前写入 config，使 factory.create
/// 收到注入的 config。
#[rustfmt::skip]
#[test]
fn enable_with_config_passes_config_to_runtime() {
    CAPTURED_CONFIG.with(|c| c.borrow_mut().take()); // 清残留。
    let mut reg = ModRegistry::new(CONFIG_FACTORIES, &serde_json::json!({}));
    let injected = serde_json::json!({"key": "value", "n": 42});
    reg.enable_with_config("config_capture", injected.clone()).expect("enable_with_config");
    // enable_with_config 会调用 start_one → factory.create 捕获 config。
    let got = CAPTURED_CONFIG.with(|c| c.borrow().clone());
    assert_eq!(got.as_ref(), Some(&injected), "factory.create 应收到注入的 config，got {got:?}");
}

// ------------------------------------------------------- HostChannels tests

/// **休眠回归（rc.2）**：Mod 提交 `ActionRequest` 不会到达任何地方。
///
/// 这条守的是「动作在产品路径上不存在」：host 注入的是固定的休眠 sender，
/// 请求返回 `false`（未被接受）。若有人把 `HostChannels` 的动作回路接回来，
/// 这里会变成 `true`（或有东西被记录），立刻红。
#[rustfmt::skip]
#[test]
fn action_request_is_dormant_not_delivered() {
    CAPTURED_SERVICES.with(|c| c.borrow_mut().take()); // 清上一轮残留。
    let mut reg = ModRegistry::new(
            ACTION_FACTORIES,
            &serde_json::json!({"mods":{"action_test":{"enabled":true}}}),
        )
        .with_host_channels(HostChannels {
            say: Arc::new(|_| true),
            apply_settings: Arc::new(|_| true),
            config_path: String::new(),
            read_settings: Arc::new(|| serde_json::json!({})),
            session_prompts: live2d_ai_mod_system::ModSessionPrompts::disabled(),
            cues: Arc::new(|_| false),
        });
    reg.start_all(); // 触发 factory.create → 捕获 services。
    let req = ActionRequest {
        mod_id: "action_test",
        action: "nod",
        strength: 2,
    };
    let accepted = CAPTURED_SERVICES.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| s.action_tx.request(req.clone()))
            .unwrap_or(true)
    });
    assert!(!accepted, "动作通道应休眠：ActionRequest 不得被接受");
}

/// HostChannels say 回路测试：提交文本，断言 host.say 记录到 Vec。
#[rustfmt::skip]
#[test]
fn say_reaches_host_channel() {
    CAPTURED_SERVICES.with(|c| c.borrow_mut().take()); // 清上一轮残留。
    let recorded: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let recorded_c = recorded.clone();
    let mut reg = ModRegistry::new(
            ACTION_FACTORIES,
            &serde_json::json!({"mods":{"action_test":{"enabled":true}}}),
        )
        .with_host_channels(HostChannels {
            say: Arc::new(move |t| {
                recorded.lock().unwrap().push(t);
                true
            }),
            apply_settings: Arc::new(|_| true),
            config_path: String::new(),
            read_settings: Arc::new(|| serde_json::json!({})),
            session_prompts: live2d_ai_mod_system::ModSessionPrompts::disabled(),
            cues: Arc::new(|_| false),
        });
    reg.start_all(); // 触发 factory.create → 捕获 services。
    let text = "hello from mod".to_string();
    let sent = CAPTURED_SERVICES.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| s.say_tx.say(text.clone()))
            .unwrap_or(false)
    });
    assert!(sent, "say 应被 host channel 接受");
    let got = recorded_c.lock().unwrap().clone();
    assert!(got.iter().any(|t| t == "hello from mod"), "记录应含文本，got {got:?}");
}

/// **M4 一等 apply_settings**：Mod 经 `services.apply_settings` 提交的 patch
/// 必须到达 host 注入的回调（不再需要 `__apply_settings` 事件信封）。
#[test]
fn apply_settings_applier_reaches_host() {
    CAPTURED_SERVICES.with(|c| c.borrow_mut().take()); // 清上一轮残留。
    let captured: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_c = captured.clone();
    let mut reg = ModRegistry::new(
        ACTION_FACTORIES,
        &serde_json::json!({"mods":{"action_test":{"enabled":true}}}),
    )
    .with_host_channels(HostChannels {
        say: Arc::new(|_| true),
        apply_settings: Arc::new(move |patch| {
            captured_c.lock().unwrap().push(patch);
            true
        }),
        config_path: String::new(),
        read_settings: Arc::new(|| serde_json::json!({})),
        session_prompts: live2d_ai_mod_system::ModSessionPrompts::disabled(),
        cues: Arc::new(|_| false),
    });
    reg.start_all(); // 触发 factory.create → 捕获 services。

    let patch = serde_json::json!({"llm": {"base_url": "http://127.0.0.1:11434/v1"}});
    let accepted = CAPTURED_SERVICES.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| s.apply_settings.apply(patch.clone()))
            .unwrap_or(false)
    });
    assert!(accepted, "一等 apply_settings 应被 host 接受");
    let got = captured.lock().unwrap().clone();
    assert_eq!(got.len(), 1, "host 应收到一次 patch");
    assert_eq!(got[0]["llm"]["base_url"], "http://127.0.0.1:11434/v1");
}

/// **M4 默认拒绝**：没有 HostChannels 时 `apply_settings` 返回 `false`
/// （单测 / 无 supervisor 环境不得静默写盘）。
#[test]
fn apply_settings_defaults_to_rejected() {
    let services = ModServices::new(
        ModActionSender::new(|_| false),
        SaySender::new(|_| true),
        ModEventSender::new(|_, _| true),
        ModLogger::new(|_, _| {}),
    );
    assert!(
        !services
            .apply_settings
            .apply(serde_json::json!({"llm": {}})),
        "未注入 host 时必须拒绝"
    );
}
