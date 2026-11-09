//! 大肥鱼皮肤（DeepSeek-chan，Cubism 5.0）Rust 侧支持验证（0.5.0 rc.5）。
//!
//! **模型资源不入库**（用户本机 D:\live2dai\artifacts\skins\library\dafeiyu，
//! 授权/分发边界见皮肤目录自带须知）。本文件是 **env-gated 回归**：只有设置
//! `L2D_DAFEIYU_DIR`（皮肤根目录，内含 `runtime/*.model3.json`）才真跑，否则
//! 每个用例打印 SKIP 直接返回——跳过可见，不伪造通过（与 `bai_asset.rs` 同纪律）。
//!
//! 本地真跑：
//! `L2D_DAFEIYU_DIR=/mnt/d/live2dai/artifacts/skins/library/dafeiyu cargo test -p l2d --test dafeiyu_asset`
//!
//! 断言链（任务书 ①–⑤）：
//! 1. `ModelPackage::load` 成功，格式报告 = moc3 v5.0（非 v0 锚定，预览态）；
//! 2. `ParamBinder` 命中 11/13（缺 `ParamEyeLSmile`/`ParamEyeRSmile`，与 bai 相同）；
//! 3. `Groups[LipSync]` 空 → 口型名单回落 `ParamMouthOpenY`；
//! 4. `OffscreenRenderer` headless（**自动档位**）30 帧：像素非空、帧间有差异（idle 动）；
//!    口型再用「去掉 physics3 的真实资源内存变体」隔离验证：静态控制帧逐像素相同，
//!    写 `ParamMouthOpenY` 后像素必变、写回 0 精确回到基线（131 组物理每帧数百
//!    像素的漂移会把口型变化淹没，所以隔离是必要的）；
//!    注：本皮肤贴图到 **4096×8192**，默认 4096 档的 `max_texture_dimension_2d`
//!    会以 `TextureTooLarge` 装载失败 → headless 主用例用 0.5.0 rc.6 的自动档位
//!    （`OffscreenRenderer::new_for_model` → Tier8192）；口型隔离用例保留显式
//!    `Tier8192`，证明手动覆盖语义未变。
//! 5. 与 bai（标准命名 + v4.2）对照 binder 形状一致、格式/规模不同。
//!
//! 不扩 13 白名单、不新增语义通道；epsilon 官方样本未入库，本机不可用，
//! 大写 snake 绑定已由 `asset::param_bind` 的合成单测覆盖。

use std::path::{Path, PathBuf};

use l2d::asset::{ModelPackage, ParamBinder, STANDARD_PARAM_IDS};
use l2d::format::{MocFormat, MocVersion};
use l2d::model::{LoadedModel, ModelHandle};

/// 皮肤根目录（`L2D_DAFEIYU_DIR`）下的 model3.json；未设置 / 找不到 → `None`（SKIP）。
fn dafeiyu_model3() -> Option<PathBuf> {
    let Ok(dir) = std::env::var("L2D_DAFEIYU_DIR") else {
        println!("SKIP: 未设置 L2D_DAFEIYU_DIR（大肥鱼皮肤不入库）");
        return None;
    };
    let runtime = PathBuf::from(dir).join("runtime");
    let mut found = None;
    if let Ok(entries) = std::fs::read_dir(&runtime) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".model3.json") {
                found = Some(entry.path());
                break;
            }
        }
    }
    if found.is_none() {
        println!(
            "SKIP: {} 下没有 *.model3.json（L2D_DAFEIYU_DIR 应指向皮肤根目录）",
            runtime.display()
        );
    }
    found
}

/// 仓库内 bai（对照基线；白模型随仓库分发）。
fn bai_model3() -> Option<PathBuf> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("assets/models/bai/runtime/bai.model3.json");
    if path.is_file() {
        Some(path)
    } else {
        println!("SKIP: 未找到 bai 真实资产（对照基线缺失）");
        None
    }
}

fn parameter_ids(package: &ModelPackage) -> Vec<String> {
    ModelHandle::from_package(package)
        .expect("moc3 应可解析")
        .parameter_ids()
}

/// 13 条标准名里 binder 解析不到的（缺项清单按 `STANDARD_PARAM_IDS` 顺序）。
fn missing_standard(binder: &ParamBinder) -> Vec<&'static str> {
    STANDARD_PARAM_IDS
        .iter()
        .copied()
        .filter(|id| binder.resolve(id).is_none())
        .collect()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn differing_pixels(left: &[u8], right: &[u8]) -> usize {
    left.chunks_exact(4)
        .zip(right.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count()
}

// ─────────────────────────── ① 装载 + 格式报告（无 GPU）

#[test]
fn dafeiyu_package_loads_as_cubism5() {
    let Some(path) = dafeiyu_model3() else {
        return;
    };
    let package = ModelPackage::load(&path).expect("大肥鱼包应可加载");
    let manifest = package.manifest();
    assert_eq!(manifest.version, 3);
    assert_eq!(manifest.texture_files.len(), 5, "5 张 4096 贴图");
    assert!(manifest.physics_file.is_some(), "physics3 必须声明");
    assert!(manifest.display_info_file.is_some(), "cdi3 必须声明");
    let physics: serde_json::Value =
        serde_json::from_slice(package.physics_json().expect("physics3 必须被严格读到"))
            .expect("physics3 应是合法 JSON");
    assert_eq!(
        physics["PhysicsSettings"].as_array().map(Vec::len),
        Some(131),
        "physics3 组数（大肥鱼口径 131 组）"
    );
    // Groups：LipSync 空数组；EyeBlink 两个标准名（顺序按模型文件）。
    assert!(
        manifest.lip_sync_ids.is_empty(),
        "大肥鱼 LipSync 组是空数组"
    );
    assert_eq!(
        package.eye_blink_ids(),
        ["ParamEyeROpen", "ParamEyeLOpen"].as_slice()
    );

    // 格式报告：moc3 v5.0；v0 只锚定 bai 4.2 → 这里如实是「可解析、非 v0」。
    let report = package.compatibility_report();
    assert_eq!(report.format, MocFormat::Moc3(MocVersion::V5_0_0));
    assert_eq!(report.moc_version(), Some(MocVersion::V5_0_0));
    assert!(
        !report.is_supported_v0(),
        "v0 只锚定 bai 4.2（5.0 按预览态）"
    );
    assert!(
        report.issues().iter().any(|issue| matches!(
            issue,
            l2d::report::CompatibilityIssue::UnanchoredVersion(MocVersion::V5_0_0)
        )),
        "报告必须如实标注 5.0 未锚定：{:?}",
        report.issues()
    );

    // 真实 moc3 解析：版本交叉核对一致、509 参。
    let loaded = LoadedModel::resolve(package).expect("moc3 应可被运行时解析");
    assert_eq!(loaded.handle().moc_version(), Some(MocVersion::V5_0_0));
    assert_eq!(loaded.handle().stats().parameters, 509);
    assert_eq!(
        loaded.report().moc_version(),
        loaded.handle().moc_version(),
        "头探测与运行时解析版本必须一致"
    );
    // 无 pose3 / 无 adapter 是这台皮肤的现状（不是回归）。
    assert!(loaded.package().pose3_json().is_none());
    assert_eq!(
        loaded.package().param_adapter().source(),
        l2d::asset::AdapterSource::Missing
    );
}

// ─────────────────────────── ②③ binder + 口型回落（无 GPU）

#[test]
fn dafeiyu_binder_hits_eleven_of_thirteen() {
    let Some(path) = dafeiyu_model3() else {
        return;
    };
    let package = ModelPackage::load(&path).expect("load");
    let ids = parameter_ids(&package);
    assert_eq!(ids.len(), 509, "大肥鱼 moc3 参数表规模");

    let binder = ParamBinder::from_ids(&ids);
    assert_eq!(STANDARD_PARAM_IDS.len(), 13, "13 白名单不得扩");
    assert_eq!(
        missing_standard(&binder),
        vec!["ParamEyeLSmile", "ParamEyeRSmile"],
        "与 bai 相同的 11/13（缺两个 eye smile）"
    );
    for id in STANDARD_PARAM_IDS.iter().copied() {
        if missing_standard(&binder).contains(&id) {
            continue;
        }
        assert_eq!(
            binder.resolve(id),
            Some(id),
            "标准名必须逐字绑定到同名 id（写入口行为与 0.4.x 一致）"
        );
    }
    // 509 参里有 canonical 冲突（实测有非标准 id `AngleX` 与 `ParamAngleX` 撞
    // `anglex`）——冲突必须由标准名赢，否则语义通道会被私有 id 抢走。
    for conflict in binder.conflicts() {
        if STANDARD_PARAM_IDS
            .iter()
            .any(|id| l2d::asset::canonical(id) == conflict.canonical)
        {
            assert!(
                STANDARD_PARAM_IDS.contains(&conflict.kept.as_str()),
                "语义通道的冲突必须由 13 标准名赢：{conflict:?}"
            );
        }
    }

    // ③ Groups[LipSync].Ids 空 → rc.2 的名单合成走 binder 回落。
    assert!(package.manifest().lip_sync_ids.is_empty());
    assert_eq!(
        binder.resolve("ParamMouthOpenY"),
        Some("ParamMouthOpenY"),
        "口型点名必须回落到模型真实存在的 ParamMouthOpenY"
    );
}

// ─────────────────────────── ⑤ 与 bai 对照（无 GPU）

#[test]
fn dafeiyu_binder_matches_bai_shape() {
    let Some(dafeiyu_path) = dafeiyu_model3() else {
        return;
    };
    let Some(bai_path) = bai_model3() else {
        return;
    };
    let dafeiyu = ModelPackage::load(&dafeiyu_path).expect("load dafeiyu");
    let bai = ModelPackage::load(&bai_path).expect("load bai");
    let dafeiyu_binder = ParamBinder::from_ids(&parameter_ids(&dafeiyu));
    let bai_binder = ParamBinder::from_ids(&parameter_ids(&bai));

    // 语义形状一致：同样 11/13、同样缺两个 eye smile、都逐字保留标准名。
    assert_eq!(
        missing_standard(&dafeiyu_binder),
        missing_standard(&bai_binder),
        "大肥鱼与 bai 的缺项必须一致"
    );
    assert_eq!(
        missing_standard(&bai_binder),
        vec!["ParamEyeLSmile", "ParamEyeRSmile"]
    );
    for id in STANDARD_PARAM_IDS.iter().copied() {
        if missing_standard(&bai_binder).contains(&id) {
            continue;
        }
        assert_eq!(dafeiyu_binder.resolve(id), Some(id));
        assert_eq!(bai_binder.resolve(id), Some(id));
    }
    // 口型都靠 moc3 参数（两者 LipSync 组都空）。
    for package in [&dafeiyu, &bai] {
        assert!(package.manifest().lip_sync_ids.is_empty());
    }
    assert_eq!(
        dafeiyu_binder.resolve("ParamMouthOpenY"),
        Some("ParamMouthOpenY")
    );
    assert_eq!(
        bai_binder.resolve("ParamMouthOpenY"),
        Some("ParamMouthOpenY")
    );

    // 差异如实记录：格式档位与参数规模不同（不要求行为相同）。
    assert_eq!(
        bai.compatibility_report().format,
        MocFormat::Moc3(MocVersion::V4_2_0)
    );
    assert!(bai.compatibility_report().is_supported_v0());
    assert_eq!(
        dafeiyu.compatibility_report().format,
        MocFormat::Moc3(MocVersion::V5_0_0)
    );
    assert!(!dafeiyu.compatibility_report().is_supported_v0());
    assert!(
        parameter_ids(&dafeiyu).len() > parameter_ids(&bai).len(),
        "509 参（大肥鱼）> 128 参（bai）"
    );
}

// ─────────────────────────── ③④ headless 渲染（GPU 门控）

#[test]
fn dafeiyu_headless_renders_and_mouth_moves_pixels() {
    use l2d::renderer::{FIXED_DT_60HZ, OffscreenRenderer};

    let Some(path) = dafeiyu_model3() else {
        return;
    };
    let loaded = LoadedModel::resolve(ModelPackage::load(&path).expect("load")).expect("resolve");
    // 0.5.0 rc.6：按贴图最大边自动选最小够用档（4096×8192 → Tier8192）并装载；
    // ayagami 装载是全有或全无——Ok 即证明 5 张贴图全部上传成功。
    let mut renderer = match OffscreenRenderer::new_for_model(256, 256, &loaded) {
        Ok(renderer) => renderer,
        Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
            println!("SKIP: 无可用 headless GPU 后端（{notes}）");
            return;
        }
        Err(e) => panic!("自动档位装载失败: {e}"),
    };

    // ③ 口型名单：LipSync 空 → binder 回落 ParamMouthOpenY；眨眼用 Groups 名单。
    assert_eq!(
        renderer.core().mouth_write_ids(),
        vec!["ParamMouthOpenY".to_owned()].as_slice(),
        "口型名单必须回落到 ParamMouthOpenY"
    );
    assert_eq!(
        renderer.core().eye_blink_write_ids(),
        vec!["ParamEyeROpen".to_owned(), "ParamEyeLOpen".to_owned()].as_slice(),
        "眨眼名单用模型 Groups 顺序"
    );

    // ④ idle 30 帧：每帧非空、帧间确有变化（呼吸/摆头/物理在动）。
    let mut first = None;
    let mut last = None;
    let mut distinct_frames = 0_u32;
    let mut prev_hash = None;
    for _ in 0..30 {
        renderer.update(FIXED_DT_60HZ).expect("合法 dt");
        let frame = renderer.render_frame().expect("出帧");
        assert_eq!((frame.width, frame.height), (256, 256));
        assert!(
            frame.alpha_coverage() > 0.0,
            "每帧都必须有可见像素（alpha coverage = {}）",
            frame.alpha_coverage()
        );
        let hash = fnv1a(&frame.pixels);
        if let Some(prev) = prev_hash
            && prev != hash
        {
            distinct_frames += 1;
        }
        prev_hash = Some(hash);
        if first.is_none() {
            first = Some(frame.clone());
        }
        last = Some(frame);
    }
    let (first, last) = (first.expect("至少一帧"), last.expect("至少一帧"));
    assert!(
        distinct_frames >= 1,
        "30 帧里 idle 必须让至少一对相邻帧不同（实得 {distinct_frames} 次变化）"
    );
    assert_ne!(
        fnv1a(&first.pixels),
        fnv1a(&last.pixels),
        "首帧与末帧必须不同（idle 在动，不是静止画面）"
    );
}

// ─────────────────────────── ④ 口型隔离（去 physics 的真实资源内存变体）

/// 用同一台皮肤的真实 moc3 / 贴图 / cdi3 组装**去掉 physics3** 的内存包。
///
/// 为什么：本皮肤 physics3 有 131 组，每帧都在动（实测帧间差异可达数百像素），
/// 会把「写 ParamMouthOpenY」造成的局部像素变化淹没。去掉 Physics 后、再关掉
/// idle，画面是静态的，就能把口型这一路的变化单独量出来；moc3 / 贴图仍是这台
/// 皮肤的真实文件（模型资源不落盘、不入库，只在测试内组装）。
fn static_loaded_model(model3_path: &Path) -> LoadedModel {
    let raw = std::fs::read(model3_path).expect("读大肥鱼 model3");
    let mut value: serde_json::Value = serde_json::from_slice(&raw).expect("model3 应是合法 JSON");
    let base = model3_path.parent().expect("model3 必有父目录");
    let mut resources = std::collections::BTreeMap::new();
    {
        let refs = value
            .get("FileReferences")
            .and_then(|v| v.as_object())
            .expect("model3 缺 FileReferences");
        let mut add = |rel: &str| {
            resources.insert(
                rel.to_owned(),
                std::fs::read(base.join(rel)).unwrap_or_else(|e| panic!("读资源 {rel} 失败: {e}")),
            );
        };
        add(refs.get("Moc").and_then(|v| v.as_str()).expect("缺 Moc"));
        for texture in refs
            .get("Textures")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            add(texture.as_str().expect("Textures 必须全是字符串"));
        }
        if let Some(cdi) = refs.get("DisplayInfo").and_then(|v| v.as_str()) {
            add(cdi);
        }
    }
    value
        .get_mut("FileReferences")
        .and_then(|v| v.as_object_mut())
        .expect("FileReferences 必须是对象")
        .remove("Physics");
    let model3 = serde_json::to_vec(&value).expect("序列化 model3");
    let package = ModelPackage::from_memory_map(&model3, &resources).expect("无物理内存包应可组装");
    LoadedModel::resolve(package).expect("moc3 应可解析")
}

#[test]
fn dafeiyu_mouth_param_moves_pixels() {
    use l2d::renderer::{FIXED_DT_60HZ, OffscreenRenderer};

    let Some(path) = dafeiyu_model3() else {
        return;
    };
    let loaded = static_loaded_model(&path);
    let gpu =
        match l2d::renderer::GpuContext::headless_with_tier(l2d::renderer::RenderTier::Tier8192) {
            Ok(gpu) => std::sync::Arc::new(gpu),
            Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
                println!("SKIP: 无可用 headless GPU 后端（{notes}）");
                return;
            }
            Err(e) => panic!("headless GPU 上下文创建失败: {e}"),
        };
    let mut renderer = match OffscreenRenderer::with_gpu_context(gpu, 256, 256) {
        Ok(renderer) => renderer,
        Err(e) => panic!("离屏渲染器创建失败: {e}"),
    };
    renderer
        .load_model(&loaded)
        .expect("无物理的内存变体应可装载");

    // 静态控制：关 idle + 无 physics ⇒ 连续帧（含 update）必须逐像素相同。
    renderer.core().set_idle_enabled(false);
    for _ in 0..10 {
        renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    }
    let base = renderer.render_frame().expect("基线帧");
    renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    let control = renderer.render_frame().expect("控制帧");
    assert_eq!(
        fnv1a(&base.pixels),
        fnv1a(&control.pixels),
        "无 physics + 关 idle 时画面必须静态（否则口型变化无法归因）"
    );

    assert!(
        renderer.core().set_parameter("ParamMouthOpenY", 1.0),
        "模型必须有 ParamMouthOpenY"
    );
    renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    let opened = renderer.render_frame().expect("张口帧");
    let mouth_diff = differing_pixels(&base.pixels, &opened.pixels);
    println!("口型隔离（无 physics）：写 ParamMouthOpenY=1.0 后差异 {mouth_diff} 像素");
    assert!(
        mouth_diff > 0,
        "写 ParamMouthOpenY=1.0 必须改变可见像素（口型真的驱动渲染）"
    );
    assert_ne!(fnv1a(&base.pixels), fnv1a(&opened.pixels));

    assert!(renderer.core().set_parameter("ParamMouthOpenY", 0.0));
    renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    let closed = renderer.render_frame().expect("回零帧");
    assert_eq!(
        fnv1a(&base.pixels),
        fnv1a(&closed.pixels),
        "口型写回 0 后画面必须精确回到基线"
    );
}
