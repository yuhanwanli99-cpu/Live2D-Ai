//! `mods_routes` 测试共享脚手架（自内联 `mod tests` 拆出）。
//!
//! 仅供同目录 `mods_routes_tests` 使用：最小 `ServerContext`、带 schema /
//! 带运行态的测试 Mod、取 body 的小工具。`pub(super)` 只为兄弟测试模块可见性，
//! 不进产品 API（与 `mod_registry::tests_support` 同一条拆分先例）。

use super::*;
use crate::web_api::models_routes;
use crate::web_api::security::SecurityContext;
use std::io::Read;

/// 工具：构造带 mod_registry 的 ServerContext（空 registry）。
pub(super) fn dummy_ctx(sec: SecurityContext) -> ServerContext {
    ServerContext {
        status_ctx: std::sync::Arc::new(crate::web_api::app_routes::StatusContext::new(
            live2d_ai_runtime::AppSettings::default(),
            ".scratch".to_string(),
            None,
        )),
        capabilities_ctx: std::sync::Arc::new(
            crate::web_api::app_routes::CapabilitiesContext::new(),
        ),
        supervisor_slot: crate::web_api::supervisor_slot::SupervisorSlot::new(),
        broadcaster: crate::web_api::ws::Broadcaster::new(),
        security: sec,
        models: models_routes::default_store(),
        mod_registry: std::sync::Arc::new(std::sync::Mutex::new(
            crate::mod_registry::ModRegistry::new(&[], &serde_json::json!({})),
        )),
        folder_processes: std::sync::Arc::new(crate::folder_spawn::FolderProcesses::new()),
        playback_gate: std::sync::Arc::new(crate::web_api::playback_gate::PlaybackGate::new()),
        peer_pi: std::sync::Arc::new(crate::web_api::peer_pi::PeerState::new()),
    }
}

/// 带 schema 的测试 Mod：注册 Bool + secret String + 普通 String。
pub(super) struct SpecMod;

impl live2d_ai_mod_system::ModFactory for SpecMod {
    fn descriptor(&self) -> &'static live2d_ai_mod_system::ModDescriptor {
        static D: live2d_ai_mod_system::ModDescriptor = live2d_ai_mod_system::ModDescriptor {
            id: "specmod",
            name: "SpecMod",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        _: live2d_ai_mod_system::ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn live2d_ai_mod_system::ModRuntime>, live2d_ai_mod_system::ModError> {
        Ok(Box::new(SpecRuntime))
    }
}

pub(super) struct SpecRuntime;

impl live2d_ai_mod_system::ModRuntime for SpecRuntime {
    fn start(
        &mut self,
        registrar: &mut dyn live2d_ai_mod_system::ModRegistrar,
    ) -> Result<(), live2d_ai_mod_system::ModError> {
        registrar.register_settings(ModSettingsSpec {
            mod_id: "specmod".to_string(),
            title: "规格".to_string(),
            version: 1,
            fields: vec![
                ModSettingField::Bool {
                    key: "on".to_string(),
                    label: "开关".to_string(),
                    default: true,
                },
                ModSettingField::String {
                    key: "token".to_string(),
                    label: "密钥".to_string(),
                    secret: true,
                    default: None,
                },
                ModSettingField::String {
                    key: "path".to_string(),
                    label: "路径".to_string(),
                    secret: false,
                    default: None,
                },
            ],
        })
    }
}

pub(super) static SPEC_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[&SpecMod];

/// 工具：带指定 factories + manifest 的 ServerContext（Mod 已 start_all）。
pub(super) fn ctx_with_mod(manifest: serde_json::Value) -> ServerContext {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let mut registry = crate::mod_registry::ModRegistry::new(SPEC_FACTORIES, &manifest);
    registry.start_all();
    *ctx.mod_registry.lock().unwrap() = registry;
    ctx
}

pub(super) fn body_string(resp: Response<Cursor<Vec<u8>>>) -> String {
    let mut reader = resp.into_reader();
    let mut s = String::new();
    let _ = reader.read_to_string(&mut s);
    s
}

// ------------------------------------------- Wave 2：GET {id}/state

/// 带运行态快照的测试 Mod（`state_json` 有一份可断言的内容）。
pub(super) struct StateModFactory;

impl live2d_ai_mod_system::ModFactory for StateModFactory {
    fn descriptor(&self) -> &'static live2d_ai_mod_system::ModDescriptor {
        static D: live2d_ai_mod_system::ModDescriptor = live2d_ai_mod_system::ModDescriptor {
            id: "stateful",
            name: "Stateful",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        _: live2d_ai_mod_system::ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn live2d_ai_mod_system::ModRuntime>, live2d_ai_mod_system::ModError> {
        Ok(Box::new(StatefulRuntime))
    }
}

pub(super) struct StatefulRuntime;

impl live2d_ai_mod_system::ModRuntime for StatefulRuntime {
    fn start(
        &mut self,
        _: &mut dyn live2d_ai_mod_system::ModRegistrar,
    ) -> Result<(), live2d_ai_mod_system::ModError> {
        Ok(())
    }
    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(serde_json::json!({"counter": 7, "note": "ok"}))
    }
}

pub(super) static STATE_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[&StateModFactory];

pub(super) fn ctx_with_state_mod(enabled: bool) -> ServerContext {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let mut registry = crate::mod_registry::ModRegistry::new(
        STATE_FACTORIES,
        &serde_json::json!({"mods": {"stateful": {"enabled": enabled}}}),
    );
    registry.start_all();
    *ctx.mod_registry.lock().unwrap() = registry;
    ctx
}

// ---------------------------------- 0.4.4 rc.1：导演两键旁路夹具

/// 导演两键旁路的上下文：**空工厂** registry + 临时 `live2d-ai.toml` 路径
/// （`status_ctx.config_path` 指向临时目录）+ 手写 `mods.json` + 真 supervisor。
///
/// 返回 `(ctx, supervisor, 临时目录, mods.json 路径)`：测试结束自行
/// `remove_dir_all` / `drop(supervisor)`，**绝不碰仓库根** `mods.json` /
/// `live2d-ai.toml`。
#[allow(clippy::type_complexity)]
pub(super) fn director_ctx(
    tag: &str,
    manifest: serde_json::Value,
) -> (
    ServerContext,
    std::sync::Arc<crate::supervisor::SupervisorHandle>,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let dir = std::env::temp_dir().join(format!(
        "l2d-director-config-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let toml = dir.join("live2d-ai.toml");
    std::fs::write(
        &toml,
        live2d_ai_runtime::AppSettings::default().to_toml_string(),
    )
    .expect("write toml");
    let mods = dir.join("mods.json");
    std::fs::write(&mods, manifest.to_string()).expect("write manifest");
    let config_path = toml.display().to_string();
    // 真 supervisor：reload_pending 的「是否触发 / 是否消费」只有真句柄能验。
    let supervisor = std::sync::Arc::new(crate::supervisor::spawn_supervisor(
        crate::supervisor::SupervisorConfig {
            client: live2d_ai_runtime::OpenAiClient::new(
                live2d_ai_runtime::LlmConfig::new("http://127.0.0.1:1/v1", "m"),
                live2d_ai_runtime::TtsConfig::new("http://127.0.0.1:2/v1", "alloy"),
            )
            .expect("client"),
            conversation: live2d_ai_runtime::ConversationConfig::new("人设"),
            capabilities: live2d_ai_core::ModelCapabilities::all(),
            audio: None,
            config_path: Some(config_path.clone()),
            mod_events: None,
        },
        |_ev| {},
    ));
    let mut ctx = dummy_ctx(SecurityContext::new(18099, true));
    ctx.status_ctx = std::sync::Arc::new(crate::web_api::app_routes::StatusContext::new(
        live2d_ai_runtime::AppSettings::default(),
        config_path,
        None,
    ));
    let ctx = ctx.with_supervisor(std::sync::Arc::clone(&supervisor));
    (ctx, supervisor, dir, mods)
}

// ------------------------------------ 0.3.11：启动器「进入」夹具

/// 替身：id 就是 local-tts-melo（启动器「拉起类」id 之一）。
///
/// 真 Mod 的 start() 会 spawn 外部进程；这里只把调用次数记进 MELO_STARTS——
/// 「预启动关着点进入不得生进程」这条判据就靠它。
pub(super) static MELO_STARTS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub(super) struct MeloLikeFactory;

impl live2d_ai_mod_system::ModFactory for MeloLikeFactory {
    fn descriptor(&self) -> &'static live2d_ai_mod_system::ModDescriptor {
        static D: live2d_ai_mod_system::ModDescriptor = live2d_ai_mod_system::ModDescriptor {
            id: "local-tts-melo",
            name: "MeloLike",
            version: "0.1.0",
            api_version: 1,
        };
        &D
    }
    fn create(
        &self,
        _: live2d_ai_mod_system::ModServices,
        _: serde_json::Value,
    ) -> Result<Box<dyn live2d_ai_mod_system::ModRuntime>, live2d_ai_mod_system::ModError> {
        Ok(Box::new(MeloLikeRuntime))
    }
}

pub(super) struct MeloLikeRuntime;

impl live2d_ai_mod_system::ModRuntime for MeloLikeRuntime {
    fn start(
        &mut self,
        _: &mut dyn live2d_ai_mod_system::ModRegistrar,
    ) -> Result<(), live2d_ai_mod_system::ModError> {
        MELO_STARTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

pub(super) static LAUNCH_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[&MeloLikeFactory];

/// 带「拉起类」替身 + 临时 manifest 的 ServerContext。
///
/// manifest 路径故意不设（ModRegistry::new 缺省 None）：enable / disable 的
/// persist_manifest() 是 no-op，绝不读写仓库根那份 mods.json
/// （执行单 §3：单测用临时 manifest）。
pub(super) fn ctx_with_launch_mod(manifest: serde_json::Value) -> ServerContext {
    let sec = SecurityContext::new(18099, true);
    let ctx = dummy_ctx(sec);
    let mut registry = crate::mod_registry::ModRegistry::new(LAUNCH_FACTORIES, &manifest);
    registry.start_all();
    *ctx.mod_registry.lock().unwrap() = registry;
    ctx
}
