# RUST-REWRITE-RFC — PC-only Rust 重建（v0 首批）

> 状态：**已确认（Confirmed）** — 项目所有者拍板，2026-08
> 范围：Linux 首发的 PC-only Rust 重建；旧 Android（Kotlin/Compose）与 PC（Python/open-llm-vtuber）项目**保留不动**
> 本文档是第一批实现的依据：仓库根新增 Cargo workspace（`crates/*` + `xtask`），不触碰 `Live2D-Ai-pc/`、`Live2D-Ai-Android/` 与 `shared/`
> 关联：`../README.md`（文档索引）、`../architecture/core-contracts.md`（契约）、`../verification/`（验收报告）

> **状态修订（2026-08-31，节点 D 复审后）**：本文档为**历史决策记录**，以下表述已
> 被后续批次超越，阅读时以当前代码为准：
> - 「旧 Android/Python 保留不动」→ 已归档：`Live2D-Ai-Android/` 归档到
>   `android-archive` 分支（见 `ANDROID_ARCHIVE_POINTER.md`），`Live2D-Ai-pc/`
>   已删除（git 历史保留，tag `py-legacy`）。
> - 「第一批不做 GPU/network/audio」→ 已全部落地：wgpu 原生渲染、OpenAI 兼容
>   LLM/TTS 网络层、cpal 音频输出、egui 设置面板、supervisor + WebSocket 事件流、
>   Web 配置面板（`--web`）均已实现。
> - 「desktop 只打印架构」→ `--chat` 终端对话 / `--web` Web 面板已接线。
> - 「runtime tokio 只在 dev dependency」→ 生产路径（supervisor 事件循环）已用
>   tokio current_thread runtime。
> 当前权威架构见 `docs/architecture/core-contracts.md`（已同步重写）。

---

## 0. 摘要

用 Rust 重写 PC 端桌宠（Live2D + AI + TTS 闭环），Linux 首发。第一批只建立：

- Cargo workspace 骨架（`crates/l2d`、`crates/live2d-ai-core`、`crates/live2d-ai-desktop`、`xtask`）；
- 纯 reducer 状态机骨架（Epoch/Turn/Sentence/AudioChunk + Phase + 事件/Effect）；
- 兼容报告与格式类型骨架（不绑定任何第三方 runtime）；
- Rust 占比统计工具（`xtask rust-ratio`，门槛默认 95%）；
- 来源纪律与许可边界（`SOURCES.md`、crate 级 LICENSE/README）。

所有决策见 §2，均为已确认项；第一批不实现渲染/音频/网络。

> **修订（2026-08-26，第二批增补）**：新增 `crates/live2d-ai-runtime`（已入根
> workspace members）——D6/D7 的网络层以「最小统一 API 层」形式提前落地，
> 详见 §3「runtime 统一 API 层」。渲染/音频/GUI 仍不做。

---

## 1. 背景

既有 PC 端为 Python（open-llm-vtuber + 自研前端），体积大、启动慢、依赖矩阵复杂；
Android 端为 Kotlin/Compose。项目所有者确认：**PC 端以 Rust 原生重建**，Linux 首发，
追求低门槛、单二进制、可观测的桌面宠闭环。旧端保留，不删除、不迁移（互不影响）。

---

## 2. 已确认决策（ADR）

编号 D1–D12，全部为项目所有者确认项；标注"首批命中"的决策在第一批即有落地动作。

### D1 — Linux 首发（首批命中）
- 平台顺序：Linux（X11/Wayland）→ 后续 Windows/macOS 视情况。
- 旧 WSL2 Python 环境不再作为新 PC 端运行时；Rust workspace 与旧项目并存。

### D2 — X11 完整桌宠；Wayland 标准能力检测 + 优雅降级（首批命中）
- **X11**：完整桌宠能力 —— 全局定位、置顶、点击穿透（input shape / shaped window）。
- **Wayland**：不保证全局定位/置顶/点击穿透（协议限制，无位置/穿透保证）。
  - 按标准能力检测（如 `XDG_SESSION_TYPE`、`WAYLAND_DISPLAY`、compositor 协商）分级暴露能力；
  - 能力不足时优雅降级：普通窗口 + 窗口内定位/操作，明确提示用户；
  - 后续 compositor 扩展（如 wlr-layer-shell / xdg 协议层）**仅作可选 feature**，默认不开启、不构成 v0 验收项。

### D3 — 第一方可执行源码 Rust ≥ 95%（首批命中）
- 统计口径：`rs / (rs + wgsl + py + ts + js + c + cc + cpp + h + hpp)`。
- 排除目录（按目录名，任意层级）：`Live2D-Ai-pc`、`Live2D-Ai-Android`、`dist`、`target`、`node_modules`、`.venv`；
  另跳过隐藏目录（`.` 开头：`.git`、`.uv-python`、`.hf-cache`、`.ruff_cache`、`.uv-cache` 等 VCS/托管环境/缓存，
  均非第一方源码）。口径以 `xtask rust-ratio` 实现为准。
- 工具：`cargo run -p xtask -- rust-ratio --threshold 95`（门槛参数默认 95，可改）。低于门槛退出码非 0。
- 目标：随着 Python 端退场、Rust 端增长，占比逐步收敛到 ≥ 95% 并在 RC 门槛强制校验。

### D4 — 模型 runtime：Bakeoff 后 fork/pin（首批命中：登记候选，不引入）
- **Mocari / Ayagami 对 Bai（Live2D 行为基准皮套）自动 Bakeoff** 后，选中方案 **fork/pin**（固定 commit）。
- Bakeoff 完成前：**不复制、不链接任何第三方 runtime 代码**；`crates/l2d` 只有安全的类型/兼容报告骨架。
- Runtime 引入动作与结论记录在 `SOURCES.md`（见 §5）。
- **修订（2026-08-26）**：Bakeoff 已完成，选定 **Ayagami**（pin rev `640ae4b1…`，
  git 依赖引入 `crates/l2d`，未复制源码）；Mocari 不引入、仅作参考。
  决策记录见 `docs/verification/rust-bakeoff-decision.md`。

### D5 — v0 只保证 Bai；v0 必须先支持实测档位 MOC raw 字节 4（Cubism 4.2）；raw v5/v6 为下一兼容方向；未来面向通用皮套（首批命中）
- v0 验收仅保证 **Bai/白** 皮套（其 `.moc3` 与材质资源；分发按既有法律口径，见 `docs/legal/pc-preview-publication.md`）。
- 格式支持优先级（**2026-08-26 修订**）：本仓库唯一实测样本 `bai.moc3` 的头为
  **MOC raw 版本字节 4 → Cubism 4.2**，因此 **v0 必须首先支持 raw 字节 4 档位**
  （`crates/l2d` 的版本表/兼容报告/运行时解析均以其为锚定档位）；
  **MOC raw v5 / v6 是下一批兼容方向**——先入版本表与探测白名单、逐档实测验证后放行，
  不得先于 raw 4 承诺。~~格式支持优先级：MOC raw v5 / v6 优先~~（原表述作废，
  与实测头相悖）。
- 未来面向通用皮套（社区模型），不在 v0 承诺内。

### D6 — LLM：OpenAI-compatible HTTP + SSE
- 仅统一 OpenAI-compatible `/chat/completions`（含 `stream=true` SSE）接口；
- 本地（Ollama/vLLM 等）与云端同构，无第二套协议。首批不实现网络层（无 async 依赖）。
- **落地（2026-08-26）**：网络层由 `crates/live2d-ai-runtime` 承担：
  `OpenAiClient::chat_stream` 固定 `stream=true`，独立纯 `SseDecoder`
  （支持任意 chunk 切割、多行 data、`[DONE]`、`delta.content` 与
  `tool_calls.function.name/arguments` 分块，绝不假设单个 chunk 是完整行）
  按 chunk 输出 `LlmEvent::TextDelta / ToolCallDelta{name,arguments} / Done`。

### D7 — TTS：仅统一 OpenAI-compatible HTTP API；本地/云端同构；不内置引擎（首批命中：登记约束）
- v0 **不内置任何 TTS 引擎**（不捆绑 sherpa-onnx/MeloTTS/Edge 等本地引擎），不直接调各家私有 SDK；
- 统一走 **OpenAI-compatible HTTP API**（如 `/v1/audio/speech` 或兼容实现），本地服务与云端服务同构可替换；
- 首批不实现网络层，仅把该约束写入契约与 RFC。
- **落地（2026-08-26）**：`crates/live2d-ai-runtime` 的 `OpenAiClient::synthesize_speech`
  统一 POST `<base_url>/audio/speech` 并**原样转发 chunked bytes**；
  不内置引擎、不解码音频，非 2xx 以带状态码的错误可观测。

### D8 — 口型同步：实际播放 PCM 驱动 RMS
- 口型/表情强度来源是**实际播放的 PCM 音频块**（decoded → RMS）驱动，而非"假想文本长度/静音窗"。
- 第一批不实现音频，但 reducer 已为 `AudioChunkId` + `AudioChunkReady/Finished` 事件留位（见 §3）。

### D9 — 动作：主 LLM tool action + 规则 fallback（首批命中：动作协议语义）
- 动作主路径：LLM 通过 **tool action** 输出表情/动作指令；
- fallback：LLM 不可用/未输出时，按**规则**（文本情绪关键词/标点/上下文）兜底，保证基础表演不中断；
- v0 动作集合（六动作 + release）：

  | 动作 | 说明 |
  | --- | --- |
  | `nod` | 点头 |
  | `shake_no` | 摇头（否定） |
  | `tilt` | 歪头 |
  | `look_around` | 环视/视线游移 |
  | `listen` | 倾听姿态 |
  | `surprise` | 惊讶 |
  | `release` | 释放/回到空闲 |

### D10 — renderer：提供 WASM demo；应用原生
- `crates/l2d`/渲染层未来交付**WASM demo**（浏览器可跑，便于预览与调试）；
- **应用本体为原生渲染**（不用 WebView/JS 渲染做正式路径）；
- 第一批**不实现渲染**：桌面二进制仅打印架构启动信息，明确"渲染未接入"。

### D11 — 许可：l2d 库 MIT OR Apache-2.0；应用 AGPL（首批命中）
- `crates/l2d`：**MIT OR Apache-2.0**（双许可，crate 内 `LICENSE-MIT` + `LICENSE-APACHE`，见 `crates/l2d/README.md`）；
- 应用及其余 crate（`live2d-ai-core`、`live2d-ai-desktop`、`xtask`）：**AGPL-3.0-only**，以仓库根 `LICENSE` 为准
  （crate 级 README 明示"根 LICENSE 生效"，见 `crates/live2d-ai-desktop/README.md`）。

### D12 — 验收：最终自动化 RC 后用户一次视觉验收
- 发布前先跑**自动化 RC**（构建 + 测试 + Rust 占比门槛 + 能力检测冒烟），全部通过后，
  由用户做**一次视觉验收**（桌宠外观/动作/口型/交互），验收通过才定稿；不循环反复打扰。

---

## 3. 第一批落地范围（与本次实现一致）

```
docs/plans/RUST-REWRITE-RFC.md     # 本文档（记录 D1–D12）
Cargo.toml                         # workspace 根（成员 crates/* + xtask；rustfmt/clippy 基础设置）
rustfmt.toml                       # rustfmt 基础设置（edition/max_width）
SOURCES.md                         # 来源纪律（候选仅登记，未复制代码）
crates/l2d/                        # 库：安全的类型/兼容报告骨架，无第三方 runtime（MIT OR Apache-2.0）
crates/live2d-ai-core/             # 库：纯 reducer 骨架 + 测试（AGPL-3.0-only）
crates/live2d-ai-desktop/          # 最小二进制：仅打印架构启动信息（AGPL-3.0-only）
crates/live2d-ai-runtime/          # 库：最小统一 API 层 LLM(SSE)+TTS(chunked bytes)（AGPL-3.0-only；2026-08-26 新增）
xtask/                             # 工具：rust-ratio 统计（AGPL-3.0-only）
```

第一批明确**不做**：异步/网络依赖、GPU/wgpu/winit、audio 依赖、真实渲染与播放、第三方 runtime。
`crates/live2d-ai-core` 的 reducer 事件/Effect 为语音/文本闭环预留语义位，但不接线。

### reducer 骨架（`crates/live2d-ai-core`）

- 领域类型：`Epoch`、`TurnId`、`SentenceId`、`AudioChunkId`（u64 newtype，类型安全）；
- 状态：`epoch`、`phase`（Idle/Thinking/Speaking）、`active_turn`（单主 turn）、`pending_audio`；
- 事件：`UserSubmitted`、`AudioChunkReady`、`AudioChunkFinished`、`TurnFinished`、`StopRequested`；
- Effect：`StartThinking`、`StartSpeaking`、`PlayAudio`、`StopPlayback`、`TurnAborted`、`GoIdle`、`Dropped{reason}`；
- 语义：stop 令 `epoch` +1（清空 turn/音频/回 Idle）；携带旧 turn/sentence/chunk id 的事件一律 `Dropped`；
  Idle 之外的新 `UserSubmitted` 被丢弃（单主 turn）。

### runtime 统一 API 层（`crates/live2d-ai-runtime`，2026-08-26 新增）

- **职责**：仅做 OpenAI-compatible 的最小统一客户端——LLM `/chat/completions`
  （固定 `stream=true`）与 TTS `/audio/speech`（chunked bytes 原样转发）。
  不内置 provider、不解码音频、不做 GUI；动作能力 gate / 优先级仲裁仍在
  `live2d-ai-core`，本 crate 只固定 `live2d_perform_action` 的 wire 类型
  （`action` 六动作、`strength` 1..=3、`reason` 可选）与请求 schema；
- **依赖克制**：库本体仅 `reqwest`（`default-features = false` + `rustls-tls/json/stream`，
  禁 default native-tls）、`serde`/`serde_json`、`futures-util`、`thiserror`、`url`；
  `tokio` 仅 dev-dependencies（测试用本地 tokio TCP mock，不引第三方 mock 大件）；
- **密钥安全**：自有 `ApiSecret`（不引 secrecy），`Debug`/`Display` 恒定脱敏、
  不实现 serde；key 只进 `Authorization: Bearer …` 头；
- **可观测**：HTTP 错误 / 非 2xx 状态码（附截断响应体）/ JSON 与 SSE 解析失败
  均为独立错误变体（`error::Error`）；
- **测试**：纯 `SseDecoder` 单测（逐字节切割、`\r\n` 跨 chunk、多行 data、BOM、
  finish 兜底）+ 本地 TCP mock 集成测试（SSE 半行切割与 tool arguments 跨事件
  分块拼接、`[DONE]` 与无 `[DONE]` 关流兜底、401/500 状态可观测、密钥不出现在
  请求体/Debug 输出且无 key 时 Authorization 头不存在、TTS 请求形状与分片转发）。

---

## 4. 里程碑（后续批次，不在第一批实现）

| 批次 | 内容（草案） |
| --- | --- |
| 2 | ~~l2d：MOC raw v5/v6 头解析 + 兼容报告落地；Bakeoff 结论进 `SOURCES.md`~~ **已按修订执行（2026-08-26）**：Bakeoff 结论进 `SOURCES.md` 并引入 Ayagami（D4 修订）；`crates/l2d` 落地 raw 字节 4 锚定档位的头解析 + 兼容报告 + 运行时解析/离屏渲染封装；MOC raw v5/v6 仅入版本表，为**下一兼容方向**（逐档实测后放行） |
| 3 | ~~core：LLM OpenAI-compatible SSE 客户端 + TTS HTTP 客户端（同构抽象）~~ **已由 `crates/live2d-ai-runtime` 提前落地（2026-08-26）**：独立于 core 的最小统一 API 层（reqwest/rustls），SSE 流式 LLM 与 TTS chunked bytes 均可观测；core 只在接线时消费其事件 |
| 4 | audio：PCM 解码 → RMS → 口型驱动 |
| 5 | renderer：原生渲染 + WASM demo。**部分推进（2026-08-25，窗口壳第一批）**：`live2d-ai-desktop` 落地 winit 0.30 + wgpu 29 透明/无边框/可缩放原生窗口壳（surface/adapter/device/queue、按 surface 实测 caps 协商 alpha 模式、透明清屏连续重绘、resize / ScaleFactorChanged / 取帧错误分类处理）；Live2D 模型上屏仍未做，属批次 5 剩余范围 |
| 6 | desktop：X11 完整桌宠 / Wayland 能力检测与降级。**接口与口径已就位（2026-08-25）**：会话线索改名 `LinuxSessionHint`、声明表 `DeclaredCapabilities` 明确非实际 gate；`RuntimeCapabilities` 只记录真实窗口/surface 初始化结果（tray/click_through/global_position/always_on_top 未实现前恒 unknown，`DesktopBackend::request_feature` 显式拒绝并运行时打印“不可用”）；X11 专项能力实现仍为后续阶段，禁止假装已实现 |
| 7 | 自动化 RC（构建/测试/占比门槛/能力冒烟）→ 用户一次视觉验收（D12） |

---

## 5. 来源纪律（首批）

- 任何第三方代码进入源码树前，先在 `SOURCES.md` 登记（来源、许可证、版本/commit、目的），再引入。
- 当前候选/ Oracle（**仅登记，未复制代码**）：Mocari、Ayagami（runtime 候选，待 Bakeoff）、
  PurismCore（MIT，渲染行为 Oracle 对照，未复制/未链接）。
- `crates/l2d` 第一批零依赖、无第三方 runtime。

---

## 6. 验收标准（第一批）

1. `cargo fmt --all` 无改动（或仅格式化自身新增）；`cargo test --workspace` 全绿。
2. `cargo run -p xtask -- rust-ratio --threshold 95` 可运行、输出口径可审计（见 D3）；
   当前基线为第一方 Python 脚本/测试（`shared/`、`tests/`、`scripts/`），Rust 占比低于门槛 → 退出码非 0 为**预期正确行为**。
3. `crates/live2d-ai-core` 测试覆盖：stop 增加 epoch、旧事件丢弃、单主 turn。
4. 未触碰 `shared/persona.yaml` 及其余 9 个既有工作区文件；未删除任何旧源码；未 git commit。

---

## 7. 风险与开放问题

| 项 | 说明 |
| --- | --- |
| 占比口径 | 隐藏目录跳过策略可能与"只排除 6 个目录"的直读不同；以"第一方源码"为准，见 D3。 |
| Wayland 桌面宠 | 置顶/穿透/全局定位无协议保证（D2），UX 上需显式降级提示。 |
| Buy 皮套分发 | v0 只保证 Bai；Bai 原始资源分发遵循既有法律口径（`docs/legal/pc-preview-publication.md`）。 |
| Runtime 选型 | Mocari/Ayagami 许可证与长期维护待 Bakeoff 确认；未决前不引入。 |
| Cargo.lock | 根 Cargo.lock 保留入库（可跟踪、可复现构建），不加入 .gitignore。 |

---

*决策记录：2026-08 项目所有者确认 D1–D12；本文档为第一批实现依据，后续决策增补以修订历史追加。*

## 修订历史

- **2026-08-25（窗口壳增补）**：`crates/live2d-ai-desktop` 引入 winit 0.30 +
  wgpu 29 + pollster + tracing/tracing-subscriber（与 l2d/ayagami wgpu 大版本一致，
  暂不引 egui），落地 Linux 原生透明无边框可缩放窗口壳：`ApplicationHandler`
  生命周期、surface 实测 alpha 协商（PreMultiplied/PostMultiplied/Inherit/Opaque
  → available/unknown/unavailable）、连续重绘、resize / ScaleFactorChanged /
  CurrentSurfaceTexture（Success/Suboptimal/Timeout/Occluded/Outdated/Lost/
  Validation）分类处理、每帧 `device.poll(Poll)`；环境检测降级改名
  `LinuxSessionHint`/`DeclaredCapabilities`（明确非实际能力 gate），新增
  `RuntimeCapabilities` 只记录真实初始化结果；`DesktopBackend` 可扩展接口 +
  `request_feature` 对未实现能力显式拒绝；CLI 默认无参数打印架构/能力即退出
  （无显示 CI 安全），`--window-smoke` 启动真实窗口，`--smoke-frames N`/
  `--smoke-timeout-secs S` 冒烟自动退出；退出码契约 0/1/2/3 区分代码错误与
  环境不满足。§4 批次 5/6 同步修订。
- **2026-08-26（第二批增补）**：新增 `crates/live2d-ai-runtime` 并加入根 workspace
  members——D6/D7 的网络层以「最小统一 API 层」提前落地：OpenAI-compatible LLM
  （`stream=true` SSE，纯 `SseDecoder` 支持任意 chunk 切割 / 多行 data /
  `[DONE]` / tool_calls 分块）+ TTS（`/audio/speech`，chunked bytes 原样转发）。
  密钥用自有 `ApiSecret`（Debug/Display 恒定脱敏）且只进 Authorization 头；
  TLS 固定 rustls、禁 native-tls；`live2d_perform_action` 仅定 wire 类型，
  能力/优先级仍归 core。§0/§2(D6,D7)/§3/§4(批次 3) 同步修订。
- **2026-08-26**：D4 增补 Bakeoff 结论（选 Ayagami，pin rev `640ae4b1…` 引入 `crates/l2d`；
  Mocari 仅参考）；D5 修订格式优先级——实测 `bai.moc3` 头为 MOC raw 字节 4（Cubism 4.2），
  v0 必须先支持 raw 4，raw v5/v6 为下一兼容方向（原文「MOC raw v5/v6 优先」作废）；
  §4 批次 2 同步修订。依据：`docs/verification/rust-bakeoff-{ayagami,mocari}.md`、
  `docs/verification/rust-bakeoff-decision.md`。