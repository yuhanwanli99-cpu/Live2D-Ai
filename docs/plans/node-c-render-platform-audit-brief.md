# 高级节点 C —— 渲染性能与平台 RC 审计（交接文档）

> **给谁看**：被指派对本文档所列问题做最终裁决的高级 Agent。
> **何时**：实时渲染非阻塞改造动工前；发布候选（RC）平台语义定稿前。
> **前置证据**：`docs/verification/desktop-platform-smoke-2026-08-26.md`（WSLg 实测矩阵）。
> **产出要求**：对 C1–C8 逐条给出「裁决 + 理由（≤3 句）+ 实现注意点」，另设
> 「额外风险」一节。实现方以裁决为唯一规则源。
>
> **裁决状态（2026-08-27）**：C1–C11 已正式裁决；唯一规则源见
> [`node-c-c1-c11-formal-audit-2026-08-27.md`](node-c-c1-c11-formal-audit-2026-08-27.md)。

---

## 一、现状事实（file:line 均已核实）

### 渲染热路径的阻塞点（2026-08-27 拆分后重新核实）

> 拆分说明：`crates/live2d-ai-desktop/src/app.rs` 已拆分为 `src/app/` 目录
> （mod.rs / types.rs / capability.rs / bootstrap.rs / surface.rs / frame.rs /
> interaction.rs / handler.rs / shutdown.rs / tests.rs）。
> `crates/l2d/src/renderer/model_core.rs` 与 `mod.rs` 未动，仅行号随上下文
> 略有偏移。下表所有行号以拆分后当前文件为准；C1–C11 裁决问题措辞不变。

| 位置（旧） | 位置（新） | 行为 | 定性 |
|---|---|---|---|
| `crates/l2d/src/renderer/model_core.rs:176-179` | `crates/l2d/src/renderer/model_core.rs:175-179`（submit 在 175，poll 在 176-179） | `render_to_view` 末尾 `device.poll(PollType::wait_indefinitely())` | **实时路径阻塞点**：每帧 CPU 等 GPU 全部完成 |
| `crates/l2d/src/renderer/mod.rs:210-211` | `crates/l2d/src/renderer/mod.rs:209-213`（poll 在 209-211，`recv()` 在 213） | 离屏读回路径 `wait_indefinitely()` 后 `mpsc::recv()` 取回调 | 合法阻塞（截图必须等结果） |
| `crates/live2d-ai-desktop/src/app.rs:942-944` | `crates/live2d-ai-desktop/src/app/frame.rs:55-61`（submit 55、present 56、Poll 59 + 失败 warn 60） | 纯透明壳路径每帧后 `device.poll(PollType::Poll)`（非阻塞泵） | 已是目标形态 |
| `crates/live2d-ai-desktop/src/app.rs:18-21`（模块文档） | `crates/live2d-ai-desktop/src/app/mod.rs:40-43`（模块文档，措辞基本一致：「功能优先、耗时如实统计」） + `crates/live2d-ai-desktop/src/app/frame.rs:64-69`（**新增**函数级文档细化「包含 GPU 等待的诚实值，不代表纯 CPU 提交成本」） | 自述承认技术债 + 拆分时新增函数级口径说明 | 承认技术债（措辞更细，行为未变） |
| 初始化 `request_adapter/request_device`（app.rs:621,642；l2d gpu.rs:92,104） | 初始化 `request_adapter/request_device`（`app/bootstrap.rs:66-71` 与 `87-94`；`l2d/src/renderer/gpu.rs:92,104`） | `pollster::block_on` 一次性 | 不在热路径，无需改 |
| `crates/live2d-ai-desktop/src/app.rs:872`（resize/set_viewport 先后契约单行注释） | `crates/live2d-ai-desktop/src/app/surface.rs:5-8`（模块文档说明）+ `surface.rs:33-34`（`reconfigure_current` 函数文档「必须在旧 SurfaceTexture drop/present 之后」+ `surface.rs:45-47`（configure 在 45、`sync_model_viewport` → `set_viewport` 在 46-47，**契约已成代码顺序而非注释**） | `surface.configure` → `sync_model_viewport` → `set_viewport` 顺序已落实 | 契约从单行注释升级为模块/函数文档 + 代码顺序 |

#### 拆分后位置更新（2026-08-27）

| 旧引用 | 新位置 | 备注 |
|---|---|---|
| `l2d/renderer/model_core.rs:176-179` | `l2d/renderer/model_core.rs:175-179` | 行号基本一致（submit 行上移 1 行） |
| `l2d/renderer/mod.rs:210-211` | `l2d/renderer/mod.rs:209-213` | poll 段略上移 1 行；`recv()` 收回调在 213 单独成行 |
| `app.rs:942-944` | `app/frame.rs:55-61` | 3 行 → 4 行（多 1 行失败 warn），行为不变 |
| `app.rs:18-21` 模块文档 | `app/mod.rs:40-43` + `app/frame.rs:64-69` | 措辞保留 + 拆分时**新增**函数级细化说明（不影响行为） |
| `app.rs:621, 642` | `app/bootstrap.rs:66-71, 87-94` | 行号偏移（模块拆出去后行号整体下移） |
| `l2d gpu.rs:92, 104` | `l2d/src/renderer/gpu.rs:92, 104` | 未动 |
| `app.rs:872` 单行 resize 注释 | `app/surface.rs:5-8, 33-34, 45-47` | 从单行注释升级为模块/函数文档 + 代码顺序契约（**行为描述更精确，待裁决复核 C4 引用源是否需指向 surface.rs 而非 app.rs:872**） |

> 拆分后所有事实描述**行为定性不变**，仅行号与文件位置调整；C1–C11
> 裁决问题原措辞保留。**待裁决复核**：C4 原文「`app.rs:872` 已有注释」
> 现指向 `app/surface.rs:5-8, 33-34, 45-47`，是否在 C4 措辞里同步更新为
> 新引用（高级 Agent 决定），本文件不动裁决措辞。

量化基线（llvmpipe 软渲染、X11、120 帧、当前阻塞实现）：
**frame_time avg≈116ms / max≈144ms**（见验证文档）。改造前后必须同法对比。

### 平台能力实测结论（WSLg，2026-08-26）

- X11：置顶 ✅ / 穿透 ✅ / 透明 ✅；**全局定位被合成器拒绝**（回读远偏离目标，
  如实记 unavailable）；拖动代码路径在但无法自动化注入鼠标，待人工复验；
- Wayland：置顶 ❌ unavailable（平台性）、穿透 ✅、透明 ✅；
- 托盘：无 D-Bus → 可见降级不阻塞启动；pet-mode 无托盘 ⇒ 自动穿透禁用 ✅。

## 二、渲染部分待裁决问题

- **C1 API 拆分形状**：建议 `render_to_view` 保留阻塞语义（离屏截图专用，
  文档注明），另增 `render_to_view_submit`（encode→submit→present 由调用方做，
  返回 `Result<(), RenderError>`，不做任何 wait）；desktop 实时路径改调后者 +
  每帧末尾既有非阻塞 Poll 兜底。是否接受该拆分？还是倾向单一函数加
  `PollPolicy` 参数？
- **C2 非阻塞后的错误可见性**：不再 wait ⇒ 提交期错误仍同步返回，但 GPU 侧
  失效（device lost / validation）只能靠后续 poll 暴露。裁决：轮次内如何把
  异步失效映射进现有 `FrameOutcome` / 退出码分类（environment 3 vs code 1）？
- **C3 多帧 in-flight 上限**：Fifo present + 每帧非阻塞 Poll 下，是否显式限制
  在飞帧数（如 ≤2，防编码快于呈现导致队列堆积）？还是依赖 Fifo 背压即可？
- **C4 resize 时序**：surface reconfigure 与 `set_viewport`（mask 重建）的先后
  契约需成文：旧 texture drop/present 之后才能 configure（当前契约见
  `app/surface.rs:5-8, 33-47`）——mask 资源更新放在哪一步？resize 抖动时是否
  跳帧防抖？
- **C5 Ayagami prepare() 时机**：每帧 `renderer.prepare(&mut encoder, &options)`
  是否存在逐帧重复上传（mask/纹理）？若上游不可控，是否在 l2d 层缓存
  未变更的 bind group？请给出可核查的判定方法（不只是「应该没问题」）。
- **C6 WASM 差异**：wasm32 下 `Device::poll` 语义与 native 不同（浏览器无独立
  设备线程，poll 时机受 requestAnimationFrame 约束）。demo 当前仅构建门禁通过
  （trunk --release ✅，dist wasm 5.2MB）。裁决：WASM demo 是否承诺运行时行为，
  还是 v0 只保留编译门禁 + 浏览器手测清单？
- **C7 benchmark 协议**：定义改造前后对比口径——同帧数/同动作序列/同会话，
  报 avg/p95/max 三档 + presented/skipped；llvmpipe 数据作下限参考不作验收线；
  真实 GPU 环境数据列为发布前补测项。是否同意？
- **C8 性能验收线**：软渲染 ≥15 fps 即视为改造生效（116ms→≤66ms）？
  还是不设数值线、只要求「无阻塞 wait 且帧时间分布改善」？

## 三、平台 RC 部分待裁决问题

- **C9 能力承诺表终稿**：README「能力语义」按实测改写后，对外承诺哪些为
  「正式能力」、哪些标「环境相关」？特别是：全局定位在 WSLg 不可用是否接受
  作为已知限制发布（真实桌面 WM 待测）；Wayland 置顶明确标注不支持。
- **C10 拖动人工复验项**：drag_window 无法自动化——列入手动验收清单并给出
  操作步骤与判据（拖动后窗口跟随、松开后位置保持、runtime cap 记录 available）。
- **C11 托盘恢复链路**：托盘可用环境下的完整闭环（隐藏→托盘唤回→穿透开关）
  本沙箱无法验证；是否同样列入手动验收清单？
