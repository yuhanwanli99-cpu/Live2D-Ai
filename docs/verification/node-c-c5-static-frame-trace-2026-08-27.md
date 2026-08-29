# 节点 C / C5 静止帧上传 trace 实测报告（2026-08-27）

> **任务**：P1-1，验证 `docs/plans/node-c-c1-c11-formal-audit-2026-08-27.md`
> C5 节 93-98 行可核查门禁（仅取证，不裁决）。
> **工具**：`crates/l2d/examples/static_frame_trace.rs`（新增 example）。
> **运行主机**：Linux x86_64，wgpu 29.0.4 后端 Vulkan → Mesa llvmpipe 21.1.8。
> **皮套**：仓库内 Bai（475 artmeshes / 64 368 triangles / 128 params / 1 texture）。

---

## ① 观测方法

**选定方法**：C5 裁决文档 4.3 节「tracing 日志注入」的**外置变体**——
**不修改 l2d 渲染器代码**，在 example 入口处安装自定义 `log::Log` impl，
把 `ayagami_render=trace` 输出捕获到 `Mutex<Vec<String>>`，再按行模式
分类计数。

为什么不选 4.2 `Trace::File`（在 wgpu 29 下叫 `Trace::Directory`）：
- 需要给 wgpu 加 `wgpu/trace` 特性，会拉入 `serde` / `ron` / `naga/serialize`，
  对 `crates/l2d` 共享依赖是侵入式改动；
- `Trace::Directory` 输出二进制 trace.bin，需自写解析器（无现成 `wgpu-info`
  crate 在 registry 中可复用），增加 P1-1 风险面。

为什么不直接用 4.4 自写计数器：l2d 层无法 hook `ayagami_render::queue.write_buffer`
（私有 API），强行包装 InnerRenderer 等价于改 renderer 语义，违反任务门禁。

**实际采用的混合方法**（4.3 + 4.4 子集 + 源码推导）：

| 维度 | 工具 | 备注 |
|---|---|---|
| ArtMesh 脏标志（→ 早退成败） | `ArtMesh #N changed` debug 行计数 | 见 `renderer.rs:995` |
| mask 纹理重建 | `Create clip set N texture` debug 行 | 见 `renderer.rs:971` |
| mask RenderPass | `Render ArtMesh` trace 行 | 见 `renderer.rs:1268`（mask pass 内） |
| 主 pass draw | `Draw ArtMesh` trace 行 | 见 `renderer.rs:1611`（必经，不属 C5 门禁） |
| HAL 资源增量 | `wgpu::Device::get_internal_counters()` 取差值 | wgpu 默认未开 `counters` 特性时 HAL 字段恒为 0；源码已保证 bind group 一次性（`renderer.rs:877-901`），0 是必然结果 |
| BufferWrite 字节数 | 源码静态推导 | `prepare()` L962 `write_buffer(&self.global_buffer, 0, &[camera])`，64B 无条件；在早退 L1024 **之前**。早退成立 ⇒ 整帧 BufferWrite 字节 = 64 |

**诚实标注的口径近似**：

- 「BufferWrite 字节数」**不是**直接 hook 截获，而是经由
  `s_artmesh_changed == 0 ⟹ L1024 早退成立 ⟹ 仅 L962 write 触发` 推理。
  这是 C5 裁决 97 行明确认可的口径（"若 Trace::File 不能直接区分 ArtMesh
  uniform slot 写入，用…近似"），不是测量降级。
- 「TextureWrite / copy 次数」**不**是日志计数的——`prepare()` 内
  `queue.write_texture` 命中为 0（`docs/verification/node-c-c5-prepare-upload-audit.md`
  ① 节源码列出全部 GPU 上传动作），属源码静态事实，不依赖观测。
- 「bind group create 次数」**不**依赖 HAL counters（wgpu 默认未开
  `counters` 特性），由源码 `renderer.rs:877-901` 保证——`uniform_bind_group`
  在 `reload_model` 一次性创建，每帧 `set_bind_group(0, &md.uniform_bind_group,
  &[offset])` 切 slot，无 recreate 路径。

---

## ② 实验设置

- **画布**：`1024×1024` 固定 viewport。
- **GPU 后端**：`gpu adapter: llvmpipe (LLVM 21.1.8, 256 bits) (Vulkan, Mesa 26.0.3-1ubuntu1)`。
  Vulkan 优先、GL 回退由 l2d `init_headless_device` 自动选（`gpu.rs:75-119`）。
- **warm-up 帧**：5 帧（`update(FIXED_DT_60HZ) + render_frame()`），
  让物理 / idle / 首帧 mask 重建 settle。
- **静止帧**：300 帧，仅 `render_frame()`（内部走 `core.render_to_view`
  阻塞路径），**不**调 `update` / `set_parameter` / `override_parameter`。
- **采样时点**：
  1. `renderer.load_model(&loaded)` 后 → `capture.drain()` 截断 load 阶段噪声；
  2. warm-up 5 帧后 → 取 `baseline_counters = device.get_internal_counters()`；
  3. 静止 300 帧后 → 取 `final_counters = device.get_internal_counters()`；
  4. 静止段与 warm-up 段日志按相同模式分别计数（基线对比）。

---

## ③ 原始数据

### 3.1 复现命令

```text
cargo run -p l2d --example static_frame_trace
```

完整原始输出落盘 `docs/verification/logs/static_frame_trace_raw.txt`（38 行）。

### 3.2 warm-up 段（5 帧基线）

| 事件 | 行数 | 含义 |
|---|---|---|
| `ArtMesh #N changed` | **1903** | 物理 + idle 驱动下大量 ArtMesh 状态翻转（5 帧累计） |
| `Create clip set N texture` | **17** | 初次 mask 纹理建 17 个（视口 1024×1024 + clip set 划分） |
| `Render ArtMesh` (mask pass) | **55** | warm-up 段 mask 重绘（mask 纹理初次落内容） |
| `Draw ArtMesh` (主 pass) | **860** | warm-up 段主 pass 绘制（与 artmesh 数同量级） |

### 3.3 静止段（300 帧核心观测）

| 事件 | 行数 | 每帧均值 | C5 期望 | 结论 |
|---|---|---|---|---|
| `ArtMesh #N changed` | **0** | 0.0 | 0 | ✅ 完全符合 |
| `Create clip set N texture` | **0** | 0.0 | 0 | ✅ 完全符合 |
| `Render ArtMesh` (mask pass) | **0** | 0.0 | 0 | ✅ 完全符合 |
| `Draw ArtMesh` (主 pass) | 51 600 | 172.0 | （不强制） | 主 pass 必经，与 475 ArtMesh × ~36% 可见一致 |
| `BufferWrite` (64B global uniform) | （推 300） | 1.0 | 1.0 | ✅ 由 L962 源码 + 早退成立推导 |
| `BufferWrite` (ArtMesh 全段) | （推 0） | 0.0 | 0 | ✅ 由 `s_artmesh_changed == 0` 推导 |
| `TextureWrite` / `copy_texture_to_texture` | 0（源码静态事实） | 0.0 | 0 | ✅ |
| `bind group create` | 0（源码静态事实） | 0.0 | 0 | ✅ |

### 3.4 HAL 资源增量（warm-up 末 vs 静止段末）

| 资源 | baseline | final | 增量 |
|---|---|---|---|
| `buffers` | 0 | 0 | **0** |
| `textures` | 0 | 0 | **0** |
| `bind_groups` | 0 | 0 | **0** |
| `render_pipelines` | 0 | 0 | **0** |

> wgpu 29.0.4 默认未启用 `counters` 特性，HAL 字段恒为 0（`InternalCounter::read()`
> 在无特性时返回 0）。即便如此，C5 门禁仍可由 3.3 表中的日志事件 + 源码静态事实
> 双重确认。
>
> 若需 HAL 实测增量，可临时给 l2d 的 wgpu dep 加 `features = ["counters"]`——
> 不需要重新写工具；`get_internal_counters()` 接口已就绪。

### 3.5 耗时

静止段 300 帧总耗时 **≈ 29.8 s**（llvmpipe 软光栅，不代表真机性能）。
本任务的关注点是**次数与门禁符合性**，不做 perf 裁决。

---

## ④ C5 四条门禁逐条对照（裁决 95-98 行）

### 门禁 1：300 个静止帧记录 BufferWrite / TextureWrite / Mask RenderPass

- 静止段日志事件汇总：
  - `ArtMesh #N changed` = **0**（0/0 期望 0）✅
  - `Create clip set N texture` = **0**（0/0 期望 0）✅
  - `Render ArtMesh` (mask pass) = **0**（0/0 期望 0）✅
  - `BufferWrite` 字节数：`64B × 300 = 19 200B`（由 L962 源码 + 早退成立推导）✅
  - `TextureWrite` / `copy`：**0**（`prepare()` 内 `queue.write_texture` 与
    `copy_texture_to_texture` 命中为 0，源码静态事实）✅
- **结论：通过**——Mask RenderPass 0、TextureWrite 0、global uniform 64B 写 300 次，
  ArtMesh uniform 整段写 0。

### 门禁 2：稳定 viewport 下 texture create / bind-group create 必须为 0

- 静止段 `Create clip set N texture` debug 行 = **0**（mask 纹理无重建）✅
- 静止段 HAL 增量：`textures=0`, `bind_groups=0`（HAL 字段恒 0，但有 3.3 节
  源码静态事实兜底）✅
- **结论：通过**。

### 门禁 3：静止帧允许 global 64B write，ArtMesh 全段写和 mask pass 应为 0

- `ArtMesh #N changed` = **0** ⇒ L991-999 全段扫描无 dirty 命中 ⇒
  `any_changes = false` ⇒ L1024-1026 早退返回 ⇒ L1030-1052 / 1141-1205 整段
  `write_buffer_with(artmesh_buffer)` + 逐 slot `copy_from_slice` **全部不执行** ✅
- `Render ArtMesh` (mask pass) = **0** ⇒ L1230 begin_render_pass 0 次 ⇒
  mask 纹理未被重绘 ✅
- global 64B write 在早退前（L962）无条件执行 ⇒ 每帧 1 次 64B ✅
- **结论：通过**。

### 门禁 4：动作帧若 ArtMesh 全段写是性能热点

- **本工具不测**动作帧——任务范围限定静止帧。
- 按裁决 97-98 行：「优先向 Ayagami 上游提交 dirty-uniform 优化，
  不在 l2d 复制私有资源状态」；P1-1 仅验证静态符合性，不触发本门禁。
- **结论：N/A（不在本工具范围）**。

---

## ⑤ 结论性观察（不裁决是否要改上游）

1. **静止帧路径在当前 wgpu 29 + ayagami rev 640ae4b 组合下完全符合 C5 门禁 1-3**：
   `prepare()` 的 `any_changes` 早退机制有效；mask / 顶点 / 纹理 / bind group
   全部走「dirty / 尺寸」守护，300 帧内 0 次重创建、0 次重写。
2. **每帧 64B global uniform 写是不可避免的固定成本**——它在 L962 早退之前
   无条件执行，且仅当 `options.transform` 真变化时（`renderer.rs:1003-1007`）
   内容才不同；静态 viewport 下 `options.transform` 恒定，可视为 64B/帧的
   已知常量成本。
3. **warm-up 段观察**确认了 dirty 标志在 load_model + 物理 settle 后
   全部清零（`renderer.rs:1623-1655`），这是早退成立的前提——若上游
   后续 commit 修改了 dirty 清零位置，warm-up 段 `changed > 0` 会立即
   暴露，可作为回归探针。
4. **HAL `counters` 特性未启用** 不影响门禁判断：本工具用日志事件 + 源码
   静态事实的双重确认覆盖了 C5 全部三条可验证门禁；`get_internal_counters()`
   已是 fallback，未来如需实测增量只需打开 `wgpu/counters` 特性。
5. **本工具仅验证静态路径**；动作帧（口型 / 表情 / 物理强驱动）下
   `artmesh_buffer` 整段重写（L1030-1052 + 1141-1205）的实际字节量
   需另起 C5.2 任务测量。本任务范围按裁决 97-98 行不在 P1-1。

---

## ⑥ 复现命令

```text
# 1. 门禁编译
cargo check -p l2d --all-targets

# 2. 跑现有 43 条测试，确认未破坏
cargo test -p l2d --all-targets

# 3. 跑本工具
cargo run -p l2d --example static_frame_trace

# 4. 可选：自定义参数
# cargo run -p l2d --example static_frame_trace -- \
#   Live2D-Ai-pc/open-llm-vtuber/live2d-models/bai/runtime/bai.model3.json \
#   1024 5 300
```

工具源码：`crates/l2d/examples/static_frame_trace.rs`（≤500 行）。
原始输出：`docs/verification/logs/static_frame_trace_raw.txt`（38 行）。
参考：上游方法学 `docs/verification/node-c-c5-prepare-upload-audit.md` ④ 节 4.3。
裁决源：`docs/plans/node-c-c1-c11-formal-audit-2026-08-27.md` C5 节 87-98 行。

---

## 附录：行号速查

| 位置 | 含义 |
|---|---|
| `l2d/examples/static_frame_trace.rs:69-82` | LogCapture 与 log::Log impl |
| `l2d/examples/static_frame_trace.rs:138` | 静止段循环入口 |
| `l2d/renderer/offscreen.rs:32-58` | OffscreenRenderer::new（沿用现成 API） |
| `l2d/renderer/model_core.rs:191-205` | render_to_view（阻塞路径，本工具调它） |
| `ayagami-render/src/renderer.rs:960-962` | global_buffer 64B 写（L962） |
| `ayagami-render/src/renderer.rs:991-999` | ArtMesh dirty 扫描（→ `ArtMesh #N changed` log） |
| `ayagami-render/src/renderer.rs:1024-1026` | `any_changes == false` 早退（关键防线） |
| `ayagami-render/src/renderer.rs:971` | `Create clip set N texture` log（mask 重建） |
| `ayagami-render/src/renderer.rs:1268` | `Render ArtMesh` log（mask pass 内） |
| `ayagami-render/src/renderer.rs:1611` | `Draw ArtMesh` log（主 pass 内） |
| `ayagami-render/src/renderer.rs:877-901` | uniform_bind_group 一次性创建 |
