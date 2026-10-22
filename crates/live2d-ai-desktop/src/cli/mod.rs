//! 命令行解析（纯逻辑，可单测）。
//!
//! 拆分：`mod.rs`（本文件：模块文档 + `Command` 定义 + `parse_args` 入口）、
//! `usage`（`USAGE` 字面量）、`tests`（解析回归）。
//!
//! 2026-10-01（W2-B / D1 第二段）：原生壳移出构建 ⇒ `--window-smoke` /
//! `--model-smoke` / `--smoke-frames` / `--smoke-timeout-secs` / `--pet-mode` /
//! `--chat` / `--config` / `--benchmark*` 与 `path_resolve`（Bai 缺省模型路径解析）
//! 一并删除。理由、退出测试清单与恢复条件见
//! `docs/architecture/ARCHIVED-native-shell.md`（tag `checkpoint/pre-d1-dormant`）。
//!
//! 现存用法（完整说明见 `USAGE`）：
//! - 无参：Windows 包（0.4.0 rc.1 起）直接起 `--web` 的 18080；其它目标打印架构/
//!   会话线索/能力表后退出（无显示 CI 可安全运行，**不访问声卡**）。
//! - `--audio-smoke` 音频冒烟：默认 0.8 s、440 Hz 低音量正弦经实际声卡播放
//!   （无音频设备 → 退出码 3）；`--audio-smoke-secs S` 调时长（≤30 s）；
//!   `--audio-smoke-silence` 改播数字静音（全链路照常跑通）。
//! - `--web [--http-port P] [--dev-mode]`：Web API 后台模式（产品主路径）。
//! - `--desk-pet [--http-port P] [--dev-mode]`（0.4.3 rc.1）：桌宠 demo，**隐含
//!   `--web`**；**仅 Windows**——非 Windows 是用法错误（退出码 2），不启动服务。
//!   rc.1 只做参数解析与拒绝；宠物窗在 rc.2（本笔还没有 `desk_pet` 模块）。
//! - `--help` / `-h` 打印用法（`USAGE`）。
//! - `--dev-mode`（W7 任务）：覆盖 `live2d-ai.toml` 的 dev_mode 开关；
//!   优先级最高，与 `--web` 一同使用立即开放 `/api/v1/logs*` 端点。

use std::time::Duration;

pub mod usage;

#[cfg(test)]
mod tests;

pub use usage::USAGE;

/// `--audio-smoke` 默认时长：短促、可听见但不扰人。
pub const DEFAULT_AUDIO_SMOKE_SECS: f64 = 0.8;
/// 冒烟时长上限（秒）：防止无人值守模式长时间占用声卡。
pub const MAX_AUDIO_SMOKE_SECS: f64 = 30.0;

/// `--http-port` 默认端口（D1 §0 写 39221，但本批用 18080 避免与 Py 版 12393 冲突）。
pub const DEFAULT_HTTP_PORT: u16 = 18080;

/// 解析结果对应的执行动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 仅打印信息后退出（默认；无显示环境安全，不访问声卡）。
    Info,
    /// 打印信息并执行音频输出冒烟（访问声卡；无设备退出码 3）。
    AudioSmoke {
        /// 播放时长（默认 [`DEFAULT_AUDIO_SMOKE_SECS`]）。
        duration: Duration,
        /// true = 播放数字静音而非 440 Hz 低音量正弦。
        silence: bool,
    },
    /// **Web API 后台模式**（产品主路径）：监听 127.0.0.1:<port>，
    /// 暴露 HTTP 路由 + WS 事件流，并同源托管 Flutter Web `/app/`。
    ///
    /// **关键**：本模式**不**校验 LLM/TTS 配置（无配置时端点正常响应，
    /// `has_api_key`/`configured` 派生字段如实标 `false`）。
    Web {
        /// 监听端口（默认 [`DEFAULT_HTTP_PORT`]）。
        port: u16,
        /// dev-mode 启动覆盖（`--dev-mode` flag）。`true` 时优先级最高，
        /// 覆盖 settings 文件的 `dev_mode` 字段；日志端点依赖此开关。
        dev_mode: bool,
    },
    /// **桌宠 demo**（0.4.3 rc.1；`--desk-pet`，隐含 `--web`，仅 Windows）。
    ///
    /// 与 [`Command::Web`] 用**独立变体**而不是给 Web 加字段：现有 10 处 Web
    /// 断言因此一字不动。rc.1 只做参数解析与「非 Windows 拒绝」；窗口在 rc.2
    /// 加，所以本笔的执行与 Web 相同（见 `main.rs` 对应分支）。
    DeskPet {
        /// 监听端口（默认 [`DEFAULT_HTTP_PORT`]；`--http-port` 可覆盖）。
        port: u16,
        /// dev-mode 启动覆盖（同 [`Command::Web::dev_mode`]）。
        dev_mode: bool,
    },
}

/// CLI 解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub message: String,
}
impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// 桌宠窗是否受**当前目标**支持：只认 Windows（macOS / Linux 都 `false`）。
///
/// 抽成纯函数是为了在 Linux 上钉住两侧语义（0.4.3 rc.1：非 Windows 的
/// `--desk-pet` 是**用法错误**——不启动服务、不编译窗口模块）。
pub fn desk_pet_supported(os: &str) -> bool {
    os == "windows"
}

/// 解析参数列表（不含 argv[0]）。顺序无关、可组合。
pub fn parse_args(args: &[String]) -> Result<Command, CliError> {
    parse_args_for(std::env::consts::OS, args)
}

/// 与 [`parse_args`] 相同，但**目标 OS 可注入**（`std::env::consts::OS` 的字符串）。
///
/// 纯函数：Linux 单测可同时钉住 `"windows"` 与 `"linux"` / `"macos"` 两侧语义。
pub fn parse_args_for(os: &str, args: &[String]) -> Result<Command, CliError> {
    let mut audio_smoke = false;
    let mut audio_secs: Option<f64> = None;
    let mut audio_silence = false;
    // D2 HTTP/WS 控制平面：--web/--http-port 显式选择；与音频冒烟互斥。
    let mut web = false;
    let mut http_port: Option<u16> = None;
    // 0.4.3 rc.1：--desk-pet（隐含 --web；仅 Windows）。
    let mut desk_pet = false;
    // W7 任务：--dev-mode CLI 覆盖。
    let mut dev_mode = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" => return Err(CliError::new("__help__")),
            "--audio-smoke" => audio_smoke = true,
            "--audio-smoke-secs" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::new("--audio-smoke-secs 需要一个秒数参数"))?;
                let secs: f64 = value.parse().map_err(|_| {
                    CliError::new(format!("--audio-smoke-secs 参数非法: {value:?}"))
                })?;
                if !secs.is_finite() || secs <= 0.0 {
                    return Err(CliError::new("--audio-smoke-secs 必须为正的有限值"));
                }
                if secs > MAX_AUDIO_SMOKE_SECS {
                    return Err(CliError::new(format!(
                        "--audio-smoke-secs 最大 {MAX_AUDIO_SMOKE_SECS} 秒"
                    )));
                }
                audio_secs = Some(secs);
                audio_smoke = true; // 时长隐含音频冒烟
                i += 1;
            }
            "--audio-smoke-silence" => {
                audio_silence = true;
                audio_smoke = true; // 静音开关隐含音频冒烟
            }
            "--web" => {
                web = true;
            }
            "--desk-pet" => {
                // 与 --http-port 同款：隐含 web 模式（服务照常起，另开宠物窗）。
                desk_pet = true;
                web = true;
            }
            "--http-port" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::new("--http-port 需要一个端口号参数"))?;
                let p: u16 = value
                    .parse()
                    .map_err(|_| CliError::new(format!("--http-port 参数非法: {value:?}")))?;
                if p < 1024 {
                    return Err(CliError::new(
                        "--http-port 必须 >= 1024（避开系统保留端口）",
                    ));
                }
                http_port = Some(p);
                web = true; // 端口隐含 web 模式
                i += 1;
            }
            "--dev-mode" => {
                // W7 任务：dev_mode CLI 覆盖（web 模式装配时优先于 settings）。
                dev_mode = true;
            }
            other => {
                return Err(CliError::new(format!(
                    "未知参数: {other:?}（--help 查看用法）"
                )));
            }
        }
        i += 1;
    }

    // 两种模式互斥：一次只验证一条链路，避免语义含糊。
    if audio_smoke && web {
        return Err(CliError::new("--audio-smoke 与 --web 不能同时使用（互斥）"));
    }

    // 0.4.3 rc.1：非 Windows 的 --desk-pet 是**用法错误**（main.rs 在
    // `run_web_mode` 之前打印「参数错误 + USAGE」并返回退出码 2）——因此不会
    // bind 端口、不会启动服务，也不会编译窗口模块。错误消息**不得**是
    // `"__help__"`（那会变成退出码 0 的帮助路径）。
    if desk_pet && !desk_pet_supported(os) {
        return Err(CliError::new(format!(
            "--desk-pet 仅 Windows 可用（当前目标：{os}）；非 Windows 不开桌宠窗，请改用 --web"
        )));
    }
    if audio_smoke {
        let secs = audio_secs.unwrap_or(DEFAULT_AUDIO_SMOKE_SECS);
        return Ok(Command::AudioSmoke {
            duration: Duration::from_secs_f64(secs),
            silence: audio_silence,
        });
    }
    if desk_pet {
        return Ok(Command::DeskPet {
            port: http_port.unwrap_or(DEFAULT_HTTP_PORT),
            dev_mode,
        });
    }
    if web {
        return Ok(Command::Web {
            port: http_port.unwrap_or(DEFAULT_HTTP_PORT),
            dev_mode,
        });
    }
    // 纯 Info 模式：`--dev-mode` 单独出现时无 host 可覆盖 ⇒ 静默忽略（与旧语义一致）。
    // 0.4.0 rc.1：Windows 包**双击（无参数）**直接起 Web 18080（0.4.md「拷到
    // Windows 磁盘后双击。进程听 127.0.0.1:18080」）；其它目标仍是 Info。
    Ok(default_command(os, dev_mode))
}

/// 无 `--web` / `--audio-smoke` 时的缺省命令：**Windows 包双击即可用**。
///
/// `os` 取 `std::env::consts::OS` 的值（`"windows"` / `"linux"` / `"macos"`）。
/// 抽成纯函数是为了在 Linux 上钉住两侧语义（Linux 上没有 Windows 实机）：
/// Windows → `Web { port: 18080 }`（随后由 `browser` 模块自开启动器页）；
/// 其它 → `Info`（Linux 点火仍由 `scripts/ignite.sh` 显式带 `--web`）。
pub fn default_command(os: &str, dev_mode: bool) -> Command {
    if os == "windows" {
        Command::Web {
            port: DEFAULT_HTTP_PORT,
            dev_mode,
        }
    } else {
        Command::Info
    }
}
