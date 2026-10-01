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
//! - 许可：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准（见本 crate `README.md`）。

mod app_event;
mod audio;
mod cli;
mod logging;
mod mod_registry;
mod platform;

/// 已编译进本二进制的 Mod 工厂（静态注册；启用与否由 manifest `[mods.<id>]` 决定）。
/// 由 `cli_entry::run_web_mode` 装配 `ModRegistry`（节点 E5，2026-08-30）。
///
/// 2026-09-12（rc.2）：director Mod 已删除——它唯一的职责是**驱动序列**，
/// 而动作在产品路径上不存在（见 `docs/architecture/core-chain-baseline.md` §3.3）。
/// 归档点在分支 `archive/action-layer-p6`。**不要再挂回去**：
/// 下方 `mod_count_is_five` 是防回归断言。
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
/// 2026-09-14（产品级加强波次，未 bump 版本）：**封存 `wallpaper` + `pet-desktop`**
/// ——两者移出本表，数字 7 → **5**，并标 ARCHIVED，**禁止挂回**；理由与恢复条件见
/// `docs/architecture/ARCHIVED-mods.md`。用户手动的舞台/壳背景能力（`DisplayPrefs`）
/// **不受影响**——被拆掉的只是 wallpaper **Mod** 的接线。
/// （2026-10-01 W2-A：两个 crate 已**物理删除**，见同一 tag。）
pub static AVAILABLE_MOD_FACTORIES: &[&dyn live2d_ai_mod_system::ModFactory] = &[
    // 0.2.0-rc.1 起**缺省启用**（直播弹幕/礼物经 sidecar 注入，见 docs/external-input.md）。
    &live2d_ai_mod_external_input::FACTORY,
    // rc.4 M5：角色卡标准 Mod（缺省停用；启用后把卡合成 system_prompt 写回主链）。
    &live2d_ai_mod_persona::FACTORY,
    // Wave 1（0.2.0-rc.2）：语音转写 → 清洗 → say_tx；ASR 后端尚未接线，**缺省停用**。
    &live2d_ai_mod_voice_input::FACTORY,
    // Wave 2（0.2.0-rc.3）：会话记忆 v0（本地 JSONL + 词元重叠检索）。缺省停用是
    // 刻意的——它会写 `persona.system_prompt`，用户得先明确打开。
    &live2d_ai_mod_memory::FACTORY,
    // Wave 3（2026-09-14）：导演最小骨架（Wave 3 从 RFC 推进一格）。缺省停用；
    // **零投递**——只订阅 TurnPrompt/TurnEnded、只产决策日志与 state_json。
    &live2d_ai_mod_director::FACTORY,
];

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

    /// 防回归：**恰好 5 个** Mod 工厂。
    ///
    /// 2026-09-12（rc.2）那个 director 已删除——它是动作序列的唯一驱动方，而动作在产品
    /// 路径上不存在。数字断言存在的意义就是「不要再挂回去」：若有人把**驱动动作**的
    /// Mod 加回静态注册，这条会立刻红。
    /// rc.4 M5：+1 个角色卡 Mod（`persona`），数字与之同步。
    /// 0.2.0-rc.1：**local-llm 移出** → 4 → 3。数字断言继续守住「别再挂回来」。
    /// 0.2.0-rc.2（Wave 1）：+ `voice-input` / `wallpaper` → 3 → 5。
    /// 0.2.0-rc.3（Wave 2）：+ `memory` → 5 → 6。
    /// Wave 3（2026-09-14）：+ `director` **最小骨架**（零投递）→ 6 → 7。
    /// 与 rc.2 删除的那个不同：它不驱动动作，见 `director-mod-v0.md`。
    /// 产品级加强波次（2026-09-14）：**封存 `wallpaper` / `pet-desktop`** → 7 → 5。
    /// 封存（ARCHIVED）与废除（DEPRECATED）同口径：移出本表即不在启动注册表，
    /// 断言继续守住「别再挂回来」（理由见 `docs/architecture/ARCHIVED-mods.md`）。
    #[test]
    fn mod_count_is_five() {
        assert_eq!(
            super::AVAILABLE_MOD_FACTORIES.len(),
            5,
            "AVAILABLE_MOD_FACTORIES must contain exactly 5 Mod factories (external-input, persona, voice-input, memory, director)"
        );
    }

    #[test]
    fn mod_factory_ids_match_expected() {
        let mut ids: Vec<&str> = super::AVAILABLE_MOD_FACTORIES
            .iter()
            .map(|f| f.descriptor().id)
            .collect();
        ids.sort();

        // local-llm 已废除（0.2.0-rc.1）：不在此表即不在启动注册表。
        // wallpaper / pet-desktop 已封存（2026-09-14，产品级加强波次）：
        // 同样不在此表即不在启动注册表，见 docs/architecture/ARCHIVED-mods.md。
        let mut expected = vec![
            "director",
            "external-input",
            "memory",
            "persona",
            "voice-input",
        ];
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
