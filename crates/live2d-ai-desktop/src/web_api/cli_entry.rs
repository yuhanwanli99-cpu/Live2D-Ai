//! Web API 控制平面的 CLI 入口（`--web [--http-port P]`，D2 接线，2026-08-28）。
//!
//! 拆分承载：`main.rs` 加 web 模式装配后逼近 500 行上限；本文件隔离
//! 「启动 server + 配置解析 + 退出码映射」逻辑，便于单测（不进 main 进程）。
//!
//! 关键不变量：
//! - **无 LLM/TTS 配置也可正常启动**（`AppSettings::default()`，端点返回
//!   `configured=false`、`has_api_key=false`，不退出码 1）；
//! - 监听仅 loopback（P0-3：127.0.0.1，不绑 0.0.0.0/::）；
//! - server 启动失败（端口占用 / 权限不足）→ 退出码 3（环境错误）。
//! - 配置文件存在 + 可解析 → 装配 supervisor（W2 任务：与 chat 模式共用
//!   装配逻辑）。**WS 广播器与 `ServerContext.broadcaster` 是同一实例**——
//!   `supervisor.emit` 闭包内捕获的 bc 与 WS 客户端订阅的 ctx.broadcaster
//!   **必须**是同一 Broadcaster，否则 WS 客户端永远收不到 AppEvent。
//! - 配置文件不存在 = 当作空配置（`AppSettings::default()`），端点正常响应。
//!   这与 `--chat` 不同（chat 启动时若配置不存在**会**报错）——web 模式是
//!   控制平面，缺失配置时前端的 `app/status` 自然标 `configured=false`。

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use live2d_ai_core::ModelCapabilities;
use live2d_ai_mod_system::ModEventTopic;
use live2d_ai_runtime::{AppSettings, OpenAiClient};
use tracing::warn;

use crate::audio::AudioOutputFacade;
use crate::mod_registry::ModRegistry;
use crate::supervisor::{
    AudioCb, ModEventSink, SupervisorConfig, SupervisorHandle, spawn_supervisor,
    spawn_supervisor_with_audio,
};
use crate::web_api::ws;
use crate::web_api::{ServerContext, start_server};

/// 退出码：与 `main.rs` 契约保持一致（0 成功 / 1 错误 / 3 环境）。
pub const EXIT_OK: u8 = 0;
pub const EXIT_ERROR: u8 = 1;
pub const EXIT_ENVIRONMENT: u8 = 3;

/// Web 模式入口（返回 `u8` 退出码，便于 main.rs 直接喂给 `ExitCode::from`）。
///
/// **`dev_mode_cli`**：W7 任务 CLI 覆盖。`true` 时优先级最高（覆盖
/// `live2d-ai.toml` 的 `dev_mode` 字段）；`false` 时由 settings 文件决定
/// （settings 缺省 = `AppSettings::default().dev_mode = false`）。
pub fn run_web_mode(port: u16, dev_mode_cli: bool) -> u8 {
    println!("web: 启动 HTTP 控制平面，监听 127.0.0.1:{port}（loopback；D1 §2 P0-3）");

    // 配置文件路径：复用 supervisor 装配时的逻辑（cwd 查找，缺失则默认）。
    let config_path = config_path_for_web();
    println!("web: 配置文件路径 {config_path}");

    // 读取配置（缺失 = 默认）。
    let settings = if std::path::Path::new(&config_path).is_file() {
        match AppSettings::load_from_path(&config_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[error] 加载配置失败: {e}");
                return EXIT_ERROR;
            }
        }
    } else {
        println!("web: 配置文件不存在，使用 AppSettings::default()（端点会标 configured=false）");
        AppSettings::default()
    };

    // W7：dev_mode 优先级 = CLI flag > settings 文件 > 默认 false。
    // 用 `Option<bool>` 透传到 `StatusContext::new`：CLI 为 true 时强制覆盖；
    // CLI 为 false 时仍走 settings 派生（保留 PATCH 切 false 的能力）。
    let dev_mode_override = if dev_mode_cli { Some(true) } else { None };
    let effective_dev_mode = dev_mode_override.unwrap_or(settings.dev_mode);
    if effective_dev_mode {
        println!("web: dev_mode=ON（日志端点 /api/v1/logs* 已开放）");
    } else if dev_mode_cli {
        // CLI 显式 false → 与 settings 派生保持一致；此处仅说明行为。
        println!("web: dev_mode=OFF（CLI 未覆盖，走 settings）");
    } else {
        println!("web: dev_mode=OFF（默认关闭；可用 PATCH /api/v1/settings 打开）");
    }

    // 1) 构造 ServerContext（内部自带空 Broadcaster；后续若装配 supervisor，
    //    会**替换**为 emit 闭包用的同一实例——保证 WS 客户端与 supervisor
    //    共享订阅者集合）。
    let mut ctx = ServerContext::new(settings, config_path.clone(), dev_mode_override);

    // 2) 装配 supervisor（仅当配置文件存在且可解析）。
    //    **先于 ModRegistry 启动**：registry 需把 supervisor 作为 HostChannels
    //    回路注入；supervisor 持 Arc 副本供 HostChannels 闭包捕获。
    //
    //    **Mod 事件桥（d2）**：supervisor 在 turn 生命周期点 emit `ModEventTopic`，
    //    但 registry 在此刻尚未装配（下步才建）。用晚绑定 holder
    //    `mod_bridge: Arc<Mutex<Option<dyn Fn>>>`——supervisor 捕获该 holder 的
    //    转发闭包（此时为 no-op/待注入），registry 建好后（步骤 3 末尾）把真实
    //    路由到 `dispatch_event` 的闭包**安装**进去。动态装配路径
    //    （`supervisor_slot::ensure_after_patch`）另走直接捕获 `ctx.mod_registry`。
    let mod_bridge: Arc<std::sync::Mutex<Option<ModEventSink>>> =
        Arc::new(std::sync::Mutex::new(None));
    let mod_events_for_supervisor: Option<ModEventSink> = {
        let bridge = mod_bridge.clone();
        Some(Arc::new(move |t: ModEventTopic, p: &str| {
            if let Some(f) = bridge.lock().ok().and_then(|g| g.clone()) {
                f(t, p);
            }
        }))
    };
    let supervisor_opt = if std::path::Path::new(&config_path).is_file() {
        // 把 ctx.broadcaster 借给 supervisor emit 闭包（避免双 Broadcaster）。
        match build_web_supervisor(
            &config_path,
            ctx.broadcaster.clone(),
            mod_events_for_supervisor,
        ) {
            Ok(handle) => {
                println!("web: supervisor 已启动（详见日志）");
                // P1WS-1：先**包成 Arc** 再注入到 StatusContext（`Arc` 是
                // SupervisorHandle 内部已持有的等价物；这里 clone Arc 即可
                // 让读源闭包独立于 ctx 生命周期）。然后再 `with_supervisor`
                // 把同一 Arc 放到 `supervisor_slot`（D-P0C 槽位）；退出时
                // `take_supervisor_for_reclaim` 取回。
                let handle_for_epoch = std::sync::Arc::clone(&handle);
                ctx.status_ctx
                    .set_epoch_source(Box::new(move || handle_for_epoch.current_epoch()));
                ctx = ctx.with_supervisor(handle);
                Some(Arc::clone(
                    ctx.try_get_supervisor().as_ref().expect("just set"),
                ))
            }
            Err(e) => {
                // 配置不完整 / base_url 缺失 / 解析失败 → 不阻断 web 启动，
                // 但 `/api/v1/chat` 会回 503。错误可见给运维。
                eprintln!("[warn] web supervisor 装配失败，chat 端点将回 503: {e}");
                warn!(error = %e, "web supervisor 装配失败");
                None
            }
        }
    } else {
        println!("web: 无配置文件，跳过 supervisor 装配（纯控制平面）");
        None
    };

    // 3) 装配 ModRegistry（节点 E5，2026-08-30）：用静态编译的
    //    AVAILABLE_MOD_FACTORIES + manifest 配置；start_all() 启动 enabled Mod。
    //    若 supervisor 已就绪，注入 HostChannels（Mod→host 真实回路）：
    //    say → supervisor.say；Mod 探测就绪的事件 → settings 写盘 + reload。
    //    **动作不在其中**（2026-09-12，rc.2）：`ActionRequest → core` 那条线已整体
    //    删除，动作在产品路径上不存在（core-chain-baseline.md §3.3）。
    let registry = match &supervisor_opt {
        Some(supervisor) => {
            let sup_for_say = Arc::clone(supervisor);
            let sup_for_reload = Arc::clone(supervisor);
            let cfg_path_for_settings = config_path.clone();
            ModRegistry::new(crate::AVAILABLE_MOD_FACTORIES, &mods_manifest_for_web())
                .with_host_channels(crate::mod_registry::HostChannels {
                    say: Arc::new(move |t| sup_for_say.say(t)),
                    // P0-4 真实闭环：Mod 探测就绪后通过 event_tx 发送
                    // `{"__apply_settings":true,"patch":{...}}`，这里复用
                    // settings_routes 的 PATCH 内核完成写盘 + supervisor.reload。
                    apply_settings: Arc::new(move |patch_json: serde_json::Value| {
                        use live2d_ai_runtime::settings::patch::SettingsPatch;
                        use live2d_ai_runtime::AppSettings;
                        // patch_json 是 namespaced JSON（如 {"llm":{"base_url":...}}），
                        // 反序列化为 SettingsPatch 三态结构。
                        let patch: SettingsPatch =
                            match serde_json::from_value(patch_json) {
                                Ok(p) => p,
                                Err(e) => {
                                    tracing::error!(target: "mod", "apply_settings patch 反序列化失败: {e}");
                                    return false;
                                }
                            };
                        // 读取当前 settings，应用 patch，写盘。
                        let current = match AppSettings::load_from_path(&cfg_path_for_settings) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!(target: "mod", "apply_settings 读取当前配置失败: {e}");
                                return false;
                            }
                        };
                        let (next, outcome) = match live2d_ai_runtime::settings::patch::apply_patch(&current, &patch) {
                            Ok(r) => r,
                            Err(e) => {
                                tracing::error!(target: "mod", "apply_settings apply_patch 失败: {e}");
                                return false;
                            }
                        };
                        // 只有真正变更才写盘。
                        if !matches!(outcome, live2d_ai_runtime::settings::patch::PatchOutcome::Updated) {
                            tracing::info!(target: "mod", "apply_settings 无变更，跳过写盘");
                            // 即使无变更，也触发 reload 让 supervisor 重读（探活确认）。
                            sup_for_reload.reload();
                            return true;
                        }
                        // **合并写回**：保留现有配置的注释与排版。
                        let toml_text = next
                            .to_toml_string_merging(std::path::Path::new(
                                &cfg_path_for_settings,
                            ));
                        let target = std::path::Path::new(&cfg_path_for_settings);
                        if let Some(parent) = target.parent()
                            && !parent.as_os_str().is_empty()
                        && let Err(e) = std::fs::create_dir_all(parent)
                        {
                            tracing::error!(target: "mod", "apply_settings create_dir_all 失败: {e}");
                            return false;
                        }
                        let tmp = match live2d_ai_runtime::settings::patch::plan_atomic_write(target, &toml_text) {
                            Ok(t) => t,
                            Err(e) => {
                                tracing::error!(target: "mod", "apply_settings plan_atomic_write 失败: {e}");
                                return false;
                            }
                        };
                        if let Err(e) = std::fs::rename(&tmp, target) {
                            let _ = std::fs::remove_file(&tmp);
                            tracing::error!(target: "mod", "apply_settings rename 失败: {e}");
                            return false;
                        }
                        tracing::info!(target: "mod", "apply_settings 写盘成功，触发 supervisor.reload()");
                        sup_for_reload.reload();
                        true
                    }),
                    config_path: config_path.clone(),
                })
        }
        None => {
            println!(
                "web: 无 supervisor，ModRegistry 以 no-op HostChannels 启动（Mod action 会被丢弃）"
            );
            ModRegistry::new(crate::AVAILABLE_MOD_FACTORIES, &mods_manifest_for_web())
        }
    };
    let mut registry = registry;
    registry.start_all();
    println!(
        "web: ModRegistry 已装配 + 启动（注册 {} 个 Mod）",
        crate::AVAILABLE_MOD_FACTORIES.len()
    );
    ctx = ctx.with_mod_registry(registry);

    // **Mod 事件桥（d2）**：registry 已装配（`ctx.mod_registry`），把真实路由
    // 到 `dispatch_event` 的闭包安装进晚绑定 holder——supervisor 从此 emit 的
    // `ModEventTopic` 都会经 `dispatch_event`（有界 channel，失败隔离）投递到
    // Mod worker。只安装一次（startup 装配）；无 supervisor 时 skip。
    if supervisor_opt.is_some() {
        let reg = ctx.mod_registry.clone();
        if let Ok(mut guard) = mod_bridge.lock() {
            *guard = Some(Arc::new(move |t: ModEventTopic, p: &str| {
                if let Ok(reg) = reg.lock() {
                    let _ = reg.dispatch_event(t, p);
                }
            }));
        }
    }

    // 配置文件热重载文件监听器（2026-08-31）：监听 `live2d-ai.toml` 外部修改，
    // 检测到变更后触发 supervisor reload。仅在 supervisor 已装配时启用。
    //
    // 2026-09-11：reload 之后**还要刷新设置快照**（`post_reload`）。只 reload
    // client 会让「界面显示的值」与「磁盘上的值」静默分叉——用户手改提示词后
    // 界面上看到的还是旧值，而且下一次界面「保存」会把旧快照整份写回、
    // 把手改的内容覆盖掉（实测复现）。
    let _file_watcher = if let Some(sup) = &supervisor_opt {
        let status_for_watch = ctx.status_ctx.clone();
        let path_for_watch = config_path.clone();
        match crate::web_api::file_watcher::FileWatcher::watch_config(
            &config_path,
            Arc::clone(sup),
            500, // 500ms 防抖
            Some(Box::new(move || {
                if status_for_watch.refresh_from_disk(&path_for_watch) {
                    tracing::info!(path = %path_for_watch, "外部修改已同步进设置快照");
                }
            })),
        ) {
            Ok(w) => {
                println!("web: 配置文件热重载监听已启用（{}）", config_path);
                Some(w)
            }
            Err(e) => {
                eprintln!("[warn] 配置文件热重载监听启用失败: {e}");
                None
            }
        }
    } else {
        None
    };

    match start_server(ctx.clone(), port) {
        Ok(addr) => {
            println!("web: server 退出（{addr:?}）");
            // 退出后回收 supervisor（**D-P0C 优先**用 `take_supervisor_for_reclaim`
            // 拿回槽位；外部 `supervisor_opt` 仍持 Arc（cli_entry 局部变量）作
            // 兜底——若槽位已被别处 take 走（如未来动态替换路径），走兜底）。
            if let Some(slot_handle) = ctx.take_supervisor_for_reclaim() {
                reclaim_supervisor(slot_handle);
                println!("web: supervisor 已 join（来自 slot）");
            } else if let Some(sup) = supervisor_opt {
                reclaim_supervisor(sup);
                println!("web: supervisor 已 join（来自局部）");
            }
            EXIT_OK
        }
        Err(e) => {
            eprintln!("[environment] 启动 HTTP server 失败: {e}");
            eprintln!("[environment] 提示：检查端口 {port} 是否被占用 / 当前用户是否有监听权限。");
            EXIT_ENVIRONMENT
        }
    }
}

/// 装配 web 模式 supervisor（与 `backend/chat.rs` 同源的最小子集）。
///
/// `broadcaster` 必须**与 `ServerContext.broadcaster` 是同一实例**——emit
/// 闭包内捕获它的 clone；WS 客户端订阅的也是它。`audio` 默认 dry-run（web
/// 模式无 winit 事件循环；纯播放意义不大），有设备时仍可启用 cpal 流。
///
/// D-P0C：标记 `pub(crate)` 让 `mod.rs::ServerContext::ensure_supervisor_after_patch`
/// 在 PATCH 200 路径上复用本函数（首次配置闭环：从无配置启动 → PATCH 保存
/// → 动态装配 → 立即可对话）。返回 `Result<Arc<SupervisorHandle>, String>`
/// 失败时 caller 决定如何回报（dispatch → `RestartRequired`，cli_entry 启动
/// 路径 → eprintln + 槽位保持空）。
///
/// `mod_events`：host→Mod 事件桥回调（`SupervisorConfig.mod_events`）。调用方
/// 需传入能在 ModRegistry 装配后路由到 `dispatch_event` 的闭包；启动路径用
/// 晚绑定 holder（registry 后建），动态路径直接捕获 `ctx.mod_registry`。
pub(crate) fn build_web_supervisor(
    config_path: &str,
    broadcaster: ws::Broadcaster,
    mod_events: Option<ModEventSink>,
) -> Result<Arc<SupervisorHandle>, String> {
    let path = PathBuf::from(config_path);
    let settings =
        AppSettings::load_from_path(&path).map_err(|e| format!("读取/解析配置失败: {e}"))?;
    let resolved = settings
        .resolve_with(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .map_err(|e| format!("配置解析失败（URL/环境变量名）: {e}"))?;
    let client = OpenAiClient::new(resolved.llm.clone(), resolved.tts.clone())
        .map_err(|e| format!("构建 LLM/TTS 客户端失败: {e}"))?;
    // **音频单源（2026-09-10 点火轮）**：默认「仅浏览器出声」——服务端不开 cpal，
    // PCM 经 WS 广播给 Flutter Web（浏览器播放 + 浏览器侧真实 RMS 驱动口型）。
    // `LIVE2D_AI_WEB_AUDIO=server` 退回旧行为（服务端 cpal 播放；届时默认关 WS 音频）。
    let server_audio = std::env::var("LIVE2D_AI_WEB_AUDIO").as_deref() == Ok("server");
    let audio: Option<Box<dyn crate::audio::PcmProducer>> = if server_audio {
        match AudioOutputFacade::open_default(resolved.tts.spec, Duration::from_millis(200)) {
            Ok(facade) => {
                println!("[web] LIVE2D_AI_WEB_AUDIO=server —— 服务端 cpal 输出已开启");
                Some(Box::new(facade))
            }
            Err(e) => {
                warn!("web audio 打开失败（按 dry-run 继续）: {e}");
                None
            }
        }
    } else {
        println!(
            "[web] 音频单源=browser —— 服务端不开 cpal；PCM 经 WS 交给浏览器播放，口型由浏览器 RMS 驱动"
        );
        None
    };

    // emit 闭包：捕获 broadcaster 的 clone（与 ctx.broadcaster 同一实例）。
    let emit_bc = broadcaster;
    // **输出静音（2026-09-10 用户裁决）**：**默认出声**。
    //
    // 静音仍然在**服务端**强制（而不是客户端）：客户端静音绕不过 Service Worker
    // 缓存的旧前端、遗留的 JS 前端 `/`、以及任何第三方客户端。所以它保留为一个
    // 显式开关，而不是被删掉：
    //   - 都不设（默认）：出声。
    //   - `LIVE2D_AI_MUTE_AUDIO=1`：强制静音——**跑测试 / 无人值守时用**，
    //     保证任何客户端都发不出声音，不打扰用户。
    //   - `LIVE2D_AI_UNMUTE_AUDIO=1`：显式出声。保留的旧开关，**优先级最高**，
    //     免得旧脚本/旧文档因为默认值翻转而反直觉地变成静音。
    let force_unmute = std::env::var("LIVE2D_AI_UNMUTE_AUDIO").as_deref() == Ok("1");
    let force_mute = std::env::var("LIVE2D_AI_MUTE_AUDIO").as_deref() == Ok("1");
    if force_unmute && force_mute {
        println!("web: 同时设置了 LIVE2D_AI_UNMUTE_AUDIO 与 LIVE2D_AI_MUTE_AUDIO，按出声处理");
    }
    emit_bc.set_muted(ws::resolve_muted(force_unmute, force_mute));
    // 回读实际生效值（而不是复用入参）：确认 set/get 这条链路是通的。
    if emit_bc.is_muted() {
        println!(
            "web: 输出静音=ON（LIVE2D_AI_MUTE_AUDIO=1：服务端下发零 PCM + 真实 volume）\
             ——不出声，口型照常"
        );
    } else {
        println!(
            "web: 输出静音=OFF——默认出声（浏览器播放）；\
             跑测试/静默运行请设 LIVE2D_AI_MUTE_AUDIO=1"
        );
    }
    let handle = Arc::new({
        // WS 音频广播（F6-T2）：browser 模式下它是出声的**唯一路径**，默认开；
        // server 模式下需显式 `LIVE2D_AI_WS_AUDIO=1` 才额外广播（旧行为，二者都响）。
        let ws_audio_on =
            !server_audio || std::env::var("LIVE2D_AI_WS_AUDIO").as_deref() == Ok("1");
        if ws_audio_on {
            println!("web: WS 音频广播已开启（browser 播放 / 口型驱动）");
            let bc = emit_bc.clone();
            let audio_cb: Arc<AudioCb> = Arc::new(
                move |epoch: u64,
                      samples: &[f32],
                      spec: live2d_ai_runtime::AudioSpec,
                      sentence_seq: u64,
                      first_chunk: bool,
                      final_chunk: bool| {
                    // f32 ∈ [-1,1] → s16le 字节（crate::audio::f32_to_i16），
                    // 交给 ws::Broadcaster::broadcast_audio_frames（内部切片 + base64）。
                    let i16s: Vec<i16> = samples
                        .iter()
                        .map(|&s| crate::audio::f32_to_i16(s))
                        .collect();
                    let bytes: Vec<u8> = i16s.iter().flat_map(|v| v.to_le_bytes()).collect();
                    // 句子边界一并下传：WS 的 start/end 要按句给（不是按 epoch）。
                    bc.broadcast_audio_frames(
                        epoch,
                        &bytes,
                        spec.sample_rate(),
                        sentence_seq,
                        first_chunk,
                        final_chunk,
                    );
                },
            );
            spawn_supervisor_with_audio(
                SupervisorConfig {
                    client,
                    conversation: resolved.conversation,
                    capabilities: ModelCapabilities::all(),
                    audio,
                    config_path: Some(path.to_string_lossy().into_owned()),
                    mod_events: mod_events.clone(),
                },
                move |ev| {
                    // 投影 + 广播；P1 事件 None → 静默。
                    if let Some(frame) = ws::app_event_to_ws_frame(&ev) {
                        emit_bc.broadcast(&frame.to_string());
                    }
                },
                audio_cb,
            )
        } else {
            spawn_supervisor(
                SupervisorConfig {
                    client,
                    conversation: resolved.conversation,
                    capabilities: ModelCapabilities::all(),
                    audio,
                    config_path: Some(path.to_string_lossy().into_owned()),
                    mod_events: mod_events.clone(),
                },
                move |ev| {
                    // 投影 + 广播；P1 事件 None → 静默。
                    if let Some(frame) = ws::app_event_to_ws_frame(&ev) {
                        emit_bc.broadcast(&frame.to_string());
                    }
                },
            )
        }
    });

    Ok(handle)
}

/// 解析 web 模式下的配置文件路径（cwd 优先；找不到回退到 XDG 默认）。
///
/// 与 `--chat` 装配的逻辑同源——但 web 模式下**不**要求文件存在（缺则
/// `AppSettings::default()`），仅打印提示。
pub fn config_path_for_web() -> String {
    let cwd_candidate = std::path::Path::new("live2d-ai.toml");
    if cwd_candidate.is_file() {
        return cwd_candidate.display().to_string();
    }
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = std::path::PathBuf::from(home);
        p.push(".config");
        p.push("live2d-ai");
        p.push("live2d-ai.toml");
        return p.display().to_string();
    }
    "live2d-ai.toml".to_string()
}

/// Mod manifest：`<config_dir>/mods.json`；**不存在时回落到内建缺省**。
///
/// 格式：`{"mods":{"<id>":{"enabled":bool,"config":{...}}}}`
///
/// - 路径 = `config_path_for_web()` 所在目录的 `mods.json`
///   （即 `live2d-ai.toml` 同目录；web 模式下 config_path 可能来自 cwd
///   或 `~/.config/live2d-ai/`）。
/// - 文件存在且可解析 → **完全覆盖**内建缺省。
/// - 文件不存在 → 用 [`default_mods_manifest`]（2026-09-10 M1 修复）。
fn mods_manifest_for_web() -> serde_json::Value {
    let dir = std::path::Path::new(&config_path_for_web())
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let path = dir.join("mods.json");
    if let Some(v) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        return v;
    }
    default_mods_manifest()
}

/// 内建缺省 Mod manifest（`mods.json` 缺失时使用；M1 修复）。
///
/// 启用「OpenAI 兼容端点适配器」Mod，且 `externally_managed=true`：
/// **只探活 + 写 `base_url`，绝不 spawn 子进程**。用户自带的 LLM
/// （缺省 `127.0.0.1:11434`）一旦就绪，即自动写入 settings 并
/// `supervisor.reload()`。探测不到只是「未就绪」日志（15s 超时），不影响主链路。
///
/// # 为什么缺省里**没有** TTS（2026-09-11 用户裁决）
///
/// 语音合成是**核心链路**（LLM → TTS → 口型），不是可选扩展。
/// 它的端点唯一权威来源是 `live2d-ai.toml` 的 `[tts]` 段
/// （`base_url` / `voice` / `response_format` / `sample_rate` / `channels`），
/// 由 `live2d-ai-runtime` 直接读取并调用——**没有 Mod 夹在中间**。
/// 详细论证见 `docs/architecture/tts-is-core.md`。
fn default_mods_manifest() -> serde_json::Value {
    serde_json::json!({
        "mods": {
            "local-llm": {
                "enabled": true,
                "config": { "externally_managed": true, "port": 11434 }
            }
        }
    })
}

/// 回收 supervisor 线程：server 退出后取回所有权并 join。
///
/// server 期间 ServerContext 持 Arc clone；server 退出后 ServerContext 被
/// drop（除非外部再持有），所以 `try_unwrap` 通常能成功。
fn reclaim_supervisor(handle: Arc<SupervisorHandle>) {
    match Arc::try_unwrap(handle) {
        Ok(h) => {
            h.quit();
            h.join();
        }
        Err(_) => {
            // ServerContext 仍持 Arc（用户 Ctrl-C 时 server 可能未完全 drop）。
            // 退路：直接 quit()；进程退出时由 OS 回收线程。
            warn!("reclaim_supervisor: Arc 多 owner（ServerContext 仍存活），仅 quit 不 join");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_falls_back_when_no_file() {
        let p = config_path_for_web();
        assert!(!p.is_empty());
    }

    #[test]
    fn exit_codes_match_main_contract() {
        assert_eq!(EXIT_OK, 0);
        assert_eq!(EXIT_ERROR, 1);
        assert_eq!(EXIT_ENVIRONMENT, 3);
    }

    /// M1（2026-09-10）：无 `mods.json` 时内建缺省启用端点适配器 Mod，
    /// 且为 `externally_managed=true`（只探活、不 spawn）。
    ///
    /// 2026-09-11：**TTS 已从 Mod 系统移出**（语音合成是核心链路，不是扩展），
    /// 所以缺省里只剩 `local-llm`。这条断言同时守住「别再把它加回来」。
    #[test]
    fn default_mods_manifest_enables_local_llm_only() {
        let m = default_mods_manifest();
        assert_eq!(
            m["mods"]["local-llm"]["enabled"], true,
            "local-llm 应默认启用"
        );
        assert_eq!(
            m["mods"]["local-llm"]["config"]["externally_managed"], true,
            "local-llm 应默认外部管理（不 spawn）"
        );
        assert_eq!(m["mods"]["local-llm"]["config"]["port"], 11434);
        assert!(
            m["mods"].get("local-tts").is_none(),
            "TTS 是核心链路（live2d-ai.toml 的 [tts]），不该再以 Mod 形式出现"
        );
    }
}
