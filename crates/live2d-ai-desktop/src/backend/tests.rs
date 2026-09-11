//! backend 单元测试（自旧 backend.rs 原样迁出，断言一字未改）。

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use super::{
    BackendKind, FeatureRequest, RunOptions, StopReason, available_backends, create_backend,
};
use crate::model_smoke::ModelSmokeStats;
use crate::platform::RuntimeCapabilities;

fn base_options() -> RunOptions {
    RunOptions {
        initial_logical_size: (480.0, 640.0),
        frame_target: Some(10),
        timeout: None,
        model_smoke: None,
        pet_mode: false,
    }
}

#[test]
fn only_winit_wgpu_backend_is_registered() {
    assert_eq!(available_backends(), &[BackendKind::WinitWgpu]);
    assert!(create_backend(BackendKind::WinitWgpu).is_ok());
    assert_eq!(BackendKind::WinitWgpu.to_string(), "winit-wgpu");
}

#[test]
fn description_declares_pet_capabilities_and_runtime_gate() {
    let backend = create_backend(BackendKind::WinitWgpu).expect("backend");
    let desc = backend.describe();
    assert_eq!(desc.kind, BackendKind::WinitWgpu);
    assert!(!desc.title.is_empty());
    let joined = desc.notes.join("\n");
    // 能力现状说明必须点名全部桌宠能力 + 运行时 gate 口径。
    for feature in [
        "always_on_top",
        "global_position",
        "click_through",
        "drag_window",
        "ksni",
        "RunReport.runtime",
    ] {
        assert!(joined.contains(feature), "notes 应说明 {feature}: {joined}");
    }
}

#[test]
fn request_feature_declares_implemented_code_paths_only() {
    let backend = create_backend(BackendKind::WinitWgpu).expect("backend");
    // 本批全部能力都有代码路径 → Ok（静态声明；实际以 runtime 实测为准）。
    for feature in [
        FeatureRequest::AlwaysOnTop,
        FeatureRequest::GlobalPosition,
        FeatureRequest::ClickThrough,
        FeatureRequest::DragMove,
        FeatureRequest::Tray,
    ] {
        assert!(
            backend.request_feature(feature).is_ok(),
            "{feature} 应有代码路径声明"
        );
    }
}

#[test]
fn run_options_validation_and_default_timeout() {
    let mut opts = base_options();
    assert_eq!(opts.validate(), Ok(()));
    // 冒烟帧数模式默认带 60s 兜底超时。
    assert_eq!(opts.effective_timeout(), Some(Duration::from_secs(60)));
    // 显式超时优先。
    opts.timeout = Some(Duration::from_secs(5));
    assert_eq!(opts.effective_timeout(), Some(Duration::from_secs(5)));

    // 非法输入逐一拒绝。
    for bad in [
        RunOptions {
            initial_logical_size: (0.0, 10.0),
            ..opts.clone()
        },
        RunOptions {
            initial_logical_size: (480.0, -1.0),
            ..opts.clone()
        },
        RunOptions {
            initial_logical_size: (f64::NAN, 10.0),
            ..opts.clone()
        },
        RunOptions {
            frame_target: Some(0),
            ..opts.clone()
        },
        RunOptions {
            model_smoke: Some(super::ModelSmokeOptions {
                model3_path: PathBuf::new(),
            }),
            ..opts.clone()
        },
    ] {
        assert!(bad.validate().is_err(), "{bad:?} 应校验失败");
    }

    // 模型冒烟配置（合法路径）通过校验，且不影响超时兜底语义。
    let model_opts = RunOptions {
        model_smoke: Some(super::ModelSmokeOptions {
            model3_path: PathBuf::from("bai.model3.json"),
        }),
        frame_target: Some(120),
        timeout: None,
        ..opts.clone()
    };
    assert_eq!(model_opts.validate(), Ok(()));
    assert_eq!(
        model_opts.effective_timeout(),
        Some(Duration::from_secs(60))
    );

    // 常驻模式（无帧目标）：无隐式超时；pet-mode 合法且可带模型渲染。
    let resident_pet = RunOptions {
        initial_logical_size: (480.0, 640.0),
        frame_target: None,
        timeout: None,
        model_smoke: Some(super::ModelSmokeOptions {
            model3_path: PathBuf::from("bai.model3.json"),
        }),
        pet_mode: true,
    };
    assert_eq!(resident_pet.validate(), Ok(()));
    assert_eq!(resident_pet.effective_timeout(), None);
}

#[test]
fn stop_reason_display_is_stable() {
    assert_eq!(StopReason::WindowClosed.to_string(), "window-closed");
    assert_eq!(
        StopReason::FrameTargetReached(12).to_string(),
        "frame-target-reached(12)"
    );
    assert!(
        StopReason::TimeoutElapsed(Duration::from_secs(3))
            .to_string()
            .starts_with("timeout-elapsed")
    );
    assert_eq!(StopReason::TrayExit.to_string(), "tray-exit");
}

#[test]
fn run_report_summary_mentions_real_capabilities_only() {
    let report = super::RunReport {
        stop_reason: StopReason::FrameTargetReached(5),
        runtime: RuntimeCapabilities::not_probed(),
        gpu_adapter: String::from("llvmpipe (Vulkan)"),
        frames_presented: 5,
        frames_skipped: 1,
        wall_time: Duration::from_millis(1200),
        model: None,
    };
    let lines = report.summarize_lines().join("\n");
    assert!(lines.contains("frame-target-reached(5)"));
    assert!(lines.contains("llvmpipe"));
    assert!(lines.contains("presented=5 skipped=1"));
    // 未探测能力必须显示 unknown，而非编造的 true。
    assert!(lines.contains("transparent_alpha=unknown"));
    // 纯透明壳：不含模型冒烟段。
    assert!(!lines.contains("model smoke"));
}

#[test]
fn run_report_includes_model_smoke_section_when_present() {
    let report = super::RunReport {
        stop_reason: StopReason::FrameTargetReached(120),
        runtime: RuntimeCapabilities::not_probed(),
        gpu_adapter: String::from("D3D12 (Vulkan)"),
        frames_presented: 120,
        frames_skipped: 0,
        wall_time: Duration::from_secs(4),
        model: Some(ModelSmokeStats {
            model3_path: "bai.model3.json".into(),
            compat_lines: vec![],
            compat_supported_v0: true,
            frames_rendered: 120,
            sim_steps: 128,
            avg_frame_time: Some(Duration::from_millis(11)),
            max_frame_time: Some(Duration::from_millis(30)),
            actions_completed: 2,
            mouth_writes: 120,
            mouth_peak: 1.0,
            missing_params: vec!["ParamBrowLY".into()],
            viewport: (480, 640),
        }),
    };
    let lines = report.summarize_lines().join("\n");
    for needle in [
        "model smoke",
        "bai.model3.json",
        "rendered=120",
        "completed=2",
        "缺失参数 [\"ParamBrowLY\"]",
    ] {
        assert!(lines.contains(needle), "摘要缺少 {needle}:\n{lines}");
    }
}

#[test]
fn chat_config_path_resolves_explicit_and_cwd_fallback() {
    let dir = std::env::temp_dir().join(format!("l2d-chat-cfg-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("live2d-ai.toml"),
        "[llm]\nbase_url = \"http://a/v1\"\nmodel = \"m\"\n",
    )
    .unwrap();

    // 显式路径存在 → 原样返回。
    let explicit = dir.join("live2d-ai.toml");
    assert_eq!(
        super::chat::resolve_chat_config_path_in(Some(explicit.as_path()), Path::new(".")).unwrap(),
        explicit
    );
    // 缺省 → cwd 候选命中。
    assert_eq!(
        super::chat::resolve_chat_config_path_in(None, &dir).unwrap(),
        explicit
    );
    // 显式缺失 → 错误并提示模板（退出码 1 的文案来源）。
    let err =
        super::chat::resolve_chat_config_path_in(Some(&dir.join("nope.toml")), &dir).unwrap_err();
    assert!(err.contains("live2d-ai.toml"), "{err}");
    assert!(err.contains("live2d-ai.toml.example"), "{err}");

    std::fs::remove_dir_all(&dir).ok();
}
