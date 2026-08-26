//! 命令行解析（纯逻辑，可单测）。
//!
//! 用法：
//! - （无参数）打印架构/会话线索/能力表后退出 —— 无显示 CI 可安全运行，
//!   **不访问声卡**；
//! - `--window-smoke` 启动真实透明窗口壳，持续重绘直到窗口关闭
//!   （纯透明清屏壳，不加载 Live2D 模型）；
//! - `--model-smoke [model3]` Bai 实时渲染冒烟：同一 device 上加载皮套并
//!   上屏渲染，自动播放六动作 + 合成口型（不访问 LLM/TTS/声卡）；可选参数
//!   是 `*.model3.json` 路径，缺省用仓库内 Bai 相对路径；默认保持**交互可关闭**
//!   （不置顶、不穿透）；
//! - `--pet-mode` 桌宠模式：默认**置顶 + 点击穿透** + 托盘菜单（穿透/置顶/显隐/
//!   退出）。安全规则：托盘未确认就绪时**禁止自动进入穿透**（防不可恢复），
//!   窗口保持交互。可单独使用（常驻桌宠壳），也可搭配窗口/模型冒烟；
//! - `--smoke-frames N` 冒烟模式：渲染 N 帧后自动退出（隐含窗口冒烟，
//!   并默认带 60s 超时兜底，见 `RunOptions::effective_timeout`）；搭配
//!   `--model-smoke` 时即模型冒烟帧数；
//! - `--smoke-timeout-secs S` 显式超时兜底；
//! - `--audio-smoke` 音频冒烟：默认 0.8 s、440 Hz 低音量正弦经实际声卡播放
//!   （无音频设备 → 退出码 3）；`--audio-smoke-secs S` 调时长（≤30 s）；
//!   `--audio-smoke-silence` 改播数字静音（全链路照常跑通）；
//! - `--help` / `-h` 打印用法。

use std::path::{Path, PathBuf};
use std::time::Duration;

/// `--audio-smoke` 默认时长：短促、可听见但不扰人。
pub const DEFAULT_AUDIO_SMOKE_SECS: f64 = 0.8;
/// 冒烟时长上限（秒）：防止无人值守模式长时间占用声卡。
pub const MAX_AUDIO_SMOKE_SECS: f64 = 30.0;

/// 缺省 Bai 皮套清单路径（相对仓库根；`--model-smoke` 不带路径时使用）。
pub const DEFAULT_BAI_MODEL3_RELATIVE: &str =
    "Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json";

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
    /// 六动作固定顺序 + 合成口型；模型/资产错误退出码 1、GPU 环境退出码 3）。
    ///
    /// 默认保持交互可关闭（不置顶、不穿透）；`pet_mode` 为 true 时按桌宠
    /// 模式启动（托盘失败同样禁自动穿透）。
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

    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "-h" | "--help" => return Err(CliError::new("__help__")),
            "--pet-mode" => pet_mode = true,
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
            other => {
                return Err(CliError::new(format!(
                    "未知参数: {other:?}（--help 查看用法）"
                )));
            }
        }
        i += 1;
    }

    // 三种冒烟互斥：一次只验证一条链路，避免语义含糊。
    let mode_count =
        usize::from(window_smoke) + usize::from(model_smoke.is_some()) + usize::from(audio_smoke);
    if mode_count > 1 {
        return Err(CliError::new(
            "--window-smoke / --model-smoke / --audio-smoke 不能同时使用（三者互斥）",
        ));
    }
    // pet-mode 与音频冒烟互斥（桌宠常驻与声卡冒烟语义不兼容）。
    if pet_mode && audio_smoke {
        return Err(CliError::new("--pet-mode 不能与音频冒烟同时使用"));
    }

    // 兼容既有语义：孤立的 --smoke-frames N 隐含纯透明窗口壳冒烟。
    if !window_smoke && model_smoke.is_none() && !audio_smoke && frame_target.is_some() {
        window_smoke = true;
    }
    // pet-mode 单独出现 = 常驻桌宠壳（隐含窗口运行；直到托盘退出/关窗/超时）。
    if !window_smoke && model_smoke.is_none() && !audio_smoke && pet_mode {
        window_smoke = true;
    }
    // 音频冒烟不消费帧数/超时参数（保持旧防呆语义）。
    if audio_smoke && (frame_target.is_some() || timeout.is_some()) {
        return Err(CliError::new(
            "--smoke-frames/--smoke-timeout-secs 只能搭配窗口或模型冒烟模式使用",
        ));
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
    // 纯 Info 模式下任何冒烟专属参数都无意义，报错防呆。
    if frame_target.is_some() || timeout.is_some() {
        return Err(CliError::new(
            "--smoke-frames/--smoke-timeout-secs 只能搭配窗口或模型冒烟模式使用",
        ));
    }
    Ok(Command::Info)
}

// ---------------------------------------------------------------- 模型路径

/// 解析 model smoke 的 model3.json 路径（纯函数；显式输入便于单测）。
///
/// - 显式路径（`--model-smoke <path>`）：按原样使用，不存在 → 错误；
/// - 缺省：依次尝试 `cwd_base/`[`DEFAULT_BAI_MODEL3_RELATIVE`] 与
///   `manifest_dir/../../`[`DEFAULT_BAI_MODEL3_RELATIVE`]（后者对应
///   `cargo run` 时仓库内相对布局，与 `crates/l2d` 测试同口径），
///   命中第一个存在的文件；都不存在 → 错误并列出尝试过的路径。
pub fn resolve_model3_path_in(
    explicit: Option<&str>,
    cwd_base: &Path,
    manifest_dir: &Path,
) -> Result<PathBuf, String> {
    if let Some(raw) = explicit {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("--model-smoke 路径为空".to_owned());
        }
        let path = PathBuf::from(trimmed);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!("指定的模型清单不存在: {}", path.display()));
    }

    let relative = Path::new(DEFAULT_BAI_MODEL3_RELATIVE);
    let candidates = [
        cwd_base.join(relative),
        manifest_dir.join("../../").join(relative),
    ];
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    let tried = candidates
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n  ");
    Err(format!(
        "未找到缺省 Bai 皮套清单（{}），尝试过:\n  {tried}\n\
         请用 --model-smoke <model3.json> 显式指定路径",
        DEFAULT_BAI_MODEL3_RELATIVE
    ))
}

/// [`resolve_model3_path_in`] 的进程环境版（cwd + 本 crate 编译期 manifest 目录）。
pub fn resolve_model3_path(explicit: Option<&str>) -> Result<PathBuf, String> {
    resolve_model3_path_in(
        explicit,
        Path::new("."),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
}

/// 用法文本（--help 与错误提示共用）。
pub const USAGE: &str = "\
live2d-ai-desktop — Live2D-Ai PC 桌面应用（Linux 原生窗口 + Bai 实时渲染）

用法:
  live2d-ai-desktop                     打印架构/会话线索/能力后退出（不访问声卡）
  live2d-ai-desktop --window-smoke      纯透明无边框窗口壳，持续重绘直到关闭
  live2d-ai-desktop --model-smoke [P]   Bai 实时渲染冒烟：加载皮套上屏 + 六动作 +
                                        合成口型（不访问 LLM/TTS/声卡；P 缺省为仓库内 Bai；
                                        默认交互可关闭，不置顶不穿透）
  live2d-ai-desktop --pet-mode          桌宠模式（常驻）：默认置顶 + 点击穿透 + 托盘菜单
                                        （穿透/置顶/显隐/退出；托盘未就绪时禁自动穿透）
  live2d-ai-desktop --pet-mode --model-smoke [P]
                                        桌宠模式 + 模型实时渲染
  live2d-ai-desktop --smoke-frames N    渲染 N 帧后自动退出（CI 冒烟，60s 超时兜底；
                                        搭配 --model-smoke 即模型冒烟帧数）
  live2d-ai-desktop --window-smoke --smoke-timeout-secs S
                                        显式超时兜底秒数
  live2d-ai-desktop --audio-smoke       音频冒烟：0.8s、440Hz 低音量经声卡播放
  live2d-ai-desktop --audio-smoke-secs S [--audio-smoke-silence]
                                        自定义时长（S <= 30）；--audio-smoke-silence 改播静音

选项:
  -h, --help                 打印本帮助
  --pet-mode                 桌宠模式：默认置顶 + 点击穿透 + 托盘恢复入口
                             （托盘初始化失败/超时 → 禁自动穿透，窗口保持交互；
                             可单独使用或搭配窗口/模型冒烟；与音频冒烟互斥）
  --window-smoke             纯透明窗口壳（不加载模型；与模型/音频冒烟互斥）
  --model-smoke [<model3>]   模型实时渲染冒烟（可选 model3.json 路径，不以 - 开头；
                             与窗口/音频冒烟互斥；缺参走 Bai 相对路径默认值）
  --smoke-frames <N>         冒烟帧数目标（N >= 1；孤立出现时隐含纯窗口壳冒烟）
  --smoke-timeout-secs <S>   窗口/模型冒烟超时兜底（S >= 1 秒）
  --audio-smoke              音频输出冒烟（访问默认声卡；无设备退出码 3）
  --audio-smoke-secs <S>     音频冒烟时长秒数（0 < S <= 30，可小数；隐含 --audio-smoke）
  --audio-smoke-silence      音频冒烟改播数字静音（链路照常，无声可听；隐含 --audio-smoke）

退出码: 0 成功；1 代码/资产/模型错误；2 CLI 用法错误；3 运行环境不满足（GPU/显示/声卡）。
日志: RUST_LOG=info|debug|trace|off（默认 info）";

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use super::{Command, parse_args};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn no_args_means_info_only() {
        assert_eq!(parse_args(&args(&[])), Ok(Command::Info));
    }

    #[test]
    fn window_smoke_plain() {
        assert_eq!(
            parse_args(&args(&["--window-smoke"])),
            Ok(Command::WindowSmoke {
                frame_target: None,
                timeout: None,
                pet_mode: false
            })
        );
    }

    #[test]
    fn pet_mode_alone_implies_resident_window_shell() {
        // 桌宠模式单独出现：隐含窗口运行（常驻；无帧目标/无显式超时）。
        assert_eq!(
            parse_args(&args(&["--pet-mode"])),
            Ok(Command::WindowSmoke {
                frame_target: None,
                timeout: None,
                pet_mode: true
            })
        );
        // 与帧数目标组合：冒烟语义保留。
        assert_eq!(
            parse_args(&args(&["--pet-mode", "--smoke-frames", "9"])),
            Ok(Command::WindowSmoke {
                frame_target: Some(9),
                timeout: None,
                pet_mode: true
            })
        );
    }

    #[test]
    fn pet_mode_combines_with_window_and_model_smoke() {
        assert_eq!(
            parse_args(&args(&["--window-smoke", "--pet-mode"])),
            Ok(Command::WindowSmoke {
                frame_target: None,
                timeout: None,
                pet_mode: true
            })
        );
        assert_eq!(
            parse_args(&args(&[
                "--model-smoke",
                "--pet-mode",
                "--smoke-frames",
                "30"
            ])),
            Ok(Command::ModelSmoke {
                model3: None,
                frame_target: Some(30),
                timeout: None,
                pet_mode: true
            })
        );
    }

    #[test]
    fn pet_mode_rejects_audio_smoke() {
        assert!(parse_args(&args(&["--pet-mode", "--audio-smoke"])).is_err());
        assert!(parse_args(&args(&["--pet-mode", "--audio-smoke-secs", "2"])).is_err());
    }

    #[test]
    fn flags_are_order_independent_and_combinable() {
        assert_eq!(
            parse_args(&args(&["--smoke-frames", "5", "--window-smoke"])),
            parse_args(&args(&["--window-smoke", "--smoke-frames", "5"])),
        );
        assert_eq!(
            parse_args(&args(&[
                "--window-smoke",
                "--smoke-frames",
                "5",
                "--smoke-timeout-secs",
                "9"
            ])),
            Ok(Command::WindowSmoke {
                frame_target: Some(5),
                timeout: Some(Duration::from_secs(9)),
                pet_mode: false,
            })
        );
    }

    #[test]
    fn smoke_frames_implies_window_smoke_without_forced_timeout() {
        // 显式超时缺省 → None；60s 兜底由 RunOptions::effective_timeout 提供。
        assert_eq!(
            parse_args(&args(&["--smoke-frames", "12"])),
            Ok(Command::WindowSmoke {
                frame_target: Some(12),
                timeout: None,
                pet_mode: false
            })
        );
    }

    #[test]
    fn help_and_errors() {
        assert!(parse_args(&args(&["--help"])).is_err());
        assert!(parse_args(&args(&["-h"])).is_err());
        assert!(parse_args(&args(&["--bogus"])).is_err());
        assert!(parse_args(&args(&["--smoke-frames"])).is_err());
        assert!(parse_args(&args(&["--smoke-frames", "0"])).is_err());
        assert!(parse_args(&args(&["--smoke-frames", "abc"])).is_err());
        assert!(parse_args(&args(&["--smoke-timeout-secs", "0"])).is_err());
        // 孤立的冒烟参数（未进入窗口模式）应报错防呆。
        assert!(parse_args(&args(&["--smoke-timeout-secs", "5"])).is_err());
    }

    #[test]
    fn audio_smoke_defaults_to_short_quiet_tone() {
        assert_eq!(
            parse_args(&args(&["--audio-smoke"])),
            Ok(Command::AudioSmoke {
                duration: Duration::from_secs_f64(super::DEFAULT_AUDIO_SMOKE_SECS),
                silence: false,
            })
        );
        // 静音开关只改内容，不改时长。
        assert_eq!(
            parse_args(&args(&["--audio-smoke", "--audio-smoke-silence"])),
            Ok(Command::AudioSmoke {
                duration: Duration::from_secs_f64(super::DEFAULT_AUDIO_SMOKE_SECS),
                silence: true,
            })
        );
    }

    #[test]
    fn audio_smoke_flags_imply_audio_mode_and_are_order_free() {
        let expected = Ok(Command::AudioSmoke {
            duration: Duration::from_millis(1500),
            silence: true,
        });
        assert_eq!(
            parse_args(&args(&[
                "--audio-smoke-secs",
                "1.5",
                "--audio-smoke",
                "--audio-smoke-silence"
            ])),
            expected
        );
        assert_eq!(
            parse_args(&args(&[
                "--audio-smoke-silence",
                "--audio-smoke-secs",
                "1.5"
            ])),
            parse_args(&args(&[
                "--audio-smoke-secs",
                "1.5",
                "--audio-smoke-silence"
            ])),
        );
        // 时长/静音可以脱离显式 --audio-smoke 单独出现（已隐含模式）。
        assert_eq!(
            parse_args(&args(&["--audio-smoke-secs", "2"])),
            Ok(Command::AudioSmoke {
                duration: Duration::from_secs(2),
                silence: false,
            })
        );
    }

    #[test]
    fn audio_smoke_rejects_bad_durations() {
        for bad in [
            vec!["--audio-smoke-secs"],
            vec!["--audio-smoke-secs", ""],
            vec!["--audio-smoke-secs", "0"],
            vec!["--audio-smoke-secs", "-1"],
            vec!["--audio-smoke-secs", "abc"],
            vec!["--audio-smoke-secs", "NaN"],
            vec!["--audio-smoke-secs", "inf"],
            vec!["--audio-smoke-secs", "31"],
        ] {
            assert!(parse_args(&args(&bad)).is_err(), "{bad:?} 应被拒绝");
        }
        // 边界值合法：30 秒整。
        assert!(parse_args(&args(&["--audio-smoke-secs", "30"])).is_ok());
    }

    #[test]
    fn window_and_audio_smoke_modes_conflict_and_orphan_flags_rejected() {
        // 三种冒烟互斥。
        assert!(parse_args(&args(&["--window-smoke", "--audio-smoke"])).is_err());
        assert!(parse_args(&args(&["--smoke-frames", "3", "--audio-smoke"])).is_err());
        assert!(parse_args(&args(&["--window-smoke", "--model-smoke"])).is_err());
        assert!(parse_args(&args(&["--model-smoke", "x.json", "--audio-smoke"])).is_err());
        assert!(parse_args(&args(&["--model-smoke", "--window-smoke"])).is_err());
        // 孤立音频参数挂在窗口模式下 → 防呆报错。
        assert!(parse_args(&args(&["--window-smoke", "--audio-smoke-silence"])).is_err());
        assert!(parse_args(&args(&["--window-smoke", "--audio-smoke-secs", "1"])).is_err());
        // 孤立音频参数挂在模型模式下同样防呆。
        assert!(parse_args(&args(&["--model-smoke", "--audio-smoke-silence"])).is_err());
        // 对照：静音参数单独出现即隐含音频模式，合法。
        assert!(parse_args(&args(&["--audio-smoke-silence"])).is_ok());
    }

    #[test]
    fn model_smoke_defaults_and_explicit_paths() {
        // 缺省：Bai 相对路径（None 交由路径解析层处理）。
        assert_eq!(
            parse_args(&args(&["--model-smoke"])),
            Ok(Command::ModelSmoke {
                model3: None,
                frame_target: None,
                timeout: None,
                pet_mode: false
            })
        );
        // 显式路径 + 帧数（无显式超时 → None，兜底由 RunOptions 提供）。
        let frames_only = Ok(Command::ModelSmoke {
            model3: Some("assets/foo.model3.json".to_owned()),
            frame_target: Some(120),
            timeout: None,
            pet_mode: false,
        });
        assert_eq!(
            parse_args(&args(&[
                "--model-smoke",
                "assets/foo.model3.json",
                "--smoke-frames",
                "120"
            ])),
            frames_only
        );
        // 全参数乱序组合：路径/帧数/超时全部生效（顺序无关）。
        let full = Ok(Command::ModelSmoke {
            model3: Some("assets/foo.model3.json".to_owned()),
            frame_target: Some(120),
            timeout: Some(Duration::from_secs(90)),
            pet_mode: false,
        });
        assert_eq!(
            parse_args(&args(&[
                "--smoke-timeout-secs",
                "90",
                "--smoke-frames",
                "120",
                "--model-smoke",
                "assets/foo.model3.json"
            ])),
            full
        );
        // 以 `-` 开头的下一个参数不被当作路径（仍是纯缺省）。
        assert_eq!(
            parse_args(&args(&["--model-smoke", "--smoke-frames", "5"])),
            Ok(Command::ModelSmoke {
                model3: None,
                frame_target: Some(5),
                timeout: None,
                pet_mode: false
            })
        );
        // --smoke-frames 单独出现保持旧语义：隐含纯窗口壳冒烟。
        assert_eq!(
            parse_args(&args(&["--smoke-frames", "7"])),
            Ok(Command::WindowSmoke {
                frame_target: Some(7),
                timeout: None,
                pet_mode: false
            })
        );
    }

    #[test]
    fn model_smoke_path_resolution_prefers_explicit_then_cwd_then_manifest() {
        let dir = std::env::temp_dir().join(format!("l2d-cli-path-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("repo/Live2D-Ai-pc/x")).unwrap();
        std::fs::write(dir.join("explicit.json"), "{}").unwrap();
        let bai_rel = "Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json";
        std::fs::create_dir_all(dir.join("repo").join(bai_rel).parent().unwrap()).unwrap();
        std::fs::write(dir.join("repo").join(bai_rel), "{}").unwrap();

        // 显式存在 → 原样返回。
        let explicit = dir.join("explicit.json");
        assert_eq!(
            super::resolve_model3_path_in(Some(explicit.to_str().unwrap()), Path::new("."), &dir),
            Ok(explicit)
        );
        // 显式缺失 → 错误并带路径。
        assert!(super::resolve_model3_path_in(Some("nope.json"), Path::new("."), &dir).is_err());
        // 缺省 → cwd 候选命中（cwd_base = repo）。
        assert_eq!(
            super::resolve_model3_path_in(None, &dir.join("repo"), &dir),
            Ok(dir.join("repo").join(bai_rel))
        );
        // cwd 未命中 → manifest 相对候选命中（manifest_dir = repo/crates/desktop 布局；
        // 注意内核逐级解析 ..，中间目录必须真实存在）。
        let manifest_like = dir.join("repo/crates/desktop");
        std::fs::create_dir_all(&manifest_like).unwrap();
        let resolved = super::resolve_model3_path_in(None, &dir.join("nowhere"), &manifest_like)
            .expect("manifest 相对候选应命中");
        // join 不归一化 `..`：以 canonicalize 后的真实文件位置为准。
        assert_eq!(
            resolved.canonicalize().unwrap(),
            dir.join("repo").join(bai_rel).canonicalize().unwrap()
        );
        // 全部未命中 → 错误列出尝试过的候选。
        let err =
            super::resolve_model3_path_in(None, Path::new("/nonexistent-cwd"), &dir).unwrap_err();
        assert!(err.contains("bai.model3.json") && err.contains("--model-smoke"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
