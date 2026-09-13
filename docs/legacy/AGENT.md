# Live2D-Ai — 项目总览

> 更新日期: 2026-09
> 项目定位: **通用人形皮套 AI 接入一体化平台**（Live2D humanoid-avatar AI-integration platform）——
> AI + Live2D 人形皮套的通用接入与应用层。**不绑定任何单一模型**（模型由用户合法导入，`assets/models/`
> 不捆绑二进制），**不做复杂上层**（实现保持最简：核心链路 + Mod 边界）。
> 实现: Rust workspace（`crates/`），Python/Android 双端已归档（`py-legacy` / `android-archive`）。
> 架构理念: **pi-agent 精神** — 最小核心 + 可扩展（核心只装不可拆的原子能力，其余全部走 Mod 边界）。

---

## 快速导航

| 文档 | 说明 |
|------|------|
| **`README.md`** | **首读** — 定位、快速开始、架构、Mod 接入 |
| **`AGENTS.md`** | 项目级通用说明（结构/背景，任何 CLI agent 可读） |
| `docs/README.md` | 文档索引 |
| `docs/architecture/core-contracts.md` | 核心契约与架构边界 |
| `docs/architecture/directory.md` | 目录约定（Rust 结构） |
| `docs/architecture/ARCHITECTURE.md` | 技术栈详表 |
| `docs/plans/node-e-execution-plan.md` | Mod 系统（节点 E）执行计划 |
| `docs/plans/node-f-execution-plan.md` | 核心链路收口 → 5 Mod 开箱即用（节点 F）|
| `CHANGELOG.md` | 版本变更记录 |
| `HANDOVER.md` | 历史交接文档 |

---

## 当前架构

```
┌─────────────────────────────────────────────┐
│         通用人形皮套 AI 接入一体化平台        │
│  （最小最简核心链路 + Mod 边界，非绑死单模型）    │
├─────────────────────────────────────────────┤
│   核心链路：文本/语音 → LLM → TTS → Live2D    │
│   表情/口型/动作                              │
│                                             │
│   Rust workspace（crates/）                  │
│   ├── l2d                 渲染封装（wgpu 29） │
│   ├── live2d-ai-core      纯 reducer 状态机   │
│   ├── live2d-ai-runtime   LLM/TTS + settings  │
│   ├── live2d-ai-desktop   supervisor + Web API│
│   ├── l2d-wasm-demo       Web 渲染核心         │
│   ├── live2d-ai-mod-system  Mod trait/注册中心  │
│   ├── …-mod-external-input/director/pet-desktop│
│   ├── …-mod-local-llm                          │
│   └── xtask（根目录）      工程工具（Rust 占比）    │
└─────────────────────────────────────────────┘
```

**Mod 边界**：增强能力（外部事件接入 / 动作编排 / 桌宠窗口 / 本地 LLM）均为 workspace crate，
经 `live2d-ai-mod-system` trait 注册，默认不启用；失败不影响核心链路。

---

## 快速命令

```bash
# 全量门禁（提交前必须）
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio   # 门槛 95%

# 构建 + 启动 Web 端（主入口）
cargo build --release
./target/release/live2d-ai-desktop --web --http-port 18080
```

---

## 开发须知

1. **门禁四件**：workspace 全量测试 + doc 测试 + fmt + clippy `-D warnings` + rust-ratio（≥95%）。
2. **源码 ≤500 行**（豁免 ≤1000 需头注理由）；测试文件 ≤800 行。
3. **密钥安全**：密钥不进 GET/日志/WS/导出；loopback-only；mutating 需 `application/json`。
4. **模型不捆绑**：`assets/models/` 仅目录+说明，模型二进制由用户合法导入后放置。
5. **Mod 边界**：Mod 必须经 `live2d-ai-mod-system` 接入，不得绕过 core 仲裁。

---

## 经验教训

> Rust 时代几个值得记下的坑：

| 教训 | 说明 |
|------|------|
| 浏览器缓存 | `run-web.sh` 构建+启动后必须 Ctrl+Shift+R 硬刷新；用户报「旧实现」先查 dist hash 与二进制 mtime（`scripts/diag-stale.sh` 五步）|
| 迁移丢 class 是系统性模式 | preview 用 `class=… id=…`，迁移只搬 id →「样式不生效」先查 CSS 选择器失配 |
| headless 无独立 GPU | 软渲染（llvmpipe）无法验证 wgpu HighPerformance / 16384 档，真机验收需外部渲染环境 |
