//! 单元测试（二）：命令 / 关闭 / 快照 / 身份与边界。
//!
//! 共享假件在 tests_support；拉起时机与 argv 在 tests。

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::tests_support::*;

// ─────────────────────────────────────────────────────────── 命令与关闭

#[test]
fn launch_refuses_a_second_child_and_stop_needs_one() {
    let log = Arc::new(SpawnerLog::default());
    // on_apply：Boot 不拉起，留给下面的 command(launch) 走第一次。
    let (mut rt, _) = runtime_with(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "on_apply" }),
        StartCause::Boot,
        &log,
    );
    start(&mut rt).unwrap();

    let err = rt
        .command("stop", &serde_json::json!({}))
        .expect_err("没有子进程时 stop 必须 Err");
    assert!(matches!(err, ModError::Other(_)), "got {err:?}");

    rt.command("launch", &serde_json::json!({}))
        .expect("首次 launch 应成功");
    assert_eq!(log.calls(), 1);
    let err = rt
        .command("launch", &serde_json::json!({}))
        .expect_err("已有子进程时 launch 必须 Err（禁止起第二个）");
    assert!(matches!(err, ModError::Other(_)), "got {err:?}");
    assert_eq!(log.calls(), 1, "失败的重试不得再起一个进程");
}

#[test]
fn unknown_command_is_unsupported() {
    let log = Arc::new(SpawnerLog::default());
    let (mut rt, _) = runtime_with(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "on_apply" }),
        StartCause::Boot,
        &log,
    );
    let err = rt
        .command("restart", &serde_json::json!({}))
        .expect_err("未知命令必须报 UnsupportedCommand");
    assert_eq!(
        err,
        ModError::UnsupportedCommand {
            command: "restart".to_string()
        }
    );
}

#[test]
fn shutdown_stops_and_reaps_the_child() {
    let log = Arc::new(SpawnerLog::default());
    let (mut rt, _) = runtime_with(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
    );
    start(&mut rt).unwrap();
    rt.shutdown().expect("正常关闭应成功");
    assert_eq!(log.with_state(|s| s.stops), 1, "shutdown 必须停子进程");
    assert_eq!(log.with_state(|s| s.waits), 1, "停完必须收尸（wait）");
}

#[test]
fn shutdown_keeps_the_handle_when_stop_fails() {
    let log = Arc::new(SpawnerLog::default());
    let (mut rt, _) = runtime_with(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
    );
    start(&mut rt).unwrap();
    log.with_state(|s| s.stop_error = true);

    let err = rt.shutdown().expect_err("停止失败必须返回 Err（不许吞）");
    assert!(err.to_string().contains("kill 失败"), "got {err}");
    let snap = rt.state_json().unwrap();
    assert_eq!(
        snap["pid"],
        serde_json::json!(4242),
        "句柄必须放回运行时结构"
    );
    assert_eq!(
        snap["child_running"],
        serde_json::json!(true),
        "没停掉就得如实说还在跑"
    );
    assert_eq!(log.with_state(|s| s.waits), 0, "停止失败不该接着 wait");
}

// ─────────────────────────────────────────────────────────── 快照

#[test]
fn snapshot_reports_exit_code_and_wait_error_honestly() {
    // 子进程自己退出：如实报退出码。
    let log = Arc::new(SpawnerLog::default());
    let (mut rt, _) = runtime_with(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
    );
    start(&mut rt).unwrap();
    log.with_state(|s| {
        s.running = false;
        s.exit_code = Some(7);
    });
    let snap = rt.state_json().unwrap();
    assert_eq!(snap["child_running"], serde_json::json!(false));
    assert_eq!(snap["exit_code"], serde_json::json!(7));
    assert_eq!(
        snap["mod_id"],
        serde_json::json!("local-tts"),
        "两个引擎共用快照：靠 mod_id 区分是谁"
    );
    assert!(
        snap.get("wait_error").is_none(),
        "正常观察不该有 wait_error"
    );

    // try_wait 自己出错：错误进 wait_error，且**不得**写成「在跑」。
    let log2 = Arc::new(SpawnerLog::default());
    let (mut rt2, _) = runtime_with(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log2,
    );
    start(&mut rt2).unwrap();
    log2.with_state(|s| s.probe_error = true);
    let snap2 = rt2.state_json().unwrap();
    assert_eq!(snap2["child_running"], serde_json::json!(false));
    assert!(
        snap2["wait_error"]
            .as_str()
            .is_some_and(|s| s.contains("probe 失败")),
        "got {snap2}"
    );
    assert_eq!(log2.calls(), 1, "快照里绝不许再 spawn");
}

// ─────────────────────────────────────────────────────────── 边界与身份

#[test]
fn apply_settings_is_never_called_on_any_path() {
    let log = Arc::new(SpawnerLog::default());
    let (mut rt, applied) = runtime_with(
        serde_json::json!({ "program": "/bin/echo", "launch_mode": "with_app" }),
        StartCause::Boot,
        &log,
    );
    start(&mut rt).unwrap();
    rt.state_json();
    rt.command("launch", &serde_json::json!({})).ok();
    rt.command("stop", &serde_json::json!({})).ok();
    rt.command("nope", &serde_json::json!({})).ok();
    rt.shutdown().unwrap();
    assert_eq!(
        applied.load(Ordering::SeqCst),
        0,
        "local-tts 对 [tts] 的写次数必须是 0（含经由 apply_settings）"
    );
}

/// 界面名用计划书定下的**原文**（中文、下划线、点都不进 id）。
#[test]
fn descriptors_use_the_agreed_names_and_ascii_ids() {
    assert_eq!(DESCRIPTOR.id, "local-tts");
    assert_eq!(DESCRIPTOR.name, "本地tts_CosyVoice3-0.5B");
    assert_eq!(MELO_DESCRIPTOR.id, "local-tts-melo");
    assert_eq!(MELO_DESCRIPTOR.name, "本地tts_MeloTTS");
    for id in [DESCRIPTOR.id, MELO_DESCRIPTOR.id] {
        assert!(id.is_ascii(), "id 必须是 ASCII：{id}");
        assert!(
            !id.contains('_'),
            "id 不得带下划线（crate 名映射约定）：{id}"
        );
    }
    // 两个 Mod 是**两个**身份，不是一个 Mod 的两个别名。
    assert_ne!(DESCRIPTOR.id, MELO_DESCRIPTOR.id);
}

#[test]
fn descriptor_api_version_and_spec_shape() {
    assert_eq!(DESCRIPTOR.api_version, MOD_API_VERSION);

    let spec = FACTORY
        .settings_spec()
        .expect("缺省停用的 Mod 必须静态提供 schema");
    assert_eq!(spec.mod_id, "local-tts");
    assert_eq!(spec.title, "本地tts_CosyVoice3-0.5B", "标题 = 描述符名");
    spec.validate().expect("字段 key 必须唯一");
    assert_eq!(
        spec.fields.iter().map(|f| f.key()).collect::<Vec<_>>(),
        vec!["launch_mode", "program", "args_json", "workdir", "base_url"]
    );
    match &spec.fields[0] {
        ModSettingField::Select { default, .. } => {
            assert_eq!(
                default.as_deref(),
                Some("with_app"),
                "2026-10-09 起缺省拉起方式必须是 with_app"
            );
        }
        other => panic!("launch_mode 应是 Select，got {other:?}"),
    }
    match &spec.fields[1] {
        ModSettingField::String { default, .. } => {
            assert_eq!(
                default.as_deref(),
                Some(COSYVOICE_PROGRAM),
                "program 缺省必须指向仓库内脚本"
            );
        }
        other => panic!("program 应是 String，got {other:?}"),
    }

    // start 注册的 schema 与静态的是同一份（前端先填后启用不会看到两套）。
    let log = Arc::new(SpawnerLog::default());
    let (mut rt, _) = runtime_with(
        serde_json::json!({ "launch_mode": "on_apply" }),
        StartCause::Boot,
        &log,
    );
    let mut reg = RecordingRegistrar::default();
    rt.start(&mut reg).unwrap();
    assert_eq!(reg.specs.len(), 1);
    assert_eq!(reg.specs[0], spec);
}

#[test]
fn melo_factory_identity_and_spec_shape() {
    assert_eq!(MELO_DESCRIPTOR.api_version, MOD_API_VERSION);
    let spec = MELO_FACTORY
        .settings_spec()
        .expect("缺省停用的 Mod 必须静态提供 schema");
    assert_eq!(spec.mod_id, "local-tts-melo");
    assert_eq!(spec.title, "本地tts_MeloTTS");
    spec.validate().expect("字段 key 必须唯一");
    assert_eq!(
        spec.fields.iter().map(|f| f.key()).collect::<Vec<_>>(),
        vec!["launch_mode", "program", "args_json", "workdir", "base_url"]
    );
    match &spec.fields[1] {
        ModSettingField::String { default, .. } => {
            assert_eq!(default.as_deref(), Some(MELO_PROGRAM));
        }
        other => panic!("program 应是 String，got {other:?}"),
    }
    match &spec.fields[4] {
        ModSettingField::String { default, .. } => {
            assert_eq!(
                default.as_deref(),
                Some(MELO_BASE_URL),
                "MeloTTS 缺省声明地址必须是 8091（不占 8080）"
            );
        }
        other => panic!("base_url 应是 String，got {other:?}"),
    }
}

#[test]
fn create_fills_declared_defaults() {
    let applied = Arc::new(AtomicUsize::new(0));
    let rt = FACTORY
        .create(spy_services(&applied), serde_json::json!({}))
        .expect("空配置应可 create（缺省程序 = 仓库内脚本，到 spawn 才预检）");
    drop(rt);

    let config = LaunchConfig::from_json(&serde_json::json!({})).unwrap();
    assert_eq!(config.mode, LaunchMode::WithApp);
    assert_eq!(config.program, COSYVOICE_PROGRAM, "缺省程序 = 仓库内脚本");
    assert_eq!(config.base_url, DEFAULT_BASE_URL, "base_url 缺省是声明地址");
    assert_eq!(config.args_json, "[]", "args_json 缺键当 []");
    assert_eq!(config.workdir_path(), None);
    assert_eq!(LaunchMode::parse("with_app"), Some(LaunchMode::WithApp));
    assert_eq!(LaunchMode::parse("WITH_APP"), None, "值大小写敏感");

    // MeloTTS 工厂也吃空配置（同一个 crate 的第二个引擎）。
    let melo = MELO_FACTORY
        .create(spy_services(&applied), serde_json::json!({}))
        .expect("MeloTTS 空配置也应可 create");
    drop(melo);
}
