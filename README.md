# Live2D-Ai 🐱

> **v0.3.0（Rust 完全重构）**：Live2D + AI + TTS 桌面桌宠，主仓库为**最小最简核心链路**
> （TTS + LLM + Live2D + Web UI），增强能力通过 **Mod 边界**隔离（失败不影响主链路）。
> 平台：Linux / WSL2（PC 主力）。Android、原生 Windows 不属本次范围。
> 模型不捆绑分发，用户需从作者渠道合法取得并自行导入（`assets/models/` 仅保留目录与说明）。

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

Live2D-Ai = Live2D + AI + TTS。全部意义只有一个：把「文本/语音 → LLM → TTS → Live2D
表情/口型/动作」这条链路打磨到极致——架构干净、代码精简、每行有测试证据，**并留下
扩展接口供后续和社区开发 Mod**。

```
┌──────────────────────────────────────┐
│            Mod 扩展层（边界隔离）       │
│  桌宠透传 / 外部事件接入 / 导演系统      │  ← 子仓库/独立 crate，失败不影响主链路
├──────────────────────────────────────┤
│            最小最优秀核心              │
│  文本/语音 → LLM → TTS → 表情/口型/动作 │  ← 只打磨这一条链路
└──────────────────────────────────────┘
```

- **主仓库**：只留核心链路（TTS + LLM + Live2D + Web UI），保持 **95-99% Rust**。
- **Mod**：通过 `live2d-ai-mod-system` 的 trait 注册中心接入（`apply/factory` 心智，
  对齐 pi / DSH），**静态编译组合**，enable/disable 运行时开关，失败仅 disable 该 Mod。
- **不做功能竞赛**：不比 N.E.K.O. 生态、不比 Neuro-sama 包装。

---

## 📖 项目简介 | Overview

**Live2D-Ai** 是一个 **AI × Live2D 桌宠**——用 **Live2D 模型角色**进行**语音对话**，
通过 **LLM** 理解并回应，同时根据对话实时切换表情、驱动口型。核心只留最小闭环，
增强能力全部走 Mod 边界。

> "欢迎回来～今天也要一起加油哦！" —— 白 (Bai)

**Live2D 渲染**：基于 [Ayagami](https://github.com/AyagamiDev/ayagami)（MIT OR
Apache-2.0，pin rev）+ wgpu 29，不依赖 Live2D 专有 Cubism 闭源 SDK。模型需用户合法取得。

### 🗄️ Python / Android 退役说明

`Live2D-Ai-pc/`（Python）与 `Live2D-Ai-Android/`（Kotlin）已从主线移除：
- Python 版保留在 git 历史（tag `py-legacy`）
- Android 版归档到远端 `android-archive` 分支（见 `ANDROID_ARCHIVE_POINTER.md`）

Rust 重构（`crates/`）是**唯一主线**。

---

## ✨ 特性 | Features（Rust 主线现状）

### 核心链路（text → LLM → TTS → Live2D）
- **OpenAI 兼容单协议**：LLM `/chat/completions`（SSE 流式）+ TTS `/audio/speech`，
  不绑定厂商 SDK
- **纯 reducer 状态机**（`live2d-ai-core`）：epoch 闸门 + 双闩锁 + 动作仲裁
  （6 基础动作：nod/shake_no/tilt/look_around/listen/surprise，强度 1-3）
- **supervisor 事件循环**：独立线程 + current_thread tokio，五阶段 turn

### Web 前端（主仓库 UI）
- **65/35 布局**：左 Live2D 展示区 65% + 右对话区 35%
- 右上角 ⚙ 设置抽屉 8 分组：模型展示 / AI 与语音 / 角色 / 动作与互动 /
  模型管理 / 系统与高级 / 开发者日志 / **Mod 管理**
- 对话区（酒馆风格消息流）+ 动作快捷按钮（dev_mode 门控）
- WASM 渲染入口：`/render` 页面 + `/models/*` 模型资产（`l2d-wasm-demo` / `wasm_assets.rs`）

### Mod 系统（live2d-ai-mod-system）
- **trait 注册中心**：`ModFactory` / `ModRuntime` + `ModRegistrar`
- **实态**：3 个 Mod 已编译进二进制，`/api/v1/mods` 支持 enable/disable/restart；
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
# 根级 Python 测试（健康检查）
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
  live2d-ai-mod-director      动作编排 Mod（config 驱动白名单动作序列）
  live2d-ai-mod-pet-desktop   桌宠窗口 Mod v1 骨架（窗口生命周期接线后置）
xtask                 工程工具（Rust 占比统计）
```

### 核心链路
```
文本/语音
  → LLM（OpenAI /chat/completions, SSE 流式）
  → TTS（OpenAI /audio/speech）
  → Live2D（表情/口型/动作，wgpu 原生 + Web wasm）
```

### Mod 边界
```
主仓库核心链路 ←── live2d-ai-mod-system trait 接口 ──→ 3 个 Mod（workspace crate，已编译进二进制）
                     │                                      │
                     │ ModContext: trigger_action/say/on_event │
                     │            /register_settings/...      │
                     │ enable/disable/restart/reload_config   │
                     │ 事件有界 channel + 独立 worker         │
```

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
- `register_settings(schema)` → 注入设置子面板（前端统一渲染）
- `subscribe(topic)` → 订阅 turn/text/action 等事件（有界隔离）
- `ModServices.say/action_tx` → 驱动主链路（固定优先级映射）

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
