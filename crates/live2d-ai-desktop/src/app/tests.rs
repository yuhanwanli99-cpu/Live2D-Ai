//! `app` 模块的单元测试：纯函数与分类的稳定性断言。
//!
//! - `RawWindowHandle` / `RawDisplayHandle` → `WindowBackendKind` 分类；
//! - `panic_message` 提取；
//! - `choose_surface_format` 偏好顺序；
//! - alpha 模式观测/回转 round-trip；
//! - C2 裁决 GPU fault 分类（[`route_gpu_fault`] 三档映射 +
//!   [`classify_poll_failure`] 环境错误归类）；
//! - C4 裁决 P0-3 surface 生命周期：resize latest-size 合并 +
//!   `Suboptimal` 先 present 再 configure + `Outdated` 跳过本帧
//!   + `Lost` 重建（[`merge_pending_resize`] / [`decide_after_acquire`]）。

use raw_window_handle::RawDisplayHandle;
use raw_window_handle::RawWindowHandle;
use winit::dpi::PhysicalSize;

use super::capability::{
    choose_surface_format, classify_raw_display_handle, classify_raw_window_handle,
    observed_alpha_mode, wgpu_alpha_mode,
};
use super::frame::{GpuFaultKind, classify_poll_failure, route_gpu_fault};
use super::surface::{
    AcquireOutcome, StreakDecision, SurfaceAction, advance_validation_streak, decide_after_acquire,
    merge_pending_resize,
};
use super::types::panic_message;
use crate::model_smoke::ModelSmokeError;
use crate::platform::ObservedAlphaMode;
use crate::platform::WindowBackendKind;

#[test]
fn raw_window_handle_classification_marks_x11_and_wayland() {
    // Xlib / Xcb → X11（需求 2 口径）。
    let xlib = RawWindowHandle::Xlib(raw_window_handle::XlibWindowHandle::new(12345));
    assert_eq!(
        classify_raw_window_handle(xlib),
        Some(WindowBackendKind::X11)
    );
    let xcb = RawWindowHandle::Xcb(raw_window_handle::XcbWindowHandle::new(
        std::num::NonZeroU32::new(6789).unwrap(),
    ));
    assert_eq!(
        classify_raw_window_handle(xcb),
        Some(WindowBackendKind::X11)
    );
    // Wayland → Wayland。
    let wayland = RawWindowHandle::Wayland(raw_window_handle::WaylandWindowHandle::new(
        std::ptr::NonNull::<std::ffi::c_void>::dangling(),
    ));
    assert_eq!(
        classify_raw_window_handle(wayland),
        Some(WindowBackendKind::Wayland)
    );
    // 其他句柄（如 Web）→ None：不识别就不假设。
    let web = RawWindowHandle::Web(raw_window_handle::WebWindowHandle::new(1));
    assert_eq!(classify_raw_window_handle(web), None);
}

#[test]
fn raw_display_handle_classification_matches_window_side() {
    let xlib = RawDisplayHandle::Xlib(raw_window_handle::XlibDisplayHandle::new(None, 0));
    assert_eq!(
        classify_raw_display_handle(xlib),
        Some(WindowBackendKind::X11)
    );
    let xcb = RawDisplayHandle::Xcb(raw_window_handle::XcbDisplayHandle::new(None, 0));
    assert_eq!(
        classify_raw_display_handle(xcb),
        Some(WindowBackendKind::X11)
    );
    let wayland = RawDisplayHandle::Wayland(raw_window_handle::WaylandDisplayHandle::new(
        std::ptr::NonNull::<std::ffi::c_void>::dangling(),
    ));
    assert_eq!(
        classify_raw_display_handle(wayland),
        Some(WindowBackendKind::Wayland)
    );
    let web = RawDisplayHandle::Web(raw_window_handle::WebDisplayHandle::new());
    assert_eq!(classify_raw_display_handle(web), None);
}

#[test]
fn panic_payload_message_extraction() {
    let boxed: Box<dyn std::any::Any + Send> = Box::new(String::from("libxkbcommon-x11 missing"));
    assert_eq!(panic_message(boxed.as_ref()), "libxkbcommon-x11 missing");
    let boxed: Box<dyn std::any::Any + Send> = Box::new("static str panic");
    assert_eq!(panic_message(boxed.as_ref()), "static str panic");
    let boxed: Box<dyn std::any::Any + Send> = Box::new(42_u32);
    assert_eq!(panic_message(boxed.as_ref()), "<non-string panic payload>");
}

#[test]
fn surface_format_prefers_non_srgb_with_alpha() {
    assert_eq!(
        choose_surface_format(&[
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
        ]),
        Some(wgpu::TextureFormat::Bgra8Unorm)
    );
    // 无偏好命中时退回首项而非 None。
    assert_eq!(
        choose_surface_format(&[wgpu::TextureFormat::Rgba16Float]),
        Some(wgpu::TextureFormat::Rgba16Float)
    );
    assert_eq!(choose_surface_format(&[]), None);
}

#[test]
fn alpha_mode_conversion_round_trip_is_lossless_for_explicit_modes() {
    for mode in [
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::PostMultiplied,
        wgpu::CompositeAlphaMode::Inherit,
        wgpu::CompositeAlphaMode::Opaque,
    ] {
        assert_eq!(wgpu_alpha_mode(observed_alpha_mode(mode)), mode);
    }
    // Auto 观测侧保守折叠为 Opaque（不会作为显式选择被回写）。
    assert_eq!(
        observed_alpha_mode(wgpu::CompositeAlphaMode::Auto),
        ObservedAlphaMode::Opaque
    );
}

// ====================================================================
// C4 裁决 P0-3：surface 生命周期（resize 合并 + Suboptimal/Outdated/Lost）
// ====================================================================

/// 模拟 `Resized` 事件批处理：连续多次 `Resized(size)` 调用
/// `merge_pending_resize`，断言最终保留的是**最后非零值**
/// （C4 第 81 行：同一事件批次的多次 resize 合并为最后值）。
#[test]
fn resize_multiple_events_collapse_to_last_nonzero_size() {
    // 起始 None，第一次写入非零 → 保留第一次。
    let mut pending = merge_pending_resize(None, PhysicalSize::new(800, 600));
    assert_eq!(pending, Some(PhysicalSize::new(800, 600)));

    // 同一批次后续多次非零 Resize → 全部合并为最后值（last-wins）。
    pending = merge_pending_resize(pending, PhysicalSize::new(1024, 720));
    assert_eq!(pending, Some(PhysicalSize::new(1024, 720)));
    pending = merge_pending_resize(pending, PhysicalSize::new(1280, 800));
    assert_eq!(pending, Some(PhysicalSize::new(1280, 800)));
    pending = merge_pending_resize(pending, PhysicalSize::new(640, 480));
    assert_eq!(pending, Some(PhysicalSize::new(640, 480)));

    // 真正"拖动"场景：连续 8 次递增到 1920×1080。
    let mut burst = None;
    let trail = [
        (320, 240),
        (640, 480),
        (800, 600),
        (1024, 768),
        (1280, 720),
        (1366, 768),
        (1600, 900),
        (1920, 1080),
    ];
    for (w, h) in trail {
        burst = merge_pending_resize(burst, PhysicalSize::new(w, h));
    }
    assert_eq!(burst, Some(PhysicalSize::new(1920, 1080)));
}

/// 零尺寸 Resize（最小化/折叠）不覆盖已有 pending：保留最近非零值，
/// 等下一帧 restore 后的非零 Resize 再覆盖（C4 收敛行为）。
#[test]
fn resize_zero_size_does_not_overwrite_existing_pending() {
    // 先有非零值，零尺寸过来不覆盖。
    let mut pending = Some(PhysicalSize::new(1024, 768));
    pending = merge_pending_resize(pending, PhysicalSize::new(0, 720));
    assert_eq!(pending, Some(PhysicalSize::new(1024, 768)));
    pending = merge_pending_resize(pending, PhysicalSize::new(1024, 0));
    assert_eq!(pending, Some(PhysicalSize::new(1024, 768)));

    // 零尺寸后再来非零 → 正常覆盖（恢复后的尺寸）。
    pending = merge_pending_resize(pending, PhysicalSize::new(800, 600));
    assert_eq!(pending, Some(PhysicalSize::new(800, 600)));

    // 起点为 None 时，零尺寸不写入（保持 None，避免污染后续合并）。
    let none_then_zero = merge_pending_resize(None, PhysicalSize::new(0, 0));
    assert_eq!(none_then_zero, None);
}

/// `Suboptimal` 必须先 present 再 configure（C4 第 82 行 + 第 85 行 WASM 同步
/// 落地）。状态机里它映射为 `PresentThenReconfigure`，语义上等价于：
/// "本帧仍要 render_and_present，下一步才 reconfigure"——由调用方按此顺序
/// 执行；测试只断言状态机的合约标记。
#[test]
fn suboptimal_action_marks_present_then_reconfigure_in_order() {
    let action = decide_after_acquire(AcquireOutcome::Suboptimal);
    assert_eq!(action, SurfaceAction::PresentThenReconfigure);

    // 顺序合约：Suboptimal 分支对应的动作必须出现"Present"语义
    // 而不是单纯的 Skip。本测试通过枚举等值 + 调试字符串双重断言
    // 防止有人把 Suboptimal 静默退化为 SkipAndReconfigure（frame 仍存活
    // 时 configure 会 panic）。
    let debug = format!("{action:?}");
    assert!(
        debug.contains("Present"),
        "Suboptimal 决策必须保留 Present 语义（先 present 再 configure），实际: {debug}"
    );
    assert!(
        debug.contains("Reconfigure"),
        "Suboptimal 决策必须后续 configure，实际: {debug}"
    );
}

/// `Outdated` 无 texture：直接 configure 跳过本帧（C4 第 83 行）。
/// 测试断言：动作不是 Present 也不是 PresentThenReconfigure，且理由是
/// "configure 而非重建 surface"。
#[test]
fn outdated_action_skips_present_and_only_reconfigures() {
    let action = decide_after_acquire(AcquireOutcome::Outdated);
    assert_eq!(action, SurfaceAction::SkipAndReconfigure);

    let debug = format!("{action:?}");
    assert!(
        !debug.contains("Present"),
        "Outdated 无 texture 不得 Present，实际: {debug}"
    );
    assert!(
        !debug.contains("Recreate"),
        "Outdated 不是 Lost 不得重建 surface，实际: {debug}"
    );
    assert!(
        debug.contains("Reconfigure"),
        "Outdated 必须 configure（让下帧按新 config 出帧），实际: {debug}"
    );
}

/// `Lost` 走重建路径：`Recreate` 隐含 configure + 视口同步
/// （C4 第 84 行：重建 surface → configure → 同步 viewport）。
#[test]
fn lost_action_triggers_recreate_branch() {
    let action = decide_after_acquire(AcquireOutcome::Lost);
    assert_eq!(action, SurfaceAction::SkipAndRecreate);

    let debug = format!("{action:?}");
    assert!(
        debug.contains("Recreate"),
        "Lost 必须走 Recreate 分支，实际: {debug}"
    );
    assert!(
        !debug.contains("Present"),
        "Lost 不得 Present（surface 失效），实际: {debug}"
    );
}

/// 瞬态分支（Timeout/Occluded）不重配，单纯跳过；Validation 同样
/// 静默跳过（带 reason 便于日志）。
#[test]
fn transient_outcomes_are_silent_skip() {
    assert_eq!(
        decide_after_acquire(AcquireOutcome::Timeout),
        SurfaceAction::Skip("timeout")
    );
    assert_eq!(
        decide_after_acquire(AcquireOutcome::Occluded),
        SurfaceAction::Skip("occluded")
    );
    assert_eq!(
        decide_after_acquire(AcquireOutcome::Validation),
        SurfaceAction::Skip("validation")
    );

    for outcome in [
        AcquireOutcome::Timeout,
        AcquireOutcome::Occluded,
        AcquireOutcome::Validation,
    ] {
        let debug = format!("{:?}", decide_after_acquire(outcome));
        assert!(
            !debug.contains("Reconfigure"),
            "瞬态/校验不得触发重配，避免无谓的 wgpu 调用，实际: {debug}"
        );
        assert!(
            !debug.contains("Recreate"),
            "瞬态/校验不得重建 surface，实际: {debug}"
        );
    }
}

/// 正常出帧：Success 单纯 Present，不重配、不重建。
#[test]
fn success_outcome_is_plain_present() {
    assert_eq!(
        decide_after_acquire(AcquireOutcome::Success),
        SurfaceAction::Present
    );
}

/// 端到端合并 + 决策：模拟"用户连续拖动窗口 → 拖动停止 → 下一次 redraw
/// 落地"的整段流水线。仅断言决策层面，wgpu 副作用由
/// [`ShellApp::apply_pending_resize`] / `render_frame` 执行。
#[test]
fn resize_burst_then_drain_pipeline_keeps_latest_size() {
    // 起点 None。
    let mut pending: Option<PhysicalSize<u32>> = None;
    // 模拟拖动 burst：5 次递增 Resized。
    for w in [400, 600, 800, 1000, 1200] {
        pending = merge_pending_resize(pending, PhysicalSize::new(w, 800));
    }
    assert_eq!(pending, Some(PhysicalSize::new(1200, 800)));

    // 最小化/折叠：零尺寸 Resize，pending 不变。
    pending = merge_pending_resize(pending, PhysicalSize::new(0, 0));
    assert_eq!(pending, Some(PhysicalSize::new(1200, 800)));

    // 恢复后再 resize：覆盖为新值。
    pending = merge_pending_resize(pending, PhysicalSize::new(1024, 768));
    assert_eq!(pending, Some(PhysicalSize::new(1024, 768)));

    // 与状态机交互：本次 redraw 取帧 Suboptimal（用户拖动中可能发生），
    // 状态机决策 = PresentThenReconfigure（先 present 再 configure）。
    // 真正的 configure 由 reconfigure_current 走 `window.inner_size()` 而
    // 非 pending——合并目的是把"最新窗口尺寸"在合适时机落地到 wgpu，
    // 取帧 Suboptimal 时则按当前已生效 config 重配。
    assert_eq!(
        decide_after_acquire(AcquireOutcome::Suboptimal),
        SurfaceAction::PresentThenReconfigure
    );
}

/// `AcquireOutcome::from_current` 把 wgpu 变体映射到纯状态机值
/// （不含 surface 句柄，避免单测持有 GPU 对象）。`Outdated` / `Lost` /
/// `Timeout` / `Occluded` / `Validation` 不携带 texture，可以在测试里
/// 直接构造并断言。
#[test]
fn acquire_outcome_from_current_maps_plain_variants() {
    use super::surface::AcquireOutcome as A;
    assert_eq!(
        AcquireOutcome::from_current(&wgpu::CurrentSurfaceTexture::Outdated),
        A::Outdated
    );
    assert_eq!(
        AcquireOutcome::from_current(&wgpu::CurrentSurfaceTexture::Lost),
        A::Lost
    );
    assert_eq!(
        AcquireOutcome::from_current(&wgpu::CurrentSurfaceTexture::Timeout),
        A::Timeout
    );
    assert_eq!(
        AcquireOutcome::from_current(&wgpu::CurrentSurfaceTexture::Occluded),
        A::Occluded
    );
    assert_eq!(
        AcquireOutcome::from_current(&wgpu::CurrentSurfaceTexture::Validation),
        A::Validation
    );
}

// ====================================================================
// C2 裁决 GPU fault 分类专项测试（`route_gpu_fault` / `classify_poll_failure`）
// ====================================================================

/// C2 裁决第 55-57 行三档映射：`Validation → ModelCode（退出 1）`、
/// `OutOfMemory → Environment（退出 3）`、`Internal → Environment（退出 3）`。
/// 单测一次跑三档避免拆得过细——核心不变式是"kind 决定路由，detail 透传"。
#[test]
fn route_gpu_fault_three_tier_mapping_matches_c2() {
    use super::frame::FaultRouting;
    let detail = "kind=validation, desc=invalid bind group; adapter=llvmpipe".to_owned();

    // Validation → ModelCode（退出 1）。
    match route_gpu_fault(GpuFaultKind::Validation, detail.clone()) {
        FaultRouting::ModelCode(msg) => {
            assert!(msg.contains("退出码 1"), "Validation 必须标退出 1: {msg}");
            assert!(msg.contains("kind=validation"), "detail 必须透传: {msg}");
        }
        FaultRouting::Environment(msg) => {
            panic!("Validation 不得归 Environment: {msg}")
        }
    }
    // OutOfMemory → Environment（退出 3）。
    match route_gpu_fault(GpuFaultKind::OutOfMemory, detail.clone()) {
        FaultRouting::Environment(msg) => {
            assert!(msg.contains("退出码 3"), "OOM 必须标退出 3: {msg}");
            assert!(msg.contains("kind=validation"), "detail 必须透传: {msg}");
        }
        FaultRouting::ModelCode(msg) => panic!("OOM 不得归 ModelCode: {msg}"),
    }
    // Internal → Environment（退出 3）。
    match route_gpu_fault(GpuFaultKind::Internal, detail) {
        FaultRouting::Environment(msg) => {
            assert!(msg.contains("退出码 3"), "Internal 必须标退出 3: {msg}");
        }
        FaultRouting::ModelCode(msg) => panic!("Internal 不得归 ModelCode: {msg}"),
    }
}

/// C2 第 58 行：`Device::poll` 失败 → `GpuEnvironment`（退出 3）。
/// path / submission / adapter 必须出现在消息中，`Display` 走 GpuEnvironment
/// 分支以便 `run_shell` 映射为退出码 3。`SubmissionIndex` 字段私有不可构造，
/// 用 `None`（首帧/未提交过）也能代表"无最近提交"语义。
#[test]
fn classify_poll_failure_returns_gpu_environment() {
    let err = classify_poll_failure("model", "SurfaceError::Lost".to_owned(), None, "llvmpipe");
    match &err {
        ModelSmokeError::GpuEnvironment(msg) => {
            assert!(msg.contains("退出码 3"), "消息必须标退出 3: {msg}");
            assert!(msg.contains("model"), "消息必须含 path 'model': {msg}");
            assert!(msg.contains("llvmpipe"), "消息必须含 adapter: {msg}");
        }
        other => panic!("poll 失败必须归 GpuEnvironment，实际: {other:?}"),
    }
    let display = err.to_string();
    assert!(
        display.contains("GPU 环境不满足"),
        "Display 走 GpuEnvironment 分支: {display}"
    );

    // None submission：消息中显式 None + 另一 path。
    let err_none = classify_poll_failure(
        "transparent-shell",
        "PollError::Timeout".to_owned(),
        None,
        "nvidia-rtx",
    );
    match err_none {
        ModelSmokeError::GpuEnvironment(msg) => {
            assert!(
                msg.contains("last_submission=None"),
                "None 应显式呈现: {msg}"
            );
            assert!(
                msg.contains("transparent-shell"),
                "path 标识必须出现: {msg}"
            );
        }
        other => panic!("None submission 也应归 GpuEnvironment，实际: {other:?}"),
    }
}

/// GpuFaultKind 的分类名（`as_str`）必须稳定——日志/报告 key 一致。
#[test]
fn gpu_fault_kind_as_str_is_stable() {
    assert_eq!(GpuFaultKind::Validation.as_str(), "validation");
    assert_eq!(GpuFaultKind::OutOfMemory.as_str(), "out-of-memory");
    assert_eq!(GpuFaultKind::Internal.as_str(), "internal");
}

// C2 第 59 行：surface Validation streak 状态机序列测试。纯函数
// `advance_validation_streak` 必须 1:1 模拟生产路径 frame.rs::render_frame
// 的 SurfaceAction::Skip 分支；streak reset 回归会让下列测试直接断红。
//
// 头注豁免：本文件因 P1-0 序列测试加入后 ~504 行（>500）。任务约定 ≤1000
// 行可在此豁免（见 P1-0 任务门禁）；后续如需进一步扩展，应拆分为
// `app::tests::streak` 子模块或独立测试文件以保持本文件 ≤500。

#[test]
fn advance_validation_streak_first_validation_is_skip_first() {
    let mut streak: u8 = 0;
    let d = advance_validation_streak(&mut streak, true);
    assert_eq!(d, StreakDecision::SkipFirst);
    assert_eq!(streak, 1);
}

#[test]
fn advance_validation_streak_second_validation_is_fatal_code() {
    let mut streak: u8 = 1;
    let d = advance_validation_streak(&mut streak, true);
    assert_eq!(d, StreakDecision::FatalCode);
    assert_eq!(streak, 2);
}

#[test]
fn advance_validation_streak_non_validation_clears_streak() {
    let mut streak: u8 = 1;
    let d = advance_validation_streak(&mut streak, false);
    assert_eq!(d, StreakDecision::NotValidation);
    assert_eq!(streak, 0);
}

#[test]
fn advance_validation_streak_resets_after_non_validation() {
    let mut streak: u8 = 0;
    assert_eq!(
        advance_validation_streak(&mut streak, true),
        StreakDecision::SkipFirst
    );
    assert_eq!(streak, 1);
    assert_eq!(
        advance_validation_streak(&mut streak, false),
        StreakDecision::NotValidation
    );
    assert_eq!(streak, 0);
    assert_eq!(
        advance_validation_streak(&mut streak, true),
        StreakDecision::SkipFirst
    );
    assert_eq!(streak, 1);
}

#[test]
fn advance_validation_streak_three_in_a_row_fatal_at_second() {
    let mut streak: u8 = 0;
    assert_eq!(
        advance_validation_streak(&mut streak, true),
        StreakDecision::SkipFirst
    );
    assert_eq!(
        advance_validation_streak(&mut streak, true),
        StreakDecision::FatalCode
    );
    assert_eq!(streak, 2);
    assert_eq!(
        advance_validation_streak(&mut streak, true),
        StreakDecision::FatalCode
    );
    assert_eq!(streak, 3);
}

#[test]
fn advance_validation_streak_success_breaks_streak() {
    let mut streak: u8 = 0;
    assert_eq!(
        advance_validation_streak(&mut streak, true),
        StreakDecision::SkipFirst
    );
    assert_eq!(streak, 1);
    assert_eq!(
        advance_validation_streak(&mut streak, false),
        StreakDecision::NotValidation
    );
    assert_eq!(streak, 0);
    assert_eq!(
        advance_validation_streak(&mut streak, false),
        StreakDecision::NotValidation
    );
    assert_eq!(streak, 0);
    assert_eq!(
        advance_validation_streak(&mut streak, true),
        StreakDecision::SkipFirst
    );
    assert_eq!(streak, 1);
}
