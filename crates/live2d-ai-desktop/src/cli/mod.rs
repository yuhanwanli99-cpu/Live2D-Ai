//! 命令行解析（纯逻辑，可单测）。
//!
//! 拆分：`mod.rs`（本文件：模块文档 + `Cli`/`Command` 定义 + `parse` 入口 + `USAGE`）、
//! `path_resolve`（模型路径解析纯函数）、`tests`（全部 17 条测试原样迁出）。
//!
//! 用法（完整说明见 `USAGE`）：
//! - 无参：打印架构/会话线索/能力表后退出（无显示 CI 可安全运行，**不访问声卡**）。
//! - `--window-smoke` 真实透明窗口壳（不加载 Live2D 模型），持续重绘直到窗口关闭。
//! - `--model-smoke [model3]` Bai 实时渲染冒烟：同一 device 上加载皮套并上屏渲染，
//!   自动播放六动作 + 合成口型（不访问 LLM/TTS/声卡）；可选参数是 `*.model3.json` 路径，
//!   缺省用仓库内 Bai 相对路径；默认保持**交互可关闭**（不置顶、不穿透）。
//! - `--pet-mode` 桌宠模式：默认**置顶 + 点击穿透** + 托盘菜单（穿透/置顶/显隐/退出）。
//!   安全规则：托盘未确认就绪时**禁止自动进入穿透**（防不可恢复），窗口保持交互。
//! - `--smoke-frames N` 冒烟模式：渲染 N 帧后自动退出（隐含窗口冒烟，60s 超时兜底）。
//! - `--smoke-timeout-secs S` 显式超时兜底。
//! - `--audio-smoke` 音频冒烟：默认 0.8 s、440 Hz 低音量正弦经实际声卡播放
//!   （无音频设备 → 退出码 3）；`--audio-smoke-secs S` 调时长（≤30 s）；
//!   `--audio-smoke-silence` 改播数字静音（全链路照常跑通）。
//! - `--chat [--config <path>]` 终端 chat 模式：stdin 逐行输入，经 supervisor 走
//!   「LLM → TTS → 声卡 → 动作/口型」闭环；`/stop` `/release` `/quit` 控制。
//! - `--benchmark [model3] [--benchmark-mode blocking|submit] [--benchmark-warmup N]
//!   [--benchmark-frames N] [--benchmark-output <P>]`：**benchmark-only** A/B（C7/C8，
//!   **禁止**成为生产默认）。自建 winit+wgpu 窗口、加载皮套、双模式渲染。
//! - `--help` / `-h` 打印用法（`USAGE`）。
//! - `--dev-mode`（W7 任务）：覆盖 `live2d-ai.toml` 的 dev_mode 开关；
//!   优先级最高，与 `--web` 一同使用立即开放 `/api/v1/logs*` 端点。

use std::path::PathBuf;
use std::time::Duration;

pub mod path_resolve;
pub mod usage;

#[cfg(test)]
mod tests;

pub use path_resolve::resolve_model3_path;
#[cfg(test)]
pub use path_resolve::resolve_model3_path_in;
pub use usage::USAGE;

/// `--audio-smoke` 默认时长：短促、可听见但不扰人。
pub const DEFAULT_AUDIO_SMOKE_SECS: f64 = 0.8;
/// 冒烟时长上限（秒）：防止无人值守模式长时间占用声卡。
pub const MAX_AUDIO_SMOKE_SECS: f64 = 30.0;

/// 缺省 Bai 皮套清单路径（相对仓库根；`--model-smoke` 不带路径时使用）。
pub const DEFAULT_BAI_MODEL3_RELATIVE: &str = "assets/models/bai/runtime/bai.model3.json";

/// `--benchmark` 默认 warm-up 帧数（C7 协议）。
pub const DEFAULT_BENCHMARK_WARMUP: u64 = 60;
/// `--benchmark` 默认正式帧数（C7 协议）。
pub const DEFAULT_BENCHMARK_FRAMES: u64 = 600;

/// `--http-port` 默认端口（D1 §0 写 39221，但本批用 18080 避免与 Py 版 12393 冲突）。
pub const DEFAULT_HTTP_PORT: u16 = 18080;

/// 解析结果对应的执行动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// 仅打印信息后退出（默认；无显示环境安全，不访问声卡）。
    Info,
    /// 打印信息并启动真实窗口壳冒烟（纯透明壳，不加载模型）。
    WindowSmoke {
        /// 渲染到该帧数后自动退出（None = 直到窗口关闭/超时）。
        frame_target: Option<u64>,
        /// 超时兜底（None = 无显式超时；帧数模式下由
        /// `RunOptions::effective_timeout` 提供 60s 默认兜底）。
        timeout: Option<Duration>,
        /// 桌宠模式（默认置顶 + 点击穿透 + 托盘；托盘失败禁自动穿透）。
        pet_mode: bool,
    },
    /// 打印信息并启动 **Bai 实时渲染冒烟**（同一 device 加载皮套上屏；
    /// 六动作固定顺序 + 合成口型）。模型/资产错误退出码 1、GPU 环境退出码 3。
    /// 默认交互可关闭（不置顶、不穿透）；`pet_mode=true` 按桌宠模式启动
    ///（托盘失败同样禁自动穿透）。
    ModelSmoke {
        /// model3.json 路径（None = [`DEFAULT_BAI_MODEL3_RELATIVE`] 的解析结果）。
        model3: Option<String>,
        /// 渲染到该帧数后自动退出（None = 直到窗口关闭/超时）。
        frame_target: Option<u64>,
        /// 超时兜底（语义同 WindowSmoke）。
        timeout: Option<Duration>,
        /// 桌宠模式。
        pet_mode: bool,
    },
    /// 打印信息并执行音频输出冒烟（访问声卡；无设备退出码 3）。
    AudioSmoke {
        /// 播放时长（默认 [`DEFAULT_AUDIO_SMOKE_SECS`]）。
        duration: Duration,
        /// true = 播放数字静音而非 440 Hz 低音量正弦。
        silence: bool,
    },
    /// 打印信息并进入终端 chat 模式：stdin 逐行输入 → LLM → TTS → 声卡 →
    /// 动作/口型闭环（supervisor 装配；无音频设备降级 dry-run）。
    /// 与三种冒烟互斥；`/quit`/关窗/托盘退出/超时兜底结束常驻。
    Chat {
        /// `live2d-ai.toml` 路径（None = cwd 查找，再找不到提示复制模板）。
        config_path: Option<PathBuf>,
        /// 桌宠模式（默认置顶 + 点击穿透 + 托盘；托盘失败禁自动穿透）。
        pet_mode: bool,
        /// 超时自动退出兜底（None = 常驻直到 /quit/关窗/托盘退出）。
        smoke_timeout: Option<Duration>,
    },
    /// **benchmark-only** A/B：C7/C8 llvmpipe 性能对比（同一 binary、双模式、
    /// 显式 `--benchmark` 调度；**禁止**成为生产默认）。
    /// - `mode` = `submit`（默认）：`render_to_view_submit`（非阻塞 + 每帧 Poll）；
    /// - `mode` = `blocking`：`render_to_view`（阻塞 wrapper；`submit_ms` 含内部 `Wait`）；
    /// - warm-up + 正式帧数同 C7 协议；不与三种冒烟/chat 共存；
    /// - `headless=true` 跳过 winit 事件循环，经 `GpuContext::headless()` + 离屏纹理
    ///   （沙箱无 X server 时的回退，口径差异见文档）。
    Benchmark {
        /// model3.json 路径（None = [`DEFAULT_BAI_MODEL3_RELATIVE`] 的解析结果）。
        model3: Option<String>,
        /// 渲染模式。
        mode: BenchmarkCliMode,
        /// warm-up 帧数（默认 [`DEFAULT_BENCHMARK_WARMUP`]）。
        warmup: u64,
        /// 正式帧数（默认 [`DEFAULT_BENCHMARK_FRAMES`]）。
        frames: u64,
        /// 是否走 headless 后端（无显示服务时用）。
        headless: bool,
        /// JSON 输出路径（None = 仅 stdout）。
        output_json: Option<PathBuf>,
    },
    /// **Web API 后台模式**（D2 接线，2026-08-28）：监听 127.0.0.1:<port>，
    /// 暴露 D1 §1 HTTP 路由（app/capabilities + app/status + settings GET/PATCH
    /// + settings/test/{llm,tts}）；WS 端点（runtime/state）D2 后续批次实现。
    ///
    /// **关键**：本模式**不**校验 LLM/TTS 配置（无配置时端点正常响应，
    /// `has_api_key`/`configured` 派生字段如实标 `false`），与 D1 §2 P0-3
    /// 「监听仅 loopback」一致。
    Web {
        /// 监听端口（默认 [`DEFAULT_HTTP_PORT`]）。
        port: u16,
        /// dev-mode 启动覆盖（`--dev-mode` flag）。`true` 时优先级最高，
        /// 覆盖 settings 文件的 `dev_mode` 字段；日志端点依赖此开关。
        dev_mode: bool,
    },
}

/// `--benchmark-mode` 的 CLI 字符串形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkCliMode {
    Blocking,
    Submit,
}

impl BenchmarkCliMode {
    /// CLI 字符串形态（与 `benchmark::BenchmarkMode::as_str` 对齐）。
    /// `#[allow(dead_code)]`：CLI 解析直接用字符串匹配，本方法仅供外部工具使用。
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::Submit => "submit",
        }
    }
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

/// 解析参数列表（不含 argv[0]）。顺序无关、可组合。
pub fn parse_args(args: &[String]) -> Result<Command, CliError> {
    let mut window_smoke = false;
    let mut model_smoke: Option<Option<String>> = None;
    let mut frame_target: Option<u64> = None;
    let mut timeout: Option<Duration> = None;
    let mut audio_smoke = false;
    let mut audio_secs: Option<f64> = None;
    let mut audio_silence = false;
    let mut pet_mode = false;
    let mut chat = false;
    let mut config: Option<PathBuf> = None;
    // benchmark（P1-2）：--benchmark 显式选择；模式/帧数/输出可选。
    let mut benchmark: Option<Option<String>> = None;
    let mut benchmark_mode: Option<BenchmarkCliMode> = None;
    let mut benchmark_warmup: Option<u64> = None;
    let mut benchmark_frames: Option<u64> = None;
    let mut benchmark_output: Option<PathBuf> = None;
    let mut benchmark_headless = false;
    // D2 HTTP/WS 控制平面：--web/--http-port 显式选择；与三种冒烟/chat/benchmark 互斥。
    let mut web = false;
    let mut http_port: Option<u16> = None;
    // W7 任务：--dev-mode CLI 覆盖。
    let mut dev_mode = false;

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" => return Err(CliError::new("__help__")),
            "--pet-mode" => pet_mode = true,
            "--chat" => chat = true,
            "--config" => {
                // chat 配置文件：值必填；下一个以 `-` 开头的参数不视为路径
                //（防误吞后续 flag，如 `--config --pet-mode`）。
                let value = args
                    .get(i + 1)
                    .filter(|v| !v.starts_with('-'))
                    .ok_or_else(|| CliError::new("--config 需要一个文件路径参数"))?;
                config = Some(PathBuf::from(value));
                chat = true; // 配置文件隐含 chat 模式（与音频时长隐含音频冒烟同风格）
                i += 1;
            }
            "--window-smoke" => window_smoke = true,
            "--model-smoke" => {
                // 可选路径参数：下一个存在且不以 `-` 开头的参数视为路径。
                let value = args.get(i + 1).filter(|v| !v.starts_with('-'));
                if value.is_some() {
                    i += 1;
                }
                if model_smoke.is_some() {
                    return Err(CliError::new("--model-smoke 只能出现一次"));
                }
                model_smoke = Some(value.cloned());
            }
            "--smoke-frames" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::new("--smoke-frames 需要一个帧数参数"))?;
                let n: u64 = value
                    .parse()
                    .map_err(|_| CliError::new(format!("--smoke-frames 参数非法: {value:?}")))?;
                if n == 0 {
                    return Err(CliError::new("--smoke-frames 必须 >= 1"));
                }
                frame_target = Some(n);
                i += 1;
            }
            "--smoke-timeout-secs" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::new("--smoke-timeout-secs 需要一个秒数参数"))?;
                let secs: u64 = value.parse().map_err(|_| {
                    CliError::new(format!("--smoke-timeout-secs 参数非法: {value:?}"))
                })?;
                if secs == 0 {
                    return Err(CliError::new("--smoke-timeout-secs 必须 >= 1"));
                }
                timeout = Some(Duration::from_secs(secs));
                i += 1;
            }
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
            "--benchmark" => {
                // 可选路径参数：下一个不以 `-` 开头的参数视为路径。
                let value = args.get(i + 1).filter(|v| !v.starts_with('-'));
                if value.is_some() {
                    i += 1;
                }
                if benchmark.is_some() {
                    return Err(CliError::new("--benchmark 只能出现一次"));
                }
                benchmark = Some(value.cloned());
            }
            "--benchmark-mode" => {
                let value = args.get(i + 1).ok_or_else(|| {
                    CliError::new("--benchmark-mode 需要一个值（blocking | submit）")
                })?;
                let m = match value.as_str() {
                    "blocking" => BenchmarkCliMode::Blocking,
                    "submit" => BenchmarkCliMode::Submit,
                    other => {
                        return Err(CliError::new(format!(
                            "--benchmark-mode 非法: {other:?}（仅 blocking|submit）"
                        )));
                    }
                };
                benchmark_mode = Some(m);
                i += 1;
            }
            "--benchmark-warmup" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::new("--benchmark-warmup 需要一个帧数参数"))?;
                let n: u64 = value.parse().map_err(|_| {
                    CliError::new(format!("--benchmark-warmup 参数非法: {value:?}"))
                })?;
                if n == 0 {
                    return Err(CliError::new("--benchmark-warmup 必须 >= 1"));
                }
                benchmark_warmup = Some(n);
                i += 1;
            }
            "--benchmark-frames" => {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| CliError::new("--benchmark-frames 需要一个帧数参数"))?;
                let n: u64 = value.parse().map_err(|_| {
                    CliError::new(format!("--benchmark-frames 参数非法: {value:?}"))
                })?;
                if n == 0 {
                    return Err(CliError::new("--benchmark-frames 必须 >= 1"));
                }
                benchmark_frames = Some(n);
                i += 1;
            }
            "--benchmark-output" => {
                let value = args
                    .get(i + 1)
                    .filter(|v| !v.starts_with('-'))
                    .ok_or_else(|| CliError::new("--benchmark-output 需要一个文件路径参数"))?;
                benchmark_output = Some(PathBuf::from(value));
                i += 1;
            }
            "--benchmark-headless" => {
                benchmark_headless = true;
            }
            "--web" => {
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
    // 五种模式互斥：一次只验证一条链路，避免语义含糊。
    let mode_count = usize::from(window_smoke)
        + usize::from(model_smoke.is_some())
        + usize::from(audio_smoke)
        + usize::from(chat)
        + usize::from(benchmark.is_some())
        + usize::from(web);
    if mode_count > 1 {
        return Err(CliError::new(
            "--chat / --window-smoke / --model-smoke / --audio-smoke / --benchmark / --web 不能同时使用（互斥）",
        ));
    }
    // pet-mode 与音频冒烟互斥（桌宠常驻与声卡冒烟语义不兼容）。
    if pet_mode && audio_smoke {
        return Err(CliError::new("--pet-mode 不能与音频冒烟同时使用"));
    }
    // pet-mode 与 benchmark 互斥：bench 是自建窗口，不进入生产桌宠模式。
    if pet_mode && benchmark.is_some() {
        return Err(CliError::new("--pet-mode 不能与 --benchmark 同时使用"));
    }

    // 兼容既有语义：孤立的 --smoke-frames N 隐含纯透明窗口壳冒烟
    // （chat 模式无帧数概念，不参与该隐含规则）。
    if !window_smoke && model_smoke.is_none() && !audio_smoke && !chat && frame_target.is_some() {
        window_smoke = true;
    }
    // pet-mode 单独出现 = 常驻桌宠壳（隐含窗口运行；直到托盘退出/关窗/超时）；
    // 已显式进入 chat 时不再隐含窗口冒烟（chat 自己的 ShellApp 装配）。
    if !window_smoke && model_smoke.is_none() && !audio_smoke && !chat && pet_mode {
        window_smoke = true;
    }
    // 音频冒烟不消费帧数/超时参数（保持旧防呆语义）。
    if audio_smoke && (frame_target.is_some() || timeout.is_some()) {
        return Err(CliError::new(
            "--smoke-frames/--smoke-timeout-secs 只能搭配窗口或模型冒烟模式使用",
        ));
    }

    // chat 模式：只消费 config/pet-mode/smoke-timeout；帧数与音频专属参数防呆。
    if chat {
        if frame_target.is_some() {
            return Err(CliError::new(
                "--smoke-frames 只能搭配窗口或模型冒烟模式使用（chat 常驻，无帧数概念）",
            ));
        }
        if audio_secs.is_some() || audio_silence {
            return Err(CliError::new(
                "--audio-smoke-secs/--audio-smoke-silence 只能搭配音频冒烟模式使用",
            ));
        }
        return Ok(Command::Chat {
            config_path: config,
            pet_mode,
            smoke_timeout: timeout,
        });
    }

    if window_smoke || model_smoke.is_some() {
        // 孤立的音频参数（未进入音频模式）防呆。
        if audio_secs.is_some() || audio_silence {
            return Err(CliError::new(
                "--audio-smoke-secs/--audio-smoke-silence 只能搭配音频冒烟模式使用",
            ));
        }
        return if let Some(model3) = model_smoke {
            Ok(Command::ModelSmoke {
                model3,
                frame_target,
                timeout,
                pet_mode,
            })
        } else {
            Ok(Command::WindowSmoke {
                frame_target,
                timeout,
                pet_mode,
            })
        };
    }
    if audio_smoke {
        let secs = audio_secs.unwrap_or(DEFAULT_AUDIO_SMOKE_SECS);
        return Ok(Command::AudioSmoke {
            duration: Duration::from_secs_f64(secs),
            silence: audio_silence,
        });
    }
    // benchmark：与三种冒烟/chat 互斥（上面已校验）；专属参数防呆。
    if let Some(model3) = benchmark {
        if frame_target.is_some() {
            return Err(CliError::new(
                "--smoke-frames 不能与 --benchmark 同时使用（--benchmark-frames/-warmup 替代）",
            ));
        }
        if audio_secs.is_some() || audio_silence {
            return Err(CliError::new("--audio-smoke* 不能与 --benchmark 同时使用"));
        }
        if timeout.is_some() {
            return Err(CliError::new(
                "--smoke-timeout-secs 与 --benchmark 互斥（bench 自带帧数兜底）",
            ));
        }
        let mode = benchmark_mode.unwrap_or(BenchmarkCliMode::Submit);
        let warmup = benchmark_warmup.unwrap_or(DEFAULT_BENCHMARK_WARMUP);
        let frames = benchmark_frames.unwrap_or(DEFAULT_BENCHMARK_FRAMES);
        return Ok(Command::Benchmark {
            model3,
            mode,
            warmup,
            frames,
            headless: benchmark_headless,
            output_json: benchmark_output,
        });
    }
    // 纯 Info 模式下任何冒烟专属参数都无意义，报错防呆。
    if frame_target.is_some() || timeout.is_some() {
        return Err(CliError::new(
            "--smoke-frames/--smoke-timeout-secs 只能搭配窗口或模型冒烟模式使用",
        ));
    }
    if web {
        return Ok(Command::Web {
            port: http_port.unwrap_or(DEFAULT_HTTP_PORT),
            dev_mode,
        });
    }
    Ok(Command::Info)
}
