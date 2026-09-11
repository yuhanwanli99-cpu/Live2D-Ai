# Ayagami Rust Bakeoff 报告（Bai 模型）

> 结论：**通过**。Ayagami @ `640ae4b10bad8def1adcacdada4f8241b484c169` 的 `ayagami` / `ayagami-render`
> 公开 API 可以完整加载 Bai（model3 + moc3 + PNG 纹理 + physics3），固定 dt 更新，
> 在无头 wgpu 设备上离屏渲染首帧，并通过公开的 `Texture::download_to_image` 读回写 PNG。
> 未发现阻塞性不兼容；过程中遇到的 API 差异均已在 bakeoff 工程侧修复（见 §4）。

- 日期：2026-08-25（UTC）
- 执行环境：Linux x86_64 容器（无显示服务），rustc/cargo 1.98.0 stable
- 被测对象：[AyagamiDev/ayagami](https://github.com/AyagamiDev/ayagami) rev `640ae4b1`（默认分支，2026-08-26 查询登记于 SOURCES.md）
- 目标模型：`Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json`

## 1. 工程与依赖

独立工程位于 `/tmp/ayagami-bakeoff`（**不属于仓库根 workspace，未修改根 Cargo.toml/workspace；未做 git commit**）。

```toml
# /tmp/ayagami-bakeoff/Cargo.toml （节选）
ayagami         = { git = "https://github.com/AyagamiDev/ayagami", rev = "640ae4b10bad8def1adcacdada4f8241b484c169" }
ayagami-render  = { git = "https://github.com/AyagamiDev/ayagami", rev = "640ae4b10bad8def1adcacdada4f8241b484c169" }
wgpu = "29"          # 与 ayagami-render 内部 wgpu 29.0.4 统一
glam = { version = "0.33", features = ["bytemuck"] }
pollster = "0.3"
image = { version = "0.25", default-features = false, features = ["png"] }
serde_json = "1"
```

实现要点（仅用公开 API，参照 `ayagami-demo/src/app.rs` 用法）：

1. **加载**：`meta::Model3`（serde 解析 model3.json，Layout 块自行从 JSON 读取）→
   `file::ParsedModel::load(&mut reader)` 读 moc3 → 纹理文件字节交给
   `ModelRenderer::load_model(Arc<ParsedModel>, &[&[u8]])` →
   `meta::Physics3` + `physics::PhysicsEngine::new(setting, PhysicsOptions::compatible(None))`；
   cdi3.json 用 `meta::DisplayInfo` 解析。
2. **固定 dt 更新**：dt = 1/60 s，120 步（模拟 2 s）。每步：
   ParamBreath 正弦驱动、ParamAngleZ 摆动输入 → `physics_pose.update(&user_pose)` →
   `engine.update(&mut physics_pose, FIXED_DT)` → 合并后 `renderer.driver().set_pose(&pose)`。
   首步前执行 `engine.settle()` 得到确定性初始姿态。
3. **离屏渲染**：headless `wgpu::Instance`（Vulkan→GL→all 回退）+ `request_adapter` +
   `request_device`；目标纹理 `texture::Texture::new(device, 1024×1024, Rgba8Unorm, Some(1))`；
   `RenderOptions { transform, mask_dimensions: 1024×1024（1:1 mask），colorspace: SRgb }`；
   `renderer.prepare(encoder, opts)` → render pass 清透明色 → `renderer.render(pass, format)`。
   变换矩阵 = Layout(center/width/height) × 整画布映射（同 demo WholeModel 公式）。
4. **读回**：`TextureManager::unpremultiply(.., clamp=true)` 转直通 alpha 后
   `Texture::download_to_image(device, queue, cb)` 回调拿到 `DynamicImage`，编码 PNG。
   回调需要持续 `device.poll(PollType::wait_indefinitely())` 泵送直至送达。

## 2. Bai 模型元数据（程序实测输出）

| 项目 | 值 |
| --- | --- |
| model3 Version | 3 |
| moc3 版本 | 4.2 |
| 画布 | 4000×6000，center=(2000,3000)，scale=4000 |
| ArtMesh / Deformer / Part | 475 / 549 / 159 |
| Parameter / Glue / DrawGroup | 128 / 8 / 1 |
| 顶点 / 三角形 | 41,386 / 64,368 |
| 纹理 | 1 张：`bai.16384/texture_00_4096.png`（4096×4096，3,996,527 B） |
| Mask/clipping | 84 个被裁剪 ArtMesh、共 86 条 clip 引用、28 个反相 mask；renderer 报告 17 组去重 clip mask |
| Offscreen part | 0 |
| Physics3 | version 3，42 settings，108 inputs / 106 outputs / 149 pendulum vertices |
| 引擎生效 | physics inputs=9、outputs=105 参数键 |
| cdi3 | version 3，128 parameters，176 parts |

## 3. 运行结果与耗时

GPU adapter（实测打印）：

```text
GPU adapter: name=llvmpipe (LLVM 21.1.8, 256 bits) backend=Vulkan vendor=0x10005 device=0x0000
             driver=llvmpipe (Mesa 26.0.3-1ubuntu1 (LLVM 21.1.8))
最大纹理尺寸限制: 8192
```

说明：本机虽有 NVIDIA GTX 1060 3GB（nvidia-smi 可见），但容器内无 `/dev/nvidia*`、
`/dev/dri` 节点，且系统只装 Mesa Vulkan ICD（含 lavapipe），无 NVIDIA ICD——因此 wgpu 实际
落到 **llvmpipe（CPU 软件光栅化的 Vulkan 实现）**。功能验证完全有效；下述耗时是软件光栅
数字，不能当作真机 GPU 性能预期。

Release 构建（`cargo run --release`，RUST_LOG=warn）：

| 阶段 | 耗时 |
| --- | --- |
| model3.json 解析 | 114 µs |
| moc3 加载（ParsedModel::load） | 320 ms |
| physics3.json 解析 | 555 µs |
| wgpu headless 初始化 | 2.57 s（首次 ICD/shader 编译为主） |
| renderer.load_model（PNG 解码+GPU 上传） | 1.74 s（其中文件读取 51 ms） |
| 固定 dt 循环（120 步 × 16.67 ms） | 17.77 ms（0.148 ms/步墙钟） |
| 首帧 prepare+render+submit+wait（1024²，含 17 组 mask pass） | 733 ms |
| Texture::download_to_image 读回 | 63.6 ms |
| 总计 | 5.53 s |

Debug 构建对照（同一代码）：固定 dt 循环 0.86 ms/步，首帧 1.18 s，读回 243 ms，总计 7.1 s。

帧统计（程序自检 + 独立 imgcheck 复核）：

```text
frame stats: 1024x1024, non-transparent px: 86,347 (8.23%), mean alpha(covered)=243.6
imgcheck:    visible 84,923 (>a16, 采样口径略异), bbox x 350..676 y 294..822,
             46 种粗量化颜色 —— 剪影为完整人物（头部/发型/躯干/双腿）
```

输出 PNG：`verification/rust-bakeoff/ayagami-bai.png`（1024×1024 RGBA PNG，174 KB）。
构图符合 model3 Layout（center_y=-0.07、width/height=0.56）：人物居中略偏下、占画面高约一半。

## 4. API 兼容性问题清单（逐项修复记录）

Ayagami 自身 API 无阻塞问题；以下差异均在 bakeoff 工程侧解决：

1. **wgpu 29 相对旧版/文档的签名漂移**（ayagami-render 锁定 wgpu 29.0.x）：
   - `Device::poll` 不再接受 `Maintain::Wait`，改为 `device.poll(wgpu::PollType::wait_indefinitely()) -> Result<PollStatus, PollError>`；
   - `DeviceDescriptor` 增加 `experimental_features` 字段（`ExperimentalFeatures::disabled()`）且 label 为泛型 `L`；
   - `InstanceDescriptor` 没有 `Default`，需显式构造全部字段（backends/flags/memory_budget_thresholds/backend_options/display）；`Instance::new(desc)` 按值传入；
   - `AdapterInfo.driver_info` 是 `String` 而非 `Option<String>`。
2. **`core::Collection`/`ItemArray` 不是 Iterator**：`model.artmeshes()` 等返回 opaque 类型，
   计数需在作用域内引入 `ItemArray` 用 `.count()`，过滤遍历用 `.into_iter().filter(..)`；
   trait 方法要求显式导入 `core::{ArtMesh, ItemArray, Model, Param, Part}`。
3. **`serde_json::Value` 无 `as_f32`**：经 `as_f64` 转 f32 解析 Layout 数值。
4. **meta::Model3 不建模 `Layout`/`Groups` 字段**：serde 忽略未知字段不影响解析；Layout 由工程内
   `Layout::from_json` 补齐并参与变换计算。
5. **读回回调时序（工程自身 bug，已修）**：`download_to_image` 的回调在 `device.poll` 中触发，
   循环里先 `try_recv` 会把消息取走导致后续误判“未回调”；改为循环内保存结果再统一判定。

## 5. 复现步骤

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /tmp/ayagami-bakeoff
cargo fmt --check && cargo check && cargo build --release
RUST_LOG=warn ./target/release/ayagami-bakeoff \
  Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json \
  verification/rust-bakeoff/ayagami-bai.png
# 结构复核（可选）：cargo run --example imgcheck -- <png>
```

## 6. 对选型的含义（供 RUST-REWRITE-RFC D4 参考）

- Ayagami 公开 API 即可覆盖「加载→参数/物理更新→离屏出图→像素读回」闭环，无需 fork 改动；
  pin commit 直接可用。
- 渲染器自动完成 1:1 mask、部分重算与不可见物跳过；Bai 的 84 masked meshes / 17 clip 组在
  软件光栅下首帧 733 ms，真机 GPU 上该量级（64k tri）预期为毫秒级，待有 GPU 的环境复测。
- 风险项：API 尚不稳定（README 自述）、crates.io 未发包只能 git pin；physics 输入键只有 9 个
  但输出 105 个，行为正确性建议后续用姿态对拍（vs Cubism Web SDK 或 VTube Studio 截图）验证。
