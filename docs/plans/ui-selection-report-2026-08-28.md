# UI 面板选型调研报告（2026-08-28）

> **地位**：选型调研，未实现任何代码。实现待用户拍板后另批次执行。
> **需求来源**：用户裁决「Rust 重构加个 UI 即可」；对标 Py 版设置面板（9 tab Web 表单，346 行 TS + 433 行 FastAPI）。
> **范围**：设置面板（读写 `live2d-ai.toml`），不是通用 UI 框架选型。

---

## 一、硬约束（项目现状）

| 约束 | 现状 | 影响 |
|---|---|---|
| 渲染栈 | wgpu **29**（app/frame.rs，节点 C 刚审计锁定语义） | 任何 GUI 方案必须兼容 wgpu 29，否则双版本共存 |
| 窗口 | winit **0.30**（0.30.13） | 同上 |
| 原则 | 原生轻量（无 Electron/浏览器栈） | 排除 Web 前端方案作为主形态 |
| 侵入度 | frame.rs 500 行贴线、非阻塞提交/fault 闭环/状态机刚过审 | 集成点必须最小侵入，新代码放新文件 |
| 验证环境 | 沙箱无 X server（实测确认） | GUI 方案只能编译级验证，无法真机冒烟 |
| 配置 | `live2d-ai.toml`（runtime::settings 三段 + api_key_env），无 write 能力 | 面板需补 settings 写回 + round-trip 测试 |

## 二、候选方案对比

### A. egui overlay（egui + egui-winit + egui-wgpu）⭐ 推荐

**兼容性实测证据**（egui workspace Cargo.toml 逐版本核查）：

| egui 版本 | 依赖 wgpu | 依赖 winit | 与项目匹配 |
|---|---|---|---|
| 0.32 | 25.0 | 0.30.13 | ✗（wgpu 25） |
| 0.33 | 27.0.1 | 0.30.13 | ✗（wgpu 27） |
| **0.34 / 0.35** | **29.0 / 29.0** | **0.30.13** | **✓ 完全对齐** |
| 0.36（最新 0.36.1） | 30.0 | 0.30.13 | ✗（wgpu 30，超前） |

- 同栈共享：egui-wgpu 直接用项目现有 `Device/Queue` 渲染 egui pass，**零第二个 GPU 上下文**；
- 集成点：`draw_and_present_*` 里 present 前插一段 egui render pass（约 40 行），fullscreen texture 复用同帧 SurfaceTexture——不改变非阻塞提交/fault/状态机语义（egui pass 在 submit 之前编码，同属一次提交）；
- 交互：egui-winit 消费 WindowEvent（F10 显隐 + 鼠标交互），窗口事件需要按 `egui_winit::on_window_event` 分流——handler.rs 增加一个前置分支（约 15 行）；
- 保留模式：仅在面板可见时跑 egui pass（不可见时零开销，不影响 benchmark 口径——C7 基准不含 egui pass）；
- 风险：①egui 0.34/0.35 非 LATEST，后续升级 egui 0.36 需同步 wgpu 30（一次性的全栈升级，可接受）；②沙箱无 X server 只能编译验证，真机交互需外部手测（与现有桌宠功能同等待遇）。

**工作量估计**：集成 ~1 天 + 面板 v1（四组表单 + toml 写回）~1 天。

### B. 内嵌本地 HTTP 设置页（tiny_http/axum + 静态 HTML）— 备选

- 服务端 ~300 行 + HTML 模板 ~200 行，浏览器访问 `127.0.0.1:<port>`；
- 优点：与渲染循环**零耦合**（frame.rs 一行不改，节点 C 代码零风险）；沙箱内可 curl 冒烟（无 X server 也能完整验证）；与 Py 版设置面板的 Web 心智一致；
- 缺点：桌宠场景下「开浏览器改配置」体验割裂；引入 HTTP 服务面（即使 loopback-only）增加安全审查面；`--settings-port` flag 生命周期管理（端口占用/关闭时序）；
- 适合：作为 A 路线的补充（远程/无头场景），不建议作为主形态。

### C. iced — 不推荐（本轮）

- iced 0.13/0.14 自带渲染器（wgpu 27/28 档），与 wgpu 29 同栈但**own 渲染循环**——与现有 winit 手写循环是两套事件模型，集成等于重写窗口层；除非整应用迁移 iced（否决：重写量远超收益）。

### D. Vizia / Xilem / Slint — 不推荐（本轮）

- 同样是 own-loop 或独立渲染栈；Slint 需 DSL 编译步骤；Xilem 尚未稳定（实验性）。引入成本均高于 egui overlay。

### E. 不做面板，仅 REPL/CLI 改配置 — 最小兜底

- `/config` 命令行式修改（REPL 已有骨架）；零新依赖零渲染侵入；
- 缺点：不满足「加个 UI」需求本身，作为 A 路线受阻时的 fallback 保留。

## 三、推荐与决策点

**推荐：A 路线（egui 0.35 overlay）**，理由：
1. wgpu 29/winit 0.30.13 逐版本证据对齐（上表），无双版本共存；
2. 与现有渲染循环叠加而非替代，节点 C 锁定语义零改动；
3. 原生体验符合桌宠形态，面板显隐零开销；
4. egui 纯 Rust + MIT/Apache 双许可，与仓库许可兼容（SOURCES.md 登记）。

**实施前置（拍板项）**：
1. egui 版本选 **0.35**（wgpu 29 对齐；0.34 同参数可选，取新不取旧）；
2. 设置写入策略：v1 保存后**提示重启生效**（热重载 v1 不做——supervisor 热应用 LLM/TTS client 重建是独立工作，单独批次）；
3. 面板入口：F10 呼出 + 托盘菜单项（可选，托盘已有骨架）。

**实施清单（拍板后执行）**：
1. `Cargo.toml`：egui/egui-winit/egui-wgpu 0.35（desktop crate）+ SOURCES.md 许可登记；
2. `app/settings_ui.rs`（新，≤500）：egui context 初始化 + pass 渲染 + 面板 UI（LLM/TTS/Persona/操作 四组）；
3. `app/frame.rs`：present 前插入 egui pass（≤40 行，语义零变化；P0-2 代码不触碰）；
4. `app/handler.rs`：F10 分支 + egui_winit::on_window_event 前置分流（≤15 行）；
5. `app/bootstrap.rs`：egui 初始化（renderer/context/state）；
6. `runtime::settings`：补 write 能力 + round-trip 测试；
7. 门禁：fmt/clippy/全测试 + 编译验证（真机交互手测列外部 RC——沙箱无 X server）。

---

## 复审检查单（供实现后复审用）

- [ ] egui pass 是否在 submit 之前编码（同帧一次提交，无额外 wait）？
- [ ] 面板不可见时是否完全跳过 egui pass（零开销路径）？
- [ ] egui_winit 事件分流是否在现有 winit 分支之前且不吞键盘/鼠标（面板隐藏时零拦截）？
- [ ] frame.rs 净增 ≤40 行且文件 ≤500？
- [ ] settings 写回是否走 toml round-trip 测试？api_key_env 语义是否保持（密钥不进文件）？
- [ ] SOURCES.md 是否登记 egui 全家许可？
