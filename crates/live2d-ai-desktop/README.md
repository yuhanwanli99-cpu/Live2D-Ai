# live2d-ai-desktop — PC 桌面宠应用（Rust 重建）

Linux 原生窗口/GPU 壳（**桌宠能力批**）：winit **0.30** + wgpu **29**
（与 `crates/l2d` / ayagami-render 的 wgpu 大版本一致，见 bakeoff 记录）。
透明无边框可调整大小窗口 + Bai 实时渲染（`--model-smoke`）、实际声卡输出链路
（`src/audio.rs`，cpal + 无锁 SPSC 环）、ksni 托盘、置顶/底部右侧定位/点击穿透/
交互态拖动；网络（LLM/TTS HTTP）尚未接入应用编排
（RFC `docs/plans/RUST-REWRITE-RFC.md` §4 批次 5/6；D10：应用本体原生渲染）。

## 运行

```bash
cargo run -p live2d-ai-desktop                          # 打印架构/会话线索/能力后退出（无显示 CI 安全）
cargo run -p live2d-ai-desktop -- --window-smoke        # 启动真实透明无边框窗口，持续重绘直到关闭
cargo run -p live2d-ai-desktop -- --smoke-frames N      # 渲染 N 帧自动退出（CI 冒烟；60s 超时兜底）
cargo run -p live2d-ai-desktop -- --model-smoke [<model3>]   # Bai 实时渲染冒烟（六动作固定顺序 + 合成口型）
cargo run -p live2d-ai-desktop -- --pet-mode [--window-smoke|--model-smoke]  # 桌宠模式：置顶+穿透+托盘恢复入口
cargo run -p live2d-ai-desktop -- --audio-smoke-secs S [--audio-smoke-silence]  # 音频输出冒烟（无设备退出码 3）
cargo run -p live2d-ai-desktop -- --smoke-timeout-secs S [--window-smoke]   # 显式超时兜底
```

日志走 `tracing`：`RUST_LOG=info|debug|trace|off`（默认 `info`）。

## 能力语义（重要）

三层口径，勿混淆（实现见 `src/platform.rs`，纯逻辑、零第三方依赖）：

| 层 | 类型 | 含义 |
| --- | --- | --- |
| 会话线索 | `LinuxSessionHint` | 从 `XDG_SESSION_TYPE`/`WAYLAND_DISPLAY`/`DISPLAY` 猜测的会话类型，非实测连接 |
| 声明性期望 | `DeclaredCapabilities` | hint 推导的期望表，**明确不是实际能力 gate** |
| 实际能力 | `RuntimeCapabilities` | 只记录真实初始化/API 调用结果 |

约束：

- `RuntimeCapabilities.transparent_alpha` 来自 wgpu surface 对 adapter 的实测
  alpha 模式协商（PreMultiplied/PostMultiplied → available；Inherit → unknown；
  Opaque → unavailable），不凭 X11 全 true 推断；
- tray / click_through / global_position / always_on_top 在真正初始化前一律
  `unknown`；只有 API **真实调用成功**才记 available——「有代码路径」永远不等于
  「运行时生效」（`request_feature=Ok` 仅代表代码路径存在）；
- pet-mode 安全规则：托盘未确认就绪（无 D-Bus / 初始化失败）时**禁用自动点击
  穿透**，窗口保持交互（防「穿透 + 无恢复入口」的不可恢复状态），见
  `src/user_event.rs::plan_launch`；
- Wayland 由合成器管理窗口布局：位置请求预期不采纳，以回读验证为准、按实测
  如实记录（unavailable 不是错误）。

## 架构

```
src/main.rs     CLI 入口：信息打印 / 冒烟调度 / 退出码契约
src/cli.rs      参数解析（纯逻辑 + 单测）
src/platform.rs LinuxSessionHint / DeclaredCapabilities / RuntimeCapabilities（纯逻辑 + 单测）
src/backend.rs  可扩展 DesktopBackend 接口、RunOptions/RunReport、后端工厂（+ 单测）
src/app.rs      ShellApp：winit ApplicationHandler + wgpu surface + 模型/桌宠状态机
src/user_event.rs 跨线程 UserEvent 与 pet-mode 启动决策（纯逻辑 + 单测）
src/tray.rs     ksni 托盘（菜单回调只经 EventLoopProxy 发事件）
src/model_smoke.rs 六动作确定性序列 + 合成口型驱动（+ 单测）
src/adapter.rs  core ParameterFrame → Bai 参数 ID 适配（BaiParamAdapter）
src/audio.rs    cpal 输出 + ringbuf SPSC + epoch 打断 + RMS 口型快照
src/repl.rs     终端 REPL 输入纯解析（接线批次启用）
```

- wgpu 29 取帧结果按 `CurrentSurfaceTexture` 分类处理：
  Success/Suboptimal 正常呈现（后者随后重新 configure）、Timeout/Occluded 跳帧、
  Outdated 重配置、Lost 重建 surface、Validation 记录跳过；
  每帧 `device.poll(PollType::Poll)` 泵送设备。
- 退出码契约：0 成功；1 代码错误；2 CLI 用法错误；3 **运行环境不满足**
  （无显示服务 / 缺系统库 / 无兼容 GPU / 无音频设备）。CI 据此区分
  “环境缺依赖”与“代码缺陷”。

## 冒烟验证记录（WSLg）

### 2026-08-26（能力批复跑）

- X11(XWayland) pet-mode 90 帧：backend=x11（RawWindowHandle 实测分类），
  presented=90 skipped=0；transparent_alpha=available(PreMultiplied)；
  always_on_top=set_window_level 生效（available）；**点击穿透：方向修复完成
  （业务布尔 → winit hittest 取反），set_cursor_hittest API 本身返回 Ok；但
  是否真让事件穿过窗口仍待人工手测验证后才能承诺**（见 C9-P0 / C11：原 P0
  布尔反转已修，仍需在真实 X11 WM 与 Wayland 合成器上各跑一轮 C10/C11 闭环，
  本记录不把 click_through 标 available；本环境无 D-Bus → 托盘降级可见、
  自动穿透正确禁用（保持交互，
  符合安全规则）；全局定位请求被合成器拒绝（回读 -32774,-32795 ≠ 目标）→
  如实记 unavailable；
- Wayland 会话 60 帧：backend=wayland，presented=60 skipped=0，透明 available；
  aot=unavailable（Wayland 无此 API，如实记录）；点击穿透方向修复完成、API 调用
  成功，但用户可见穿透效果仍待真实合成器手测（同 X11 口径，本记录不标
  click_through=available）；
- 模型冒烟 120 帧（X11）：moc V4_2_0 加载成功（475 artmesh / 549 deformer /
  128 参数）；六动作固定顺序执行（nod→shake_no→tilt→look_around→listen…）；
  mouth_channel writes=120 peak=0.871；llvmpipe 软渲染 frame_time avg≈116ms
  max≈144ms（阻塞式 GPU wait + 软件光栅的量化基线，非阻塞化改造前数据）；
- 音频冒烟：本沙箱无声卡（ALSA no card）→ 退出码 3，环境分类正确。

### 历史（窗口壳第一批）

- Wayland/X11 `--smoke-frames 20` 通过；`--smoke-timeout-secs 3` → timeout 兜底；
  无显示 `env -u DISPLAY -u WAYLAND_DISPLAY` → 退出码 3。

## 许可

**AGPL-3.0-only**。以仓库根 `LICENSE` 为准（完整文本见仓库根 `LICENSE`）；
本 crate 不提供独立 LICENSE 文本。商业使用条款见根 `LICENSE` 与 `COMMERCIAL-LICENSE.md`。
