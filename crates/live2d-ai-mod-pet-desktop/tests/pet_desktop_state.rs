//! 桌宠窗口 Mod 的可测面回归（Wave 2，E 轨）。
//!
//! 这些用例**只碰本 crate + `live2d-ai-mod-system` 的公开面**，不构造
//! `ModRegistry`（它是 `live2d-ai-desktop` 的，不是本 crate 的依赖）——
//! 因此不会与别的轨争共享测试文件。`GET /api/v1/mods/{id}/state` 的 host 侧
//! 语义（404 / 503 / 200）已有基座回归，本文件负责证明**喂给它的 `state_json`
//! 内容正确，且随 config 立刻变化**。
//!
//! 「改配置 → state 立刻反映」在真实服务上的 curl 步骤见
//! `docs/architecture/pet-desktop-mod-v0.md` §5（含期望 JSON）。

use live2d_ai_mod_pet_desktop::{
    DEFAULT_ALWAYS_ON_TOP, DEFAULT_CLICK_THROUGH, DEFAULT_OPACITY, DESCRIPTOR, FACTORY,
    MAX_OPACITY, MIN_OPACITY, PetDesktopRuntime, WINDOW_REASON_NATIVE_SHELL_DORMANT,
    always_on_top_from_config, build_state_json, clamp_opacity, click_through_from_config,
    opacity_from_config, pet_desktop_settings_spec,
};
use live2d_ai_mod_system::{
    MOD_API_VERSION, ModActionSender, ModError, ModEventSender, ModEventTopic, ModFactory,
    ModLogger, ModRegistrar, ModRuntime, ModServices, ModSettingField, ModSettingsSpec, SaySender,
    SubscriptionId,
};

/// 无副作用 services（日志丢弃、动作拒绝、say 接受、事件接受）。
fn noop_services() -> ModServices {
    ModServices::new(
        ModActionSender::new(|_| false),
        SaySender::new(|_| true),
        ModEventSender::new(|_t, _p| true),
        ModLogger::new(|_l, _m| {}),
    )
}

/// 只接受 settings 注册与事件订阅的 registrar（记录顺序）。
#[derive(Default)]
struct MockRegistrar {
    specs: Vec<ModSettingsSpec>,
    subs: Vec<ModEventTopic>,
}

impl ModRegistrar for MockRegistrar {
    fn register_settings(&mut self, spec: ModSettingsSpec) -> Result<(), ModError> {
        self.specs.push(spec);
        Ok(())
    }
    fn subscribe(&mut self, topic: ModEventTopic) -> Result<SubscriptionId, ModError> {
        self.subs.push(topic);
        Ok(SubscriptionId(self.subs.len() as u64))
    }
    fn unsubscribe(&mut self, _id: SubscriptionId) -> Result<(), ModError> {
        Ok(())
    }
}

fn runtime_with(config: serde_json::Value) -> PetDesktopRuntime {
    PetDesktopRuntime::new(noop_services(), config)
}

#[test]
fn descriptor_is_static_and_api_version_matches_host() {
    assert_eq!(DESCRIPTOR.id, "pet-desktop");
    assert_eq!(DESCRIPTOR.name, "桌宠窗口");
    assert_eq!(DESCRIPTOR.version, "0.1.0");
    assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);
    assert_eq!(FACTORY.descriptor().id, DESCRIPTOR.id);
}

/// **两处不分叉**：工厂静态 schema 与 `start` 注册的 schema 逐字段相等。
///
/// 旧骨架只在 `start` 里注册 → 「Mod 未启用时前端拿不到表单」；Wave 2 起
/// 两者由同一个 `pet_desktop_settings_spec()` 产出，本断言是那条纪律的守卫。
#[test]
fn static_settings_spec_equals_start_registered_spec() {
    let static_spec = FACTORY.settings_spec().expect("必须有静态 settings_spec");

    let mut rt = runtime_with(serde_json::json!({}));
    let mut reg = MockRegistrar::default();
    rt.start(&mut reg).expect("start 应成功");

    assert_eq!(reg.specs.len(), 1, "start 只注册一份 schema");
    assert_eq!(static_spec, reg.specs[0], "静态与运行时 schema 不得分叉");
    assert_eq!(static_spec, pet_desktop_settings_spec());
}

#[test]
fn static_settings_spec_shape_is_pinned() {
    let spec = FACTORY.settings_spec().expect("静态 schema");
    assert!(spec.validate().is_ok(), "字段 key 不得重复");
    assert_eq!(spec.mod_id, "pet-desktop");
    let keys: Vec<&str> = spec.fields.iter().map(|f| f.key()).collect();
    assert_eq!(keys, vec!["always_on_top", "click_through", "opacity"]);
    assert!(
        !keys.contains(&"enabled"),
        "启停唯一真源是 Mod manifest，schema 里不得有第二个 enabled"
    );

    match &spec.fields[0] {
        ModSettingField::Bool { default, .. } => assert_eq!(*default, DEFAULT_ALWAYS_ON_TOP),
        other => panic!("always_on_top 应为 Bool，实际 {other:?}"),
    }
    match &spec.fields[1] {
        ModSettingField::Bool { default, .. } => assert_eq!(*default, DEFAULT_CLICK_THROUGH),
        other => panic!("click_through 应为 Bool，实际 {other:?}"),
    }
    match &spec.fields[2] {
        ModSettingField::Number { min, max, .. } => {
            assert_eq!(*min, MIN_OPACITY);
            assert_eq!(*max, MAX_OPACITY);
        }
        other => panic!("opacity 应为 Number，实际 {other:?}"),
    }
}

#[test]
fn start_subscribes_voice_topics_only() {
    let mut rt = runtime_with(serde_json::json!({}));
    let mut reg = MockRegistrar::default();
    rt.start(&mut reg).unwrap();
    assert!(rt.is_ready());
    assert_eq!(
        reg.subs,
        vec![ModEventTopic::VoiceStarted, ModEventTopic::VoiceEnded],
        "v1 语义：只订阅语音开始/结束"
    );
}

/// 缺省回落：空 config → `true / false / 0.95`，窗口休眠。
#[test]
fn state_falls_back_to_documented_defaults() {
    let mut rt = runtime_with(serde_json::json!({}));
    let state = rt.state_json().expect("pet-desktop 必须实现 state_json");
    assert_eq!(state["always_on_top"], serde_json::json!(true));
    assert_eq!(state["click_through"], serde_json::json!(false));
    assert_eq!(state["opacity"], serde_json::json!(0.95));
    assert_eq!(state["voice_active"], serde_json::json!(false));
    assert_eq!(
        state["window"],
        serde_json::json!({"opened": false, "reason": "native_shell_dormant"})
    );
}

/// config JSON 优先：三个字段全部来自配置。
#[test]
fn state_reflects_config_values() {
    let mut rt = runtime_with(serde_json::json!({
        "always_on_top": false,
        "click_through": true,
        "opacity": 0.42,
    }));
    let state = rt.state_json().unwrap();
    assert_eq!(state["always_on_top"], serde_json::json!(false));
    assert_eq!(state["click_through"], serde_json::json!(true));
    assert_eq!(state["opacity"], serde_json::json!(0.42));
}

/// `opacity` 钳在 `0.1..=1.0`（越界钳位、非有限值回落缺省）。
#[test]
fn opacity_is_clamped_and_bad_numbers_fall_back() {
    assert_eq!(clamp_opacity(-3.0), MIN_OPACITY);
    assert_eq!(clamp_opacity(0.0), MIN_OPACITY);
    assert_eq!(clamp_opacity(9.5), MAX_OPACITY);
    assert_eq!(clamp_opacity(f64::NAN), DEFAULT_OPACITY);
    assert_eq!(clamp_opacity(f64::INFINITY), DEFAULT_OPACITY);
    assert_eq!(clamp_opacity(f64::NEG_INFINITY), DEFAULT_OPACITY);
    assert_eq!(clamp_opacity(0.5), 0.5);

    for raw in [-1.0, 0.0, 1.5, 100.0] {
        let mut rt = runtime_with(serde_json::json!({ "opacity": raw }));
        let got = rt.state_json().unwrap()["opacity"].as_f64().unwrap();
        assert!(
            (MIN_OPACITY..=MAX_OPACITY).contains(&got),
            "opacity={raw} 钳位后仍越界：{got}"
        );
        assert_eq!(got, clamp_opacity(raw));
    }
    // 边界值原样保留（不被误钳）。
    let mut lo = runtime_with(serde_json::json!({ "opacity": MIN_OPACITY }));
    assert_eq!(lo.state_json().unwrap()["opacity"], serde_json::json!(0.1));
    let mut hi = runtime_with(serde_json::json!({ "opacity": MAX_OPACITY }));
    assert_eq!(hi.state_json().unwrap()["opacity"], serde_json::json!(1.0));
}

/// 类型写错 → 该字段回落缺省，**不连坐**其它字段。
#[test]
fn bad_types_fall_back_per_field() {
    let config = serde_json::json!({
        "always_on_top": "yes",
        "click_through": 1,
        "opacity": "0.5",
    });
    assert!(always_on_top_from_config(&config));
    assert!(!click_through_from_config(&config));
    assert_eq!(opacity_from_config(&config), DEFAULT_OPACITY);

    let mut rt = runtime_with(config);
    let state = rt.state_json().unwrap();
    assert_eq!(state["always_on_top"], serde_json::json!(true));
    assert_eq!(state["click_through"], serde_json::json!(false));
    assert_eq!(state["opacity"], serde_json::json!(0.95));

    // 部分合法：合法字段生效，非法字段各自回落。
    let mut rt = runtime_with(serde_json::json!({
        "always_on_top": false,
        "opacity": "oops",
    }));
    let state = rt.state_json().unwrap();
    assert_eq!(state["always_on_top"], serde_json::json!(false));
    assert_eq!(state["click_through"], serde_json::json!(false));
    assert_eq!(state["opacity"], serde_json::json!(0.95));
}

/// 窗口恒为「休眠」——这是 Wave 2 的范围声明，不是漏实现。
#[test]
fn window_is_always_dormant() {
    for config in [
        serde_json::json!({}),
        serde_json::json!({"always_on_top": true, "click_through": true, "opacity": 1.0}),
    ] {
        let mut rt = runtime_with(config);
        let window = rt.state_json().unwrap()["window"].clone();
        assert_eq!(window["opened"], serde_json::json!(false));
        assert_eq!(
            window["reason"],
            serde_json::json!(WINDOW_REASON_NATIVE_SHELL_DORMANT)
        );
    }
}

/// 事件态：`VoiceStarted` / `VoiceEnded` 翻转 `voice_active`，其它主题不动它。
#[test]
fn state_tracks_voice_events_and_ignores_others() {
    let mut rt = runtime_with(serde_json::json!({}));
    assert_eq!(
        rt.state_json().unwrap()["voice_active"],
        serde_json::json!(false)
    );
    let ready_before = rt.is_ready();

    rt.on_event(ModEventTopic::VoiceStarted, "").unwrap();
    assert!(rt.voice_active());
    assert_eq!(
        rt.state_json().unwrap()["voice_active"],
        serde_json::json!(true)
    );

    // 重复 VoiceStarted 幂等。
    rt.on_event(ModEventTopic::VoiceStarted, "payload 不参与状态")
        .unwrap();
    assert_eq!(
        rt.state_json().unwrap()["voice_active"],
        serde_json::json!(true)
    );

    rt.on_event(ModEventTopic::VoiceEnded, "").unwrap();
    assert!(!rt.voice_active());
    assert_eq!(
        rt.state_json().unwrap()["voice_active"],
        serde_json::json!(false)
    );

    for topic in [
        ModEventTopic::TurnStarted,
        ModEventTopic::TurnPrompt,
        ModEventTopic::TextDelta,
        ModEventTopic::ActionFinished,
        ModEventTopic::ModelActivated,
    ] {
        rt.on_event(topic, "x").unwrap();
        assert!(!rt.voice_active(), "{topic:?} 不得改变口型信号");
    }
    assert_eq!(rt.is_ready(), ready_before, "事件处理不得改变 start 状态");
}

/// `shutdown` 复位既有语义：ready=false、voice_active=false。
#[test]
fn shutdown_resets_readiness_and_voice() {
    let mut rt = runtime_with(serde_json::json!({}));
    let mut reg = MockRegistrar::default();
    rt.start(&mut reg).unwrap();
    rt.on_event(ModEventTopic::VoiceStarted, "").unwrap();
    assert!(rt.is_ready() && rt.voice_active());

    rt.shutdown().unwrap();
    assert!(!rt.is_ready());
    assert!(!rt.voice_active());
    assert_eq!(
        rt.state_json().unwrap()["voice_active"],
        serde_json::json!(false)
    );
}

/// **闭环（进程内版）**：换一份 config → 下一次 `state_json` 立刻反映。
///
/// 真实服务上等价于 `POST /api/v1/mods/pet-desktop/config`（host
/// `reload_config` + `restart` → 新实例带新 config），curl 步骤见架构文档 §5。
#[test]
fn reconfigure_is_reflected_immediately() {
    let mut rt = runtime_with(serde_json::json!({
        "always_on_top": true,
        "click_through": false,
        "opacity": 0.95,
    }));
    assert_eq!(
        rt.state_json().unwrap()["click_through"],
        serde_json::json!(false)
    );

    rt.reconfigure(serde_json::json!({
        "always_on_top": false,
        "click_through": true,
        "opacity": 0.2,
    }));
    let state = rt.state_json().unwrap();
    assert_eq!(state["always_on_top"], serde_json::json!(false));
    assert_eq!(state["click_through"], serde_json::json!(true));
    assert_eq!(state["opacity"], serde_json::json!(0.2));
    // 事件态不受配置改动影响。
    assert_eq!(state["voice_active"], serde_json::json!(false));
}

/// `create` 把 config 带进实例——host `start_one` 就是这么造的
/// （`entries[id].config` → `factory.create`），所以 HTTP 改配置 + restart 后
/// 状态面必然是新值。
#[test]
fn factory_create_carries_config_into_state() {
    let mut boxed: Box<dyn ModRuntime> = FACTORY
        .create(
            noop_services(),
            serde_json::json!({
                "always_on_top": false,
                "click_through": true,
                "opacity": 0.31,
            }),
        )
        .expect("create 应成功");
    let state = boxed.state_json().expect("state_json 应可用");
    assert_eq!(state["always_on_top"], serde_json::json!(false));
    assert_eq!(state["click_through"], serde_json::json!(true));
    assert_eq!(state["opacity"], serde_json::json!(0.31));
    assert_eq!(state["window"]["opened"], serde_json::json!(false));
}

/// 只读语义：连续取快照内容一致，且**不改动** runtime 的 config / 事件态。
#[test]
fn state_json_is_a_read_only_snapshot() {
    let mut rt = runtime_with(serde_json::json!({"click_through": true, "opacity": 0.7}));
    rt.on_event(ModEventTopic::VoiceStarted, "").unwrap();

    let config_before = rt.config().clone();
    let first = rt.state_json().unwrap();
    let second = rt.state_json().unwrap();

    assert_eq!(first, second, "同一状态下快照必须稳定");
    assert_eq!(*rt.config(), config_before, "取快照不得改配置");
    assert!(rt.voice_active(), "取快照不得改事件态");
    assert_eq!(rt.always_on_top(), DEFAULT_ALWAYS_ON_TOP);
    assert!(rt.click_through());
    assert_eq!(rt.opacity(), 0.7);
}

/// 纯函数 `build_state_json` 与 runtime 版本是同一形状（不动 runtime 也能测）。
#[test]
fn build_state_json_matches_runtime_shape() {
    let config = serde_json::json!({"always_on_top": false, "opacity": 2.0});
    let pure = build_state_json(&config, true);
    let mut rt = runtime_with(config);
    rt.on_event(ModEventTopic::VoiceStarted, "").unwrap();
    assert_eq!(pure, rt.state_json().unwrap());
    assert_eq!(pure["opacity"], serde_json::json!(1.0), "纯函数也走钳位");
}
