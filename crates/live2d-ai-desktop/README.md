# live2d-ai-desktop — PC 桌面宠应用（Rust 重建）

Linux 原生窗口/GPU 壳（**窗口壳第一批**）：winit **0.30** + wgpu **29**
（与 `crates/l2d` / ayagami-render 的 wgpu 大版本一致，见 bakeoff 记录）。
透明、无边框、可调整大小窗口 + surface 清屏连续重绘；
Live2D 模型渲染、音频、网络仍未接入（RFC `docs/plans/RUST-REWRITE-RFC.md`
§4 批次 5/6；D10：应用本体原生渲染）。

## 运行

```bash
cargo run -p live2d-ai-desktop                          # 打印架构/会话线索/能力后退出（无显示 CI 安全）
cargo run -p live2d-ai-desktop -- --window-smoke        # 启动真实透明无边框窗口，持续重绘直到关闭
cargo run -p live2d-ai-desktop -- --smoke-frames N      # 渲染 N 帧自动退出（CI 冒烟；60s 超时兜底）
cargo run -p live2d-ai-desktop -- --smoke-timeout-secs S [--window-smoke]   # 显式超时兜底
```

日志走 `tracing`：`RUST_LOG=info|debug|trace|off`（默认 `info`）。

## 能力语义（重要）

三层口径，勿混淆（实现见 `src/platform.rs`，纯逻辑、零第三方依赖）：

| 层 | 类型 | 含义 |
| --- | --- | --- |
| 会话线索 | `LinuxSessionHint` | 从 `XDG_SESSION_TYPE`/`WAYLAND_DISPLAY`/`DISPLAY` 猜测的会话类型，非实测连接 |
| 声明性期望 | `DeclaredCapabilities` | hint 推导的期望表，**明确不是实际能力 gate** |
| 实际能力 | `RuntimeCapabilities` | 只记录真实窗口/surface 初始化结果 |

约束：

- `RuntimeCapabilities.transparent_alpha` 来自 wgpu surface 对 adapter 的实测
  alpha 模式协商（PreMultiplied/PostMultiplied → available；Inherit → unknown；
  Opaque → unavailable），不凭 X11 全 true 推断；
- tray / click_through / global_position / always_on_top 在真正初始化前一律
  `unknown`——本批没有任何初始化路径，CLI 会逐项打印“不可用”作为运行时证据；
- X11 完整桌宠能力（置顶/全局定位/点击穿透）属**后续阶段**（RFC §4 批次 6）：
  `backend::DesktopBackend::request_feature` 默认全部拒绝，禁止假装已实现。

## 架构

```
src/main.rs     CLI 入口：信息打印 / 冒烟调度 / 退出码契约
src/cli.rs      参数解析（纯逻辑 + 单测）
src/platform.rs LinuxSessionHint / DeclaredCapabilities / RuntimeCapabilities（纯逻辑 + 单测）
src/backend.rs  可扩展 DesktopBackend 接口、RunOptions/RunReport、后端工厂（+ 单测）
src/app.rs      ShellApp：winit ApplicationHandler + wgpu surface 壳（环境边界层）
```

- wgpu 29 取帧结果按 `CurrentSurfaceTexture` 分类处理：
  Success/Suboptimal 正常呈现（后者随后重新 configure）、Timeout/Occluded 跳帧、
  Outdated 重配置、Lost 重建 surface、Validation 记录跳过；
  每帧 `device.poll(PollType::Poll)` 泵送设备。
- 退出码契约：0 成功；1 代码错误；2 CLI 用法错误；3 **运行环境不满足**
  （无显示服务 / 缺 `libxkbcommon-x11` 等系统库 / 无兼容 GPU）。CI 据此区分
  “环境缺依赖”与“代码缺陷”。

## 冒烟验证记录（2026-08，WSLg）

- Wayland 路径：`--smoke-frames 20` → Vulkan(llvmpipe) + Bgra8Unorm +
  PreMultiplied，presented=20 skipped=0，自动退出；
- X11(XWayland) 路径：同上通过；
- 超时兜底：`--smoke-timeout-secs 3` → timeout-elapsed(3s)，presented=149；
- 无显示：`env -u DISPLAY -u WAYLAND_DISPLAY … --window-smoke` → 退出码 3，
  stderr 给出环境原因；默认模式照常打印并退出 0。

## 许可

**AGPL-3.0-only**。以仓库根 `LICENSE` 为准（完整文本见仓库根 `LICENSE`）；
本 crate 不提供独立 LICENSE 文本。商业使用条款见根 `LICENSE` 与 `COMMERCIAL-LICENSE.md`。
