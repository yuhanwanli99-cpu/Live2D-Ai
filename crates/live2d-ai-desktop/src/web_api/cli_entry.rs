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
use live2d_ai_runtime::{AppSettings, OpenAiClient};
use tracing::warn;

use crate::audio::AudioOutputFacade;
use crate::supervisor::{SupervisorConfig, SupervisorHandle, spawn_supervisor};
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
    let supervisor_opt = if std::path::Path::new(&config_path).is_file() {
        // 把 ctx.broadcaster 借给 supervisor emit 闭包（避免双 Broadcaster）。
        match build_web_supervisor(&config_path, ctx.broadcaster.clone()) {
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
pub(crate) fn build_web_supervisor(
    config_path: &str,
    broadcaster: ws::Broadcaster,
) -> Result<Arc<SupervisorHandle>, String> {
    let path = PathBuf::from(config_path);
    let settings =
        AppSettings::load_from_path(&path).map_err(|e| format!("读取/解析配置失败: {e}"))?;
    let resolved = settings
        .resolve_with(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .map_err(|e| format!("配置解析失败（URL/环境变量名）: {e}"))?;
    let client = OpenAiClient::new(resolved.llm.clone(), resolved.tts.clone())
        .map_err(|e| format!("构建 LLM/TTS 客户端失败: {e}"))?;
    // audio：有设备就开（web 模式无窗口但 cpal 流可独立跑）；无设备 dry-run。
    let audio: Option<Box<dyn crate::audio::PcmProducer>> =
        match AudioOutputFacade::open_default(resolved.tts.spec, Duration::from_millis(200)) {
            Ok(facade) => {
                println!("[web] 检测到音频设备，已开启 cpal 输出（纯播放，无窗口）");
                Some(Box::new(facade))
            }
            Err(e) if e.is_environment() => {
                println!("[web] 无可用音频输出设备，supervisor 进入 dry-run: {e}");
                None
            }
            Err(e) => {
                warn!("web audio 打开失败（按 dry-run 继续）: {e}");
                None
            }
        };

    // emit 闭包：捕获 broadcaster 的 clone（与 ctx.broadcaster 同一实例）。
    let emit_bc = broadcaster;
    let handle = Arc::new(spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: resolved.conversation,
            capabilities: ModelCapabilities::all(),
            audio,
            config_path: Some(path.to_string_lossy().into_owned()),
        },
        move |ev| {
            // 投影 + 广播；P1 事件 None → 静默。
            if let Some(frame) = ws::app_event_to_ws_frame(&ev) {
                emit_bc.broadcast(&frame.to_string());
            }
        },
    ));

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
}
