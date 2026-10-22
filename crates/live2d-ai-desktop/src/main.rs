//! `live2d-ai-desktop` — PC 桌面应用（Rust 重建）。
//!
//! **产品主路径 = `--web`**：Rust 服务 + Flutter Web `/app/`（`./scripts/ignite.sh` 点火）。
//!
//! 2026-10-01（W2-B / D1 第二段）：**原生壳岛已移出构建** —— egui 原生壳
//! （`src/app/`）、`--chat` 终端壳（`src/repl.rs`）、托盘（`src/tray/`）、
//! 窗口/模型冒烟（`src/model_smoke/`、`src/adapter/`）、benchmark（`src/benchmark/`）
//! 及其枢纽 `src/backend/` 与 `src/user_event.rs` 一并删除。理由、逐条退出测试
//! 清单与恢复条件见 `docs/architecture/ARCHIVED-native-shell.md`
//! （还原点 tag `checkpoint/pre-d1-dormant`）。**不要挂回去。**
//!
//! 现存能力：
//! - **Web API**（[`web_api`]）：`--web [--http-port P] [--dev-mode]`——supervisor +
//!   WebSocket 事件流，同源托管 Flutter `/app/`；
//! - **实际声卡输出链路**（[`audio`]）：cpal 默认输出设备 + ringbuf 无锁 SPSC，
//!   TTS 域 PCM 经确定性重采样/声道映射入环，回调内欠载补 0 并对**实际写给
//!   声卡的 f32** 计算 RMS，经原子 f32-bits 快照暴露 mouth level；
//!   epoch 打断保证 stop 后旧音频不再继续；
//! - **音频冒烟**：`--audio-smoke [--audio-smoke-secs S] [--audio-smoke-silence]`
//!   （无音频设备退出码 3）；
//! - Linux 会话线索与能力簿记（[`platform`]：`LinuxSessionHint` /
//!   `DeclaredCapabilities` / `RuntimeCapabilities`——后者只记录真实初始化/API
//!   调用结果，绝不凭 X11 线索推断）；
//! - CLI（[`cli`]）：默认无参数打印架构与能力后退出（无显示 CI 安全，
//!   **不访问声卡**）。
//! - 许可：**AGPL-3.0-only**，以仓库根 LICENSE 为准（见本 crate README.md）。
//!
//! 2026-10-19（一 Mod 一本地仓库）：七个 Mod 实现（external-input / persona /
//! voice-input / memory / local-tts / local-engine / a2a）各自迁到一个本地仓库；
//! 主仓**缺省不引用任何 Mod 实现**——[AVAILABLE_MOD_FACTORIES] 为空（工厂数
//! **0**），Cargo.toml 不再有指向旧 sibling checkout（Mod 合集目录）的
//! path 依赖。POST /api/v1/external/chat、POST /api/v1/voice/transcript、
//! GET /.well-known/agent-card.json、POST /api/v1/a2a 在未挂载时恒定
//! 404 mod_disabled（见 web_api/{external_routes,voice_routes,a2a_gate}）。
//! 核心对话 / 导演（motion_bias）/ TTS / 前端仍在本仓。要挂回某个 Mod：维护者
//! 手动加回依赖 + 工厂表 + manifest 一行（见 docs/architecture/two-repo-mods.md）。

mod app_event;
mod audio;
/// 0.4.0 rc.1：Windows 包在监听成功后自开默认浏览器（非 Windows 不编译本模块）。
#[cfg(windows)]
mod browser;
mod cli;
/// 0.4.3 rc.2：桌宠 demo（`--desk-pet`）的分层窗。本模块平台无关（纯函数 + 单测）；
/// 真正的 Win32 代码在 `desk_pet/window.rs` 与 `desk_pet/win32.rs`，只
/// `#[cfg(windows)]` 编译。**真实窗口未验**。
mod desk_pet;
/// 0.4.0 rc.1：Windows 文件夹子进程的 Job Object（KILL_ON_JOB_CLOSE）。
#[cfg(windows)]
mod folder_job;
/// 0.3.13：文件夹 Mod 的子进程生命周期（PDEATHSIG / 独立进程组 / env 清洗）。
/// 只有 POST /api/v1/mods/discovered 在勾选写盘成功后调用；扫描与刷新不碰它。
mod folder_spawn;
/// 0.4.0 rc.1：exe 目录锚点 / 子进程环境白名单 / Windows 入口规则的纯函数。
mod host_layout;
mod log_paths;
mod logging;
mod mod_registry;
mod platform;

/// 已编译进本二进制的 Mod 工厂（静态注册；启用与否由 manifest `[mods.<id>]` 决定）。
/// 由 `cli_entry::run_web_mode` 装配 `ModRegistry`（节点 E5，2026-08-30）。
///
/// 2026-09-12（rc.2）：director Mod 已删除——它唯一的职责是**驱动序列**，
/// 而动作在产品路径上不存在（见 `docs/architecture/core-chain-baseline.md` §3.3）。
/// 归档分支 `archive/action-layer-p6` **已不存在**（2026-10-06 复核：本地 / 远端 /
/// 历史 bundle 的 heads 里都没有该 ref）——被删内容从 `main` 自己的历史取回：
/// `git show 98469df^:crates/live2d-ai-mod-director/src/lib.rs`（director）、
/// `git show ef9f428^:crates/l2d-wasm-demo/src/web/surface.rs`（渲染面编舞）。
/// **不要再挂回去**：
/// 下方的工厂数断言是防回归断言（2026-10-19 起为 `mod_count_is_zero`；此前的
/// `mod_count_is_six` / `mod_count_is_seven` 已随工厂数归零改名）。
///
/// 2026-10-17（rc4）：**决策/按句 cue 版 director Mod 也已退役**——按句清洗与
/// cue 收归核心第二路（单一 cue 写者），本表由 6 → **5**。
///
/// 2026-09-14（0.2.0-rc.1）：**local-llm 已废除启动**（移出本表）——本地推理进程
/// 管理/探活不再是产品路径；LLM 端点由 `live2d-ai.toml` 的 `[llm]` 人工配置。
/// 数字 4 → 3。（2026-10-01 W2-A：该 crate 已**物理删除**，见 tag
/// `checkpoint/pre-d1-dormant`。）
///
/// 2026-09-14（0.2.0-rc.2，Wave 1 合并）：追加 Wave 1 的 `voice-input` / `wallpaper`
/// 两个工厂（均**缺省停用**，`cli_entry::default_mods_manifest` 未收录）。数字 3 → 5。
///
/// 2026-09-14（0.2.0-rc.3，Wave 2 合并）：追加 `memory`（会话记忆：本地 JSONL +
/// 词元重叠检索 → 注入会话注入槽，**本轮请求体即带上**），同样**只注册、
/// 缺省停用**。数字 5 → 6。director 当时仍**不注册**（Wave 2 只交 RFC）。
///
/// 2026-09-14（Wave 3 合并，未 bump 版本）：追加 `director` **最小骨架**——只读
/// `TurnPrompt`/`TurnEnded` 产出确定性的 `{emotion,intent,suggested_tts}` 决策，
/// **只写日志 + `state_json`，零投递**（`action_tx` 仍休眠、不写 `live2d-ai.toml`）。
/// 数字 6 → 7，**同样缺省停用**（`cli_entry::default_mods_manifest` 未收录）。
/// 与 rc.2 删除的那个 director 不同：它**不驱动动作序列**，见
/// `docs/architecture/director-mod-v0.md` 与 `docs/architecture/director-rfc.md` §8。
/// 现状（2026-10-12）：「零投递 / 不驱动动作」只描述 2026-09-14 骨架当时。现行舞台
/// 驱动是 `ModServices.cues` → WS `action_cue`。`action_tx` 仍然休眠。
/// （2026-10-17 rc4：该 Mod 已退役、移出本表；按句 cue 改由核心第二路产出。）
/// 2026-09-14（产品级加强波次，未 bump 版本）：**封存 `wallpaper` + `pet-desktop`**
/// ——两者移出本表，数字 7 → **5**，并标 ARCHIVED，**禁止挂回**；理由与恢复条件见
/// `docs/architecture/ARCHIVED-mods.md`。用户手动的舞台/壳背景能力（`DisplayPrefs`）
/// **不受影响**——被拆掉的只是 wallpaper **Mod** 的接线。
/// （2026-10-01 W2-A：两个 crate 已**物理删除**，见同一 tag。）
///
/// 2026-10-19（一 Mod 一本地仓库）：**本表清空，工厂数 = 0**。七个实现
/// （external-input / persona / voice-input / memory / local-tts-melo /
/// local-engine / a2a）各自迁到一个本地仓库；主仓 Cargo.toml 不再引用任何
/// Mod 实现，故本二进制**缺省不编译进任何 Mod**。挂载方式（维护者手动）：
/// 把对应 crate 加回 crates/live2d-ai-desktop/Cargo.toml 的 [dependencies]，
/// 在这里加回 &...::FACTORY 一行，并在 cli_entry::default_mods_manifest 写
/// 该 Mod 的一行 manifest——**缺省提交里不要加**（本表是防回归断言的对象，
/// 见 mod_count_is_zero）。缺省核心（对话 / 导演 motion_bias / TTS / 前端）不受影响。
pub static AVAILABLE_MOD_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[];

/// 0.3.12：工作目录 `mods/<目录名>/mod.toml` 的**只读清单预检**。
///
/// 只读校验、不执行 entry / program、不把文件夹写进 `AVAILABLE_MOD_FACTORIES`。
/// 扫描根 = 进程当前目录下的 `mods/`（执行单 §3）；0.4.0 rc.1 起再追加
/// exe 目录下的 `mods/`（cwd 优先，见 `host_layout::mods_scan_roots`）。
mod mods_scan;
/// 0.4.0 rc.1：多锚点扫描（cwd 的 `mods/` 在前，exe 目录的在后；同 id 以 cwd 为准）。
mod mods_scan_multi;
/// `mod.toml` 的严格 TOML 子集解析（不新增依赖；子集外一律判不可用）。
mod mods_scan_toml;
mod session_scope;
mod supervisor;
mod web_api;

use std::process::ExitCode;

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
                "(未启动服务/声卡：默认模式仅打印信息；--web 起 Web API；\
                 --audio-smoke 进入音频冒烟)"
            );
            ExitCode::from(EXIT_OK)
        }
        cli::Command::AudioSmoke { duration, silence } => run_audio_smoke(duration, silence),
        cli::Command::Web { port, dev_mode } => {
            // 0.4.0 rc.1：只有 Windows 包的生产 `--web` 进程在监听成功后自开
            // 浏览器（测试 / 库调用不 arm；Linux 仍由 `scripts/ignite.sh` 打开）。
            #[cfg(windows)]
            browser::arm();
            ExitCode::from(web_api::cli_entry::run_web_mode(port, dev_mode))
        }
        cli::Command::DeskPet { port, dev_mode } => {
            // 0.4.3 rc.2：`--desk-pet` 隐含 `--web`——服务照常起（监听失败就不开窗，
            // 见 `web_api::start_server` 里 bind 成功之后的 start_if_armed），旁边
            // 再开一扇只画皮套的分层窗。非 Windows 的 `--desk-pet` 在 cli 层就被拒
            // （退出码 2），走不到这里；`desk_pet` 本身平台无关，arm 与线程启动都
            // 在模块内按 `cfg(windows)` 分叉。
            #[cfg(windows)]
            browser::arm();
            #[cfg(windows)]
            desk_pet::arm();
            ExitCode::from(web_api::cli_entry::run_web_mode(port, dev_mode))
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
        "  phase  : web service — Web API（supervisor + WebSocket 事件流）与 Flutter Web \
         /app/ 同源托管；实际声卡输出链路（cpal + ringbuf）与音频冒烟 --audio-smoke 可用。\
         原生窗口壳/托盘/终端 chat/benchmark 已于 2026-10-01（W2-B/D1）移出构建"
    );
    println!("  target : {arch}-{os} (family: {family})");
    println!("  rust   : MSRV {}", env!("CARGO_PKG_RUST_VERSION"));

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

    // 第二层：真实运行时能力 —— W2-B 后本 crate 没有窗口/surface/托盘的初始化路径，
    // 因此恒为「未探测」；保留该表是为了让默认模式的输出与历史契约同形
    // （不允许凭 X11 线索把任何字段变成 available）。
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

    /// 防回归：**缺省恰好 0 个** Mod 工厂（2026-10-19「一 Mod 一本地仓库」）。
    ///
    /// 历史（不删，逐条记录工厂数怎么走到 0 的）：
    /// 2026-09-12（rc.2）那个 director 已删除——它是动作序列的唯一驱动方，而动作在产品
    /// 路径上不存在。数字断言存在的意义就是「不要再挂回去」：若有人把**驱动动作**的
    /// Mod 加回静态注册，这条会立刻红。
    /// rc.4 M5：+1 个角色卡 Mod（`persona`），数字与之同步。
    /// 0.2.0-rc.1：**local-llm 移出** → 4 → 3。
    /// 0.2.0-rc.2（Wave 1）：+ `voice-input` / `wallpaper` → 3 → 5。
    /// 0.2.0-rc.3（Wave 2）：+ `memory` → 5 → 6。
    /// Wave 3（2026-09-14）：+ `director` **最小骨架**（零投递）→ 6 → 7。
    /// 产品级加强波次（2026-09-14）：**封存 `wallpaper` / `pet-desktop`** → 7 → 5。
    /// 2026-10-09「两类 TTS」：+ `local-tts` → 5 → 6；+ `local-tts-melo` → 6 → 7；
    /// **CosyVoice3（`local-tts`）移出并封存** → 7 → **6**。
    /// 2026-10-17（rc4）：**director Mod 退役**（按句清洗与 cue 收归核心第二路）→ 6 → 5。
    /// 2026-10-19：**七个 Mod 实现各自一个本地仓库**，主仓缺省不引用 → 5 → **0**。
    ///
    /// 为什么是 0：主仓只留核心（对话 / 导演 motion_bias / TTS / 前端）。要挂某个
    /// Mod，由维护者**手动**加回 Cargo 依赖 + 本表一行工厂 + 缺省 manifest 一行；
    /// 缺省提交里不得夹带任何 Mod 实现（本断言就是拦它的）。
    #[test]
    fn mod_count_is_zero() {
        assert_eq!(
            super::AVAILABLE_MOD_FACTORIES.len(),
            0,
            "一 Mod 一本地仓库，缺省工厂 0；挂载由维护者手动加回依赖+工厂（本提交不得夹带 Mod 实现）"
        );
        // 空表本身也钉死：防止有人把断言改成 `<=` 或删掉后偷偷挂回。
        assert!(
            super::AVAILABLE_MOD_FACTORIES.is_empty(),
            "AVAILABLE_MOD_FACTORIES 必须为空表（缺省不挂任何 Mod）"
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
