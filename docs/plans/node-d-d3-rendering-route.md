# 节点 D3 — Live2D 渲染路线裁决（2026-08-28）

> **地位**：节点 D（原生应用 + Web 前端产品化闭环）D3 任务裁决——模型
> 导入/选择/展示/缩放/移动/背景。**先**冻结渲染路线再实现资产 API：
> 资产索引与路线无关，但渲染层决定组件化抽象、StageConfig 字段语义、
> 部署形态（dev/headless/Window）。本文即路线裁决单一真理。

## 0. 决策范围

- 节点 D 终态：用户可导入皮套 → 在浏览器或桌面上看模型 → 可缩放/移动/
  旋转 → 切背景（透明/纯色/图片）。本批先做**资产 API + registry**，
  展示控件延后到 D5（Web 前端）或原生 `app::settings_ui`。
- 裁决的**两条路线**：
  1. **路线 A — 浏览器直接渲染**：WASM + WebGPU/WebGL2，复用 `l2d-wasm-demo`
     的渲染核心；前端/原生窗口都通过 iframe / 嵌入式 webview 显示。
  2. **路线 B — 原生后端渲染**：复用现有 wgpu `ModelRendererCore` 与
     `OffscreenRenderer`，后端渲染到纹理 → 截图/视频流推到 Web。
- **裁决** = 选 A（路线 A）。下面给理由、落地步骤与 B 的退路。

## 1. 现状

- **`crates/l2d-wasm-demo`**：trunk 编译的独立 demo（编译门禁已通过
  `cargo check --target wasm32-unknown-unknown`；native stub 不渲染）。
  渲染核心走 `ModelRendererCore::render_to_view_submit`（实时路径
  `PollType::Poll`，**不**调阻塞 `render_to_view`——C1/C6/C8 硬门禁）。
  - 入口 `main.rs::web::run()`：fetch model3.json → 按 manifest 拉资源
    → `ModelPackage::from_memory_map`（**内存构造**）→ `LoadedModel::resolve`
    → `init_gpu`（WebGPU 优先、WebGL2 回退）→ rAF 60Hz 固定 dt 循环。
  - 关键文件：`web/surface.rs`（surface/FrameState/C4 状态机）、
    `web/net.rs`（fetch + 引用解析）。`cargo check` 全绿。
  - 现状：单文件 main + 两个子模块（632 行），**非组件化**；模型由
    `?model=<URL>` query 指定；无缩放/移动/旋转控件（仅 canvas resize）。
- **`crates/live2d-ai-desktop`**：原生 egui + wgpu 0.30 + winit 0.30
  透明无边框窗口；Bai 已通过 `ModelRendererCore` 接入；声音经 cpal 0.18
  输出；supervisor 在独立 OS 线程跑 `current_thread` runtime。Web 层走
  节点 D2：tiny_http 0.12，loopback 39221，21 个端点。Web 静态壳在
  `web_api/index.html`（W8，单页配置面板，**无**渲染区）。
- **沙箱现状**：CI/开发环境无 X server / 无 GPU；`l2d-wasm-demo` 已通过
  `cargo check` 编译门禁但**没有实跑**；原生 `ModelRendererCore` 走
  `pollster::block_on` 等待 device，无 display 走不通。**这意味着 D3
  期间我们没有「真渲染」验证手段**，只能写编译/单测/契约门禁。

## 2. 对比矩阵

| 维度 | A 浏览器直接渲染 | B 原生后端渲染 |
|---|---|---|
| **交互自然度** | 浏览器原生事件（拖拽/滚轮/触摸），零 IPC | 需后端发鼠标/键盘事件到客户端；或客户端发 control 命令 |
| **现有复用** | 复用 l2d-wasm-demo 渲染核心 | 复用 desktop 已有的 ModelRendererCore |
| **延迟** | 本地进程内（wasm 共享主线程，零序列化） | 后端渲染 → 编码 → 推送；**至少一帧延迟**（截图/视频流） |
| **透明背景** | `<canvas>` 直接合成 | 视频流 alpha 通道不通用；截图需 PNG → 序列化（贵） |
| **实现复杂度** | 组件化 + 嵌入式 webview | wgpu surface + 编码管线（H.264/VP9/WebRTC） |
| **沙箱可测性** | `cargo check --target wasm32` 已绿；浏览器实跑需 trunk serve + 静态目录 | **无 display 时无法真跑**（wgpu device 请求失败） |
| **WebGPU 兼容** | 浏览器矩阵（Chrome ≥113 / Edge ≥113 / Firefox Nightly / Safari 17+） | 不依赖 |
| **D5 前端集成** | 天然 — 前端本身是 Web，零桥接 | 前端通过 WS 收帧；视频走 MSE/WebRTC |
| **沙箱模型分发** | 浏览器 fetch `/models/<id>/...`（已代理思路见 l2d-wasm-demo Trunk.toml） | 后端把模型字节经 WS 推给浏览器（不优雅） |
| **离线场景** | 文件直 fetch，零中间层 | 仍需视频/截图通路 |
| **性能/CPU** | 浏览器 GPU 加速（WebGPU 优先） | 桌面 GPU 加速；额外编码 CPU |
| **复杂度风险** | WebGPU 兼容矩阵外部 RC | 自研编码管线 = 多月工作量 |

## 3. 路线 A 的关键问题与回答

### 3.1 l2d-wasm-demo 能抽组件化吗？

**答：能。** 当前 demo 是单文件入口，模型来源由 query 决定。组件化三步：

1. **抽 `Live2DView` 组件**（新建 `crates/l2d-wasm-demo/src/view.rs`）：
   - 输入：`BTreeMap<String, Vec<u8>>`（model3 清单 + 资源），或
     `&LoadedModel`（已解析的句柄）；
   - 输出：`<canvas>` + `Rc<RefCell<...>>` 共享态；
   - 接口：`new(window, canvas, model_url) -> Result<Live2DView, String>` +
     `start(self)` 一次性启动 rAF 循环；`update_model(...)` 切皮套。
2. **解耦模型加载与渲染**：`net.rs` 抽 `pub fn load_model_from_url(window, url) -> Future<Output=Result<LoadedModel, String>>`，
   组件只接已解析的 `LoadedModel`；原 `run()` 拆为「load → view.spawn」。
3. **状态从 query 改为可注入**：组件接受 `InitOpts { model_url, scale,
   offset_x, offset_y, rotation, fit_mode, background_type, ... }`；D5
   通过 postMessage / window 注入，避开 query string 序列化。

预计组件化增量 200~300 行（不破坏现有 `cargo check` 门禁）。具体落
地推迟到 D5 阶段；本批（D3 资产 API）**仅**做索引与契约。

### 3.2 WebGPU 浏览器兼容矩阵是外部 RC？

**接受为外部风险。** WebGPU 在主流桌面浏览器（Chrome 113+/Edge 113+
2023-05 起、Firefox Nightly、Safari 17+）已稳定；本项目不需要覆盖
老浏览器——节点 D 用户群是开发者自部署，浏览器可升级。**退路**：demo
已启用 wgpu `webgl` feature（WebGL2 回退），实测覆盖率接近 100%
桌面浏览器。**对端用户无影响**。

### 3.3 嵌入式 webview（如果原生要显示）？

D5 不需要：D5 是 Web 前端，跑在 `127.0.0.1:39221`（与原生 API 同源），
用户**直接**用浏览器打开，不需要原生 webview。原生窗口（egui/winit）
**只**承担"系统托盘 + 音频输出"——这是 D2 已锁定的形态（节点 D1 §1.1
`runtime_ws` / `state_ws`）。

## 4. 路线 B 的退路（备选，仅记录）

如果未来发现路线 A 真的不行（例如 wgpu 升级破坏了 wasm 路径、或用户
强制要求「无浏览器」的体验），**降级路径**：

1. 复用 desktop 的 `ModelRendererCore` + `OffscreenRenderer`；
2. 渲染到 RGBA 纹理 → `wgpu::Texture::as_image_copy` 拷到 readback
   buffer → 上传 PNG / H.264 帧到 Web；
3. 走 WS 或 Server-Sent Events 把帧推给前端；前端用 `<canvas>` blit。
4. 鼠标事件：前端发 `MoveModel { dx, dy }` / `ScaleModel { factor }` →
   supervisor 改 StageConfig → 下一帧应用。

**预计工时**：1+ 人月（编码管线 + WebRTC/MSE 适配 + 透明背景方案），
**不是 D3 阶段**的合理选择。

## 5. 裁决与落地

**裁决：选 A — 浏览器直接渲染。**

理由（按权重排序）：

1. **延迟/体验**：路线 A 0 IPC，路线 B 至少一帧延迟（截图/视频流）。
2. **沙箱可测性**：路线 A 已通过 `cargo check`；路线 B 无 display 跑不通。
3. **实现复杂度**：路线 A 仅组件化（200~300 行）；路线 B 编码管线是
   1+ 人月量级的独立工程。
4. **透明背景**：路线 A 天然；路线 B 视频流 alpha 难（截图+PNG 又贵）。
5. **D5 集成**：路线 A 零桥接；路线 B 需要 WS/MSE 协议。
6. **复用**：两边都复用 `l2d` 的 `ModelRendererCore`（路线 A 经 wasm），
   复用面相当；但 A 多一个优势：直接走 `ModelPackage::from_memory_map`，
   不必经磁盘字节。

**本批（D3）落地**：

- **资产 API**（D3 本批）：实现 `GET/POST /api/v1/models*` + registry
  （`~/.local/share/live2d-ai/model_registry.json`）；registry 与渲染
  路线**完全解耦**——任何路线都能读。
- **StageConfig**（D3.1 落地）：定义 `ModelDisplay { scale, offset_x,
  offset_y, rotation, fit_mode }` + `StageConfig { width, height,
  background_type, background_color, background_image, background_opacity }`，
  作为 D1 §8 schema 的 Rust 类型；存到 registry。
- **浏览器组件化**（D3 延后到 D5 / W11）：在 D5 阶段从 l2d-wasm-demo
  抽 `Live2DView` 组件；本批不写。
- **原生面板**（D3 延后）：egui 的"模型" tab 用同样 StageConfig schema；
  本批**不**写（settings_ui 已超 500 行上限，分到 D5）。

## 6. 安全/契约要点（与资产 API 同步冻结）

- **路径安全**（P0-4）：import 端点**只**接受 `assets/models/<id>/` 下
  相对路径；拒绝 `..`、绝对路径、符号链接外指（`symlink_metadata`）。
- **激活模型不可删**（D1 §1.3 409 model_active）：与 `DELETE` 端点配对。
- **本地导入 vs ZIP 上传**：本批**只**做"受控本地导入"（body 指定
  相对路径）。ZIP 上传标 D3.2 后置（上传校验复杂、目录穿越风险面更
  大；本批先把路径安全 + 校验跑通）。
- **display 配置持久化**：`StageConfig` 作为 registry 内 `display` 段；
  PATCH 走 `display` 段（与 settings 的 `stage` 段平级——D1 §8 schema
  是单一真理）。

## 7. 范围

**本批（D3）做**：

- 资产 API 端点实现（GET list / GET by id / POST import local /
  POST activate / PATCH display / DELETE）；
- registry JSON 持久化（原子写回 + 路径安全）；
- StageConfig Rust 类型（与 D1 §8 对齐）；
- 路由注册 + mutating 安全校验（`is_mutating_route` 加入 Model* 变体）。

**本批不做**（明确划到 D5/W11）：

- ZIP 上传端点（`POST /api/v1/models` multipart）—— D3.2 后置；
- l2d-wasm-demo 组件化（抽 `Live2DView` + 注入模型源）；
- 原生 egui 展示面板（`settings_ui::Draft.stage` 字段扩展）；
- D5 前端的拖拽/缩放控件与 Live2DView 集成。

## 8. 退路 / 风险

| 风险 | 触发条件 | 退路 |
|---|---|---|
| WebGPU 浏览器不可用 | 用户浏览器 < Chrome 113 / Safari 16 | 启用 wgpu `webgl` 回退（已就位） |
| l2d-wasm-demo 组件化阻塞 | wasm 编译破坏或重构代价 > 1 周 | 维持 demo 单文件形态，D5 直接 fork |
| 用户强制要求「无浏览器」 | 显式需求 | 走 §4 路线 B（编码管线，1+ 人月） |
| 路径穿越攻击 | mutating 端点绕过校验 | Origin + Content-Type 双重门禁（已冻结） |

## 9. 与节点 D 其它子任务的关系

- **D1 契约**：D1 §1.3 列出 models 端点；§6.3 字段对齐表；§8
  StageConfig。本批落 §1.3 + §6.3 + §8。
- **D2 接线**：D2 已冻结 RouteId NotImplemented = 501；本批把 models
  系列从 501 切到 200/4xx。
- **D4 命令**：models 不属于 command，路由独立；D4 `POST /api/v1/commands/
  {id}/invoke` 不动。
- **D5 前端**：D5 用本批的 StageConfig 渲染表单（拖拽/缩放控件直接
  PATCH `display` 段）；Live2DView 组件在 D5 抽自 l2d-wasm-demo。
