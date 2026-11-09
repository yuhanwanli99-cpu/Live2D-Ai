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

/// **待机开关跨 `load_model` 存活（回归）**：`ModelRendererCore::load_model` 会整份
/// 替换 `PoseStack`，新栈的 `idle_enabled` 缺省为 `true`。若不把旧值抄过去，
/// 「待机小动作」关闭会在 ready 后异步换模型（`/app/` 打开、换模型同路径）
/// 被悄悄拨回开——表现为眨眼停着、头的 ±3° 又回来。
#[test]
fn idle_switch_survives_model_reload() {
    let Some(model3) = bai_model3_path() else {
        println!("SKIP: 未找到 Bai 真实资产 `{BAI_MODEL3}`（门控跳过）");
        return;
    };
    let loaded = LoadedModel::resolve(ModelPackage::load(&model3).expect("load")).expect("resolve");

    let mut renderer = match OffscreenRenderer::new(64, 64) {
        Ok(r) => r,
        Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
            println!("SKIP: 无可用 headless GPU 后端（{notes}）");
            return;
        }
        Err(e) => panic!("离屏渲染器创建失败: {e}"),
    };

    // 加载前关待机 → 首次 load_model 后必须仍是关。
    renderer.core().set_idle_enabled(false);
    renderer.load_model(&loaded).expect("首次加载");
    assert!(
        !renderer.core().stack().idle_enabled(),
        "首次 load_model 后待机开关被丢回缺省 true"
    );

    // 换模型走同一条 load_model → 仍关。
    renderer.load_model(&loaded).expect("换模型");
    assert!(
        !renderer.core().stack().idle_enabled(),
        "换模型后待机开关被丢回缺省 true"
    );

    // 反向：打开后换模型仍开（开关是双向存活，不是恒 false）。
    renderer.core().set_idle_enabled(true);
    renderer.load_model(&loaded).expect("再换模型");
    assert!(
        renderer.core().stack().idle_enabled(),
        "打开后换模型开关被丢"
    );
}

/// **0.5.0 rc.1 参数绑定接线（回归）**：`ModelRendererCore` 装载后必须按该模型
/// moc3 参数 id 表建绑定，写参数入口把 13 条标准语义名解析成实际 id。
///
/// bai 是「标准命名」样本：11/13 可绑（缺两个 eye smile），且绑定结果与标准名
/// 逐字相同 → 与 0.4.x 的精确写入逐值一致（行为不变）。
#[test]
fn binder_wires_standard_names_to_actual_bai_ids() {
    let Some(model3) = bai_model3_path() else {
        println!("SKIP: 未找到 Bai 真实资产 `{BAI_MODEL3}`（门控跳过）");
        return;
    };
    let loaded = LoadedModel::resolve(ModelPackage::load(&model3).expect("load")).expect("resolve");

    let mut renderer = match OffscreenRenderer::new(64, 64) {
        Ok(r) => r,
        Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
            println!("SKIP: 无可用 headless GPU 后端（{notes}）");
            return;
        }
        Err(e) => panic!("离屏渲染器创建失败: {e}"),
    };
    renderer.load_model(&loaded).expect("加载 bai");

    let core = renderer.core();
    // bai：11/13；缺的两个如实 None，其余绑定到同名字面 id。
    assert_eq!(core.binder().resolve("ParamAngleX"), Some("ParamAngleX"));
    assert_eq!(
        core.binder().resolve("ParamMouthOpenY"),
        Some("ParamMouthOpenY")
    );
    assert_eq!(
        core.binder().resolve("ParamEyeLSmile"),
        None,
        "bai 缺 eye smile"
    );
    assert_eq!(
        core.binder().resolve("ParamEyeRSmile"),
        None,
        "bai 缺 eye smile"
    );

    // 0.5.0 rc.2 写名单：bai `LipSync` 空数组 → binder 回落到 `ParamMouthOpenY`
    // （与 0.4.1 逐值相同）；`EyeBlink` 组就是两个标准名。两者都证明 bai 行为不变。
    assert_eq!(
        core.mouth_write_ids(),
        vec!["ParamMouthOpenY".to_owned()].as_slice()
    );
    assert_eq!(
        core.eye_blink_write_ids(),
        vec!["ParamEyeLOpen".to_owned(), "ParamEyeROpen".to_owned()].as_slice()
    );
    // 名单写入不 panic（input 层；缺 id 时为空操作）。
    core.set_mouth_open(0.5);
    core.set_eye_open(1.0);

    // 写参数入口（input / final_override 两条）经 binder：bai 标准名可用。
    assert!(core.set_parameter("ParamAngleX", 5.0));
    assert!(core.override_parameter("ParamBodyAngleX", -3.0));
    // 真缺的参数仍返回 false（保持「缺 id 静默 false」，不 panic）。
    assert!(!core.set_parameter("ParamEyeLSmile", 1.0));
    assert!(!core.set_parameter("ParamNope", 1.0));
}

/// **0.5.0 rc.3 每模型 adapter（端到端）**：用 bai 真实资源 + 测试内 adapter
/// （不入库）验证「alias 覆盖 / disable_channels / loaded.param_bind 报告」一路
/// 到渲染核心的写参数入口。
#[test]
fn adapter_override_and_report_reach_the_core() {
    let Some(model3) = bai_model3_path() else {
        println!("SKIP: 未找到 Bai 真实资产 `{BAI_MODEL3}`（门控跳过）");
        return;
    };
    // bai 的 FileReferences 在仓库里是稳定的四件（moc / 1 张纹理 / physics / cdi）。
    let base = model3.parent().expect("model3 必有父目录");
    let model3_bytes = std::fs::read(&model3).expect("读 bai.model3.json");
    let mut resources = std::collections::BTreeMap::new();
    for name in [
        "bai.moc3",
        "bai.16384/texture_00_4096.png",
        "bai.physics3.json",
        "bai.cdi3.json",
    ] {
        resources.insert(
            name.to_owned(),
            std::fs::read(base.join(name)).expect("读 bai 资源"),
        );
    }
    // 测试专用 adapter：AngleX 显式改绑到模型确实存在的 AngleY；禁用嘴形。
    let adapter = br#"{"version":1,
        "param_aliases":{"ParamAngleX":"ParamAngleY"},
        "disable_channels":["ParamMouthForm"]}"#;
    let package = ModelPackage::from_memory_map_with_adapter(
        &model3_bytes,
        &resources,
        Some(adapter.as_slice()),
    )
    .expect("带 adapter 的内存组包");
    let loaded = LoadedModel::resolve(package).expect("bai moc3 解析");

    // 报告：adapter 生效；被禁用的嘴形算「不可写」；其余通道照常。
    let report = loaded.param_bind_report();
    assert_eq!(report.source(), l2d::asset::AdapterSource::External);
    assert!(report.missing().contains(&"ParamMouthForm"));
    assert!(!report.missing().contains(&"ParamAngleX"));
    assert_eq!(report.to_json()["source"], "adapter");

    let mut renderer = match OffscreenRenderer::new(64, 64) {
        Ok(r) => r,
        Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
            println!("SKIP: 无可用 headless GPU 后端（{notes}）");
            return;
        }
        Err(e) => panic!("离屏渲染器创建失败: {e}"),
    };
    renderer.load_model(&loaded).expect("加载带 adapter 的 bai");

    // alias 生效：写语义名 ParamAngleX 实际落到 ParamAngleY。
    assert!(renderer.core().set_parameter("ParamAngleX", 5.0));
    assert_eq!(
        renderer.core().stack().get_parameter("ParamAngleY"),
        Some(5.0),
        "adapter alias 必须把写入改绑到实际 id"
    );
    // disable_channels 生效：该通道一律不写（返回 false，不是静默写进别的 id）。
    assert!(!renderer.core().set_parameter("ParamMouthForm", 1.0));
    assert!(!renderer.core().override_parameter("ParamMouthForm", 1.0));
}
