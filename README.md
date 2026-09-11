# Live2D-Ai 🐱

> **v0.1.0-rc.1（核心链路基线）**：**通用人形皮套 AI 接入一体化平台**——AI × Live2D 人形皮套的
> 通用接入与应用层。**不绑定任何单一模型**（模型由用户合法导入，`assets/models/` 不捆绑二进制），
> **不做复杂上层**（实现保持最小）：核心为**最小最简核心链路**
> （LLM 纯对话 → TTS → 驱动口型 → Live2D 皮套渲染 + 前端 UI），
> 增强能力通过 **Mod 边界**隔离（失败不影响主链路）。
> 平台：Linux / WSL2（PC 主力）。Android、原生 Windows 不属本次范围。
>
> **版本线说明**：本版由 `0.5.1` **重置**为 `0.1.0-rc.1`。基线不是 0.5.x 线上的增量，
> 而是重新起算的第一个对外候选版——**LLM 工具层与动作系统已从前后端整体拆除**
> （LLM 不暴露任何工具，只做对话），故不再沿用旧线号。见
> [`docs/releases/v0.1.0-rc.1.md`](./docs/releases/v0.1.0-rc.1.md)。

<p align="center">
  <img src="https://img.shields.io/badge/license-AGPL--3.0--only-blueviolet" alt="License AGPL-3.0-only"/>
  <img src="https://img.shields.io/badge/language-Rust-blueviolet" alt="Language Rust"/>
  <img src="https://img.shields.io/badge/Live2D-Ayagami%20%2B%20wgpu-orange" alt="Live2D Ayagami + wgpu"/>
  <img src="https://img.shields.io/badge/LLM-OpenAI%20Compatible-blue" alt="LLM OpenAI Compatible"/>
  <img src="https://img.shields.io/badge/UI-Web%20Frontend-brightgreen" alt="UI Web Frontend"/>
  <img src="https://img.shields.io/badge/Mod-trait%20Registry-lightgrey" alt="Mod trait Registry"/>
</p>

---

## 🎯 项目哲学 | Philosophy

> **不做功能竞赛，只打磨一个最小最优秀的核心。**

Live2D-Ai = Live2D + AI + TTS。全部意义只有一个：把「文本 → LLM → TTS → 驱动口型 →
Live2D 皮套渲染」这条链路打磨到极致——架构干净、代码精简、每行有测试证据，**并作为
一个通用接入容器**：模型由用户合法导入，**不绑定任何单一模型/形象**；核心保持最小，
复杂能力留给社区通过 **Mod 边界**接入。

**LLM 只做对话，不暴露任何工具**（2026-09-11 用户裁决）：工具/动作层已从前后端整体抹去，
不保留字段、不做「待接线」的空壳。皮套的呼吸/眨眼/微表情来自**待机生命体征**，
属于渲染侧机制，与对话无关。

```
┌──────────────────────────────────────────┐
│            Mod 扩展层（边界隔离）          │
│  桌宠透传 / 外部事件接入 / 导演系统         │  ← 独立 crate，失败不影响主链路
├──────────────────────────────────────────┤
│              最小最优秀核心                │
│  文本 → LLM（纯对话）→ TTS → 口型 → 皮套渲染 │  ← 只打磨这一条链路
└──────────────────────────────────────────┘
```

- **主仓库**：只留核心链路（TTS + LLM + Live2D + Web UI），保持 **95-99% Rust**。
- **Mod**：通过 `live2d-ai-mod-system` 的 trait 注册中心接入（`apply/factory` 心智，
  对齐 pi / DSH），**静态编译组合**，enable/disable 运行时开关，失败仅 disable 该 Mod。
- **不做功能竞赛**：不比 N.E.K.O. 生态、不比 Neuro-sama 包装。

---

## 📖 项目简介 | Overview

**Live2D-Ai** 是一个 **通用人形皮套 AI 接入平台**——用 **Live2D 人形皮套角色**进行**语音对话**，
通过 **LLM** 理解并回应，同时根据对话实时切换表情、驱动口型。核心只留最小闭环，
增强能力全部走 Mod 边界；**模型不绑定**，用户合法导入任意人形皮套接入。

> "欢迎回来～今天也要一起加油哦！" —— 白 (Bai)

**Live2D 渲染**：基于 [Ayagami](https://github.com/AyagamiDev/ayagami)（MIT OR
Apache-2.0，pin rev）+ wgpu 29，不依赖 Live2D 专有 Cubism 闭源 SDK。模型需用户合法取得。

### 🗄️ Python / Android 退役说明

作为通用人形皮套一体化平台的**唯一实现基线**，Rust 主线（`crates/`）取代了旧的
Python/Android 双端栈。`Live2D-Ai-pc/`（Python）与 `Live2D-Ai-Android/`（Kotlin）已从主线移除：
- Python 版保留在 git 历史（tag `py-legacy`）
- Android 版归档到远端 `android-archive` 分支（见 `ANDROID_ARCHIVE_POINTER.md`）

Rust 重构（`crates/`）是**唯一实现主线**；模型与形象的接入由用户合法导入承载（非平台实现内置）。

---

## ✨ 特性 | Features（Rust 主线现状）

### 核心链路（文本 → LLM → TTS → 口型 → 皮套渲染）
- **LLM 纯对话**：OpenAI 兼容 `/chat/completions`（SSE 流式）。**请求体不含
  `tools` / `tool_choice` / `functions`**（有回归测试断言这三个键不存在），不绑定厂商 SDK，
  不做工具调用循环
- **TTS 是核心链路、不是 Mod**：`/audio/speech`，端点唯一权威来源是 `live2d-ai.toml`
  的 `[tts]` 段（见 [`docs/architecture/tts-is-core.md`](./docs/architecture/tts-is-core.md)）
- **一句一单元**：分句只按真实句读边界切分，不按字符位置硬切；句中不切分、不重叠。
  句子首/尾由引擎 `AudioChunk` 的 `first_chunk` / `final_chunk` 给出，
  **不在发射侧用记账推断**（旧实现实测产出「0 个 start、13 个 end」）
- **纯 reducer 状态机**（`live2d-ai-core`）：epoch 闸门 + 双闩锁，无 IO、无 async
- **supervisor 事件循环**：独立线程 + current_thread tokio，五阶段 turn

### Web 前端（主仓库 UI）
- **65/35 布局**：左 Live2D 舞台 65% + 右对话区 35%
- 设置面板 **8 个分区**：角色卡 / 模型库 / LLM / 语音合成 / 外观与互动 / Mod / 诊断 / 开发模式
- 对话区（酒馆风格消息流）；**单入口**：`GET /` 与 `/index.html` 302 → `/app/`
  （旧的原生 JS 前端已于 2026-09-11 删除——同一个服务两套界面是明确的坑）
- **音频走 `<audio>` 媒体元素 + Blob(WAV)**，**不用 Web Audio**：后端经 WS 下发 PCM
  （按句分片，带 `start`/`end`/`sentence_seq`），前端按句封 WAV 交给媒体元素播放，
  口型用 `audio.currentTime` 查服务端 `volume` 包络。站点级静音作用在媒体元素上，
  逐片 `AudioContext` 播放拦不住
- **离线优先**：构建必须带 `--no-web-resources-cdn`（否则 CanvasKit 指向 Google CDN，
  断网白屏）；中文字体自托管子集（缺字会去 `fonts.gstatic.com` 取，断网即豆腐块），
  两条都有门禁
- WASM 渲染入口：`/render` 页面 + `/models/*` 模型资产（`l2d-wasm-demo` / `wasm_assets.rs`）

### Mod 系统（live2d-ai-mod-system）
- **trait 注册中心**：`ModFactory` / `ModRuntime` + `ModRegistrar`
- **实态**：4 个 Mod 已编译进二进制，`/api/v1/mods` 支持 enable/disable/restart；
  **TTS 不是 Mod**（核心链路，端点由 `live2d-ai.toml` 的 `[tts]` 段决定，见 `docs/architecture/tts-is-core.md`）
  设置抽屉第 8 组为 Mod 管理，运行时开关 + 失败隔离
- 静态编译组合 + enable/disable + restart + reload_config + 状态机（Disabled/Starting/Running/Failed/Stopping）
- 事件有界 channel + 独立 worker（Mod 失败不影响主链路）
- Settings 纯数据 schema（前端统一渲染，禁 Mod 注入 HTML/JS）

---

## 🚀 快速开始 | Quick Start

### 环境要求
| 工具 | 版本 | 用途 |
|---|---|---|
| Rust | 1.92（MSRV） | workspace 编译 |
| wasm32 目标 | `wasm32-unknown-unknown` | Web 渲染（`rustup target add wasm32-unknown-unknown`）|
| trunk | 最新 | WASM 构建（`cargo install trunk`）|
| WSLg / X11 | — | 原生桌宠窗口显示 |

### 构建 + 启动 Web 端（主入口）
```bash
# 配置 LLM/TTS（OpenAI 兼容；本地无鉴权可删 api_key_env）
cp live2d-ai.toml.example live2d-ai.toml

cargo build --release
./target/release/live2d-ai-desktop --web --http-port 18080
# 浏览器访问 http://localhost:18080/
```

### 启动原生桌宠（Mod: live2d-ai-mod-pet-desktop）
```bash
./target/release/live2d-ai-desktop --web --http-port 18080  # 优先：Web UI（主入口）
```
> **状态**：pet-desktop 为 v1 骨架。窗口生命周期 / 悬浮窗 / 点击穿透 / 托盘菜单接线后置，
> 当前 `--pet-mode` 为演示占位，主入口请使用 `--web`。`--chat` / `--pet-mode` 等 CLI
> 模式为历史产物，待 v1 窗口生命周期接线后逐步恢复。

### CLI 模式一览
```bash
--web [--http-port P]         Web 面板（主入口）
--chat [--config <P>]         终端对话闭环（LLM→TTS→声卡→Live2D）
--pet-mode                    桌宠模式（置顶+穿透+托盘）
--model-smoke [P]             Live2D 渲染冒烟
--window-smoke                纯透明窗口壳
--audio-smoke                 音频冒烟
--benchmark [P]               渲染 benchmark
```

### 运行测试 | Run Tests
```bash
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
# 根级健康检查（可选；纯本地无 Key 断言）
python3 -m pytest tests/ -q
```

---

## 🏗️ 架构 | Architecture

### Crate 分层（主仓库）
```
crates/
  l2d                 Live2D 渲染封装（Ayagami + wgpu 29）
  live2d-ai-core      纯 reducer 状态机（无 IO、无 async）
  live2d-ai-runtime   LLM/TTS OpenAI 兼容 + settings + conversation + dialogue
  live2d-ai-desktop   supervisor 事件循环 + Web API + 窗口 + 音频 + 托盘
  l2d-wasm-demo       Web 渲染核心（被 Web 前端复用）
  live2d-ai-mod-system  Mod trait/factory/registry/services/status/topics
  live2d-ai-mod-external-input  外部事件接入 Mod（`POST /api/v1/external/chat`）
  live2d-ai-mod-director      动作序列 Mod（config 驱动白名单；LLM 工具层拆除后**无自动驱动**）
  live2d-ai-mod-pet-desktop   桌宠窗口 Mod v1 骨架（窗口生命周期接线后置）
  live2d-ai-mod-local-llm     本地推理 Mod
xtask                 工程工具（Rust 占比统计）
```

### 核心链路
```
文本
  → LLM（OpenAI /chat/completions, SSE 流式；纯对话，请求体无 tools）
  → TTS（OpenAI /audio/speech；一句一单元，句界由引擎给出）
  → 口型（前端按句封 WAV，经 <audio> 播放，用 currentTime 查 volume 包络驱动）
  → Live2D 皮套渲染（wgpu 原生 + Web wasm；呼吸/眨眼/微表情来自待机生命体征）
  + 前端 UI（Flutter Web，同源 /app/）
```

### Mod 边界
```
主仓库核心链路 ←── live2d-ai-mod-system trait 接口 ──→ 4 个 Mod（workspace crate，已编译进二进制）
                     │                                      │
                     │ ModServices: action_tx / say_tx /     │
                     │              event_tx / logger        │
                     │ register_settings(schema)             │
                     │ enable/disable/restart/reload_config  │
                     │ 事件有界 channel + 独立 worker         │
```

> `action_tx` 与 `say_tx` 仍在 Mod API 契约里（Mod 侧不动），但 **LLM 工具层已拆除**，
> 没有任何自动驱动方会去触发动作序列——它是**休眠**的，不是「待接线的空壳」。

---

## 🔌 Mod 接入点（面向 Mod 开发者）

实现 `ModFactory` + `ModRuntime`，通过 `ModRegistrar` 注册：
```rust
pub trait ModFactory: Send + Sync {
    fn descriptor(&self) -> &'static ModDescriptor;
    fn create(&self, services: ModServices, config: serde_json::Value)
        -> Result<Box<dyn ModRuntime>, ModError>;
}
pub trait ModRuntime: Send {
    fn start(&mut self, registrar: &mut ModRegistrar<'_>) -> Result<(), ModError>;
    fn shutdown(&mut self) -> Result<(), ModError> { Ok(()) }
}
```

Mod 能力（v1）：
- `register_settings(schema)` → 注入设置子面板（前端统一渲染，**禁** Mod 注入 HTML/JS）
- `subscribe(topic)` → 订阅 `turn_started` / `text_delta` / `action_finished` /
  `voice_started` / `voice_ended` / `model_activated`（有界 channel + 独立 worker，
  慢 Mod 不卡 supervisor，失败仅 disable 该 Mod）
- `ModServices.action_tx` / `say_tx` → 提交动作与发言（固定优先级映射进 core 仲裁）

**注意**：Mod 是静态编译模块，新增 Mod 需重新构建；enable/disable 是运行时开关。

---

## 📚 文档 | Documentation

| 文档 | 说明 |
|---|---|
| [docs/README.md](./docs/README.md) | 文档索引 |
| [docs/architecture/directory.md](./docs/architecture/directory.md) | 目录约定（Rust 结构）|
| [docs/architecture/core-contracts.md](./docs/architecture/core-contracts.md) | 核心契约（Rust 架构）|
| [docs/plans/node-e-mod-system-plan.md](./docs/plans/node-e-mod-system-plan.md) | 节点 E Mod 系统计划 |
| [docs/plans/node-e-mod-adr-e0.md](./docs/plans/node-e-mod-adr-e0.md) | 节点 E 边界裁决 ADR |
| [docs/plans/RUST-REWRITE-RFC.md](./docs/plans/RUST-REWRITE-RFC.md) | Rust 重构 RFC |
| [ANDROID_ARCHIVE_POINTER.md](./ANDROID_ARCHIVE_POINTER.md) | Android 归档指针 |

---

## 📄 许可证 | License

**AGPL-3.0-only** © 2026 Sakura Motion Project

自有代码以 [GNU Affero General Public License v3.0](LICENSE)（AGPL-3.0-only）发布。
第三方组件遵循其原有许可（见 [NOTICE](NOTICE) / [CREDITS.md](CREDITS.md)）。

**.moc3 模型**：不适用本仓库代码的 AGPL，用户需从模型作者渠道合法取得。

---

<p align="center">
  Made with ❤️
  <br/>
  <sub>
    "欢迎回来～今天也要一起加油哦！" —— 白 (Bai)
  </sub>
</p>
