//! Bai 真实资产门控集成测试（RFC D5：v0 只保证 Bai 实测皮套）。
//!
//! 门控纪律：
//! - **资产缺失 → 明确 skip**（打印原因后直接返回；测试仍记为通过，
//!   但输出里可见 SKIP 行，绝不静默假装跑过）；
//! - 渲染类断言依赖 headless GPU，无后端时同样明确 skip；
//! - 无绝对 golden 像素 hash（不同 GPU 后端逐字节不可复现），
//!   改用「同进程双渲染 hash 必须一致」+ coverage/bbox 容差窗
//!   （窗口值来自 bakeoff 口径的 1024² 产物；**2026-09-10 等比映射修复后重测**：
//!   alpha coverage ≈ 12.4%，bbox x[348..672] y[164..956]。
//!   旧值 coverage 8.23% / y[294..822] 记录的是「画布 4000×6000 被逐轴拉成正方形」
//!   导致的纵向压缩 1.5× 错误形态。）

use std::path::{Path, PathBuf};

use l2d::asset::ModelPackage;
use l2d::format::MocVersion;
use l2d::model::LoadedModel;
use l2d::renderer::{FIXED_DT_60HZ, OffscreenRenderer};

/// 仓库内 Bai 皮套清单路径（相对本 crate 的 CARGO_MANIFEST_DIR）。
const BAI_MODEL3: &str = "../../assets/models/bai/runtime/bai.model3.json";

/// bakeoff 口径 1024² 产物 bbox（含端点），按比例缩放到当前渲染尺寸。
///
/// 2026-09-10 重测：`layout_transform` 改为等比后，纵向由 y[294..822] 拉伸到
/// y[164..956]（×1.5），横向 x[350..676] → x[348..672] 基本不变。
const BAKEOFF_BBOX_1024: (u32, u32, u32, u32) = (348, 164, 672, 956);

/// 解析 Bai 清单绝对路径；不存在则返回 `None`（调用方明确 skip）。
fn bai_model3_path() -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BAI_MODEL3);
    if path.is_file() { Some(path) } else { None }
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

#[test]
fn bai_package_loads_with_header_runtime_version_consistency() {
    let Some(model3) = bai_model3_path() else {
        println!("SKIP: 未找到 Bai 真实资产 `{BAI_MODEL3}`（门控跳过）");
        return;
    };

    // ---- 包加载与清单 ----
    let package = ModelPackage::load(&model3).expect("Bai 包应可加载");
    let manifest = package.manifest();
    assert_eq!(manifest.version, 3);
    assert_eq!(manifest.moc3_file, "bai.moc3");
    assert_eq!(
        manifest.texture_files,
        vec!["bai.16384/texture_00_4096.png"]
    );
    assert_eq!(manifest.physics_file.as_deref(), Some("bai.physics3.json"));
    assert_eq!(manifest.display_info_file.as_deref(), Some("bai.cdi3.json"));
    assert!(package.physics_json().is_some());
    // 真实纹理应为 PNG（magic 校验，不做尺寸假设）。
    assert_eq!(
        &package.texture_bytes()[0][..4],
        b"\x89PNG",
        "纹理必须是真实 PNG"
    );

    // ---- 统一加载路径：头探测 × 运行时版本交叉核对必须一致且通过 v0 ----
    let loaded = LoadedModel::resolve(package).expect("Bai moc3 应可被运行时解析");
    let report = loaded.report();
    assert_eq!(
        report.format,
        l2d::format::MocFormat::Moc3(MocVersion::V4_2_0)
    );
    assert_eq!(
        loaded.handle().moc_version(),
        Some(MocVersion::V4_2_0),
        "运行时解析版本"
    );
    assert_eq!(
        report.moc_version(),
        loaded.handle().moc_version(),
        "头探测与运行时版本必须一致（不一致会以 RuntimeVersionMismatch 否决）"
    );
    assert!(
        !report.has_issues(),
        "Bai 档位不应有任何兼容问题，实际: {:?}",
        report.issues()
    );
    assert!(report.is_supported_v0(), "Bai 是 v0 唯一锚定档位");
}

#[test]
fn bai_renders_256px_with_deterministic_hash_and_toleranced_geometry() {
    let Some(model3) = bai_model3_path() else {
        println!("SKIP: 未找到 Bai 真实资产 `{BAI_MODEL3}`（门控跳过）");
        return;
    };

    let loaded = LoadedModel::resolve(ModelPackage::load(&model3).expect("load")).expect("resolve");

    // GPU 门控：无 headless 后端时明确跳过（不伪造失败也不伪造成功）。
    let mut renderer = match OffscreenRenderer::new(256, 256) {
        Ok(r) => r,
        Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
            println!("SKIP: 无可用 headless GPU 后端（{notes}）");
            return;
        }
        Err(e) => panic!("离屏渲染器创建失败: {e}"),
    };
    renderer.load_model(&loaded).expect("加载进渲染器");

    // 与 bakeoff 一致的确定性序列：120 步固定 dt（模拟 2 秒）。
    for _ in 0..120 {
        renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    }

    // 双渲染：hash 必须逐字节一致（管线确定性；容差语义见模块头注释）。
    let first = renderer.render_frame().expect("第一帧");
    let second = renderer.render_frame().expect("第二帧");
    assert_eq!(first.width, 256);
    assert_eq!(first.height, 256);
    assert_eq!(first.pixels.len(), 256 * 256 * 4);
    assert_eq!(
        fnv1a(&first.pixels),
        fnv1a(&second.pixels),
        "同一状态下连续两帧必须确定一致"
    );

    // coverage 容差窗：等比修复后（2026-09-10）1024² 实测约 12.4%；256² 允许 [1%, 40%]
    // （下限防全透明回归，上限防意外放大铺满）。
    let coverage = first.alpha_coverage();
    assert!(
        (0.01..=0.40).contains(&coverage),
        "alpha coverage {:.2}% 超出容差窗 [1%, 40%]",
        coverage * 100.0
    );

    // bbox 容差窗：bakeoff bbox 按 256/1024 缩放，允许 ±10px。
    const TOL: i64 = 10;
    let scale = 256.0 / 1024.0;
    let expected = [
        (BAKEOFF_BBOX_1024.0 as f64 * scale).round() as i64,
        (BAKEOFF_BBOX_1024.1 as f64 * scale).round() as i64,
        (BAKEOFF_BBOX_1024.2 as f64 * scale).round() as i64,
        (BAKEOFF_BBOX_1024.3 as f64 * scale).round() as i64,
    ];
    let (x0, y0, x1, y1) = first.visible_bbox().expect("非空画面必有可见包围盒");
    let actual = [x0 as i64, y0 as i64, x1 as i64, y1 as i64];
    let axes = ["x0", "y0", "x1", "y1"];
    for (axis, (e, a)) in axes.iter().zip(expected.iter().zip(actual.iter())) {
        assert!(
            (e - a).abs() <= TOL,
            "bbox {axis}: 期望 {e}±{TOL}，实际 {a}（完整 bbox {actual:?} vs {expected:?}）"
        );
    }

    // resize 后仍可正常出帧（便利层职责自检）。
    renderer.resize(128, 128).expect("resize 到 128²");
    let small = renderer.render_frame().expect("resize 后出帧");
    assert_eq!(small.width, 128);
    assert_eq!(small.height, 128);
    assert!(small.alpha_coverage() > 0.0, "resize 后不应变全透明");

    // 非法尺寸必须被拒绝（0 尺寸校验）。
    assert!(matches!(
        renderer.resize(0, 100),
        Err(l2d::renderer::RenderError::InvalidSize {
            width: 0,
            height: 100
        })
    ));
}
