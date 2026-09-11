# SOURCES.md — 来源纪律与许可边界

> 用途：任何第三方代码进入本仓库源码树**之前**，必须先在本文件登记
> （来源、许可证、版本/commit、目的），经确认后再引入（RFC `docs/plans/RUST-REWRITE-RFC.md` §5 / D4）。
> 引入动作发生时，在「已引入」一节补记 fork/pin 的固定 commit 与引入位置；
> 未登记即引入视为违规。

---

## 当前状态：Ayagami 已 pin 引入；Mocari 仅登记未引入

**2026-08-26 更新**：Bakeoff 双报告（`docs/verification/rust-bakeoff-ayagami.md` /
`docs/verification/rust-bakeoff-mocari.md`）已完成，选型决策见
`docs/verification/rust-bakeoff-decision.md`：**Ayagami 胜出，作为 runtime/render 底座
以固定 rev 引入 `crates/l2d`**（git 依赖 pin，未复制/vendored 源码）；
Mocari 保持「仅登记、未引入」，作为未引入参考与动画补充候选。

### Runtime 候选（Bakeoff 结论已出，RFC D4）

| # | 名称 | 上游 | 许可证 | 语言 | 状态 | 登记目的 |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | **Mocari** | <https://github.com/Eatgrapes/Mocari> | MIT | Rust + WGSL（纯 Rust Cubism/Live2D runtime 实验） | **未引入**（参考/动画补充候选）；观察快照 `13c7ed3` @ main（2026-08-26 查询） | Bakeoff 落选方：发布过 crates.io 版本、无 unsafe、内建 motion/expression/pose；保留作渲染行为对照与未来动画能力参考 |
| 2 | **Ayagami** | <https://github.com/AyagamiDev/ayagami> | **MIT OR Apache-2.0**（双许可） | Rust + WGSL（兼容 Live2D 的 2D 皮套渲染器） | **已引入**（见下节「已引入」）；观察快照 `640ae4b` @ 默认分支（2026-08-26 查询） | Live2D runtime/render 底座：Bakeoff 胜者 |

- **Ayagami 许可证更正**：早前误记为 Apache-2.0 单许可。以上游仓库根实际文件为准：
  `COPYRIGHT`（自述 "dual-licensed under Apache 2.0 and MIT terms"）、`LICENSE-MIT`、
  `LICENSE-APACHE` 三份文件并存 → SPDX 表达为 **MIT OR Apache-2.0**，与本 crate
  （`crates/l2d`）的双许可一致，无边界冲突。
- 两候选许可证均与本项目 AGPL-3.0-only 应用侧兼容。
- Bakeoff 基准皮套为 **Bai（白）**（v0 只锚定 Bai 实测档位 moc3 raw 字节 4 → Cubism 4.2；
  注意 RFC D5 已于 2026-08-26 修订格式优先级，见该文档修订记录）。

### 渲染行为 Oracle（对照基准，不引入）

| 名称 | 来源 | 许可证 | 说明 |
| --- | --- | --- | --- |
| **PurismCore** | 本仓库既有资产：`Live2D-Ai-Android` 内 vendored 的 C99 原生层（`cpp/purism/`，25 个 `.c/.h`，全部带 `SPDX-License-Identifier: MIT`，见 `docs/research/license-report.md` §1） | MIT | 作为 Rust 渲染实现的**行为 Oracle 对照**：同一输入下核对顶点/裁剪/动画输出是否一致 |

### UI 候选（设置面板；2026-08-28 选型已定）

| 名称 | 上游 | 许可证 | 语言 | 状态 | 登记目的 |
| --- | --- | --- | --- | --- | --- |
| **egui** | <https://github.com/emilk/egui>（crates.io `egui` 0.35） | **MIT OR Apache-2.0**（双许可；上游 `LICENSE-MIT` / `LICENSE-APACHE`） | 纯 Rust 即时模式 GUI | **已引入**（见下节「已引入」）；版本 0.35（2026-08-28 查询 crates.io） | 桌面端设置面板 UI 框架：调研报告 `docs/plans/ui-selection-report-2026-08-28.md` 选型 A 路线，与 wgpu 29 / winit 0.30 大版本完全对齐 |
| **egui-winit** | <https://github.com/emilk/egui>（crates.io `egui-winit` 0.35） | **MIT OR Apache-2.0**（同 crate 仓库） | 纯 Rust winit 0.30 桥 | **已引入**（与 egui 同版本） | 面板 winit 事件桥（`State::new` / `on_window_event` / `take_egui_input`） |
| **egui-wgpu** | <https://github.com/emilk/egui>（crates.io `egui-wgpu` 0.35） | **MIT OR Apache-2.0**（同 crate 仓库） | 纯 Rust wgpu 29 渲染后端 | **已引入**（与 egui 同版本） | 面板 GPU 渲染（`Renderer::new` / `update_buffers` / `render`） |

- PurismCore 属于旧 Android 项目保留资产（RFC 范围外，不动）；Rust workspace
  （`crates/l2d` 等）**未复制、未编译链接其任何代码**，仅以其运行行为为正确性参照。
- 对照结论（后续批次）记录于此处及 `docs/verification/`。

---

## 已引入

| 名称 | 引入方式与位置 | 固定 rev | 许可证 | 引入日期 | 决策依据 |
| --- | --- | --- | --- | --- | --- |
| **Ayagami**（`ayagami` + `ayagami-render`） | `crates/l2d/Cargo.toml` git 依赖（未复制/vendored 源码；crate 对外仅暴露自有封装 API，第三方类型不越过 crate 边界） | [`640ae4b10bad8def1adcacdada4f8241b484c169`](https://github.com/AyagamiDev/ayagami)（**禁止浮动分支**；升级 rev 须重新验证并同步本文件） | MIT OR Apache-2.0（上游根 `COPYRIGHT`/`LICENSE-MIT`/`LICENSE-APACHE`） | 2026-08-26 | RFC D4 Bakeoff：`docs/verification/rust-bakeoff-decision.md`；验证报告 `docs/verification/rust-bakeoff-ayagami.md` |
| **egui** + **egui-winit** + **egui-wgpu**（0.35 全家） | `crates/live2d-ai-desktop/Cargo.toml` crates.io 依赖（`egui = "0.35"` / `egui-winit = "0.35"` / `egui-wgpu = "0.35"`；通过 `Cargo.lock` 锁版本，未复制源码） | crates.io `0.35.x` 系列（升级须重跑 `cargo check -p live2d-ai-desktop --all-targets` 并同步本文件） | MIT OR Apache-2.0（上游 `LICENSE-MIT` / `LICENSE-APACHE`，与 crates/l2d 双许可一致） | 2026-08-28 | 选型报告 `docs/plans/ui-selection-report-2026-08-28.md` §二 A 路线：与 wgpu 29 / winit 0.30 大版本完全对齐；同栈共享 device/queue，零第二 GPU 上下文 |
| **tiny_http**（D2 HTTP 接线，2026-08-28） | `crates/live2d-ai-desktop/Cargo.toml` crates.io 依赖（`tiny_http = "0.12"`；通过 `Cargo.lock` 锁版本） | crates.io `0.12.x` 系列（升级须重跑门禁并同步本文件） | **MIT OR Apache-2.0**（上游 `LICENSE-MIT` / `LICENSE-APACHE` 双许可；与 crates/l2d / egui 同款许可边界） | 2026-08-28 | 节点 D2 选型：loopback 控制平面 HTTP 服务的最小依赖面（无 TLS / 零 async 运行时 / 阻塞 handler 适合 supervisor 之外的控制命令路径）；选型理由记录在 `crates/live2d-ai-desktop/Cargo.toml` 的 `tiny_http` 注释块 |
| **reqwest** + **url** + **futures-util**（D2 settings/test/{llm,tts} 端点） | `crates/live2d-ai-desktop/Cargo.toml` crates.io 依赖（reqwest 0.12 已带 rustls-tls / json / stream features；`url = "2"`；`futures-util = "0.3"` 仅 std feature） | crates.io 现行（与 `live2d-ai-runtime` 同步锁定） | **MIT OR Apache-2.0**（reqwest 0.12 / url 2 / futures-util 0.3 上游双许可） | 2026-08-28 | 节点 D2 test 端点直发最小 LLM/TTS 请求：runtime 的 `OpenAiClient::new` 不暴露 from_parts（私有字段），且默认构造的 `reqwest::Client` 无 timeout——test 端点必须自构带 timeout 的 client；D2 与 runtime crate 共享同一 reqwest / url 版本，避免传递冲突 |
| **tracing-appender**（W3 日志落盘） | `crates/live2d-ai-desktop/Cargo.toml` crates.io 依赖（`tracing-appender = "0.2"`） | crates.io `0.2.x` 系列（升级须重跑门禁并同步本文件） | **MIT**（上游 `LICENSE` 单许可；tokio-rs 官方配套 crate） | 2026-08-29 | W3 任务：CLI 启动附带（任何模式 `--chat`/`--web`/`--pet-mode` 都落盘）——`tracing_appender::rolling::daily` 自动按天滚动（`live2d-ai.log.YYYY-MM-DD` 后缀）；非阻塞 `non_blocking` 包装 + `SanitizingWriter`（`crates/live2d-ai-desktop/src/logging.rs`）兜底脱敏 P0-1 密钥明文。许可证与本 crate AGPL-3.0-only 边界无冲突 |
| **tungstenite**（W2 WebSocket 桥，2026-08-29） | `crates/live2d-ai-desktop/Cargo.toml` crates.io 依赖（`tungstenite = "0.24"`，default features = handshake 子集） | crates.io `0.24.x` 系列（升级须重跑门禁并同步本文件） | **MIT OR Apache-2.0**（上游 `LICENSE-MIT`/`LICENSE-APACHE` 双许可，与 tiny_http / reqwest 等同款许可边界） | 2026-08-29 | W2 任务：实现 `/ws/runtime` + `/ws/state` 升级 + JSON 文本帧广播——`tungstenite::accept` 与 `tiny_http::Request::upgrade` 返回的 `Box<dyn ReadWrite + Send>` 直接拼装，**避免**引入 axum/hyper/tower 整条栈（与 D2 选型一致：依赖最小化）。`default-features = true` 仅启用 `handshake` 子集（不需要 TLS/async）。引入位置：`crates/live2d-ai-desktop/src/web_api/ws.rs`（生产 Server 角色）；测试客户端走 `tungstenite::handshake::{derive_accept_key,client::generate_key}` + std TcpStream 手工走 RFC 6455 握手。**2026-08-31 简化（M0-7）**：WS 为单向事件流，每连接**单线程** actor（`cmd_rx.recv_timeout` 周期收广播帧 + 10s 服务端 heartbeat），**不**调 `ws.read()`（tungstenite 0.24 阻塞 read 在 EOF 时 busy-wait）；写失败即退出并 unsubscribe，无 30s 客户端 idle 盲关 |

- Mocari 未引入（无 crates/l2d 依赖、无源码复制），仅作参考。
- egui 0.35 全家（egui / egui-winit / egui-wgpu）通过 crates.io 引入，与 crates/l2d
  共享 MIT OR Apache-2.0 双许可——与本 crate（`crates/live2d-ai-desktop`，AGPL-3.0-only）
  许可边界无冲突。版本 0.35 与本项目 wgpu 29 / winit 0.30 大版本对齐（选型报告第 26-31
  行证据链）。

---

## 纪律要点

1. **先登记、后引入**：来源、许可证、版本/commit、目的一并写明；
2. 引入一律 **fork/pin 固定 commit**，禁止跟踪浮动分支；
3. 引入位置只允许出现在对应 crate 目录内并在 crate README/LICENSE 注明出处；
4. 与 AGPL-3.0-only（应用侧）/ MIT OR Apache-2.0（`crates/l2d`，RFC D11）许可边界冲突的
   来源不得引入；
5. 每次引入或 Bakeoff 决策后更新本文件，并在 `CHANGELOG.md` 记一笔。

> **登记范围（2026-08-31 修订）**：人工登记只覆盖**关键架构依赖**——git 依赖
> （pin rev）、非标准许可、复制/vendored 源码、以及架构决策锚定的 crates.io 依赖
> （如 egui 全家、tiny_http、tungstenite、Ayagami）。普通 crates.io 传递依赖
> （wgpu / winit / cpal / ringbuf / tokio / serde_with / directories 等）交给
> `Cargo.lock` + 许可审计工具（cargo-deny / cargo-about）自动管理，不再逐条手工登记。
