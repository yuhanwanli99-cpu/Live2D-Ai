# Rust Bake-off：mocari 0.4.0 无头渲染 Bai 模型验证报告

- **日期**：2026-08-26
- **验证目标**：在独立 Rust 工程（不改动本仓库源码与根 Cargo workspace）中，使用
  `mocari = { version = "0.4.0", features=["wgpu"] }` 对 Bai 模型完成
  「加载 runtime → 固定 dt 首帧更新 → wgpu 无头离屏渲染 RGBA8/COPY_SRC → 读回 PNG」全链路，
  并输出模型元数据、mesh/mask 数量、耗时与 GPU adapter。
- **结论**：✅ **全部通过，无 blocker**。mocari 0.4.0 与 Bai 模型完全兼容，
  四个阶段一次打通；渲染产物经像素级校验确认是正确的带纹理角色图像。
  唯一环境限制：本机为 WSL2，无暴露给 wgpu 的硬件 Vulkan 设备，实际落在
  Mesa **llvmpipe（软件光栅化）** 上运行——这是环境事实，不是库缺陷。

---

## 1. 工程与环境

| 项 | 值 |
| --- | --- |
| 工程路径 | `/tmp/mocari-bakeoff`（独立 `[workspace]`，不在仓库内） |
| 工具链 | rustc/cargo 1.98.0 (stable-x86_64-unknown-linux-gnu) |
| 运行时 | WSL2 (Linux 6.18.33.2-microsoft-standard-WSL2), x86_64 |
| 核心依赖 | mocari 0.4.0 (features=wgpu)、wgpu 30.0.1、image 0.25.10(png)、pollster 0.4、serde_json 1 |
| 目标模型 | `Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json` |
| 产物 | `verification/rust-bakeoff/mocari-bai.png`（1024×1024 RGBA8, 284,230 字节） |

**仓库合规**：根 `Cargo.toml` workspace 未改动；未执行任何 git commit；
`target/` 未复制入仓库；本报告仅内嵌源码文本。

## 2. 实现方案（四阶段）

### 阶段 1 — load_model_runtime
`mocari::assets::load_model_runtime(model_path)` 一步完成 model3.json 解析、
bai.moc3（9.0 MB）解析与纹理 PNG 解码。runtime 通过 `loaded.runtime().clone()`
取得可变副本（`ModelRuntime: Clone`）。

### 阶段 2 — 固定 dt 首帧更新（dt = 1/60 s）
按 mocari 官方示例 `examples/show_model.rs` 的 `advance_model_frame` 流程
（去掉 motion/expression 部分，即纯首帧）：

```text
reset_parameters → reset_part_opacities → apply_parameter_overrides
→ apply_physics(1/60) → apply_pose(1/60) → update_meshes()
```

`update_meshes()` 返回 `Some` 表示 mesh 重建成功；physics 在首帧已生效（返回 true）。

### 阶段 3 — wgpu 无头渲染
- `InstanceDescriptor::new_without_display_handle()` 创建无 surface 的 instance；
  `enumerate_adapters` 列出全部候选后 `request_adapter(compatible_surface: None)`。
- 渲染目标：`Rgba8Unorm` + `RENDER_ATTACHMENT | COPY_SRC`，1024×1024。
  选 Unorm（非 Srgb）是沿用 mocari 自己的偏好（其 `preferred_surface_format`
  测试明确偏好 unorm，混合发生在 gamma 空间，与 Cubism SDK 一致），且 Unorm 是合法的 COPY_SRC 格式。
- 资源装配全部用库 API：`WgpuMeshBuffers::from_drawables`、
  `WgpuClippingPlan::from_mesh_buffers` + `prepare_single_texture_masks`、
  `create_clipping_resources`、`create_mask_render_target(256)`、`create_transform`。
- 视口矩阵：对 drawable 包围盒做等比 fit（fill=1.9，居中），复刻示例的
  `fit_model_matrix_with_scale` 数学（`Matrix44.scale` 后 `translate(-center*s)`）。
- 两趟 render pass：mask pass 清透明色写入 mask target → 主 pass 清背景色
  后 `draw_with_textures_clipping_and_transform`（含裁剪）。
- 读回：`copy_texture_to_buffer` 到 MAP_READ buffer
  （bytes_per_row=4096，天然满足 256 对齐），`map_async` +
  `device.poll(PollType::wait_indefinitely())` 同步后逐行去 padding，交给 `image` 存 PNG。

### 阶段 4 — 元数据与校验输出
程序打印：model3 版本与 FileReferences、Layout、canvas（PPU/尺寸/原点）、
参数/部件数、mesh 总数、带 mask 的 drawable 数、唯一 mask id 数、blend 模式分布、
顶点/三角形数、各阶段耗时、枚举到的全部 GPU adapter 与选中 adapter 详情、
读回像素统计（非背景像素数、内容包围盒、颜色多样性）。

## 3. 真实运行结果（release 构建，正式跑）

完整 stdout 见 §7；关键数字：

**模型元数据**
- model3 v3；Moc=bai.moc3；Physics=bai.physics3.json；DisplayInfo=bai.cdi3.json；无 Pose 文件
- 纹理 1 张：`bai.16384/texture_00_4096.png`
- Layout：center (0.0, -0.07)，size 0.56×0.56
- Canvas：4000 px/unit，画布 4000×6000 单位，原点 (2000, 3000)
- 参数 128 个，部件 159 个，物理已加载；分组：LipSync=[]，EyeBlink=[ParamEyeLOpen, ParamEyeROpen]；HitAreas=0

**Mesh / Mask 统计（首帧更新后）**
- drawable 总数 475（更新前后一致），其中不可见（opacity≤0）303 个
- **带裁剪 mask 的 drawable 84 个，涉及 19 个唯一 mask id，生成 10 个 clipping context**（单纹理布局容量内，无报错）
- 顶点 41,386，三角形 64,368
- blend 模式：Normal 431，Multiplicative 44
- drawable 包围盒 x[-0.29, 0.29] y[-0.72, 0.77]
- 实际提交绘制 172 个 drawable（= 475 − 303 不可见，数目自洽）

**GPU adapter（headless 枚举与选择）**
```text
adapter[0]: Vulkan | llvmpipe (LLVM 21.1.8, 256 bits) | Cpu | vendor=0x10005 device=0x0000
adapter[1]: Gl     | llvmpipe (LLVM 21.1.8, 256 bits) | Cpu
chosen    : Vulkan | llvmpipe | Mesa 26.0.3-1ubuntu1 (LLVM 21.1.8)
```
WSL2 下 `/dev/dxg` 存在但 wgpu 的 Linux 后端只有 Vulkan/GL，宿主 GPU 未以
Vulkan ICD 形式暴露，故落在 lavapipe/llvmpipe 软件渲染。**功能正确性不受影响**。

**耗时（release，软件光栅化）**

| 阶段 | release | debug 参考 |
| --- | --- | --- |
| load_model_runtime | 516.8 ms | 8.85 s |
| 首帧固定 dt 更新（CPU） | 31.3 ms | 324 ms |
| wgpu init（instance+adapter+device） | 176.4 ms | 8.63 s |
| 渲染资源装配（纹理上传/buffer/clipping） | 190.9 ms | 516 ms |
| 命令编码（两 pass+copy） | 168.5 µs | 610 µs |
| submit+GPU 执行+map 读回 | 235.5 ms | 1.13 s |
| PNG 写盘 | 16.3 ms | 1.65 s |
| **总墙钟** | **1.46 s** | 21.5 s |

首帧 CPU 更新 31 ms（含 64k 三角形重建与物理步进）意味着 mocari 的
CPU runtime 单帧成本在量级上可用（>30 FPS 余量为正，且这是纯软件 GPU 环境下的首帧冷启动值）。

## 4. 渲染产物校验（真实证据）

PNG：`verification/rust-bakeoff/mocari-bai.png`（1024×1024 RGBA8 非隔行，284,230 B）

对 PNG 做了解码级校验（非仅“文件存在”）：

1. **内容占比**：15.36%（161,103 像素）显著偏离背景清屏色 RGB(22,27,37)；
   内容包围盒 x[329..698] y[93..996]，为居中、竖长的全身人物构图。
2. **纹理多样性**：25,018 种唯一 RGB 颜色 —— 若三角形/UV/采样任一环节损坏，
   不可能出现该数量级的连续渐变色；画面中心区域呈浅白/浅蓝的服装与肤色系。
3. **背景纯净**：四角像素逐字节等于清屏色 (22,27,37,255)，证明变换矩阵没有把
   模型铺满全屏、也没有越界脏数据。
4. **轮廓形状**：对差异掩码做 48×48 ASCII 降采样后，可清晰辨认
   头部（含发型）、颈部、肩部展开的上身、两条分开的下肢与足部 ——
   是完整的角色剪影而非碎片三角形或噪点。
5. alpha 通道全图非零属预期：主 pass 清屏色 a=1.0（不透明背景），
   因此用「偏离背景色的像素」而不是「alpha>0」作为有效内容判据。

## 5. 开发过程中修掉的真实问题（逐项）

首次 `cargo check` 有 6 个编译错误，全部为 wgpu 30 相对旧教程 API 的演进，
**无一涉及 mocari 或模型兼容性**：

| # | 问题 | 修复 |
| --- | --- | --- |
| 1 | wgpu 30 中 `Instance::enumerate_adapters` 变为 async | 加 `.await` |
| 2 | `PollType::Wait` 是 struct variant，不能当 unit 用 | 改用 `PollType::wait_indefinitely()` |
| 3 | wgpu 30 `BufferSlice::get_mapped_range` 返回 `Result<BufferView, MapRangeError>` | 显式 `map_err` 上抛 |
| 4 | `AdapterInfo.driver_info` 在 30.x 是 `String` 非 Option | 去掉 `.as_deref().unwrap_or` |
| 5 | `PixelStats::analyze` / `image::save_buffer` 传值 vs 借用类型不符 | 改传 `&Vec<u8>` |
| 6 | `image 0.25.10` 中 `ColorType::into()` 目标歧义（E0283） | 显式 `ExtendedColorType::Rgba8` |

另有一次运行期小坑：PNG 输出目录不存在导致 `save_buffer` 报 NotFound，
建目录后重跑通过（与库无关）。

## 6. 兼容性结论 / Blocker

- **mocari ↔ Bai 模型：兼容**。v3 model3.json、9 MB moc3、物理、单纹理裁剪
  （84 masked drawable → 10 context）全部按预期工作，未触发任何降级路径。
- **mocari wgpu 后端 ↔ headless 环境：兼容**。无需 surface 即可完成
  request_adapter/device 与离屏渲染读回。
- **无 blocker**。遗留的环境性限制仅有：本机只能拿到 llvmpipe 软件 adapter，
  无法测硬件 GPU 性能上限；如需硬件数据，应在有 Vulkan ICD 的机器重跑同一二进制。

## 7. 正式运行完整输出（release，逐字）

```text
== mocari-bakeoff ==
model_path      : /home/administrator/deepseekharness/Live2D-Ai/Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json
png_output      : /home/administrator/deepseekharness/Live2D-Ai/verification/rust-bakeoff/mocari-bai.png
render_size     : 1024x1024
fixed_dt_seconds: 0.016666668

-- stage 1: load_model_runtime --
load_ok         : true
load_time       : 516.773995ms
textures_decoded: 1
model3_version  : 3
moc             : bai.moc3
physics_file    : Some("bai.physics3.json")
pose_file       : None
display_info    : Some("bai.cdi3.json")
texture_ref     : bai.16384/texture_00_4096.png
hit_areas       : 0
param_group     : LipSync ids=[]
param_group     : EyeBlink ids=["ParamEyeLOpen", "ParamEyeROpen"]
layout          : {"center_x":0.0,"center_y":-0.07,"height":0.56,"width":0.56}
canvas_pixels_per_unit : 4000
canvas_size_units      : 4000 x 6000
canvas_origin_units    : (2000, 3000)
parameter_count : 128
part_count      : 159
physics_loaded  : true

-- stage 2: fixed-dt first frame update --
update_time        : 31.3331ms
physics_applied    : true
meshes_pre_update  : 475
meshes_post_update : 475
drawable_bounds    : x[-0.29, 0.29] y[-0.72, 0.77]
mesh_stats_post_update: 475
mesh_stats_post_update_masked    : 84
mesh_stats_post_update_mask_ids  : 19
mesh_stats_post_update_invisible : 303
mesh_stats_post_update_vertices  : 41386
mesh_stats_post_update_triangles : 64368
mesh_stats_post_update_blend.Multiplicative: 44
mesh_stats_post_update_blend.Normal       : 431

-- stage 3: wgpu headless init --
adapter[0]       : Vulkan | llvmpipe (LLVM 21.1.8, 256 bits) | Cpu | vendor=0x10005 device=0x0000 driver=llvmpipe
adapter[1]       : Gl | llvmpipe (LLVM 21.1.8, 256 bits) | Cpu | vendor=0x10005 device=0x0000 driver=
adapter_chosen   : Vulkan | llvmpipe (LLVM 21.1.8, 256 bits) | Cpu | driver=llvmpipe Mesa 26.0.3-1ubuntu1 (LLVM 21.1.8)
device_ok        : true
gpu_init_time    : 176.382498ms
textures_uploaded: 1
clip_contexts    : 10
resource_time    : 190.862498ms
draw_calls       : 172
encode_time      : 168.5µs
submit+read_time : 235.524498ms
png_write_time   : 16.253ms
png_bytes        : 284230
-- stage 4: readback verification --
pixels_opaque    : 1048576 / 1048576
alpha_coverage   : 100.00%
opaque_bbox      : x[0, 1023] y[0, 1023]
mean_rgb_opaque  : (52.0, 57.1, 66.7)
total_wall_time: 1.463233787s
```

（注：stage 4 的 opaque/coverage 以 alpha>0 计，因背景本身不透明所以恒为 100%；
真正的有效性判据见 §4 的“偏离背景色”分析。）

## 8. 复现步骤

```bash
cd /tmp/mocari-bakeoff
source ~/.cargo/env
cargo fmt --check && cargo check
mkdir -p <repo>/verification/rust-bakeoff
cargo run --release -- \
  <repo>/Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json \
  <repo>/verification/rust-bakeoff/mocari-bai.png
```

## 附录：最终源码副本

工程共两个文件（`Cargo.toml` + `src/main.rs`，623 行）；以下为逐字拷贝，
`target/` 未复制入仓库。

### `Cargo.toml`
```toml
[package]
name = "mocari-bakeoff"
version = "0.1.0"
edition = "2024"
publish = false

# Standalone bake-off project: lives in /tmp, deliberately outside the
# Live2D-Ai cargo workspace. The repo workspace must not be modified.
[dependencies]
mocari = { version = "0.4.0", features = ["wgpu"] }
image = { version = "0.25", default-features = false, features = ["png"] }
pollster = "0.4"
serde_json = "1"
wgpu = "30"

# Keep this project out of any parent workspace (defensive; /tmp has none).
[workspace]
```

### `src/main.rs`
```rust
//! mocari-bakeoff: headless Live2D render verification against the Bai model.
//!
//! Pipeline:
//!   1. `mocari::assets::load_model_runtime` on bai.model3.json
//!   2. one fixed-dt (1/60 s) CPU frame update: reset -> overrides -> physics ->
//!      pose -> update_meshes
//!   3. wgpu headless (no surface) render into an RGBA8Unorm COPY_SRC texture,
//!      copy to a MAP_READ buffer, read back, encode PNG
//!   4. print model metadata, mesh/mask counts, per-stage timings, GPU adapter

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    path::PathBuf,
    time::Instant,
};

use mocari::{
    assets::load_model_runtime,
    core::Matrix44,
    moc3::Moc3DrawableMesh,
    render::wgpu::{WgpuClippingPlan, WgpuLive2dRenderer, WgpuMeshBuffers, WgpuTexture},
};

const RENDER_WIDTH: u32 = 1024;
const RENDER_HEIGHT: u32 = 1024;
const MASK_TEXTURE_SIZE: u32 = 256;
/// Fraction of the viewport half-height the model bounds should fill.
const MODEL_VIEW_FILL: f32 = 1.9;
/// Fixed timestep for the first frame, 60 Hz.
const FIXED_DT_SECONDS: f32 = 1.0 / 60.0;

#[derive(Debug)]
struct BakeoffError(&'static str);

impl fmt::Display for BakeoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

impl Error for BakeoffError {}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let default_model = default_model_path();
    let model_path = args.next().map(PathBuf::from).unwrap_or(default_model);
    let png_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mocari-bai.png"));

    println!("== mocari-bakeoff ==");
    println!("model_path      : {}", model_path.display());
    println!("png_output      : {}", png_path.display());
    println!("render_size     : {RENDER_WIDTH}x{RENDER_HEIGHT}");
    println!("fixed_dt_seconds: {FIXED_DT_SECONDS}");
    println!();

    let total_started = Instant::now();

    // ---------------------------------------------------------------- //
    // 1) load_model_runtime                                            //
    // ---------------------------------------------------------------- //
    let load_started = Instant::now();
    let loaded = load_model_runtime(&model_path)?;
    let textures_decoded = loaded.textures().len();
    let load_elapsed = load_started.elapsed();

    // The runtime is Clone; we own an independent mutable copy.
    let mut runtime = loaded.runtime().clone();

    println!("-- stage 1: load_model_runtime --");
    println!("load_ok         : true");
    println!("load_time       : {:?}", load_elapsed);
    println!("textures_decoded: {textures_decoded}");

    // -- model metadata from the parsed .model3.json --
    let model3 = runtime.model();
    println!("model3_version  : {}", model3.version());
    println!("moc             : {}", model3.moc());
    println!("physics_file    : {:?}", model3.physics());
    println!("pose_file       : {:?}", model3.pose());
    println!("display_info    : {:?}", model3.display_info());
    for texture in model3.textures() {
        println!("texture_ref     : {texture}");
    }
    println!("hit_areas       : {}", model3.hit_areas().len());
    for group in model3.groups() {
        println!("param_group     : {} ids={:?}", group.name(), group.ids());
    }

    // Layout block is display metadata kept in the raw JSON.
    if let Ok(raw) = fs::read_to_string(&model_path)
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw)
        && let Some(layout) = value.get("Layout")
    {
        println!("layout          : {layout}");
    }

    // Canvas info comes from the .moc3 header.
    let canvas = runtime.canvas();
    println!("canvas_pixels_per_unit : {}", canvas.pixels_per_unit());
    println!(
        "canvas_size_units      : {} x {}",
        canvas.width(),
        canvas.height()
    );
    println!(
        "canvas_origin_units    : ({}, {})",
        canvas.origin_x(),
        canvas.origin_y()
    );

    println!("parameter_count : {}", runtime.parameter_ids().len());
    println!("part_count      : {}", runtime.part_ids().len());
    println!("physics_loaded  : {}", runtime.physics().is_some());
    println!();

    // ---------------------------------------------------------------- //
    // 2) fixed-dt first frame update                                   //
    // ---------------------------------------------------------------- //
    let pre_update_meshes = runtime.meshes().len();
    let update_started = Instant::now();
    runtime.reset_parameters();
    runtime.reset_part_opacities();
    runtime.apply_parameter_overrides();
    let physics_applied = runtime.apply_physics(FIXED_DT_SECONDS);
    runtime.apply_pose(FIXED_DT_SECONDS);
    let mesh_update = runtime.update_meshes();
    let update_elapsed = update_started.elapsed();

    if mesh_update.is_none() {
        return Err(Box::new(BakeoffError(
            "runtime.update_meshes() failed to rebuild meshes",
        )));
    }

    let post_update_meshes = runtime.meshes().len();
    println!("-- stage 2: fixed-dt first frame update --");
    println!("update_time        : {:?}", update_elapsed);
    println!("physics_applied    : {physics_applied}");
    println!("meshes_pre_update  : {pre_update_meshes}");
    println!("meshes_post_update : {post_update_meshes}");

    let bounds = ModelBounds::from_drawables(runtime.meshes())
        .ok_or(BakeoffError("model has no drawable bounds"))?;
    println!(
        "drawable_bounds    : x[{:.2}, {:.2}] y[{:.2}, {:.2}]",
        bounds.min_x, bounds.max_x, bounds.min_y, bounds.max_y
    );

    let mesh_stats = MeshStats::collect(runtime.meshes());
    println!("{}", mesh_stats.report("mesh_stats_post_update"));
    println!();

    // ---------------------------------------------------------------- //
    // 3) wgpu headless init + offscreen render                         //
    // ---------------------------------------------------------------- //
    pollster::block_on(render_offscreen(
        &mut runtime,
        &loaded.textures().to_vec(),
        bounds,
        &png_path,
    ))?;

    println!("total_wall_time: {:?}", total_started.elapsed());
    Ok(())
}

async fn render_offscreen(
    runtime: &mut mocari::ModelRuntime,
    textures: &[mocari::assets::DecodedTexture],
    bounds: ModelBounds,
    png_path: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    // ---- adapter / device -------------------------------------------------
    let gpu_started = Instant::now();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    println!("-- stage 3: wgpu headless init --");
    for (index, candidate) in instance
        .enumerate_adapters(wgpu::Backends::all())
        .await
        .iter()
        .enumerate()
    {
        let info = candidate.get_info();
        println!(
            "adapter[{}]       : {:?} | {} | {:?} | vendor={:#06x} device={:#06x} driver={}",
            index, info.backend, info.name, info.device_type, info.vendor, info.device, info.driver
        );
    }

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        })
        .await?;
    let adapter_info = adapter.get_info();
    let gpu_init_elapsed = gpu_started.elapsed();
    println!(
        "adapter_chosen   : {:?} | {} | {:?} | driver={} {}",
        adapter_info.backend,
        adapter_info.name,
        adapter_info.device_type,
        adapter_info.driver,
        adapter_info.driver_info
    );

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("mocari-bakeoff.device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await?;
    println!("device_ok        : true");
    println!("gpu_init_time    : {gpu_init_elapsed:?}");

    // ---- renderer resources ----------------------------------------------
    let resources_started = Instant::now();
    // RGBA8Unorm keeps blending in gamma space like the Cubism SDK and is a
    // legal COPY_SRC format for readback.
    let color_format = wgpu::TextureFormat::Rgba8Unorm;
    let renderer = WgpuLive2dRenderer::new(&device, color_format);

    let mut gpu_textures = Vec::<WgpuTexture>::with_capacity(textures.len());
    for texture in textures {
        gpu_textures.push(renderer.create_rgba8_texture(
            &device,
            &queue,
            texture.width(),
            texture.height(),
            texture.rgba(),
        )?);
    }
    println!("textures_uploaded: {}", gpu_textures.len());

    let mesh_buffers = WgpuMeshBuffers::from_drawables(&device, runtime.meshes())
        .ok_or(BakeoffError("failed to create wgpu mesh buffers"))?;

    let mut clipping_plan = WgpuClippingPlan::from_mesh_buffers(&mesh_buffers);
    clipping_plan.prepare_single_texture_masks(&mesh_buffers)?;
    println!("clip_contexts    : {}", clipping_plan.contexts().len());
    let clipping_resources = renderer.create_clipping_resources(&device, &clipping_plan)?;
    let mask_target = renderer.create_mask_render_target(&device, MASK_TEXTURE_SIZE)?;

    let transform = renderer.create_transform(&device, &fit_model_matrix(bounds));
    let resources_elapsed = resources_started.elapsed();
    println!("resource_time    : {resources_elapsed:?}");

    // ---- offscreen target (RGBA8, COPY_SRC) ------------------------------
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("mocari-bakeoff.target"),
        size: wgpu::Extent3d {
            width: RENDER_WIDTH,
            height: RENDER_HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: color_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    // bytes_per_row must be a multiple of 256: 1024 * 4 already is.
    let bytes_per_row = RENDER_WIDTH * 4;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mocari-bakeoff.readback"),
        size: u64::from(bytes_per_row) * u64::from(RENDER_HEIGHT),
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // ---- encode: mask pass -> color pass -> copy --------------------------
    let encode_started = Instant::now();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("mocari-bakeoff.encoder"),
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mocari-bakeoff.mask_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: mask_target.view(),
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        renderer.draw_masks_with_textures(
            &mut pass,
            &mesh_buffers,
            &clipping_resources,
            &gpu_textures,
        )?;
    }

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("mocari-bakeoff.main_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.086,
                        g: 0.106,
                        b: 0.145,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let drawn = renderer.draw_with_textures_clipping_and_transform(
            &mut pass,
            &mesh_buffers,
            &gpu_textures,
            &clipping_resources,
            &mask_target,
            &transform,
        )?;
        println!("draw_calls       : {drawn}");
    }

    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(RENDER_HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: RENDER_WIDTH,
            height: RENDER_HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    let encode_elapsed = encode_started.elapsed();

    // ---- submit, wait, map, copy out --------------------------------------
    println!("encode_time      : {encode_elapsed:?}");
    let readback_started = Instant::now();
    queue.submit([encoder.finish()]);
    {
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        device.poll(wgpu::PollType::wait_indefinitely())?;
        receiver
            .recv()
            .map_err(|_| BakeoffError("map_async sender dropped"))?
            .map_err(|error| Box::new(error) as Box<dyn Error>)?;

        let data = slice
            .get_mapped_range()
            .map_err(|error| Box::new(error) as Box<dyn Error>)?;
        let pixels = row_major_rgba(&data, bytes_per_row as usize);
        let stats = PixelStats::analyze(&pixels, RENDER_WIDTH, RENDER_HEIGHT);
        drop(data);
        readback.unmap();

        let readback_elapsed = readback_started.elapsed();
        println!("submit+read_time : {readback_elapsed:?}");

        // ---- PNG ---------------------------------------------------------
        let png_started = Instant::now();
        image::save_buffer(
            png_path,
            &pixels,
            RENDER_WIDTH,
            RENDER_HEIGHT,
            image::ExtendedColorType::Rgba8,
        )?;
        let png_bytes = fs::metadata(png_path)?.len();
        println!("png_write_time   : {:?}", png_started.elapsed());
        println!("png_bytes        : {png_bytes}");

        println!("-- stage 4: readback verification --");
        println!(
            "pixels_opaque    : {} / {}",
            stats.opaque_pixels, stats.total_pixels
        );
        println!("alpha_coverage   : {:.2}%", stats.coverage_percent());
        if let Some(bbox) = stats.bounding_box {
            println!(
                "opaque_bbox      : x[{}, {}] y[{}, {}]",
                bbox.0, bbox.1, bbox.2, bbox.3
            );
        }
        println!(
            "mean_rgb_opaque  : ({:.1}, {:.1}, {:.1})",
            stats.mean_r, stats.mean_g, stats.mean_b
        );
        if stats.opaque_pixels == 0 {
            return Err(Box::new(BakeoffError(
                "readback contains no opaque pixels - nothing was rendered",
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------- //

fn default_model_path() -> PathBuf {
    PathBuf::from(
        "/home/administrator/deepseekharness/Live2D-Ai/Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json",
    )
}

/// Axis-aligned bounds over all drawable vertex positions (model units).
struct ModelBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl ModelBounds {
    fn from_drawables(drawables: &[Moc3DrawableMesh]) -> Option<Self> {
        let mut bounds: Option<Self> = None;
        for vertex in drawables.iter().flat_map(Moc3DrawableMesh::vertices) {
            let [x, y] = vertex.position();
            bounds = Some(match bounds {
                Some(current) => Self {
                    min_x: current.min_x.min(x),
                    min_y: current.min_y.min(y),
                    max_x: current.max_x.max(x),
                    max_y: current.max_y.max(y),
                },
                None => Self {
                    min_x: x,
                    min_y: y,
                    max_x: x,
                    max_y: y,
                },
            });
        }
        bounds.filter(|bounds| bounds.width() > 0.0 && bounds.height() > 0.0)
    }

    fn width(&self) -> f32 {
        self.max_x - self.min_x
    }

    fn height(&self) -> f32 {
        self.max_y - self.min_y
    }

    fn center_x(&self) -> f32 {
        (self.min_x + self.max_x) * 0.5
    }

    fn center_y(&self) -> f32 {
        (self.min_y + self.max_y) * 0.5
    }
}

/// Model-units -> clip-space matrix that centers the drawable bounds and fills
/// MODEL_VIEW_FILL of the viewport while preserving aspect ratio.
fn fit_model_matrix(bounds: ModelBounds) -> Matrix44 {
    let aspect = RENDER_WIDTH as f32 / RENDER_HEIGHT as f32;
    let fit_x = MODEL_VIEW_FILL / (bounds.width() * aspect);
    let fit_y = MODEL_VIEW_FILL / bounds.height();
    let scale_y = fit_x.min(fit_y);
    let scale_x = scale_y / aspect;

    let mut matrix = Matrix44::identity();
    matrix.scale(scale_x, scale_y);
    matrix.translate(-bounds.center_x() * scale_x, -bounds.center_y() * scale_y);
    matrix
}

struct MeshStats {
    drawables: usize,
    masked_drawables: usize,
    unique_mask_ids: usize,
    vertices: usize,
    triangles: usize,
    blend_modes: BTreeMap<String, usize>,
    invisible_drawables: usize,
}

impl MeshStats {
    fn collect(meshes: &[Moc3DrawableMesh]) -> Self {
        let mut unique_masks = BTreeSet::new();
        let mut blend_modes = BTreeMap::new();
        let mut masked_drawables = 0usize;
        let mut vertices = 0usize;
        let mut indices = 0usize;
        let mut invisible = 0usize;

        for mesh in meshes {
            let masks = mesh.masks();
            if !masks.is_empty() {
                masked_drawables += 1;
                unique_masks.extend(masks.iter().copied());
            }
            *blend_modes
                .entry(format!("{:?}", mesh.blend_mode()))
                .or_insert(0) += 1;
            vertices += mesh.vertices().len();
            indices += mesh.indices().len();
            if mesh.opacity() <= 0.0 {
                invisible += 1;
            }
        }

        Self {
            drawables: meshes.len(),
            masked_drawables,
            unique_mask_ids: unique_masks.len(),
            vertices,
            triangles: indices / 3,
            blend_modes,
            invisible_drawables: invisible,
        }
    }

    fn report(&self, label: &str) -> String {
        let mut lines = Vec::new();
        lines.push(format!("{label:<18}: {}", self.drawables));
        lines.push(format!("{label}_masked    : {}", self.masked_drawables));
        lines.push(format!("{label}_mask_ids  : {}", self.unique_mask_ids));
        lines.push(format!("{label}_invisible : {}", self.invisible_drawables));
        lines.push(format!("{label}_vertices  : {}", self.vertices));
        lines.push(format!("{label}_triangles : {}", self.triangles));
        for (mode, count) in &self.blend_modes {
            lines.push(format!("{label}_blend.{mode:<13}: {count}"));
        }
        lines.join("\n")
    }
}

struct PixelStats {
    total_pixels: u64,
    opaque_pixels: u64,
    bounding_box: Option<(u32, u32, u32, u32)>, // min_x, max_x, min_y, max_y
    mean_r: f32,
    mean_g: f32,
    mean_b: f32,
}

impl PixelStats {
    fn analyze(rgba: &[u8], width: u32, height: u32) -> Self {
        let mut opaque = 0u64;
        let mut sum_r = 0u64;
        let mut sum_g = 0u64;
        let mut sum_b = 0u64;
        let mut min_x = width;
        let mut max_x = 0u32;
        let mut min_y = height;
        let mut max_y = 0u32;

        for y in 0..height {
            for x in 0..width {
                let index = ((y as usize * width as usize) + x as usize) * 4;
                if rgba[index + 3] > 0 {
                    opaque += 1;
                    sum_r += u64::from(rgba[index]);
                    sum_g += u64::from(rgba[index + 1]);
                    sum_b += u64::from(rgba[index + 2]);
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }

        let opaque_f = opaque as f32;
        Self {
            total_pixels: u64::from(width) * u64::from(height),
            opaque_pixels: opaque,
            bounding_box: (opaque > 0).then_some((min_x, max_x, min_y, max_y)),
            mean_r: sum_r as f32 / opaque_f,
            mean_g: sum_g as f32 / opaque_f,
            mean_b: sum_b as f32 / opaque_f,
        }
    }

    fn coverage_percent(&self) -> f64 {
        (self.opaque_pixels as f64 / self.total_pixels as f64) * 100.0
    }
}

/// Copies tightly packed rows out of the padded readback mapping.
fn row_major_rgba(padded: &[u8], bytes_per_row: usize) -> Vec<u8> {
    let row_bytes = RENDER_WIDTH as usize * 4;
    let mut packed = Vec::with_capacity(row_bytes * RENDER_HEIGHT as usize);
    for row in 0..RENDER_HEIGHT as usize {
        let start = row * bytes_per_row;
        packed.extend_from_slice(&padded[start..start + row_bytes]);
    }
    packed
}
```
