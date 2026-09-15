//! `sidecar` 的纯函数回归（L1 产品级，2026-09-15）。
//!
//! 从 `tests.rs` 拆出：crate 测试文件硬上限 800 行。挂载见 `sidecar.rs` 末尾。

use super::*;

// ---------------------------------------------------- sidecar argv / 校验（L1）

#[test]
fn build_sidecar_argv_orders_args_and_never_shells_out() {
    let argv = build_sidecar_argv(
        "python3",
        "/repo/docs/examples/voice-sidecar/voice_sidecar.py",
        "/tmp/a.wav",
        "http://127.0.0.1:18080/api/v1/voice/transcript",
        Some("tok"),
        "fake",
    );
    assert_eq!(
        argv,
        vec![
            "python3",
            "/repo/docs/examples/voice-sidecar/voice_sidecar.py",
            "--audio",
            "/tmp/a.wav",
            "--url",
            "http://127.0.0.1:18080/api/v1/voice/transcript",
            "--token",
            "tok",
            "--transcriber",
            "fake",
        ]
    );
    assert!(
        !argv.iter().any(|a| a == "-c" || a == "sh" || a == "-lc"),
        "绝不走 sh -c: {argv:?}"
    );
}

#[test]
fn build_sidecar_argv_skips_blank_token_and_keeps_transcriber_as_one_arg() {
    let argv = build_sidecar_argv(
        "py",
        "s.py",
        "a.wav",
        "u",
        None,
        "cmd:\"whisper --model small\"",
    );
    assert!(!argv.contains(&"--token".to_string()));
    let idx = argv
        .iter()
        .position(|a| a == "--transcriber")
        .expect("--transcriber 必须在");
    assert_eq!(
        argv[idx + 1],
        "cmd:\"whisper --model small\"",
        "transcriber 必须整体是一个 argv 元素"
    );
    let blank = build_sidecar_argv("py", "s.py", "a.wav", "u", Some("   "), "fake");
    assert!(!blank.contains(&"--token".to_string()), "纯空白 token 不传");
}

#[test]
fn validate_audio_path_rejects_missing_and_non_file() {
    let err = validate_audio_path("   ").unwrap_err();
    assert!(err.contains("audio_path"), "{err}");
    let missing = validate_audio_path("/definitely/not/here.wav").unwrap_err();
    assert!(
        missing.contains("/definitely/not/here.wav"),
        "错误必须带路径: {missing}"
    );
    let dir = std::env::temp_dir();
    let err = validate_audio_path(&dir.to_string_lossy()).unwrap_err();
    assert!(err.contains("不是普通文件"), "{err}");
    // 真 fixture 通过。
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/examples/voice-sidecar/fixtures/fake_zh.wav");
    assert!(
        validate_audio_path(&fixture.to_string_lossy()).is_ok(),
        "fixture 必须存在：{}",
        fixture.display()
    );
}

#[test]
fn resolve_sidecar_script_prefers_config_then_config_dir() {
    assert_eq!(resolve_sidecar_script(&serde_json::json!({}), ""), "");
    assert_eq!(
        resolve_sidecar_script(&serde_json::json!({}), "/repo/live2d-ai.toml"),
        "/repo/docs/examples/voice-sidecar/voice_sidecar.py"
    );
    assert_eq!(
        resolve_sidecar_script(
            &serde_json::json!({"sidecar_script": "/custom/s.py"}),
            "/repo/live2d-ai.toml"
        ),
        "/custom/s.py",
        "配置优先"
    );
}

#[test]
fn stderr_tail_keeps_only_the_end() {
    assert_eq!(stderr_tail("abc", 10), "abc");
    assert_eq!(stderr_tail("abcdef", 3), "…def");
    assert_eq!(
        stderr_tail("  x  ", 10),
        "  x",
        "尾部空白被裁掉（保留前导缩进）"
    );
    assert_eq!(stderr_tail("abc", 0), "");
}

#[test]
fn sidecar_status_transitions_match_state_json_contract() {
    let mut st = SidecarStatus::default();
    assert_eq!(st.to_json()["state"], serde_json::json!("idle"));
    assert_eq!(st.to_json()["exit_code"], serde_json::json!(null));
    st.record_spawn(42, "/tmp/a.wav");
    let running = st.to_json();
    assert_eq!(running["state"], serde_json::json!("running"));
    assert_eq!(running["pid"], serde_json::json!(42));
    assert_eq!(running["last_audio"], serde_json::json!("/tmp/a.wav"));
    st.record_finish(Some(0), "warn: x\n");
    let exited = st.to_json();
    assert_eq!(exited["state"], serde_json::json!("exited"));
    assert_eq!(exited["exit_code"], serde_json::json!(0));
    assert!(exited["finished_at"].as_u64().is_some());
    assert!(exited["stderr_tail"].as_str().unwrap().contains("warn"));
    st.record_spawn_failed("/tmp/b.wav", "python3 不存在");
    let failed = st.to_json();
    assert_eq!(failed["state"], serde_json::json!("spawn_failed"));
    assert_eq!(failed["exit_code"], serde_json::json!(null));
    assert_eq!(failed["last_audio"], serde_json::json!("/tmp/b.wav"));
}
