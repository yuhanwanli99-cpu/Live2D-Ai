# Rust Bakeoff 选型决策：Ayagami vs Mocari（RFC D4）

- 日期：2026-08-26
- 结论：**选 Ayagami** 作为 runtime/render 底座，以固定 rev
  `640ae4b10bad8def1adcacdada4f8241b484c169` 引入 `crates/l2d`；
  **Mocari 不引入**，保留为未引入参考与动画补充候选。
- 输入：两份实测报告——`rust-bakeoff-ayagami.md`（结论：通过）与
  `rust-bakeoff-mocari.md`（结论：全部通过，无 blocker）。两候选对 Bai 均能完成
  「加载 → 固定 dt 更新 → headless 离屏渲染 → PNG 读回」闭环，因此本决策不是
  「能不能跑」之争，而是在都可行的前提下按 v0 与未来需求取舍。

## 1. 双方胜负对照（依据两份报告的实测数据）

| 维度 | Ayagami | Mocari | 本轮判定 |
| --- | --- | --- | --- |
| moc3 版本覆盖 | `file::Version` 枚举明确建模到 **5.3**（版本字节 6），面向新编辑器产物的前向路径清晰 | 头表记录字节 6 → 5.3.0，但发布说明口径的完整解析支持范围以仓库为准，未在本轮验证 | **Ayagami 胜**（5.3 支持是显式 API 承诺，不是头表记录） |
| 离屏渲染定位 | 渲染器原生面向 offscreen：`RenderOptions`（transform/mask 分辨率/colorspace）+ `prepare/render` + 公开 `Texture::download_to_image` 读回，无需绕过窗口系统 | 可离屏，但资源装配步骤多（mesh buffers/clipping plan/mask target/transform 全部手工拼装），示例主线是 surface 展示 | **Ayagami 胜**（offscreen 是一等公民，封装面小） |
| 混色/混合模式 | 内建 advanced blend 管线与 `RenderColorspace::SRgb` 混色模式选项；Bai 的 44 个 Multiplicative mesh 直接走管线 | Normal/Multiplicative 两类可用（unorm gamma 空间两 pass 近似 Cubism 行为），更复杂的 blend 组合未见同等内建支持 | **Ayagami 胜**（advanced blend 内建） |
| Mask/裁剪规模 | 渲染器自动布局 mask、自动跳过不可见物；Bai 84 masked meshes / 86 clip 引用 / 17 去重组一次通过。未见 Cubism 系「单纹理最多 ~36 个裁剪上下文」的经典硬编码上限 | Bai 实测 84 masked drawable → 19 unique mask id → **10 context**（在容量内）。其布局沿用经典 Cubism 单纹理上下文模型，上限约束存在与否未在报告中排除 | **Ayagami 胜**（本轮无 mask context 数量限制问题；注意 Bai 规模不足以实测触发任一实现的上限，此判定基于架构口径而非压测） |
| 上游活跃度 | 默认分支持续迭代（pre-1.0、API 自述不稳定但演进快）；bakeoff 期间问题当天可对上游 demo 核对 | 发布过 crates.io 0.4.0 版本，工程化包装好；本轮未观察到同等级迭代节奏 | **Ayagami 胜**（活跃度）|
| 发布形态 | 未发 crates.io，只能 git pin | **crates.io 正式版本 0.4.0**，语义化版本 + feature flag（`wgpu`） | **Mocari 胜** |
| 内存安全声明 | 依赖 zerocopy/bytemuck 等，crate 自身含少量 unsafe 面（上游自述与代码结构），本项目以 `unsafe_code = deny` 只约束第一方代码，不约束依赖 | **无 unsafe**（纯安全 Rust 卖点） | **Mocari 胜** |
| 动画能力 | 无 motion/expression/pose 文件加载（pose 为参数级姿态结构，不含 .motion3 时间轴） | **内建 motion/expression/pose 加载与应用**（`apply_pose(dt)` 等），做动作系统时起点更高 | **Mocari 胜** |
| Bai 首帧实测 | 加载+moc3 解析 320 ms、首帧（1024²，17 mask 组，软件光栅）733 ms、读回 63.6 ms、总 5.53 s | 加载 516.8 ms、CPU 首帧更新 31.3 ms、总墙钟 1.46 s（同为 llvmpipe 软件光栅） | **Mocari 数字更快**；但两者耗时差异主要是装配路径不同（Ayagami 含 GPU 上传与 17 组 mask pass），软件光栅数字不能当真机预期，不构成决策项 |

## 2. 为什么选 Ayagami

1. **v0 锚定档位已实测打通且前向兼容方向更好。** v0 唯一保证的 Bai 是 moc3 raw 字节 4
   （Cubism 4.2），两家都能解析；但未来通用皮套会逐步出现 Cubism 5.x 产物，
   Ayagami 把 5.3 显式建进了版本枚举，前向风险更低。
2. **我们要的是「headless 出图」这条窄路，而 Ayagami 的公开 API 恰好就是这条路。**
   offscreen 渲染 + 变换/mask 选项 + 直通 alpha 读回都是一等公民，
   封装成 `l2d` 的自有 API 时不需要绕路（见 `crates/l2d/examples/render_model.rs`，
   其产物 `verification/rust-bakeoff/selected-bai.png` 与 bakeoff 产物经独立
   imgcheck 复核逐像素一致：visible 84,923 px、bbox x[350..676] y[294..822]、46 色）。
3. **Advanced blend 与无 36-mask-context 顾虑**让渲染管线不用为 Bai 这类
   高密度 mask + Multiplicative 皮套预留降级路径。
4. **活跃度**：pin rev 的方式本身就是对「API 不稳定」的对冲——升级必须重新走验证，
   不受上游漂移影响。

## 3. Mocari 输在哪里、为什么仍要留着

- 输点集中在**发布形态（crates.io）、无 unsafe、内建 motion/expression/pose**——
  这些是真实优势，但都不是 v0 的阻塞项：
  - git pin 已解决发布形态问题；
  - workspace 的 `unsafe_code = deny` 只约束第一方代码，依赖的 unsafe 面由
    pin+验证流程控制；
  - **v0 的 Bai 包里没有 motion/expression/pose 文件**（两份报告一致确认：
    Bai 只有 model3/moc3/纹理/physics3/cdi3），动画能力当前用不上。
- 未来要做表情/动作系统（RFC D9 六动作落地到骨骼参数时间轴）时，Mocari 的
  motion/expression 实现是现成的**参考实现**；届时以其行为作为对照来驱动
  Ayagami 底座上的动画层，或在必要时把 Mocari 作为补充 runtime 再评估引入。

## 4. 决策的边界条件（何时需要重开）

- Ayagami 长期停滞/弃坑，或 rev 升级验证连续失败；
- 目标皮套出现 Ayagami 无法解析而 Mocari 可解析的格式特性；
- 动画需求落地且评估认为直接采用 Mocari 的 motion 栈比参考重写更省。

## 5. 引入记录

- 引入位置：`crates/l2d/Cargo.toml`（`ayagami`、`ayagami-render` 均 pin 同一 rev）；
- 对外暴露：仅 `l2d` 自有 API（asset/model/renderer 三模块），第三方类型不越过 crate 边界；
- 登记同步：根 `SOURCES.md`「已引入」一节、`crates/l2d/README.md`；
- 验证产物：`verification/rust-bakeoff/ayagami-bai.png`（bakeoff 原始产物）与
  `verification/rust-bakeoff/selected-bai.png`（经 crate 封装复现，内容一致）。
