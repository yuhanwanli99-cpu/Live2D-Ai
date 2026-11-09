//! pose3 骨骼层真实样本的 headless 断言（0.5.0 rc.4）。
//!
//! 官方样本**不入库**（授权限制）：本测试只在设置环境变量 `L2D_POSE3_SAMPLE`
//! （指向带 `FileReferences.Pose` 的 model3.json 绝对路径）时启用；否则打印
//! SKIP 返回（与 `bai_asset.rs` 的资产门控同一纪律：跳过要可见、不伪造通过）。
//!
//! 本地验证（haru 官方样本，7z 只解到本机 D:，不入库）：
//! `L2D_POSE3_SAMPLE=/mnt/d/live2dai/build/pose3-sample/official_live2d_samples/haru/runtime/haru.model3.json`
//!
//! 断言链：清单 `Pose` 字段 → pose3 解析 → part 层选择（每组一个可见 + Link 陪显）
//! → 跨帧保持 → headless 渲染出可见像素。

use std::path::PathBuf;

use l2d::asset::ModelPackage;
use l2d::asset::pose3::PoseGroups;
use l2d::model::LoadedModel;
use l2d::pose_stack::PoseStack;

fn sample_path() -> Option<PathBuf> {
    let Ok(raw) = std::env::var("L2D_POSE3_SAMPLE") else {
        println!("SKIP: 未设置 L2D_POSE3_SAMPLE（官方 pose3 样本不入库）");
        return None;
    };
    let path = PathBuf::from(raw);
    if !path.is_file() {
        println!(
            "SKIP: L2D_POSE3_SAMPLE 指向的文件不存在：{}",
            path.display()
        );
        return None;
    }
    Some(path)
}

#[test]
fn pose3_sample_selects_parts_from_manifest() {
    let Some(path) = sample_path() else {
        return;
    };
    let package = ModelPackage::load(&path).expect("带 pose3 的样本包应可加载");
    let pose_file = package
        .pose3_file()
        .expect("样本必须声明 FileReferences.Pose")
        .to_owned();
    let pose_bytes = package
        .pose3_json()
        .expect("声明了 Pose 就必须读到字节")
        .to_vec();
    let groups = PoseGroups::from_json(&pose_bytes).expect("样本 pose3 应可解析");
    assert!(!groups.is_empty(), "样本 pose3 至少有一组 part");

    let loaded = LoadedModel::resolve(package).expect("样本 moc3 应可解析");
    let stack = PoseStack::from_loaded_model(&loaded).expect("physics3 + pose3 应可装载");
    assert!(stack.has_pose_layer(), "声明了 Pose 就必须挂上 part 层");
    let (applied_groups, visible, hidden) = stack.pose_part_counts();
    assert_eq!(
        applied_groups,
        groups.group_count(),
        "每组都应命中模型 part（{pose_file}）"
    );
    assert!(visible >= applied_groups, "每组至少一个可见 part");
    assert!(hidden >= 1, "至少有互斥 part 被隐藏（{pose_file}）");
    // 官方样本每组第一个 part 可见、第二个隐藏（组内少于 2 个的跳过）。
    for group in groups.groups() {
        if group.len() < 2 {
            continue;
        }
        assert_eq!(
            stack.get_part_opacity(&group[0].id),
            Some(1.0),
            "{} 应可见",
            group[0].id
        );
        assert_eq!(
            stack.get_part_opacity(&group[1].id),
            Some(0.0),
            "{} 应隐藏",
            group[1].id
        );
    }
}

#[test]
fn pose3_sample_renders_headless_with_layer_kept_across_frames() {
    use l2d::renderer::{FIXED_DT_60HZ, OffscreenRenderer};
    let Some(path) = sample_path() else {
        return;
    };
    let loaded = LoadedModel::resolve(ModelPackage::load(&path).expect("load")).expect("resolve");
    let mut renderer = match OffscreenRenderer::new(256, 256) {
        Ok(r) => r,
        Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
            println!("SKIP: 无可用 headless GPU 后端（{notes}）");
            return;
        }
        Err(e) => panic!("离屏渲染器创建失败: {e}"),
    };
    renderer
        .load_model(&loaded)
        .expect("带 pose3 的模型应可装载进渲染核心");
    assert!(
        renderer.core().stack().has_pose_layer(),
        "渲染核心的栈必须带 part 层"
    );
    let before = renderer.core().stack().pose_part_counts();
    for _ in 0..30 {
        renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    }
    let after = renderer.core().stack().pose_part_counts();
    assert_eq!(before, after, "part 层跨帧保持（不随机跳动）");
    let frame = renderer.render_frame().expect("出帧");
    assert_eq!(frame.width, 256);
    assert!(
        frame.alpha_coverage() > 0.0,
        "带 pose3 的模型必须渲染出可见像素"
    );

    // 端到端证据：把 part 层换成空 Groups（所有 part 恢复默认可见）后重渲染，
    // 像素必须变化——证明 pose3 层真的通过 part 不透明度影响渲染，而不只是
    // 改了 Pose 里的数值。
    let pose_hash = fnv1a(&frame.pixels);
    renderer
        .core()
        .stack()
        .attach_pose_json(br#"{"Groups":[]}"#)
        .expect("空 pose3 应可解析");
    renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    let flat = renderer.render_frame().expect("无 pose3 层帧");
    assert!(flat.alpha_coverage() > 0.0, "无 part 层的对照帧也必须可见");
    assert_ne!(
        pose_hash,
        fnv1a(&flat.pixels),
        "part 层必须真实影响像素（否则 pose3 只是改了 Pose 数值）"
    );
}

/// FNV-1a 64 位像素哈希（无第三方依赖）。
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
