//! `render_model` — 用 `l2d` 公开 API 加载任意 model3 皮套包并离屏出一张 PNG。
//!
//! 走**统一加载路径**（`LoadedModel::resolve`：包 → 运行时句柄原子解析 +
//! 兼容报告 + 「头探测 × 运行时版本」交叉核对），再经 headless 离屏渲染
//! （固定 dt 更新 → RGBA 读回）落盘；面向**通用模型路径**，默认参数指向
//! 本仓库的 Bai 皮套（相对仓库根的路径，非绝对路径）。
//!
//! 用法（在仓库根执行）：
//!
//! ```text
//! cargo run -p l2d --example render_model -- [model3.json] [out.png] [size]
//! ```
//!
//! - `model3.json`：皮套清单路径（默认 `assets/models/bai/runtime/bai.model3.json`）
//! - `out.png`：输出 PNG 路径（默认 `verification/rust-bakeoff/selected-bai.png`）
//! - `size`：输出边长或 `宽x高`（默认 `1024`）

use std::path::PathBuf;

use l2d::asset::ModelPackage;
use l2d::model::LoadedModel;
use l2d::renderer::{FIXED_DT_60HZ, OffscreenRenderer};

/// 默认模型：仓库内 Bai 皮套（相对仓库根；不硬编码私有绝对路径）。
const DEFAULT_MODEL3: &str = "assets/models/bai/runtime/bai.model3.json";
/// 默认输出：bakeoff 选型验证产物位置。
const DEFAULT_OUTPUT: &str = "verification/rust-bakeoff/selected-bai.png";
const DEFAULT_SIZE: u32 = 1024;
/// 与 bakeoff 一致的固定 dt 步数（60 Hz 下模拟 2 秒）。
const SIM_STEPS: u32 = 120;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let model3_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MODEL3));
    let out_png = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT));
    let (width, height) = args
        .next()
        .as_deref()
        .map(parse_size)
        .transpose()?
        .unwrap_or((DEFAULT_SIZE, DEFAULT_SIZE));

    println!("=== l2d render_model ===");
    println!("model3: {}", model3_path.display());
    println!("output: {} ({}x{})", out_png.display(), width, height);

    // ---- 1) 统一加载：包 → 绑定句柄 + 兼容报告 + 版本交叉核对 ----------
    let package = ModelPackage::load(&model3_path)?;
    let manifest = package.manifest();
    println!(
        "manifest: model3 v{}，moc3=`{}`，纹理 {} 张，physics={}",
        manifest.version,
        manifest.moc3_file,
        manifest.texture_files.len(),
        manifest.physics_file.as_deref().unwrap_or("<无>"),
    );
    let loaded = LoadedModel::resolve(package)?;
    let report = loaded.report();
    let canvas = loaded.handle().canvas();
    let stats = loaded.handle().stats();
    if let Some(version) = loaded.handle().moc_version() {
        println!("runtime moc3 version: {version}");
    }
    println!(
        "canvas: {}x{} center=({},{}) pixels/unit={}",
        canvas.width_units,
        canvas.height_units,
        canvas.center_x_units,
        canvas.center_y_units,
        canvas.pixels_per_unit,
    );
    println!(
        "stats: artmeshes={} deformers={} parts={} params={} vertices={} triangles={}",
        stats.artmeshes,
        stats.deformers,
        stats.parts,
        stats.parameters,
        stats.vertices,
        stats.triangles,
    );
    for note in report.notes() {
        println!("note: {note}");
    }
    for issue in report.issues() {
        println!("issue: {issue}");
    }
    if !report.is_supported_v0() {
        println!("该模型未通过 v0 判定——仍尝试渲染（仅格式/核对层否决）……");
    }

    // ---- 2) headless 离屏渲染 ------------------------------------------
    let started = std::time::Instant::now();
    let mut renderer = OffscreenRenderer::new(width, height)?;
    println!("gpu adapter: {}", renderer.gpu_adapter());
    renderer.load_model(&loaded)?;

    // 外部输入走 core().set_parameter(...)（input 层，优先级高于 idle、
    // 低于物理输出）；本示例保持与 bakeoff 一致的纯 idle+物理序列，
    // 以便产物可与 verification/rust-bakeoff 参照图逐像素对比。

    for _ in 0..SIM_STEPS {
        renderer.update(FIXED_DT_60HZ)?;
    }
    let frame = renderer.render_frame()?;
    println!(
        "frame: {}x{}，alpha coverage {:.2}%（耗时 {:?}）",
        frame.width,
        frame.height,
        frame.alpha_coverage() * 100.0,
        started.elapsed(),
    );
    if frame.alpha_coverage() == 0.0 {
        return Err("渲染结果全透明——没有画出任何内容".into());
    }

    frame.save_png(&out_png)?;
    println!("PNG written: {}", out_png.display());
    Ok(())
}

/// 解析尺寸参数：`N` 或 `宽x高`。
fn parse_size(spec: &str) -> Result<(u32, u32), String> {
    let invalid = || format!("非法尺寸 `{spec}`（应为 N 或 宽x高，例如 1024 / 1024x768）");
    let parse_u32 = |s: &str| s.trim().parse::<u32>().ok().filter(|n| *n > 0);
    match spec.split_once(['x', 'X']) {
        Some((w, h)) => match (parse_u32(w), parse_u32(h)) {
            (Some(w), Some(h)) => Ok((w, h)),
            _ => Err(invalid()),
        },
        None => {
            let n = parse_u32(spec).ok_or_else(invalid)?;
            Ok((n, n))
        }
    }
}
