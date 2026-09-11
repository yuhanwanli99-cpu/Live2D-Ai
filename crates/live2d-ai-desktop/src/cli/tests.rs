// cli 拆分测试：17 条，自旧 cli.rs 原样迁出（断言一字未改）

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
    let bai_rel = "assets/models/bai/runtime/bai.model3.json";
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
    let err = super::resolve_model3_path_in(None, Path::new("/nonexistent-cwd"), &dir).unwrap_err();
    assert!(err.contains("bai.model3.json") && err.contains("--model-smoke"));

    std::fs::remove_dir_all(&dir).ok();
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
fn web_is_mutually_exclusive_with_other_modes() {
    // 与 --window-smoke 互斥
    assert!(parse_args(&args(&["--web", "--window-smoke"])).is_err());
    // 与 --model-smoke 互斥
    assert!(parse_args(&args(&["--web", "--model-smoke"])).is_err());
    // 与 --audio-smoke 互斥
    assert!(parse_args(&args(&["--web", "--audio-smoke"])).is_err());
    // 与 --chat 互斥
    assert!(parse_args(&args(&["--web", "--chat"])).is_err());
    // 与 --benchmark 互斥
    assert!(parse_args(&args(&["--web", "--benchmark"])).is_err());
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
    // 非 web 模式（--chat 等）下 --dev-mode 静默忽略，输出仍是 Chat 命令。
    // 故意走「无显式 chat 触发、亦无 web」→ dev_mode 被记住但命令归 Info；
    // 真实场景下用户配合 --web 一同出现（web_alone_with_dev_mode 测试）。
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
