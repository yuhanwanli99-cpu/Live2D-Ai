# 节点 C 引用位置重新核查摘要（2026-08-27）

> 范围：仅核查 `docs/plans/node-c-render-platform-audit-brief.md`「现状事实」
> 一节中列出的 file:line 引用，在 `crates/live2d-ai-desktop/src/app.rs` 拆分为
> `src/app/` 目录后是否仍准确。**不**修改 C1–C11 任何裁决问题措辞。
>
> 方法：grep + 关键上下文 read；每条事实给出新 file:line + 原文证据。

---

## a. `render_to_view` 末尾 `device.poll(PollType::wait_indefinitely())`

- **旧引用**：`crates/l2d/src/renderer/model_core.rs:176-179`
- **新位置**：`crates/l2d/src/renderer/model_core.rs:175-179`
- **grep 命令**：`grep -n 'poll(PollType|wait_indefinitely|device\.poll' crates/l2d/src/renderer/model_core.rs`
- **命中**：`Line 178: .poll(wgpu::PollType::wait_indefinitely())`
- **上下文（read offset=165, limit=25）**：
  - L173: `self.renderer.render(&mut pass, format);`
  - L175: `self.gpu.queue().submit(Some(encoder.finish()));`
  - L176-179:
    ```rust
    self.gpu
        .device()
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| RenderError::Poll(format!("frame wait failed: {e}")))?;
    ```
- **结论**：行号基本未变（submit 上移 1 行，poll 段仍在 176-179）。事实准确。

## b. 离屏读回路径 `wait_indefinitely()` 后读 buffer

- **旧引用**：`crates/l2d/src/renderer/mod.rs:210-211`
- **新位置**：`crates/l2d/src/renderer/mod.rs:209-213`
- **grep 命令**：`grep -n 'poll(PollType|wait_indefinitely|device\.poll|map_async|read_buffer' crates/l2d/src/renderer/mod.rs`
- **命中**：
  - L194: `/// 2. device.poll(wait_indefinitely) 真实阻塞至队列排空——wgpu 保证`
  - L210: `.poll(wgpu::PollType::wait_indefinitely())`
- **上下文（read offset=185, limit=35）**：
  - L189-199: 函数级模块文档，说明三步阻塞语义
  - L200: `pub(crate) fn readback_rgba(...)`
  - L209-211: `gpu.device().poll(wgpu::PollType::wait_indefinitely()).map_err(...)?`
  - L213: `let img = rx.recv().map_err(|_| RenderError::ReadbackClosed)?;`
- **结论**：poll 段在 209-211（旧 210-211 略上移 1 行）；`recv()` 收回调在 213
  单独成行（原 211 是 poll 末行+recv）。**事实定性不变**（仍是 wait 后取回调）。

## c. 纯透明壳路径每帧后非阻塞 Poll

- **旧引用**：`crates/live2d-ai-desktop/src/app.rs:942-944`（3 行）
- **新位置**：`crates/live2d-ai-desktop/src/app/frame.rs:55-61`（4 行）
- **grep 命令**：`grep -rn 'device\.poll|PollType|wait_indefinitely' crates/live2d-ai-desktop/src/app/`
- **命中**：
  - `crates/live2d-ai-desktop/src/app/frame.rs:59: if let Err(e) = state.device.poll(wgpu::PollType::Poll) {`
  - `crates/live2d-ai-desktop/src/app/mod.rs:43: //! - 每帧 device.poll(PollType::Poll) 泵送设备队列（纯透明壳路径）；`
- **上下文（read offset=40, limit=35）**：
  - L55: `state.queue.submit(Some(encoder.finish()));`
  - L56: `frame.present();`
  - L58: `// 泵送设备（非阻塞），驱动上传/回调管线。`
  - L59-61:
    ```rust
    if let Err(e) = state.device.poll(wgpu::PollType::Poll) {
        warn!("device poll 异常: {e:?}");
    }
    ```
- **结论**：3 行（原 submit/present/Poll）→ 4 行（submit 55、present 56、Poll 59
  + 失败 warn 60）。**行为定性不变**：每帧后非阻塞 Poll；拆分时仅把失败处理
  包成 `if let Err` 块并加 warn 日志，**不**改变调用语义。

## d. 模块文档「功能优先、耗时如实统计」

- **旧引用**：`crates/live2d-ai-desktop/src/app.rs:18-21`（模块文档）
- **新位置**：
  - `crates/live2d-ai-desktop/src/app/mod.rs:40-43`（模块文档，措辞基本一致）
  - `crates/live2d-ai-desktop/src/app/frame.rs:64-69`（**新增**函数级细化说明）
- **grep 命令**：`grep -rn '功能优先|耗时如实|先通' crates/live2d-ai-desktop/src/app/`
- **命中**：
  - `crates/live2d-ai-desktop/src/app/mod.rs:41: //!   render_to_view 内部 submit + 阻塞 poll——功能优先、耗时如实统计；`
  - `crates/live2d-ai-desktop/src/app/frame.rs:69: /// 不代表纯 CPU 提交成本；功能先通，优化留待后续批次。`
- **上下文（read mod.rs 全文）**：
  - L40-43（模块文档）：
    ```rust
    //!   `set_viewport`，渲染到 **surface view** 后 present。注意当前
    //!   `render_to_view` 内部 submit + 阻塞 poll——功能优先、耗时如实统计；
    ```
- **上下文（read frame.rs offset=40, limit=35）**：
  - L64-69（函数文档，新增）：
    ```rust
    /// Live2D 实时渲染一帧：真实 dt → 固定步模拟 → 参数写入 → 渲染到
    /// **surface view** → present。失败为类型化冒烟错误。
    ///
    /// 耗时口径：`render_to_view` 内部 submit 后会阻塞 poll 等队列排空
    /// （离屏读回同款语义），因此本函数测得的帧耗时是**包含 GPU 等待的诚实值**，
    /// 不代表纯 CPU 提交成本；功能先通，优化留待后续批次。
    ```
- **结论**：模块级措辞在 mod.rs:41 保留；**拆分时新增** frame.rs:64-69 函数级
  细化说明「包含 GPU 等待的诚实值，不代表纯 CPU 提交成本」。**行为描述更精确，
  仍承认技术债，定性不变**。

## e. 初始化 `request_adapter / request_device`

- **旧引用**：
  - `crates/live2d-ai-desktop/src/app.rs:621, 642`
  - `crates/l2d/src/renderer/gpu.rs:92, 104`
- **新位置**：
  - `crates/live2d-ai-desktop/src/app/bootstrap.rs:66-71, 87-94`
  - `crates/l2d/src/renderer/gpu.rs:92, 104`（**未动**）
- **grep 命令**：
  - `grep -rn 'request_adapter|request_device|pollster::block_on' crates/live2d-ai-desktop/src/app/`
  - `grep -rn 'request_adapter|request_device|pollster::block_on' crates/l2d/src/`
- **命中**：
  - `app/bootstrap.rs:67: match pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {`
  - `app/bootstrap.rs:88: match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {`
  - `l2d/src/renderer/gpu.rs:92: pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {`
  - `l2d/src/renderer/gpu.rs:104: match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {`
- **上下文（read bootstrap.rs offset=60, limit=35）**：
  - L66-71（adapter）：
    ```rust
    let adapter =
        match pollster::block_on(self.instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })) {
    ```
  - L87-94（device）：
    ```rust
    let (device, queue) =
        match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("live2d-ai-desktop-shell"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
    ```
- **结论**：app/ 侧行号偏移（旧 621/642 → 新 67/88，模块拆出后行号整体上移）；
  l2d 侧行号 92/104 未动。**事实定性不变**：均为 `pollster::block_on` 一次性
  初始化，不在热路径。

## f. `surface reconfigure` 与 `set_viewport` 先后契约注释

- **旧引用**：`crates/live2d-ai-desktop/src/app.rs:872`（单行注释）
- **新位置**：
  - `crates/live2d-ai-desktop/src/app/surface.rs:5-8`（**模块文档**说明）
  - `surface.rs:33-34`（`reconfigure_current` **函数文档**）
  - `surface.rs:45-47`（**代码顺序**：`surface.configure` 在 45，
    `sync_model_viewport` → `set_viewport` 在 46-47）
- **grep 命令**：`grep -rn 'reconfigure|set_viewport|surface.configure' crates/live2d-ai-desktop/src/app/`
- **命中**：
  - `app/handler.rs:81: state.surface.configure(&state.device, &state.config);`
  - `app/bootstrap.rs:135: surface.configure(&device, &config);`
  - `app/bootstrap.rs:282: core.set_viewport(initial_viewport.0.max(1), initial_viewport.1.max(1))`
  - `app/frame.rs:239, 244: self.reconfigure_current();`（Lost/Suboptimal 恢复）
  - `app/surface.rs:5: //! - [ShellApp::reconfigure_current]：用当前窗口尺寸重新 configure`
  - `app/surface.rs:25: && let Err(e) = model.core.set_viewport(width.max(1), height.max(1))`
  - `app/surface.rs:35: pub(crate) fn reconfigure_current(&mut self) {`
  - `app/surface.rs:45: state.surface.configure(&state.device, &state.config);`
  - `app/surface.rs:46: // 模型视口与 surface 尺寸保持同步（aspect 修正随 set_viewport 变化）。`
  - `app/surface.rs:60: self.reconfigure_current();`（recreate_surface 内）
- **上下文（read surface.rs 全文）**：
  - L5-8（模块文档）：
    ```rust
    //! - [ShellApp::reconfigure_current]：用当前窗口尺寸重新 configure
    //!   （Outdated/Suboptimal/Lost 恢复共用）。**必须在旧 `SurfaceTexture`
    //!   drop/present 之后调用**——wgpu 在仍有存活 swapchain 纹理时 configure
    //!   会 panic。
    ```
  - L33-34（函数文档）：
    ```rust
    /// 注意：必须在旧 `SurfaceTexture` drop/present **之后**调用——
    /// wgpu 在仍有存活 swapchain 纹理时 configure 会 panic。
    ```
  - L45-47（代码顺序）：
    ```rust
    state.surface.configure(&state.device, &state.config);
    // 模型视口与 surface 尺寸保持同步（aspect 修正随 set_viewport 变化）。
    self.sync_model_viewport();
    ```
- **结论**：原 `app.rs:872` 单行注释在拆分时**升级为三处契约**：
  模块文档（5-8）、函数文档（33-34）、代码顺序（45-47）。**行为描述更精确**，
  仍表达「configure 必须在旧 SurfaceTexture drop/present 之后 + 之后才能
  set_viewport」的契约。**C4 原文 `app.rs:872` 引用源已不准确**，建议
  在 C4 裁决问题措辞中同步更新引用源（高级 Agent 决定）；本文件**不动 C4 措辞**。

---

## 汇总表

| 编号 | 旧位置 | 新位置 | 拆分后行数 | 行为定性 |
|---|---|---|---|---|
| a | `l2d/renderer/model_core.rs:176-179` | `l2d/renderer/model_core.rs:175-179` | 基本一致 | 不变 |
| b | `l2d/renderer/mod.rs:210-211` | `l2d/renderer/mod.rs:209-213` | poll 略上移，recv 单独成行 | 不变 |
| c | `app.rs:942-944` | `app/frame.rs:55-61` | 3→4 行（多 warn） | 不变 |
| d | `app.rs:18-21` 模块文档 | `app/mod.rs:40-43` + `app/frame.rs:64-69` | 模块级保留 + **新增**函数级细化 | 不变（措辞更细） |
| e | `app.rs:621,642` + `l2d/gpu.rs:92,104` | `app/bootstrap.rs:66-71,87-94` + `l2d/gpu.rs:92,104` | app/ 侧行号偏移 | 不变 |
| f | `app.rs:872` 单行注释 | `app/surface.rs:5-8, 33-34, 45-47` | 单行注释升级为模块/函数文档+代码顺序 | 不变（契约更精确） |

## 风险/待裁决复核

- **C4 引用源待裁决复核**：C4 原文「`app.rs:872` 已有注释」现指向
  `app/surface.rs:5-8, 33-34, 45-47`。C1–C11 裁决问题措辞本任务不动，
  高级 Agent 决定是否同步更新 C4 措辞引用源。
- **d 项新增细化说明**：`app/frame.rs:64-69` 是拆分时**新增**的函数级
  文档，说明「测得帧耗时是包含 GPU 等待的诚实值」，对 C1「render_to_view
  保留阻塞语义」和 C7「benchmark 协议」的解释有补充作用，**不改变行为**。
- **c 项 warn 日志新增**：`app/frame.rs:60` 把 Poll 失败显式 warn，原
  `app.rs:942-944` 无此 warn。**行为不变**（Poll 仍是非阻塞），仅失败
  可见性增强（与 C2「非阻塞后的错误可见性」议题相关，可供高级 Agent
  评估）。
