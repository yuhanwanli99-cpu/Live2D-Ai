//! 大肥鱼 8192 贴图的**渲染档位自适应**回归（0.5.0 rc.6 / task-14）。
//!
//! env-gated：`L2D_DAFEIYU_DIR`（皮肤根目录；与 `dafeiyu_asset.rs` 同一门控，
//! 未设置全部 SKIP）。模型资源不入库。
//!
//! 本地真跑：
//! `L2D_DAFEIYU_DIR=/mnt/d/live2dai/artifacts/skins/library/dafeiyu cargo test -p l2d --test dafeiyu_tier_auto`
//!
//! 断言：
//! 1. 贴图最长边 8192 → 最小够用档 **Tier8192**（不是默认 4096、也不是 16384）；
//! 2. 默认档（4096 设备上限）装载**会失败**——证明「不自动选档就会晦涩失败」；
//! 3. `OffscreenRenderer::new_for_model` 自动选档后 **5 张贴图全部装载**并出画
//!    （ayagami 装载严格全有或全无：Ok 即全部成功）；
//! 4. 显式覆盖语义不变（`new_with_tier` / `with_gpu_context` 仍是公开手动路径）；
//! 5. 设备上限检查 `texture_dimension_fits` 给出**可读原因**（wasm load/swap 用同一函数）。

use std::path::PathBuf;

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{FIXED_DT_60HZ, GpuContext, ModelRendererCore, OffscreenRenderer, RenderTier};

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

#[test]
fn auto_tier_picks_minimal_covering_and_device_check_is_readable() {
    let Some(path) = dafeiyu_model3() else {
        return;
    };
    let package = ModelPackage::load(&path).expect("大肥鱼包应可加载");
    assert_eq!(
        package.texture_max_dimension(),
        Some(8192),
        "texture_00 实测 4096×8192 → 最长边 8192"
    );
    let loaded = LoadedModel::resolve(package).expect("moc3 应可解析");

    // 最小够用档：8192（不是默认 4096，也不许默认抬到 16384）。
    assert_eq!(
        ModelRendererCore::required_tier(&loaded),
        Some(RenderTier::Tier8192)
    );
    assert_ne!(
        ModelRendererCore::required_tier(&loaded),
        Some(RenderTier::Tier16384),
        "不得把 16384 当默认"
    );
    assert_eq!(
        RenderTier::smallest_covering(8192),
        Some(RenderTier::Tier8192)
    );
    assert_eq!(
        RenderTier::smallest_covering(4096),
        Some(RenderTier::Tier4096),
        "4096 贴图仍用默认档（普通模型零变化）"
    );
    assert_eq!(RenderTier::DEFAULT, RenderTier::Tier4096, "默认档语义未变");

    // 设备上限检查（wasm load/swap 与 native 共用）：够用 → Ok；不够 → 可读原因。
    assert_eq!(
        l2d::renderer::texture_dimension_fits(Some(8192), 8192),
        Ok(())
    );
    let reason = l2d::renderer::texture_dimension_fits(Some(8192), 4096)
        .expect_err("8192 贴图装不进 4096 设备");
    println!("4096 设备的可读原因：{reason}");
    assert!(
        reason.contains("8192") && reason.contains("4096"),
        "可读原因必须带两个数字：{reason}"
    );
}

#[test]
fn default_tier_fails_but_auto_tier_loads_all_five_textures() {
    let Some(path) = dafeiyu_model3() else {
        return;
    };
    let loaded = LoadedModel::resolve(ModelPackage::load(&path).expect("load")).expect("resolve");

    // ① 默认档（4096 设备上限）装载失败：这是本任务要消灭的晦涩失败路径。
    let mut default = match OffscreenRenderer::new(64, 64) {
        Ok(renderer) => renderer,
        Err(l2d::renderer::RenderError::NoGpuBackend(notes)) => {
            println!("SKIP: 无可用 headless GPU 后端（{notes}）");
            return;
        }
        Err(e) => panic!("默认档离屏渲染器创建失败: {e}"),
    };
    match default.load_model(&loaded) {
        Ok(()) => panic!("默认 4096 档不该装下 8192 贴图（任务前提不成立）"),
        Err(e) => println!("默认 4096 档装载失败（预期，可读错误）：{e}"),
    }
    drop(default);

    // ② 自动档位：new_for_model 选 Tier8192 并装载——5 张贴图严格全有或全无。
    let mut renderer = match OffscreenRenderer::new_for_model(256, 256, &loaded) {
        Ok(renderer) => renderer,
        Err(e) => panic!("自动档位装载失败: {e}"),
    };
    assert_eq!(
        renderer
            .gpu_context()
            .device()
            .limits()
            .max_texture_dimension_2d,
        8192,
        "最小够用档 = 8192（不是 16384）"
    );
    for _ in 0..3 {
        renderer.update(FIXED_DT_60HZ).expect("合法 dt");
    }
    let frame = renderer.render_frame().expect("自动档位应能出画");
    assert!(
        frame.alpha_coverage() > 0.0,
        "自动档位渲染必须出画（贴图全部可用）"
    );
    println!(
        "自动档位（Tier8192）装载 5 贴图成功，alpha coverage = {:.2}%",
        frame.alpha_coverage() * 100.0
    );

    // ③ 显式覆盖语义不变：new_with_tier 仍是公开手动路径（这里只验证可创建，
    // 不重复装载 44MB 贴图）。
    assert!(
        OffscreenRenderer::new_with_tier(64, 64, RenderTier::Tier8192).is_ok(),
        "显式档位覆盖必须保持可用"
    );
    // 显式选 4096（低于需求）也允许创建：手动语义不因自动档新增而改变。
    assert!(
        GpuContext::headless_with_tier(RenderTier::Tier4096).is_ok(),
        "显式低档位仍可创建（装载失败由调用方处理）"
    );
}
