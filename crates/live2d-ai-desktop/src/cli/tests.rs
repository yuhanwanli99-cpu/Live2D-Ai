// cli 解析测试。
//
// 2026-10-01（W2-B / D1 第二段）：原生壳 CLI（`--window-smoke` / `--model-smoke` /
// `--smoke-frames` / `--smoke-timeout-secs` / `--pet-mode` / `--chat` / `--config` /
// `--benchmark*`）随壳删除，对应 8 条测试**随资产冻结退出**（逐条名字见
// `docs/architecture/ARCHIVED-native-shell.md`）；本文件同时**新增**一条
// 「被移除的入口必须报错」的机械钉子，防止它们悄悄回来。

use std::time::Duration;

use super::{Command, parse_args, parse_args_for};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(ToString::to_string).collect()
}

#[test]
fn no_args_means_info_only() {
    assert_eq!(parse_args(&args(&[])), Ok(Command::Info));
}

#[test]
fn help_and_errors() {
    assert!(parse_args(&args(&["--help"])).is_err());
    assert!(parse_args(&args(&["-h"])).is_err());
    assert!(parse_args(&args(&["--bogus"])).is_err());
    // 被移除的入口一律「未知参数」（防回归：不要把它们悄悄加回来）。
    for removed in [
        "--window-smoke",
        "--model-smoke",
        "--smoke-frames",
        "--smoke-timeout-secs",
        "--pet-mode",
        "--chat",
        "--config",
        "--benchmark",
        "--benchmark-mode",
        "--benchmark-warmup",
        "--benchmark-frames",
        "--benchmark-output",
        "--benchmark-headless",
    ] {
        assert!(
            parse_args(&args(&[removed])).is_err(),
            "{removed} 已随原生壳移出，必须报未知参数"
        );
    }
}

/// 被移除入口的防回归钉子（W2-B 新增）：逐个点名 + 断言错误信息是「未知参数」。
#[test]
fn removed_native_shell_flags_are_unknown_parameters_not_silently_ignored() {
    for removed in ["--window-smoke", "--model-smoke", "--chat", "--pet-mode"] {
        let err = parse_args(&args(&[removed])).expect_err("必须报错");
        assert!(
            err.message.contains("未知参数"),
            "{removed} 的错误应指明未知参数，实际: {}",
            err.message
        );
    }
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
fn web_alone_implies_web_mode_with_default_port() {
    assert_eq!(
        parse_args(&args(&["--web"])),
        Ok(Command::Web {
            port: super::DEFAULT_HTTP_PORT,
            dev_mode: false,
        })
    );
}

#[test]
fn http_port_implies_web_and_overrides_default() {
    assert_eq!(
        parse_args(&args(&["--http-port", "19999"])),
        Ok(Command::Web {
            port: 19999,
            dev_mode: false
        })
    );
}

#[test]
fn web_is_mutually_exclusive_with_the_only_remaining_mode() {
    // W2-B 后唯一与 --web 并列的模式是音频冒烟；其余「互斥」由 help_and_errors
    // 的「已移除入口」断言覆盖（它们现在是未知参数）。
    assert!(parse_args(&args(&["--web", "--audio-smoke"])).is_err());
    assert!(parse_args(&args(&["--audio-smoke", "--http-port", "19000"])).is_err());
}

#[test]
fn http_port_bounds_are_enforced() {
    // 0 不合法（u16 解析失败）
    assert!(parse_args(&args(&["--http-port", "0"])).is_err());
    // 字符串非法
    assert!(parse_args(&args(&["--http-port", "abc"])).is_err());
    // < 1024 拒绝
    assert!(parse_args(&args(&["--http-port", "80"])).is_err());
    // 1024 通过
    assert_eq!(
        parse_args(&args(&["--http-port", "1024"])),
        Ok(Command::Web {
            port: 1024,
            dev_mode: false
        })
    );
    // 65535 通过
    assert_eq!(
        parse_args(&args(&["--http-port", "65535"])),
        Ok(Command::Web {
            port: 65535,
            dev_mode: false
        })
    );
    // 65536 失败（超 u16）
    assert!(parse_args(&args(&["--http-port", "65536"])).is_err());
}

// ===== W7 任务：--dev-mode CLI 覆盖 =====

#[test]
fn dev_mode_flag_alone_is_ignored_outside_web_mode() {
    // 非 web 模式下 --dev-mode 静默忽略，命令归 Info。
    let cmd = parse_args(&args(&["--dev-mode"])).expect("parse");
    assert_eq!(cmd, Command::Info, "--dev-mode 单独出现 = Info（无 web）");
}

#[test]
fn web_alone_with_dev_mode_sets_dev_mode_true() {
    // --web + --dev-mode → dev_mode: true（CLI 覆盖，优先级最高）。
    assert_eq!(
        parse_args(&args(&["--web", "--dev-mode"])),
        Ok(Command::Web {
            port: super::DEFAULT_HTTP_PORT,
            dev_mode: true,
        })
    );
}

#[test]
fn http_port_with_dev_mode_sets_both() {
    // --http-port + --dev-mode → 端口生效 + dev_mode: true。
    assert_eq!(
        parse_args(&args(&["--http-port", "19999", "--dev-mode"])),
        Ok(Command::Web {
            port: 19999,
            dev_mode: true,
        })
    );
    // 顺序无关：--dev-mode 在前也工作。
    assert_eq!(
        parse_args(&args(&["--dev-mode", "--http-port", "19999"])),
        Ok(Command::Web {
            port: 19999,
            dev_mode: true,
        })
    );
}

#[test]
fn dev_mode_can_be_repeated_and_stays_true() {
    // --dev-mode 重复出现仍是 true（idempotent；无错误）。
    assert_eq!(
        parse_args(&args(&["--web", "--dev-mode", "--dev-mode"])),
        Ok(Command::Web {
            port: super::DEFAULT_HTTP_PORT,
            dev_mode: true,
        })
    );
}

#[test]
fn flags_are_order_independent_and_combinable() {
    // 幸存的组合：--audio-smoke-secs + --audio-smoke-silence 与顺序无关。
    let a = parse_args(&args(&[
        "--audio-smoke-silence",
        "--audio-smoke-secs",
        "3",
        "--audio-smoke",
    ]));
    let b = parse_args(&args(&[
        "--audio-smoke",
        "--audio-smoke-secs",
        "3",
        "--audio-smoke-silence",
    ]));
    assert_eq!(a, b);
    assert_eq!(
        a,
        Ok(Command::AudioSmoke {
            duration: Duration::from_secs(3),
            silence: true,
        })
    );
    // --web 与 --http-port/--dev-mode 组合亦与顺序无关。
    assert_eq!(
        parse_args(&args(&["--web", "--http-port", "18081", "--dev-mode"])),
        parse_args(&args(&["--dev-mode", "--http-port", "18081", "--web"])),
    );
}

// ===== 0.4.0 rc.1：Windows 包「双击即用」的缺省命令 =====

/// 纯函数（按 `std::env::consts::OS` 的字符串）钉住两侧语义：
/// Windows → Web 18080（随后自开浏览器）；其它目标 → Info。
#[test]
fn default_command_is_web_only_for_windows() {
    assert_eq!(
        super::default_command("windows", false),
        Command::Web {
            port: super::DEFAULT_HTTP_PORT,
            dev_mode: false,
        }
    );
    assert_eq!(
        super::default_command("windows", true),
        Command::Web {
            port: super::DEFAULT_HTTP_PORT,
            dev_mode: true,
        }
    );
    assert_eq!(super::default_command("linux", false), Command::Info);
    assert_eq!(super::default_command("macos", true), Command::Info);
    assert_eq!(
        super::DEFAULT_HTTP_PORT,
        18080,
        "包里写死的是 127.0.0.1:18080"
    );
    // 本机（Linux）无参数仍是 Info：`parse_args` 的缺省分支读同一常量。
    assert_eq!(parse_args(&args(&[])), Ok(Command::Info));
}

// ===== 0.4.3 rc.1：--desk-pet（隐含 --web；仅 Windows） =====

/// 纯函数两侧语义：Windows 上 `--desk-pet` = DeskPet，且**隐含** web（缺省端口）。
#[test]
fn desk_pet_implies_web_on_windows_with_default_port() {
    assert_eq!(
        parse_args_for("windows", &args(&["--desk-pet"])),
        Ok(Command::DeskPet {
            port: 18080,
            dev_mode: false,
        })
    );
}

/// 与 `--http-port` / `--dev-mode` 可组合；显式 `--web` 不改变结果（顺序无关）。
#[test]
fn desk_pet_composes_with_http_port_dev_mode_and_web() {
    assert_eq!(
        parse_args_for(
            "windows",
            &args(&["--desk-pet", "--http-port", "19000", "--dev-mode"])
        ),
        Ok(Command::DeskPet {
            port: 19000,
            dev_mode: true,
        })
    );
    assert_eq!(
        parse_args_for("windows", &args(&["--desk-pet", "--web"])),
        Ok(Command::DeskPet {
            port: 18080,
            dev_mode: false,
        })
    );
    assert_eq!(
        parse_args_for("windows", &args(&["--web", "--desk-pet", "--dev-mode"])),
        Ok(Command::DeskPet {
            port: 18080,
            dev_mode: true,
        })
    );
}

/// 非 Windows：**用法错误**（退出码 2 那条路），中文含「仅 Windows」并说明当前
/// 目标；消息**不是** "__help__"（否则 main.rs 会按 help 返回 0）。
#[test]
fn desk_pet_is_rejected_on_non_windows() {
    for os in ["linux", "macos"] {
        let err = parse_args_for(os, &args(&["--desk-pet"]))
            .expect_err("非 Windows 的 --desk-pet 必须是用法错误");
        assert!(
            err.message.contains("仅 Windows"),
            "{os} 的错误消息必须含「仅 Windows」，实际: {}",
            err.message
        );
        assert!(
            err.message.contains(os),
            "{os} 的错误应说明当前目标，实际: {}",
            err.message
        );
        assert_ne!(
            err.message, "__help__",
            "必须是用法错误（退出码 2），不是 help（退出码 0）"
        );
    }
}

/// 真机锁（Linux / macOS）：不注入 OS，直接走 `parse_args`（读
/// `std::env::consts::OS`）也必须被拒绝——证明 wrapper 真把目标 OS 传下去了。
#[cfg(not(windows))]
#[test]
fn desk_pet_is_rejected_by_parse_args_on_this_host() {
    let err =
        parse_args(&args(&["--desk-pet"])).expect_err("本机（非 Windows）必须拒绝 --desk-pet");
    assert!(err.message.contains("仅 Windows"), "{}", err.message);
    assert_ne!(err.message, "__help__");
}

/// `desk_pet_supported` 只认 Windows（macOS / Linux 都 false）。
#[test]
fn desk_pet_supported_is_windows_only() {
    assert!(super::desk_pet_supported("windows"));
    assert!(!super::desk_pet_supported("linux"));
    assert!(!super::desk_pet_supported("macos"));
}

/// `--desk-pet` 隐含 `--web` ⇒ 与 `--audio-smoke` 仍走现有互斥错误
/// （互斥检查在「非 Windows 拒绝」之前；两种顺序都拒绝）。
#[test]
fn desk_pet_is_mutually_exclusive_with_audio_smoke() {
    let err = parse_args_for("windows", &args(&["--desk-pet", "--audio-smoke"]))
        .expect_err("--desk-pet 与 --audio-smoke 必须互斥");
    assert!(err.message.contains("互斥"), "{}", err.message);
    assert!(
        parse_args_for("linux", &args(&["--audio-smoke", "--desk-pet"])).is_err(),
        "顺序无关：非 Windows 上同样先命中互斥"
    );
}

/// Windows 双击（无参数）仍是 Web 18080，**不开**宠物窗（0.4.3 不改缺省命令）。
#[test]
fn default_command_on_windows_stays_web_not_desk_pet() {
    assert_eq!(
        super::default_command("windows", false),
        Command::Web {
            port: 18080,
            dev_mode: false,
        }
    );
    assert_eq!(
        parse_args_for("windows", &args(&[])),
        Ok(Command::Web {
            port: 18080,
            dev_mode: false,
        })
    );
}

/// 用法文本必须出现 `--desk-pet`，并写明仅 Windows / 非 Windows 退出码 2。
#[test]
fn usage_mentions_desk_pet() {
    assert!(
        super::USAGE.contains("--desk-pet"),
        "USAGE 必须说明 --desk-pet"
    );
    assert!(
        super::USAGE.contains("仅 Windows"),
        "USAGE 必须写明仅 Windows"
    );
    assert!(
        super::USAGE.contains("退出码 2"),
        "USAGE 必须写明非 Windows 退出码 2"
    );
}
