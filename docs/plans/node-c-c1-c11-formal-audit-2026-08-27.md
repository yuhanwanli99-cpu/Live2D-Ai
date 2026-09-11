# 节点 C C1–C11 正式裁决审计（2026-08-27）

> **规则源地位**：本文是节点 C 渲染性能与平台 RC 的正式裁决。实现方应按“实施顺序与门禁”执行；与旧行号或旧说明冲突时，以本文为准。
>
> **审计方式**：独立静态审计当前 workspace，交叉核对 wgpu 29.0.4、winit 0.30.13 与锁定 Ayagami rev `640ae4b` 的实际 API/源码。未把 C1–C11 裁决委派给其他 Agent。

## 总结裁决

- **允许启动节点 C 实现**，但 RC 当前被三个 P0 阻断：实时路径阻塞 wait、异步 GPU 错误不可收口、点击穿透布尔语义反转。
- 离屏读回可以继续阻塞；desktop 与 WASM 的交换链实时路径必须改为只提交、不等待。
- X11/Wayland 能力必须按“协议支持 + 实际验证”分级承诺，不能把 API 返回成功等同于用户可见行为已经生效。
- WASM v0 只承诺编译门禁和浏览器手测协议，不承诺正式运行时兼容矩阵。

---

## C1：实时渲染 API 拆分

**裁决：接受双 API，不采用 `PollPolicy` 参数。**

保留 `ModelRendererCore::render_to_view` 的阻塞兼容语义，新增 `render_to_view_submit`；实时 desktop/WASM 只调用 submit 版本，离屏与确定性测试可继续调用阻塞版本。同步策略不是渲染内容参数，把它塞进 `PollPolicy` 会让调用点容易误选，也会把 native/WebGPU 差异扩散到业务层。

**实现注意点：**

```rust
pub fn render_to_view_submit(
    &mut self,
    view: &wgpu::TextureView,
    format: wgpu::TextureFormat,
) -> Result<wgpu::SubmissionIndex, RenderError>;

pub fn render_to_view(...) -> Result<(), RenderError> {
    let index = self.render_to_view_submit(...)?;
    self.gpu.device().poll(wgpu::PollType::Wait {
        submission_index: Some(index),
        timeout: None,
    })?;
    Ok(())
}
```

- 阻塞版本只等待自己的 submission，不再使用“等待调用时最新全部工作”的无索引 `wait_indefinitely()`。
- `draw_and_present_model` 与 WASM `render_and_present` 必须改调 submit 版本，然后 `present`。
- 离屏 `readback_rgba` 的 wait + recv 保留；截图本来就必须等待 GPU 回调。

## C2：非阻塞后的 GPU 错误可见性

**裁决：安装 `Device::on_uncaptured_error`，通过线程安全故障闩在事件循环线程收口；`Poll` 失败不得只 warn。**

wgpu 明确说明错误可能同步或异步发生，当前仅调用 `device.poll(Poll)` 而没有 uncaptured-error handler，无法把 validation/OOM/internal error 映射到现有退出码。错误回调不得直接操作 `ShellApp`，只能写入 `Arc<Mutex<VecDeque<GpuFault>>>` 或单写闩，由每帧开始/结束时消费。

**分类规则：**

| wgpu 错误 | 分类 | 退出码 |
|---|---|---:|
| `Error::Validation` | 代码/资源使用错误 | 1 |
| `Error::OutOfMemory` | GPU 运行环境不足 | 3 |
| `Error::Internal` | 驱动/后端内部失效，默认环境错误 | 3 |
| `Device::poll` 返回错误 | 设备/队列失效 | 3 |
| surface `Validation` 连续出现 | 首次记录；连续 2 帧即代码错误 | 1 |

- desktop 模型路径写 `model_failure`；纯透明壳写 `environment_failure` 或新增统一 `gpu_failure`。
- 异步 fault 一旦消费即停止继续提交，避免每帧重复刷错。
- 日志必须包含 error kind、backend、adapter 和最近 submission 序号。

## C3：多帧 in-flight 上限

**裁决：当前不加自制 semaphore；以 `PresentMode::Fifo + desired_maximum_frame_latency = 2` 作为首版上限。**

当前 desktop 已配置 Fifo 和最大帧延迟 2，再叠加手写等待队列容易重新引入 CPU 阻塞。先记录 submission/presented/skipped 与内存趋势；只有真实 GPU 证明队列仍无界增长，才增加“等待最老 submission”的可选保护。

**实现注意点：** WASM surface 配置也显式设置 `desired_maximum_frame_latency = 2`，不要依赖 `get_default_config` 的版本缺省值。

## C4：resize、surface 与 mask 生命周期

**裁决：正式引用更新为 `app/surface.rs:5-8,33-47`；采用事件合并，不采用固定毫秒防抖。**

合法顺序是：旧 `SurfaceTexture` 已 present/drop → 更新 config → `surface.configure` → `set_viewport`；Ayagami mask 由下一次 `prepare` 看到 `mask_dimensions` 改变后惰性重建。固定计时防抖会制造视觉延迟，应该只保存最新非零尺寸并在下一次 redraw/acquire 前应用一次。

**实现注意点：**

- `Resized` 只写 `pending_surface_size` 并请求 redraw；同一事件批次的多次 resize 合并为最后值。
- Suboptimal：先提交并 present 当前 texture，再 reconfigure，desktop 当前路径正确。
- Outdated：无 texture，直接应用 pending/current config 后跳过本帧。
- Lost：重建 surface，再 configure，再同步 viewport。
- WASM 当前 `Suboptimal(frame)` 分支在 frame 仍存活时先 `configure`，违反本契约，必须改为先 render/present，再 configure。

## C5：Ayagami `prepare()` 与逐帧上传

**裁决：保留每帧 `prepare`；禁止在 l2d 层重复缓存 Ayagami bind group。**

取证确认模型纹理、静态 buffer、pipeline、texture bind group 和 uniform bind group 均为一次性；mask 纹理只在尺寸变化时重建，顶点和 mask 内容已有 dirty gate。已知固定成本是每帧 64B global uniform；变化帧会整段重写 ArtMesh/Part uniform，这是锁定上游的明确设计，不应在不了解内部一致性的情况下由 l2d 绕过。

**可核查门禁：**

1. 加一个 feature-gated/外部 benchmark，300 个静止帧记录 BufferWrite、TextureWrite、Mask RenderPass；
2. 稳定 viewport 下 texture create/bind-group create 必须为 0；
3. 静止帧允许 global 64B write，ArtMesh 全段写和 mask pass 应为 0；
4. 动作帧若 ArtMesh 全段写是性能热点，优先向 Ayagami 上游提交 dirty-uniform 优化，不在 l2d 复制私有资源状态。

## C6：WASM v0 承诺

**裁决：v0 只承诺编译门禁 + 指定浏览器手测，不承诺正式运行时支持矩阵。**

WebGPU 的 `PollType::Wait` 不具备 native 阻塞语义，且回调由浏览器事件循环驱动；当前 demo 还存在 Suboptimal texture 存活时 configure 的生命周期错误。完成 C1/C2/C4 接线并取得浏览器记录后，才能把具体浏览器升级为“实验性可运行”。

**v0 门禁：**

- `cargo check -p l2d-wasm-demo --target wasm32-unknown-unknown`；
- `trunk build --release`；
- Chrome/Edge WebGPU 手测：加载、连续 300 帧、resize、错误状态栏；
- WebGL2 fallback 至少一款浏览器手测；若无法验证，只能标“未验证”，不能宣称回退可用；
- rAF 使用 submit API，禁止实时路径调用阻塞 `render_to_view`。

## C7：benchmark 协议

**裁决：接受同帧数/同动作/同会话协议，并增加 warm-up、p95 算法和双时间口径。**

非阻塞后原 `frame_time` 会从“CPU+GPU wait”变成“CPU encode+submit”，不能与旧值直接同名比较。必须同时记录提交耗时和 present 间隔，才能区分 CPU 解阻塞与真实可见帧率。

**固定协议：**

- 同一 commit 构建模式、adapter/backend、窗口尺寸、模型、动作序列；
- warm-up 60 帧不计，正式 600 帧；
- 输出 `submit_ms avg/p50/p95/max`、`present_interval_ms avg/p50/p95/max`；
- 输出 presented/skipped，按 Timeout/Occluded/Outdated/Lost/Suboptimal/Validation 分桶；
- 输出 adapter、driver、backend、present mode、frame latency；
- llvmpipe 只作回归参考；至少补一组真实独显或核显数据。

## C8：性能验收线

**裁决：不把 llvmpipe ≥15fps 设为硬发布线；采用结构硬门禁 + 相对改善 + 真实 GPU 数值线。**

软渲染速度受 Mesa、CPU 和宿主负载影响，绝对 15fps 会产生假阴性；但仅看“移除了 wait”又不足以防止新的队列/上传回归。因此 llvmpipe 用相对线，真实 GPU 用用户可感知数值线。

**验收标准：**

1. 实时 desktop/WASM 调用图中不存在 `PollType::Wait`/`wait_indefinitely`；
2. 同机 llvmpipe 的 `submit_ms p95` 相比阻塞基线至少改善 30%，且 skipped/validation 不增加；
3. 至少一组真实 GPU 600 帧：presented ≥99%、`present_interval p95 ≤33.3ms`（最低稳定 30fps）；
4. max 仅诊断，不单独阻断，因为窗口调度和 shader 首次编译会产生离群点；
5. 设备 fault 测试必须确定性映射到退出码 1/3。

## C9：平台能力承诺表

**裁决：采用“正式 / 环境相关 / 明确不支持 / 未验证”四级，不用单一 available 包办用户承诺。**

`RuntimeCapabilities` 适合作运行观测，但某些 winit API 没有可回读结果，调用返回并不证明 WM/合成器产生了可见效果。README 必须把协议事实、API 调用和人工验证分开。

| 能力 | X11/XWayland | Wayland | RC 承诺 |
|---|---|---|---|
| 透明 surface | 正式，前提是运行时 alpha 为 Pre/PostMultiplied | 正式，同前提 | 无尊重 alpha 的模式则明确降级/不可用 |
| 置顶 | 环境相关，依赖 EWMH/WM 采纳 | 明确不支持 | X11 需人工视觉验证，不能只凭 void API 标正式 |
| 全局定位 | 环境相关，不保证 | 明确不支持 | WSLg 已知 unavailable；真实 WM 待测 |
| 点击穿透 | 修复后可成为正式能力 | 修复后、人工验证后为实验性/环境相关 | 当前实现存在 P0 布尔反转，暂不得承诺 |
| 拖动 | 环境相关，人工验证 | 环境相关，依赖合成器 | 成功一次可记 runtime available，但发布表仍标环境相关 |
| 托盘 | 环境相关，依赖 D-Bus/SNI host | 环境相关，同左 | 无 host 时可见降级且禁自动穿透 |

### C9-P0：点击穿透布尔反转

winit 0.30.13 的契约是 `set_cursor_hittest(true)` 捕获事件、`false` 才把事件传给后方窗口；当前 `apply_click_through(want)` 直接调用 `set_cursor_hittest(want)`，与 `want == click-through enabled` 的业务语义相反。必须改为：

```rust
window.set_cursor_hittest(!want)
```

并增加一个业务布尔到 hittest 布尔的纯函数单测；此前 WSLg 的“ct=available”只证明 API 调用未报错，不能证明穿透方向正确。

## C10：拖动人工复验

**裁决：列入正式 RC 手动验收；草稿可转正式，但步骤 6 必须只读取同一次运行的 RunReport。**

`drag_window()` 无法在当前沙箱可靠自动化，人工验证是诚实门禁。重新启动 Info 模式会丢失内存态，不能用于证明前一进程的 `drag_move=Available`。

**通过判据：**

- 至少真实 X11 WM 和真实 Wayland 合成器各一轮；
- 交互态左键拖动跟随，松开后保持位置；
- 穿透态无法触发拖动；
- 同一运行的 RunReport 记录 `drag_move=Available`；
- Wayland 某合成器失败按环境相关记录，不伪装成普遍支持。

## C11：托盘恢复链路

**裁决：列入正式 RC 手动验收，并把无 D-Bus 降级路径作为自动/半自动门禁。**

托盘是点击穿透后的主要恢复入口，隐藏→唤回→关闭穿透必须在真实 SNI host 上形成闭环。未通过该闭环的平台不得默认启用点击穿透。

**通过判据：**

- SNI host：图标/四菜单项、隐藏、唤回、穿透开关、置顶开关、退出全部通过；
- 唤回后位置不丢失；
- 无 D-Bus：启动不阻塞、`tray=Unavailable`、自动穿透禁用、窗口保持交互；
- C9 点击穿透布尔反转修复是 C11 手测前置条件。

---

## 实施顺序与节点门禁

### P0-1：语义与安全修复

1. 修复 `set_cursor_hittest(!want)`，补纯函数和状态镜像测试；
2. 修复 WASM Suboptimal 分支：present/drop 后才能 configure；
3. 更新 README，撤销当前未验证的点击穿透正式表述。

### P0-2：实时非阻塞闭环

1. 实现 `render_to_view_submit`，阻塞 wrapper 等待指定 submission；
2. desktop/WASM 改调 submit；
3. desktop 每帧 `Poll` 失败升级为 fatal；
4. 安装 uncaptured-error handler 并验证退出码分类；
5. 保持 Fifo + frame latency 2，不新增手写等待。

### P0-3：surface 生命周期

1. desktop resize 改为 latest-size 合并；
2. Suboptimal/Outdated/Lost 顺序测试；
3. WASM 同步采用相同生命周期规则。

### P1：证据与 RC

1. 完成 C5 静止帧上传 trace；
2. 按 C7 跑 llvmpipe 前后 benchmark；
3. 补真实 GPU 600 帧；
4. 执行 C10/C11 真实桌面手测；
5. 执行 WASM 浏览器手测，未测项目保持“未验证”。

## 自动化测试最低清单

- blocking wrapper 与 submit API 的调用/文档契约测试；
- desktop 实时路径源码/行为测试：不得触发 `PollType::Wait`；
- uncaptured Validation → exit 1，OOM/Internal/Poll → exit 3；
- `click_through=true → hittest=false` 与反向测试；
- Suboptimal 必须 present 后再 configure 的状态机测试；
- resize 多事件合并到最后尺寸；
- WASM target check + trunk release build；
- benchmark 输出字段 schema 测试。

## 额外风险

1. **当前点击穿透方向相反**：这是平台 RC 的真实 P0，不是文档措辞问题。
2. **WASM Suboptimal 生命周期错误**：frame 存活期间 configure 可能触发 validation/panic，运行时承诺前必须修。
3. **异步 GPU error 默认可能 panic**：wgpu 文档说明 uncaptured error 默认转 panic；非阻塞化前必须安装 handler。
4. **性能统计口径会改变**：移除 wait 后旧 `frame_time` 不再代表同一指标，若不改名会产生虚假“提速”。
5. **置顶能力缺少回读**：`set_window_level` 无 Result/状态回读，runtime `Available` 只能表示请求已发出，不等于 WM 已采纳。
6. **WASM 固定每 rAF 一次 60Hz update**：在 30Hz/后台节流时模拟会变慢；若未来承诺运行时，应改为 timestamp 累加器和步数上限。
7. **Ayagami 上游 pin 风险**：C5 结论绑定 rev `640ae4b`，升级 pin 必须重跑上传审计。

## 节点 C 完成条件

节点 C 只有在以下全部满足后才能标记通过：

- P0-1/P0-2/P0-3 实现与测试完成；
- C7 benchmark 产物包含规定字段；
- llvmpipe 相对门禁通过，且至少一组真实 GPU 达到 RC 线；
- C10/C11 手测有真实桌面记录，或对应能力在发布说明中明确降级为未验证/不承诺；
- WASM 编译门禁通过，浏览器未测能力不写成已支持；
- README/RunReport/能力表与本文 C9 口径一致。
