//! 终端 chat 会话模式：装配配置/声卡/supervisor 与 winit 事件循环，
//! stdin REPL 线程驱动「文本 → LLM → TTS → 声卡 → 动作/口型」闭环；
//! 无音频设备自动降级 dry-run（不阻塞对话链路）。

use std::path::{Path, PathBuf};
use std::time::Duration;

use live2d_ai_core::ModelCapabilities;
use live2d_ai_runtime::{AppSettings, OpenAiClient};
use tracing::{info, warn};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopProxy};

use super::ModelSmokeOptions;
use super::{BackendError, BackendKind, FeatureRequest, RunOptions, RunReport, UnsupportedFeature};
use crate::app::{ChatBridge, ShellApp, panic_message};
use crate::app_event::{AppEvent, send_app_event};
use crate::audio::AudioOutputFacade;
use crate::model_smoke::ModelSmokeError;
use crate::repl::ReplCommand;
use crate::supervisor::{SupervisorConfig, SupervisorHandle, spawn_supervisor};
use crate::user_event::{PetUserEvent, pet_mode_tray_degradation_note, plan_launch};
use crate::{cli, repl};

pub struct ChatOptions {
    /// `live2d-ai.toml` 路径。`None` = 当前目录查找；再找不到 → `Failed`
    /// （退出码 1）并提示复制模板 `live2d-ai.toml.example`。
    pub config_path: Option<PathBuf>,
    /// 桌宠模式（默认置顶 + 点击穿透 + 托盘恢复入口；托盘失败禁自动穿透）。
    pub pet_mode: bool,
    /// 超时自动退出兜底（复用 [`RunOptions::timeout`] 语义；None = 常驻）。
    pub smoke_timeout: Option<Duration>,
}

/// 缺省 chat 配置文件（cwd 查找目标）。
const DEFAULT_CONFIG_FILENAME: &str = "live2d-ai.toml";

/// chat 配置文件解析（纯函数，可单测）。
///
/// - 显式路径：按原样使用，必须是已存在文件；
/// - 缺省：`cwd/live2d-ai.toml`；
/// - 找不到 → 可读错误并提示复制模板（配置缺失属使用/资产问题 → 退出码 1）。
pub(crate) fn resolve_chat_config_path_in(
    explicit: Option<&Path>,
    cwd: &Path,
) -> Result<PathBuf, String> {
    let path = match explicit {
        Some(p) => PathBuf::from(p),
        None => cwd.join(DEFAULT_CONFIG_FILENAME),
    };
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "未找到 chat 配置文件 {}（--chat 需要 live2d-ai.toml）；\
             请先复制 live2d-ai.toml.example 并按需填写 LLM/TTS 端点",
            path.display()
        ))
    }
}

/// [`resolve_chat_config_path_in`] 的进程环境版（cwd = 当前目录）。
fn resolve_chat_config_path(explicit: Option<&Path>) -> Result<PathBuf, String> {
    resolve_chat_config_path_in(explicit, Path::new("."))
}

/// 运行终端 chat 会话（`--chat`）：配置 → LLM/TTS 客户端 → 声卡（无设备
/// dry-run 降级）→ supervisor → winit 事件循环；stdin REPL 线程驱动对话，
/// 直到 `/quit` / 窗口关闭 / 托盘退出 / 超时兜底。
///
/// 退出码契约与 [`crate::app::run_shell`] 一致：配置/模型/代码问题 →
/// [`BackendError::Failed`]（退出码 1）；显示/GPU 环境不满足 →
/// [`BackendError::Environment`]（退出码 3）。音频设备缺失**不是**错误：
/// supervisor 原生支持 `audio: None` 的 dry-run（文字照常、口型静默）。
pub fn run_chat_session(opts: ChatOptions) -> Result<RunReport, BackendError> {
    // ---- 1. 配置文件 → 解析 → 客户端。任何失败都归为配置/资产问题（退出码 1）。
    // 为什么不用 settings.resolve()：显式写出环境读取闭包，让「密钥缺失 = 不鉴权」
    // 的语义在装配点可见（与 runtime 文档一致）。
    let config_path =
        resolve_chat_config_path(opts.config_path.as_deref()).map_err(BackendError::Failed)?;
    println!("chat: 配置文件 {}", config_path.display());
    let settings = AppSettings::load_from_path(&config_path)
        .map_err(|e| BackendError::Failed(format!("读取/解析配置文件失败: {e}")))?;
    let resolved = settings
        .resolve_with(|name| std::env::var(name).ok().filter(|v| !v.is_empty()))
        .map_err(|e| BackendError::Failed(format!("配置解析失败（URL/环境变量名）: {e}")))?;
    println!(
        "chat: LLM {} 模型 {}；TTS {} 音色 {}",
        resolved.llm.base_url, resolved.llm.model, resolved.tts.base_url, resolved.tts.voice
    );
    let client = OpenAiClient::new(resolved.llm.clone(), resolved.tts.clone())
        .map_err(|e| BackendError::Failed(format!("构建 LLM/TTS 客户端失败: {e}")))?;

    // ---- 2. 声卡：环境类失败（无设备/配置不可用）→ 可见降级 dry-run，不中断；
    // 其余失败（流构建等）同样降级但只走日志——chat 的核心价值（对话链路）
    // 不依赖声卡存在。
    let audio = match AudioOutputFacade::open_default(resolved.tts.spec, Duration::from_millis(200))
    {
        Ok(facade) => Some(facade),
        Err(e) if e.is_environment() => {
            println!("[audio] 无可用音频输出设备，chat 进入 dry-run（文字照常、口型静默）: {e}");
            warn!("chat audio 降级为 dry-run（环境错误）: {e}");
            None
        }
        Err(e) => {
            warn!("chat audio 打开失败（按 dry-run 继续）: {e}");
            None
        }
    };
    // 快照必须在 facade 随 SupervisorConfig 移交所有权之前取（D5：只读 Arc 快照）。
    let snapshot = audio.as_ref().map(|a| a.mouth_snapshot());

    // ---- 3. 事件循环（原样复用 run_shell 的环境归类 catch_unwind 块）----
    let event_loop = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        EventLoop::<AppEvent>::with_user_event().build()
    })) {
        Ok(Ok(event_loop)) => event_loop,
        Ok(Err(e)) => {
            return Err(BackendError::Environment(format!(
                "无法创建 winit 事件循环（无显示服务？DISPLAY/WAYLAND_DISPLAY 未设置或不可达）: {e}"
            )));
        }
        Err(payload) => {
            return Err(BackendError::Environment(format!(
                "winit 平台初始化失败（缺系统库？如 libxkbcommon-x11 / 显示服务不可达）: {}",
                panic_message(payload.as_ref())
            )));
        }
    };
    event_loop.set_control_flow(ControlFlow::Poll);
    let proxy = event_loop.create_proxy();

    // ---- 4. 托盘 + pet-mode 启动决策（与 run_shell 相同安全骨架）----
    let tray = crate::tray::spawn_pet_tray(proxy.clone(), crate::tray::TRAY_CONFIRM_WAIT);
    let tray_ready = tray.is_ready();
    if let Some(note) = &tray.degradation_note {
        warn!("tray 降级: {note}");
        println!("[tray] {note}");
    }
    let plan = plan_launch(opts.pet_mode, tray_ready);
    if let Some(note) = pet_mode_tray_degradation_note(opts.pet_mode, tray_ready) {
        warn!("{note}");
        println!("[pet-mode] {note}");
    }
    info!(
        ?plan,
        tray_ready, "chat 启动决策（pet-mode → 初始窗口意图）"
    );
    if let Some(state) = tray.state() {
        state.set_click_through(plan.click_through);
        state.set_always_on_top(plan.always_on_top);
        state.set_visible(true);
    }

    // ---- 5. 模型：chat 固定加载 Bai 皮套驱动动作/口型渲染（资产缺失 → 退出码 1）。
    let model3_path = cli::resolve_model3_path(None).map_err(BackendError::Failed)?;
    println!("chat: 皮套 {}", model3_path.display());
    let run_options = RunOptions {
        initial_logical_size: (480.0, 640.0),
        // chat 无帧数概念（常驻直到 /quit/关窗/托盘退出/超时兜底）。
        frame_target: None,
        timeout: opts.smoke_timeout,
        model_smoke: Some(ModelSmokeOptions { model3_path }),
        pet_mode: opts.pet_mode,
    };
    run_options
        .validate()
        .map_err(|msg| BackendError::Failed(format!("运行参数非法: {msg}")))?;

    // ---- 6. supervisor：snapshot 已取；`audio` 所有权在本 move 进配置。----
    let supervisor = std::sync::Arc::new(spawn_supervisor(
        SupervisorConfig {
            client,
            conversation: resolved.conversation,
            capabilities: ModelCapabilities::all(),
            // PcmProducer trait object：生产实现为 cpal facade；dry-run 为 None。
            audio: audio.map(|a| Box::new(a) as Box<dyn crate::audio::PcmProducer>),
            // 热重载源：与配置解析共用同一路径；String 而非 PathBuf 是为
            // 与 [`crate::web_api::ServerContext::config_path`] 类型一致。
            config_path: Some(config_path.to_string_lossy().into_owned()),
            mod_events: None,
        },
        {
            let p = proxy.clone();
            move |ev| send_app_event(&p, ev)
        },
    ));

    // ---- 7. stdin REPL 线程（不 join：进程退出即亡；Ctrl-C 由信号终止进程）。----
    // 先克隆再 move 进线程：supervisor 句柄同时还要进 ChatBridge（见下）。
    let repl_handle = std::sync::Arc::clone(&supervisor);
    std::thread::Builder::new()
        .name("repl-stdin".to_owned())
        .spawn(move || repl_stdin_loop(repl_handle, proxy))
        .expect("启动 repl-stdin 线程");

    // ---- 8. 事件循环：装配窗口壳后照抄 run_shell 尾部收尾（环境 3 / 错误 1）。----
    let mut app = ShellApp::new(
        run_options,
        tray,
        plan,
        Some(ChatBridge {
            supervisor: std::sync::Arc::clone(&supervisor),
            snapshot,
            // 与 SupervisorConfig.config_path 同源：原生设置面板写盘目标。
            config_path: Some(config_path.to_string_lossy().into_owned()),
        }),
    );
    let run_result = event_loop
        .run_app(&mut app)
        .map_err(|e| format!("事件循环异常退出: {e}"));

    // 失败归类与 run_shell 尾部逐字一致：environment/model_failure 优先于
    // 事件循环本身的结果（即使 run_app 报错，也按已记录的具体失败归类）。
    let failure = if let Some(msg) = app.environment_failure.take() {
        Some(BackendError::Environment(msg))
    } else {
        app.model_failure.take().map(|e| match e {
            ModelSmokeError::GpuEnvironment(msg) => {
                BackendError::Environment(format!("Live2D 渲染 GPU 环境不满足: {msg}"))
            }
            other => BackendError::Failed(other.to_string()),
        })
    };
    // 成功路径先消费 app（其 Arc 克隆随 ChatBridge drop），再回收 supervisor。
    let outcome = match failure {
        Some(err) => Err(err),
        None => run_result
            .map_err(BackendError::Failed)
            .map(|()| app.into_report()),
    };
    reclaim_supervisor(supervisor);
    outcome
}

/// stdin REPL 线程主体：逐行解析并把命令发往 supervisor / 事件循环。
///
/// - `Say` 走容量 1 的有界通道：满即提示「忙碌」，绝不阻塞 stdin；
/// - `Release` 经代理发**显式目标态**（不经 toggle，避免与当前态竞态）；
/// - `Quit` 先通知 supervisor 进入 shutdown handshake，再结束线程；
/// - 循环自然结束（EOF：Ctrl-D / 管道关闭）视同 `/quit`（quit 幂等，重复无害）。
fn repl_stdin_loop(handle: std::sync::Arc<SupervisorHandle>, proxy: EventLoopProxy<AppEvent>) {
    use std::io::BufRead as _;
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                // 读取失败（如终端关闭）：按退出处理，避免死循环刷错误。
                eprintln!("[stdin] 读取失败: {e}");
                break;
            }
        };
        match repl::parse_line(&line) {
            None => {} // 空行 / `#` 注释：忽略（parse_line 已返回 None）。
            Some(ReplCommand::Say(text)) => {
                if !handle.say(text) {
                    println!("忙碌：已有待播输入，请稍候或 /stop");
                }
            }
            Some(ReplCommand::Stop) => handle.stop(),
            Some(ReplCommand::Release) => {
                // 退出点击穿透恢复交互（显式 false）：穿透中窗口收不到输入，
                // 终端 `/release` 是恢复交互的显式入口之一。
                send_app_event(
                    &proxy,
                    AppEvent::Tray(PetUserEvent::TraySetClickThrough(false)),
                );
            }
            Some(ReplCommand::Quit) => {
                handle.quit();
                break;
            }
            Some(ReplCommand::Unknown(word)) => {
                println!("未知命令 /{word}（可用：/stop /release /quit）");
            }
        }
    }
    handle.quit();
}

/// 回收 supervisor 线程：事件循环结束（无论成败）后等它收尾再返回。
///
/// `Arc::try_unwrap` 只在 REPL 线程已结束（不再持有句柄）时才能取回所有权并
/// join。窗口关闭/托盘退出路径下 stdin 线程仍阻塞读 → 解包失败即跳过：安全——
/// supervisor 发完 [`AppEvent::ShutdownReady`] 已等于清理完毕，进程退出时由
/// OS 回收线程。
fn reclaim_supervisor(handle: std::sync::Arc<SupervisorHandle>) {
    match std::sync::Arc::try_unwrap(handle) {
        Ok(handle) => handle.join(),
        Err(_) => warn!("repl-stdin 仍持有 supervisor 句柄（非 /quit 退出路径），跳过显式 join"),
    }
}

/// 后端描述信息（观测/日志用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDescription {
    pub kind: BackendKind,
    pub title: &'static str,
    /// 能力现状说明（必须如实声明未实现项）。
    pub notes: &'static [&'static str],
}

/// 桌宠平台后端统一接口。
///
/// 后续阶段的扩展方式：新增 `BackendKind` 变体 + 对应实现，
/// 并在 [`create_backend`] 工厂注册；接口签名保持稳定。
pub trait DesktopBackend {
    /// 后端自述（含诚实的能力现状说明）。
    fn describe(&self) -> BackendDescription;

    /// 创建并运行真实窗口壳，直到停止条件满足。
    ///
    /// 无显示/GPU 环境应返回 [`BackendError::Environment`]，而不是 panic。
    fn run(&self, options: &RunOptions) -> Result<RunReport, BackendError>;

    /// 静态能力声明：该后端是否实现了对应能力的代码路径。
    ///
    /// ⚠️ 口径（本批起）：`Ok(())` 只表示“代码路径存在”，**不是**运行时承诺；
    /// 实际能力以 [`RunReport::runtime`] 的真实初始化/API 调用结果为准
    /// （例如置顶在 Wayland 实测后端上会记录 unavailable）。
    /// 尚未实现的代码路径必须返回 [`UnsupportedFeature`]。
    fn request_feature(&self, feature: FeatureRequest) -> Result<(), UnsupportedFeature> {
        Err(UnsupportedFeature {
            feature,
            reason: "该能力在本后端没有代码路径",
        })
    }
}

/// 简单实现占位：`describe()` 用到的静态说明文本集中在这里，保证口径一致。
///
/// 契约（见 [`DesktopBackend::request_feature`] 的口径说明）：除逐项能力现状外，
/// 必须显式声明「静态声明 ≠ 运行时承诺，实际生效以 `RunReport.runtime` 为准」——
/// 这是本 crate 对外能力叙述的既定语义，`backend::tests`
/// `description_declares_pet_capabilities_and_runtime_gate` 会逐词校验。
pub const WINIT_WGPU_NOTES: &[&str] = &[
    "透明无边框可调整大小窗口 + wgpu surface 连续重绘：已实现",
    "always_on_top/global_position（set_window_level/set_outer_position）：已实现，仅 RawWindowHandle 实测 X11 生效",
    "click_through/drag_move（set_cursor_hittest/drag_window）：方向已修复（业务布尔→hittest 取反），X11/Wayland 用户可见效果待人工 RC；drag_window 能力依具体 WM/合成器实测",
    "tray（ksni SNI）：菜单回调只经 EventLoopProxy 发 UserEvent；无 host/扩展时可见降级不阻塞启动",
    "pet-mode 默认置顶+穿透；托盘未确认就绪时禁止自动穿透（防不可恢复）",
    // 为什么放最后：先逐项说清「有什么代码路径」，再用一句钉死运行时 gate 口径，
    // 防止读者把上面的「已实现」误读成「任何环境下都可用」。
    "运行时 gate：request_feature=Ok 仅代表代码路径存在（静态声明）；置顶/定位/穿透/拖动/托盘是否真实生效，一律以 RunReport.runtime 实测结果为准",
];

/// winit+wgpu 后端（本批唯一实现）。见 [`crate::app`]。
#[derive(Debug, Default)]
pub struct WinitWgpuBackend;

impl WinitWgpuBackend {
    pub fn new() -> Self {
        Self
    }
}

impl DesktopBackend for WinitWgpuBackend {
    fn describe(&self) -> BackendDescription {
        BackendDescription {
            kind: BackendKind::WinitWgpu,
            title: "winit 0.30 + wgpu 29 桌宠窗口（UserEvent 事件循环 + ksni 托盘）",
            notes: WINIT_WGPU_NOTES,
        }
    }

    fn request_feature(&self, feature: FeatureRequest) -> Result<(), UnsupportedFeature> {
        // 全部桌宠窗口能力本批都有代码路径；实际生效以 RunReport.runtime 为准
        // （置顶/全局定位仅 X11、托盘依赖 SNI host——见 crate README 能力语义表）。
        match feature {
            FeatureRequest::AlwaysOnTop
            | FeatureRequest::GlobalPosition
            | FeatureRequest::ClickThrough
            | FeatureRequest::DragMove
            | FeatureRequest::Tray => Ok(()),
        }
    }

    fn run(&self, options: &RunOptions) -> Result<RunReport, BackendError> {
        crate::app::run_shell(options.clone())
    }
}

/// 后端工厂。
pub fn create_backend(kind: BackendKind) -> Result<Box<dyn DesktopBackend>, BackendError> {
    match kind {
        BackendKind::WinitWgpu => Ok(Box::new(WinitWgpuBackend::new())),
    }
}
