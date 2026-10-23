//! `peer_pi` / `peer_pi_parse` 单测（0.4.5 rc.1，Unix）。
//!
//! 全部用**假二进制**（临时目录里的 `#!/bin/sh` 脚本）：真 `pi` 不进单测、
//! 不碰网络、不调模型。超时测试注入 200ms，断言**整组**（脚本自己 `sleep &` 的
//! 孙进程）都被结束。
//!
//! 覆盖：argv 原文、会话目录 0700 与复用、一次性目录清理、env 白名单、PATH
//! 查找、缺失 / 非 0 / 坏 JSON / error stop / 超时（整组结束）映射。
//! 纯解析用例在 `peer_pi_parse_tests.rs`（不启动进程）。

use super::*;
use crate::web_api::peer_pi_spawn::{find_executable, whitelisted_env};
use std::ffi::OsStr;
use std::path::Path;
use std::time::{Duration, Instant};

// =====================================================================
// 夹具
// =====================================================================

/// 临时根：drop 时整目录删掉（不在 /tmp 留垃圾）。
struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "live2d-ai-peer-test-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("建临时目录");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn write_script(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("写假二进制");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod 755");
    path
}

fn run_config(program: PathBuf, root: &Path, timeout: Duration) -> RunConfig {
    RunConfig {
        program: ProgramSource::Explicit(program),
        path_env: OsString::new(),
        timeout,
        root: root.to_path_buf(),
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("目录存在")
        .permissions()
        .mode()
        & 0o777
}

/// 录好的 stdout（形状按 `pi --mode json`：header + 事件流）。
const FIXTURE_OK: &str = r#"{"type":"session","version":3,"id":"s-1","timestamp":"2026-10-23T00:00:00.000Z","cwd":"/tmp"}
{"type":"message_start","message":{"role":"user","content":"你好","timestamp":1}}
{"type":"message_update","usage":{},"assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"你"}}
{"type":"message_end","message":{"role":"assistant","api":"openai","provider":"p","model":"m","usage":{},"stopReason":"stop","timestamp":2,"content":[{"type":"thinking","thinking":"这句是内心"},{"type":"text","text":"只有这句"},{"type":"toolCall","id":"t1","name":"bash","arguments":{}}]}}
{"type":"agent_end","messages":[],"willRetry":false}
"#;

// =====================================================================
// 纯函数契约：session id / argv / env / PATH 查找
// =====================================================================

#[test]
fn session_id_contract_is_exact() {
    for ok in ["", "a", "Z9", "a_b-c", "0123456789abcdef"] {
        assert!(validate_session_id(ok), "应接受 {ok:?}");
    }
    let longest = "a".repeat(MAX_SESSION_ID_LEN);
    assert!(validate_session_id(&longest), "64 字符应接受");
    let too_long = "a".repeat(MAX_SESSION_ID_LEN + 1);
    for bad in [
        "a/b",
        "a.b",
        "a b",
        "中文",
        "a@b",
        "a\u{0}b",
        too_long.as_str(),
    ] {
        assert!(!validate_session_id(bad), "应拒绝 {bad:?}");
    }
}

#[test]
fn build_argv_is_exactly_the_order_contract() {
    let dir = Path::new("/tmp/peer-session");
    let with_session: Vec<String> = build_argv("原文", "sess-1", dir)
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        with_session,
        vec![
            "--print",
            "--mode",
            "json",
            "--no-extensions",
            "--no-tools",
            "--no-skills",
            "--no-prompt-templates",
            "--no-context-files",
            "--thinking",
            "off",
            "--session-dir",
            "/tmp/peer-session",
            "--session-id",
            "sess-1",
            "--",
            "原文",
        ]
    );
    let ephemeral: Vec<String> = build_argv("原文", "", dir)
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        ephemeral,
        vec![
            "--print",
            "--mode",
            "json",
            "--no-extensions",
            "--no-tools",
            "--no-skills",
            "--no-prompt-templates",
            "--no-context-files",
            "--thinking",
            "off",
            "--no-session",
            "--",
            "原文",
        ]
    );
}

#[test]
fn prompt_arg_only_shields_a_leading_at() {
    // pi 在 `--` 之后仍把 `@…` 当 @file（args.ts 83-90），会丢掉正文。
    assert_eq!(prompt_arg("@file 你好"), " @file 你好");
    assert_eq!(prompt_arg("@"), " @");
    // 其它位置 / 其它文本一字不动。
    assert_eq!(prompt_arg("你好 @file"), "你好 @file");
    assert_eq!(prompt_arg("地址 a@b.com"), "地址 a@b.com");
    assert_eq!(prompt_arg("  already"), "  already");
    let last = build_argv("@file 你好", "", Path::new("/tmp/x"))
        .pop()
        .unwrap();
    assert_eq!(last.to_string_lossy().into_owned(), " @file 你好");
}

#[test]
fn env_whitelist_keeps_only_allowed_keys() {
    let parent = [
        ("PATH", "/usr/bin"),
        ("HOME", "/home/someone"),
        ("LANG", "C.UTF-8"),
        ("LC_ALL", "C"),
        ("USER", "someone"),
        ("DEEPSEEK_API_KEY", "sk-secret"),
        ("https_proxy", "http://proxy"),
        ("CARGO_HOME", "/cargo"),
    ];
    let env = whitelisted_env(|key| {
        parent
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| OsString::from(v))
    });
    let keys: Vec<String> = env
        .iter()
        .map(|(k, _)| k.to_string_lossy().into_owned())
        .collect();
    assert_eq!(keys, vec!["PATH", "HOME", "LANG", "LC_ALL", "USER"]);
    // 父进程没有 `USER` 时不补默认值。
    let without_user = whitelisted_env(|_| None);
    assert!(without_user.is_empty());
}

#[test]
fn find_executable_requires_exec_bit() {
    let tmp = TempRoot::new("find-exe");
    let bin = tmp.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let fake = bin.join("pi");
    std::fs::write(&fake, "#!/bin/sh\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o644)).unwrap();
    let path_env = bin.as_os_str().to_owned();
    assert_eq!(find_executable("pi", &path_env), None, "644 不是可执行");
    // 目录也算「有路径」但不是可执行文件。
    let dir_as_pi = bin.join("dir-pi");
    std::fs::create_dir_all(&dir_as_pi).unwrap();
    assert_eq!(find_executable("dir-pi", &path_env), None);
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(find_executable("pi", &path_env), Some(fake));
    assert_eq!(find_executable("pi", OsStr::new("")), None);
}

// =====================================================================
// 全链路（假二进制）：成功 / 目录 / env / 失败映射 / 超时
// =====================================================================

#[test]
fn successful_run_returns_assistant_text_with_contract_dirs_and_env() {
    let tmp = TempRoot::new("success");
    let root = tmp.path().join("root");
    let marker = tmp.path().join("marker");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("stdout.json"), FIXTURE_OK).unwrap();
    let m = marker.display();
    let fake = write_script(
        tmp.path(),
        "pi",
        &format!(
            "echo \"$@\" > '{m}/argv.txt'\npwd > '{m}/cwd.txt'\nenv > '{m}/env.txt'\ncat '{m}/stdout.json'"
        ),
    );
    let outcome = run(
        &run_config(fake, &root, Duration::from_secs(30)),
        "只问一句",
        "sess-1",
    );
    assert_eq!(outcome, PiRunOutcome::Reply("只有这句".to_string()));

    let session_dir = root.join("sessions").join("sess-1");
    assert_eq!(
        read(&marker.join("cwd.txt")).trim(),
        session_dir.display().to_string()
    );
    assert_eq!(mode_of(&session_dir), 0o700, "会话目录必须是 0700");

    let expected_argv = build_argv("只问一句", "sess-1", &session_dir)
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        read(&marker.join("argv.txt")).trim(),
        expected_argv,
        "argv 逐字"
    );

    // env 白名单：只允许 5 个键（+ dash 自己加的 PWD/SHLVL/_/OLDPWD）。
    let env_text = read(&marker.join("env.txt"));
    let allowed = [
        "PATH", "HOME", "LANG", "LC_ALL", "USER", "PWD", "SHLVL", "_", "OLDPWD",
    ];
    for line in env_text.lines() {
        let key = line.split('=').next().unwrap_or("");
        assert!(allowed.contains(&key), "子进程环境出现白名单外的键: {key}");
        let upper = key.to_ascii_uppercase();
        assert!(!upper.contains("PROXY"), "代理变量不得进子进程: {key}");
        assert!(!key.ends_with("_API_KEY"), "密钥变量不得进子进程: {key}");
    }
    if let Ok(path) = std::env::var("PATH") {
        assert!(
            env_text.lines().any(|l| l == format!("PATH={path}")),
            "PATH 必须原样保留"
        );
    }
}

#[test]
fn stderr_never_leaks_into_the_reply() {
    let tmp = TempRoot::new("stderr");
    let root = tmp.path().join("root");
    let marker = tmp.path().join("marker");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(
        marker.join("stdout.json"),
        r#"{"type":"message_end","message":{"role":"assistant","stopReason":"stop","content":[{"type":"text","text":"干净正文"}]}}"#,
    )
    .unwrap();
    let m = marker.display();
    let fake = write_script(
        tmp.path(),
        "pi",
        &format!("echo 'Warning: No project session found' >&2\ncat '{m}/stdout.json'"),
    );
    assert_eq!(
        run(&run_config(fake, &root, Duration::from_secs(30)), "问", "s"),
        PiRunOutcome::Reply("干净正文".to_string())
    );
}

#[test]
fn ephemeral_run_uses_no_session_flags_and_removes_workdir() {
    let tmp = TempRoot::new("ephemeral");
    let root = tmp.path().join("root");
    let marker = tmp.path().join("marker");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("stdout.json"), FIXTURE_OK).unwrap();
    let m = marker.display();
    let fake = write_script(
        tmp.path(),
        "pi",
        &format!("echo \"$@\" > '{m}/argv.txt'\npwd > '{m}/cwd.txt'\ncat '{m}/stdout.json'"),
    );
    let outcome = run(
        &run_config(fake, &root, Duration::from_secs(30)),
        "只问一句",
        "",
    );
    assert_eq!(outcome, PiRunOutcome::Reply("只有这句".to_string()));
    let argv = read(&marker.join("argv.txt"));
    assert!(
        argv.contains("--no-session"),
        "空会话必须 --no-session: {argv}"
    );
    assert!(
        !argv.contains("--session-dir"),
        "空会话不得带 --session-dir: {argv}"
    );
    assert!(
        !argv.contains("--session-id"),
        "空会话不得带 --session-id: {argv}"
    );
    let cwd = PathBuf::from(read(&marker.join("cwd.txt")).trim());
    assert!(
        cwd.starts_with(root.join("tmp")),
        "一次性目录应在 root/tmp 下: {cwd:?}"
    );
    assert!(!cwd.exists(), "一次性目录跑完必须删除: {cwd:?}");
}

#[test]
fn session_dir_is_reused_across_runs() {
    let tmp = TempRoot::new("reuse");
    let root = tmp.path().join("root");
    let marker = tmp.path().join("marker");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("stdout.json"), FIXTURE_OK).unwrap();
    let m = marker.display();
    let fake = write_script(
        tmp.path(),
        "pi",
        &format!("pwd >> '{m}/cwd.txt'\ncat '{m}/stdout.json'"),
    );
    let config = run_config(fake, &root, Duration::from_secs(30));
    assert!(matches!(
        run(&config, "第一条", "same-id"),
        PiRunOutcome::Reply(_)
    ));
    assert!(matches!(
        run(&config, "第二条", "same-id"),
        PiRunOutcome::Reply(_)
    ));
    let cwd_text = read(&marker.join("cwd.txt"));
    let lines: Vec<&str> = cwd_text.lines().map(str::trim).collect();
    assert_eq!(lines.len(), 2, "两次运行都应记录 cwd: {lines:?}");
    assert_eq!(lines[0], lines[1], "同一 session_id 必须复用同一目录");
    let session_dir = root.join("sessions").join("same-id");
    assert_eq!(Path::new(lines[0]), session_dir.as_path());
    assert_eq!(mode_of(&session_dir), 0o700);
}

#[test]
fn missing_binary_maps_to_missing() {
    let tmp = TempRoot::new("missing");
    let root = tmp.path().join("root");
    // 显式路径不存在。
    let config = run_config(tmp.path().join("no-such-pi"), &root, Duration::from_secs(5));
    assert_eq!(run(&config, "问", ""), PiRunOutcome::Missing);
    // PATH 里没有 pi（空目录）。
    let empty = tmp.path().join("empty-bin");
    std::fs::create_dir_all(&empty).unwrap();
    let path_config = RunConfig {
        program: ProgramSource::Path,
        path_env: empty.as_os_str().to_owned(),
        timeout: Duration::from_secs(5),
        root,
    };
    assert_eq!(run(&path_config, "问", ""), PiRunOutcome::Missing);
}

#[test]
fn nonzero_exit_maps_to_failed_even_with_valid_stdout() {
    let tmp = TempRoot::new("exit");
    let root = tmp.path().join("root");
    let marker = tmp.path().join("marker");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(marker.join("stdout.json"), FIXTURE_OK).unwrap();
    let m = marker.display();
    let fake = write_script(tmp.path(), "pi", &format!("cat '{m}/stdout.json'\nexit 3"));
    assert_eq!(
        run(&run_config(fake, &root, Duration::from_secs(30)), "问", ""),
        PiRunOutcome::Failed("exit_nonzero")
    );
}

#[test]
fn bad_stdout_maps_to_failed() {
    let tmp = TempRoot::new("badstdout");
    let root = tmp.path().join("root");
    let fake = write_script(tmp.path(), "pi", "echo 'not the agreed json'");
    assert_eq!(
        run(&run_config(fake, &root, Duration::from_secs(30)), "问", ""),
        PiRunOutcome::Failed("no_assistant_message")
    );
}

#[test]
fn assistant_error_stop_maps_to_failed() {
    let tmp = TempRoot::new("stop");
    let root = tmp.path().join("root");
    let marker = tmp.path().join("marker");
    std::fs::create_dir_all(&marker).unwrap();
    std::fs::write(
        marker.join("stdout.json"),
        r#"{"type":"message_end","message":{"role":"assistant","stopReason":"error","errorMessage":"No models available","content":[{"type":"text","text":"半句"}]}}"#,
    )
    .unwrap();
    let m = marker.display();
    let fake = write_script(tmp.path(), "pi", &format!("cat '{m}/stdout.json'"));
    assert_eq!(
        run(&run_config(fake, &root, Duration::from_secs(30)), "问", ""),
        PiRunOutcome::Failed("assistant_error_stop")
    );
}

#[cfg(target_os = "linux")]
#[test]
fn timeout_kills_the_whole_process_group() {
    let tmp = TempRoot::new("timeout");
    let root = tmp.path().join("root");
    let marker = tmp.path().join("marker");
    std::fs::create_dir_all(&marker).unwrap();
    let m = marker.display();
    let fake = write_script(
        tmp.path(),
        "pi",
        &format!("echo $$ > '{m}/leader.pid'\nsleep 30 &\necho $! > '{m}/sleep.pid'\nwait"),
    );
    let started = Instant::now();
    let outcome = run(
        &run_config(fake, &root, Duration::from_millis(200)),
        "问",
        "",
    );
    assert_eq!(outcome, PiRunOutcome::Timeout);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "超时必须按注入时长返回，不能等 30s"
    );
    let leader: u32 = read(&marker.join("leader.pid")).trim().parse().unwrap();
    let sleeper: u32 = read(&marker.join("sleep.pid")).trim().parse().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let leader_alive = Path::new("/proc").join(leader.to_string()).exists();
        let sleeper_alive = Path::new("/proc").join(sleeper.to_string()).exists();
        if !leader_alive && !sleeper_alive {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "整组未被结束：leader_alive={leader_alive} sleeper_alive={sleeper_alive}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
