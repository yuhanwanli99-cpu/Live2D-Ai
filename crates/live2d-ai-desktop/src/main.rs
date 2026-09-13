//! `live2d-ai-desktop` — PC 桌面应用（Rust 重建）。
//!
//! 本批为 **Linux 桌宠窗口能力 + Bai 实时渲染**：
//! - winit 0.30 透明、无边框、可调整大小窗口 + wgpu 29 surface 连续重绘
//!   （[`app`]，经 [`backend`] 接口暴露）；事件循环为 **UserEvent** 型
//!   （[`user_event::PetUserEvent`] + EventLoopProxy）；
//! - **桌宠窗口能力**（[`app`] + [`platform`]）：RawWindowHandle 实测后端分类
//!   （Xlib/Xcb→X11、Wayland→Wayland，非环境线索）；底部右侧初始定位 +
//!   回读验证；置顶（仅 X11 生效）；左键交互态 `drag_window`；
//!   `set_cursor_hittest` 动态切换点击穿透；`RuntimeCapabilities` 只在 API
//!   真实调用成功时记录；
//! - **ksni 托盘**（[`tray`]）：纯 Rust SNI（0.3.6 blocking），菜单至少含
//!   点击穿透/置顶/显隐/退出；回调只经 proxy 发 UserEvent，Window 调用全部
//!   留在事件循环线程；无 D-Bus/host/扩展时可见降级且不阻塞启动；
//!   pet-mode 自动穿透以托盘确认为前提（失败即禁用，防不可恢复）；
//! - **Bai 实时渲染冒烟**（[`model_smoke`] + [`adapter`]）：`--model-smoke`
//!   在窗口 bootstrap 的同一 device/queue 上经 `l2d::GpuContext::from_parts`
//!   接入，`LoadedModel::resolve(ModelPackage::load)` 统一加载（兼容报告逐条
//!   记录），`ModelRendererCore::new/load_model` 上屏；每帧真实 dt（60Hz 固定步
//!   累加器）推进，resize 同步 set_viewport，渲染到 surface view 后 present；
//!   六动作按固定顺序自动播放（每动作 Medium），同时用低频合成口型电平验证
//!   `ParamMouthOpenY` 通道——不访问 LLM/TTS/声卡；默认保持交互可关闭
//!   （不置顶不穿透），`--pet-mode` 才进入桌宠模式；
//! - Linux 会话线索与能力簿记（[`platform`]：`LinuxSessionHint` /
//!   `DeclaredCapabilities` / `WindowBackendKind` / `RuntimeCapabilities`——
//!   后者只记录真实初始化/API 调用结果，绝不凭 X11 线索推断）；
//! - 实际声卡输出链路（[`audio`]）：cpal 默认输出设备 + ringbuf 无锁 SPSC，
//!   TTS 域 PCM 经确定性重采样/声道映射入环，回调内欠载补 0 并对**实际写给
//!   声卡的 f32** 计算 RMS，经原子 f32-bits 快照暴露 mouth level；
//!   epoch 打断保证 stop 后旧音频不再继续；
//! - CLI（[`cli`]）：默认无参数打印架构与能力后退出（无显示 CI 安全，
//!   **不访问声卡**）；`--window-smoke` 纯透明窗口壳；`--model-smoke [model3]`
//!   Bai 实时渲染冒烟（模型/资产错误退出码 1、GPU 环境退出码 3）；
//!   `--pet-mode` 桌宠模式（默认置顶+穿透+托盘恢复入口）；`--smoke-frames N`
//!   帧数后自动退出；`--audio-smoke` 音频输出冒烟（无音频设备退出码 3）。
//!
//! - 网络（LLM/TTS HTTP）经 OpenAI 兼容协议接入：`--chat` 终端对话 /
//!   `--web` Web 面板（supervisor + WebSocket 事件流，见 [`web_api`]）。
//! - 许可：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准（见本 crate `README.md`）。

mod adapter;
mod app;
mod app_event;
mod audio;
mod backend;
mod benchmark;
mod cli;
mod logging;
mod mod_registry;
mod model_smoke;
mod platform;

/// 已编译进本二进制的 Mod 工厂（静态注册；启用与否由 manifest `[mods.<id>]` 决定）。
/// 由 `cli_entry::run_web_mode` 装配 `ModRegistry`（节点 E5，2026-08-30）。
///
/// 2026-09-12（rc.2）：director Mod 已删除——它唯一的职责是**驱动动作序列**，
/// 而动作在产品路径上不存在（见 `docs/architecture/core-chain-baseline.md` §3.3）。
/// 归档点在分支 `archive/action-layer-p6`。**不要再挂回去**：
/// 下方 `mod_count_is_four` 是防回归断言（rc.4 M5 起 +1 个角色卡 Mod）。
pub static AVAILABLE_MOD_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[
    &live2d_ai_mod_external_input::FACTORY,
    &live2d_ai_mod_pet_desktop::FACTORY,
    &live2d_ai_mod_local_llm::FACTORY,
    // rc.4 M5：角色卡标准 Mod（缺省停用；启用后把卡合成 system_prompt 写回主链）。
    &live2d_ai_mod_persona::FACTORY,
];

mod repl;
mod supervisor;
mod tray;
mod user_event;
mod web_api;

use std::process::ExitCode;

use backend::{BackendError, RunOptions};
use platform::{DeclaredCapabilities, LinuxSessionHint, RuntimeCapabilities};

/// 退出码约定：
/// - 0 成功；1 一般错误（代码缺陷/意外平台行为）；
/// - 2 CLI 用法错误；3 **运行环境不满足**（无显示服务/无可用 GPU/
///   无可用音频设备）——与代码错误显式区分，CI 可据此跳过而非报警。
const EXIT_OK: u8 = 0;
const EXIT_ERROR: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_ENVIRONMENT: u8 = 3;

fn main() -> ExitCode {
    // 日志：CLI 启动附带（任何模式：--chat/--web/--pet-mode 都落盘；
    // 不只开发者模式；W3 任务）。失败仅 warn，不退出码 1。
    if let Err(e) = logging::init_logging(None) {
        eprintln!("[warn] 日志系统初始化失败（已降级仅 stdout）: {e}");
        // 单 stdout fallback（与 W3 失败语义一致）。
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    }

    // 参数解析失败时区分 --help（用法输出到 stdout，退出码 0）。
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let command = match cli::parse_args(&argv) {
        Ok(command) => command,
        Err(e) if e.message == "__help__" => {
            println!("{}", cli::USAGE);
            return ExitCode::from(EXIT_OK);
        }
        Err(e) => {
            eprintln!("参数错误: {}\n\n{}", e.message, cli::USAGE);
            return ExitCode::from(EXIT_USAGE);
        }
    };

    print_info();

    match command {
        cli::Command::Info => {
            println!(
                "(未启动窗口/声卡：默认模式仅打印信息；--window-smoke / --model-smoke / \
                 --audio-smoke 进入冒烟)"
            );
            ExitCode::from(EXIT_OK)
        }
        cli::Command::WindowSmoke {
            frame_target,
            timeout,
            pet_mode,
        } => run_backend_smoke(
            RunOptions {
                initial_logical_size: (480.0, 640.0),
                frame_target,
                timeout,
                model_smoke: None,
                pet_mode,
            },
            "window-smoke 报告",
        ),
        cli::Command::ModelSmoke {
            model3,
            frame_target,
            timeout,
            pet_mode,
        } => {
            let model3_path = match cli::resolve_model3_path(model3.as_deref()) {
                Ok(path) => path,
                Err(msg) => {
                    // 缺省路径找不到/显式路径不存在：资产问题 → 退出码 1。
                    eprintln!("[error] {msg}");
                    return ExitCode::from(EXIT_ERROR);
                }
            };
            println!(
                "model-smoke: 皮套 {}（六动作固定顺序 Medium + 低频合成口型；\
                 不访问 LLM/TTS/声卡）",
                model3_path.display()
            );
            run_backend_smoke(
                RunOptions {
                    initial_logical_size: (480.0, 640.0),
                    // 无显式帧数时与窗口壳同语义：直到窗口关闭或超时兜底。
                    frame_target,
                    timeout,
                    model_smoke: Some(backend::ModelSmokeOptions { model3_path }),
                    pet_mode,
                },
                "model-smoke 报告",
            )
        }
        cli::Command::AudioSmoke { duration, silence } => run_audio_smoke(duration, silence),
        cli::Command::Web { port, dev_mode } => {
            ExitCode::from(web_api::cli_entry::run_web_mode(port, dev_mode))
        }
        cli::Command::Chat {
            config_path,
            pet_mode,
            smoke_timeout,
        } => match backend::run_chat_session(backend::ChatOptions {
            config_path,
            pet_mode,
            smoke_timeout,
        }) {
            Ok(report) => {
                for line in report.summarize_lines() {
                    println!("{line}");
                }
                ExitCode::from(EXIT_OK)
            }
            Err(e) => {
                eprintln!("[error] chat: {e}");
                // 环境类失败（无显示服务/无 adapter 等）→ 3；其余 → 1。
                ExitCode::from(match e {
                    backend::BackendError::Environment(_) => EXIT_ENVIRONMENT,
                    backend::BackendError::Failed(_) => EXIT_ERROR,
                })
            }
        },
        cli::Command::Benchmark {
            model3,
            mode,
            warmup,
            frames,
            headless,
            output_json,
        } => {
            let model3_path = match cli::resolve_model3_path(model3.as_deref()) {
                Ok(path) => path,
                Err(msg) => {
                    eprintln!("[error] {msg}");
                    return ExitCode::from(EXIT_ERROR);
                }
            };
            let opts = benchmark::BenchmarkOptions {
                model3_path,
                window_logical: (480.0, 640.0),
                warmup_frames: warmup,
                formal_frames: frames,
                mode: match mode {
                    cli::BenchmarkCliMode::Blocking => benchmark::BenchmarkMode::Blocking,
                    cli::BenchmarkCliMode::Submit => benchmark::BenchmarkMode::Submit,
                },
                backend: if headless {
                    benchmark::BenchmarkBackend::Headless
                } else {
                    benchmark::BenchmarkBackend::Surface
                },
                output_json,
            };
            match benchmark::run_benchmark(opts) {
                Ok(report) => {
                    for line in report.summarize_lines() {
                        println!("{line}");
                    }
                    ExitCode::from(EXIT_OK)
                }
                Err(msg) => {
                    eprintln!("[error] benchmark: {msg}");
                    ExitCode::from(EXIT_ENVIRONMENT)
                }
            }
        }
    }
}

/// 打印架构、会话线索与两层能力表（声明性期望 vs 实际运行时能力）。
fn print_info() {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    let family = std::env::consts::FAMILY;
    println!("live2d-ai-desktop {}", env!("CARGO_PKG_VERSION"));
    println!(
        "  phase  : desktop-pet window — winit+wgpu 透明无边框窗口壳、置顶/底部右侧定位/\
         点击穿透/交互态拖动（RawWindowHandle 实测 X11 vs Wayland）、ksni 托盘、\
         同一 device 上的 l2d ModelRendererCore 实时渲染与实际声卡输出链路已接入；\
         网络（LLM/TTS HTTP）经 OpenAI 兼容协议接入（--chat / --web）"
    );
    println!("  target : {arch}-{os} (family: {family})");
    println!("  rust   : MSRV {}", env!("CARGO_PKG_RUST_VERSION"));
    println!(
        "  backends: {}",
        backend::available_backends()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );

    // 会话线索：platform 模块保持纯函数，环境读取集中在边界层。
    let xdg_session_type = std::env::var("XDG_SESSION_TYPE").ok();
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
    let display = std::env::var("DISPLAY").ok();
    let hint = LinuxSessionHint::detect_from(
        xdg_session_type.as_deref(),
        wayland_display.as_deref(),
        display.as_deref(),
    );
    println!("  session-hint : {hint}（仅环境线索，非实际连接）");

    // 第一层：会话推导的声明性期望（不是实际能力 gate）。
    let declared = DeclaredCapabilities::for_session(hint);
    println!("  declared caps（hint 推导的期望，非 gate）:");
    for (name, expected) in declared.as_table() {
        println!("    {name:<18} = {expected}");
    }

    // 第二层：真实运行时能力 —— 此刻尚未创建窗口/surface，全部未探测。
    // tray/click_through/global_position/always_on_top 只有后续批次真正
    // 初始化后才可能变为 available（不允许凭 X11 全 true）。
    let runtime: RuntimeCapabilities = RuntimeCapabilities::not_probed();
    println!("  runtime caps（真实初始化结果）:");
    for (name, value) in runtime.as_table() {
        println!("    {name:<28} = {value}");
    }

    let reasons = hint.degradation_reasons();
    println!("  notes:");
    if reasons.is_empty() {
        println!("    - (当前会话无线索层面的已知限制)");
    } else {
        for reason in reasons {
            println!("    - {reason}");
        }
    }
}

/// 启动真实窗口（纯透明壳或模型实时渲染）并打印运行报告；按错误类别映射退出码。
fn run_backend_smoke(options: RunOptions, report_title: &'static str) -> ExitCode {
    let backend = match backend::create_backend(backend::BackendKind::WinitWgpu) {
        Ok(backend) => backend,
        // 当前工厂对唯一内置后端不会失败；防御式处理保持退出码契约。
        Err(e) => {
            eprintln!("[error] {e}");
            return ExitCode::from(EXIT_ERROR);
        }
    };

    // 运行前逐项声明后端能力代码路径（静态口径）；**实际能力**以运行结束后的
    // runtime caps 为准（RawWindowHandle 实测 + API 调用结果，
    // 见 RFC §4 批次 6 与 crate README「能力语义」）。
    let description = backend.describe();
    println!("backend: {} — {}", description.kind, description.title);
    for feature in [
        backend::FeatureRequest::AlwaysOnTop,
        backend::FeatureRequest::GlobalPosition,
        backend::FeatureRequest::ClickThrough,
        backend::FeatureRequest::DragMove,
        backend::FeatureRequest::Tray,
    ] {
        match backend.request_feature(feature) {
            Ok(()) => {
                println!("  feature {feature:<16}: 代码路径已实现（生效与否以 runtime 实测为准）")
            }
            Err(rejected) => {
                println!(
                    "  feature {:<16}: 无代码路径 —— {}",
                    rejected.feature, rejected.reason
                )
            }
        }
    }

    match backend.run(&options) {
        Ok(report) => {
            println!("{report_title}:");
            for line in report.summarize_lines() {
                println!("  {line}");
            }
            ExitCode::from(EXIT_OK)
        }
        Err(BackendError::Environment(msg)) => {
            // 环境不满足 ≠ 代码缺陷：独立退出码，便于 CI 区分
            // （无显示服务/无可用 GPU/模型渲染的 GPU 管线失败均在此类）。
            eprintln!("[environment] {msg}");
            eprintln!(
                "[environment] 提示：本机可能没有可用的显示服务/GPU；\
                       无显示 CI 请使用默认模式（不带 --window-smoke/--model-smoke）。"
            );
            ExitCode::from(EXIT_ENVIRONMENT)
        }
        Err(e @ BackendError::Failed(_)) => {
            eprintln!("[error] {e}");
            ExitCode::from(EXIT_ERROR)
        }
    }
}

/// 执行音频输出冒烟并打印运行报告；按错误类别映射退出码。
///
/// 无音频设备属环境不满足（退出码 3）；有设备时完整走
/// 「重采样/映射 → SPSC 环 → cpal 回调 → RMS 快照」链路。
fn run_audio_smoke(duration: std::time::Duration, silence: bool) -> ExitCode {
    let mode = if silence {
        "数字静音（全链路，无声可听）"
    } else {
        "440 Hz 低音量正弦（音量 0.05）"
    };
    println!(
        "audio-smoke: 时长 {:.1?}，内容 {mode}；源域 {} Hz × 1 ch（TTS 默认）",
        duration,
        live2d_ai_runtime::AudioSpec::DEFAULT_SAMPLE_RATE
    );

    match audio::run_audio_smoke(duration, silence) {
        Ok(report) => {
            for line in report.summarize_lines() {
                println!("{line}");
            }
            if report.device.is_none() {
                // 环境无音频设备：如实 skip，不算失败也不算成功播放。
                eprintln!("[environment] 本机没有可用的默认音频输出设备；");
                eprintln!("[environment] 提示：无声卡 CI 请使用默认模式（不带 --audio-smoke）。");
                return ExitCode::from(EXIT_ENVIRONMENT);
            }
            if !report.drained {
                eprintln!("[error] 播放在超时内未能排空（回调可能未推进）");
                return ExitCode::from(EXIT_ERROR);
            }
            ExitCode::from(EXIT_OK)
        }
        Err(e) => {
            if e.is_environment() {
                eprintln!("[environment] {e}");
                eprintln!("[environment] 提示：本机可能没有可用的音频输出设备/ALSA 配置；");
                eprintln!("[environment] 无声卡 CI 请使用默认模式（不带 --audio-smoke）。");
                ExitCode::from(EXIT_ENVIRONMENT)
            } else {
                eprintln!("[error] {e}");
                ExitCode::from(EXIT_ERROR)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EXIT_ENVIRONMENT, EXIT_ERROR, EXIT_OK, EXIT_USAGE};

    #[test]
    fn exit_codes_are_distinct_and_stable_contract() {
        assert_eq!(EXIT_OK, 0);
        assert_eq!(EXIT_ERROR, 1);
        assert_eq!(EXIT_USAGE, 2);
        assert_eq!(EXIT_ENVIRONMENT, 3);
    }

    /// 防回归：**恰好 4 个** Mod 工厂。
    ///
    /// 2026-09-12（rc.2）director 已删除——它是动作序列的唯一驱动方，而动作在产品
    /// 路径上不存在。数字断言存在的意义就是「不要再挂回去」：若有人把 director（或
    /// 任何新的动作 Mod）加回静态注册，这条会立刻红。
    /// rc.4 M5：+1 个角色卡 Mod（`persona`），数字与之同步。
    #[test]
    fn mod_count_is_four() {
        assert_eq!(
            super::AVAILABLE_MOD_FACTORIES.len(),
            4,
            "AVAILABLE_MOD_FACTORIES must contain exactly 4 Mod factories (external-input, pet-desktop, local-llm, persona)"
        );
    }

    #[test]
    fn mod_factory_ids_match_expected() {
        let mut ids: Vec<&str> = super::AVAILABLE_MOD_FACTORIES
            .iter()
            .map(|f| f.descriptor().id)
            .collect();
        ids.sort();

        let mut expected = vec!["external-input", "pet-desktop", "local-llm", "persona"];
        expected.sort();

        assert_eq!(
            ids, expected,
            "Mod factory IDs must match the documented set"
        );
    }

    #[test]
    fn mod_factory_ids_are_unique() {
        let mut ids: Vec<&str> = super::AVAILABLE_MOD_FACTORIES
            .iter()
            .map(|f| f.descriptor().id)
            .collect();
        ids.sort();
        ids.dedup();
        assert_eq!(
            ids.len(),
            super::AVAILABLE_MOD_FACTORIES.len(),
            "Mod factory IDs must be unique"
        );
    }
}
