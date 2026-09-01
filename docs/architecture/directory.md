# Live2D-Ai 目录约定（Rust 主线）

> 目标：让仓库顶层保持"一眼能看懂"，生成物/本地环境不混进源码。
> 状态：2026-08-31 按 Rust 主线重写（原版描述 Android/Python 双端，已归档）。

## 顶层结构（Rust 主线）

```
Live2D-Ai/
├── README.md                 # 项目入口：定位、快速开始、架构、Mod 接入
├── AGENTS.md                 # Agent/开发者导航
├── CHANGELOG.md              # 版本变更
├── LICENSE / NOTICE / CREDITS.md
├── Cargo.toml                # workspace 清单（6 members）
├── Cargo.lock                # 依赖锁定
├── rustfmt.toml / .editorconfig
├── crates/                   # Rust 源码（核心链路）
│   ├── l2d                   # Live2D 渲染封装（Ayagami + wgpu 29）
│   ├── live2d-ai-core        # 纯 reducer 状态机（无 IO、无 async）
│   ├── live2d-ai-runtime     # LLM/TTS OpenAI 兼容 + settings + conversation + dialogue
│   ├── live2d-ai-desktop     # supervisor 事件循环 + Web API + 窗口 + 音频 + 托盘
│   ├── l2d-wasm-demo         # Web 渲染核心（被 Web 前端复用）
│   └── (live2d-ai-mod-system)  # Mod trait/factory/registry/services/status/topics（E4 新增）
├── xtask/                    # 工程工具（Rust 占比统计）
├── assets/models/            # Live2D 模型（仅目录+说明，二进制不入库）
├── docs/
│   ├── README.md             # 文档索引
│   ├── architecture/         # 架构、契约、目录、依赖、可观测性
│   ├── plans/                # 规划类文档（node-e-* 等）
│   ├── research/             # 调研类文档
│   └── verification/         # 验收/测试报告
├── scripts/                  # 开发/健康检查脚本（check_public_secrets 等）
├── tests/                    # 根级 Python 测试（健康检查）
├── live2d-ai.toml.example    # 桌面端配置模板
└── shared/                   # 历史资产（persona.yaml 等，降为模板资产）
```

## 目录职责

| 目录 | 放什么 | 不放什么 |
|---|---|---|
| `crates/` | Rust 源码（主仓库核心链路 + Mod） | 任何非 Rust 实现 |
| `crates/live2d-ai-core` | 纯 reducer 状态机 | IO/async/GUI |
| `crates/live2d-ai-runtime` | LLM/TTS/settings/conversation | 窗口/GUI |
| `crates/live2d-ai-desktop` | supervisor + Web API + 窗口 + 音频 | 核心状态机 |
| `crates/live2d-ai-mod-system` | Mod trait/factory/registry | 具体 Mod 实现 |
| `assets/models/` | Live2D 模型清单与说明 | 二进制模型（不入库）|
| `docs/architecture/` | 当前架构、契约、边界、依赖 | 历史规划草稿 |
| `docs/plans/` | 历史/未来规划、任务拆分 | 已废弃调研 |
| `docs/research/` | 竞品、技术调研 | 当前实现细节 |
| `docs/verification/` | 验收报告、测试证据 | 源码 |
| `scripts/` | 可复用脚本 | 一次性草稿 |
| `tests/` | 根级无 Key Python 测试 | crate 专测（放各自 crate）|

## Mod 目录约定（节点 E）

3 个 Mod 作为主仓库 workspace 成员（默认 enable 由 manifest 控制）：
```
crates/live2d-ai-mod-external-input/  外部事件接入 Mod（`POST /api/v1/external/chat`）
crates/live2d-ai-mod-pet-desktop/    桌宠窗口 Mod v1 骨架（settings schema + Voice 事件占位；窗口生命周期/悬浮窗/点击穿透/托盘接线后置）
crates/live2d-ai-mod-director/       动作编排 Mod（E7 落地：config 驱动白名单动作序列；performance sequence 能力由此实现）
```
各 Mod crate 内含 `src/lib.rs`（ModFactory 实现）+ `README.md`（职责/依赖/配置）。

## 清理规则

- 生成物：`build/`、`target/`、`.pytest_cache/`、`.ruff_cache/`、`__pycache__/`、`*.log`
  不应出现在源码树中，统一可被 `scripts/clean.sh` 清理。
- 本地环境：`.venv/`、`.cargo-home/`、`node_modules/` 属于本地运行环境，不提交。
- 本地 cargo 缓存：`CARGO_HOME`（如 `/tmp/cargo-ui-test`）不在仓库内。
- Agent 状态：`.pi/`、`.pi-glla/`、`spoq/` 属于本地/Agent 工作区，不入库。
