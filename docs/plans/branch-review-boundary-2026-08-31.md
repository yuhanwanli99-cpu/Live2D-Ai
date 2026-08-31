# 分支审查边界区分（2026-08-31）

> 目的：给高级 agent 审查时明确「主分支需审查的部分」与「子分支继续干的部分」边界。
> 背景：节点 D 复审暂缓，主分支冻结等裁决；D3/D4 在子分支推进。

## 一、分支拓扑（已核实）

```
main (af140744)                 ← 旧全栈基线（Py PC + Android + shared），本地领先 origin/main 5 提交
  └─ refactor/elegance (47848ed4)  ← 「主分支」Rust 重构起点：删旧双端(-118104 行) + 建 crates/(+62181 行)
       └─ feature/node-d-d3-d4 (5ba4e8c0)  ← 「子分支」= 主分支 + 3 个 D 批次提交
```

- `refactor/elegance` 是 `main` 直系后代（36 提交），`feature/node-d-d3-d4` 是 `refactor/elegance` 直系后代（+3 提交），**无分叉、线性关系**。
- 主分支冻结点：`47848ed4 chore(persona): snapshot live-stream persona edit (2026-08-26)`
- 子分支当前 HEAD：`5ba4e8c0 chore(housekeeping): WSL migration path fixes + post-archive cleanup`

## 二、主分支（refactor/elegance）需要审查的部分

### 范围
`main..refactor/elegance` 的 36 个提交，净变化 **957 文件, +62181 / -118104**。

### 构成（按提交类别）
1. **Rust 重构核心**（审查重点）：
   - `crates/` workspace：l2d(12 rs) / live2d-ai-core(5) / live2d-ai-runtime(18) / live2d-ai-desktop(10) / l2d-wasm-demo(1)，共 47 个 rs
   - `xtask/`（Rust 占比统计工具）
   - 根 `Cargo.toml` / `Cargo.lock` / `rustfmt.toml`
   - 对应计划：`docs/plans/RUST-REWRITE-RFC.md`
2. **旧 Py 功能演进**（已归档 `Live2D-Ai-pc/` 上的 30+ 提交：表演引擎 soullink / SLOP 台词治理 / 设置逻辑链 / 插件系统等）——这些提交在**已删除的 Py 目录**上，git 历史保留，**不参与 Rust 审查**，但需确认其决策是否影响 Rust 设计（如 soullink 表演引擎、动作工具语义）。
3. **冻结快照**：`91c460fa`（android archive 后全仓快照）、`47848ed4`（persona 快照）。

### 审查重点清单（主分支 Rust 部分）
| 项 | 位置 | 说明 |
|---|---|---|
| workspace 结构 | `Cargo.toml` | 6 member，edition 2024，rust-version 1.92（egui 0.35 下限；2026-08-31 从 1.87 修正） |
| core reducer | `crates/live2d-ai-core/src/` | 纯 reducer 状态机（5 rs） |
| runtime 设置 | `crates/live2d-ai-runtime/src/settings/` | 18 rs（含 serde 三态/原子写回） |
| desktop 入口 | `crates/live2d-ai-desktop/src/` | 10 rs（cli/app/tray/user_event） |
| l2d 渲染 | `crates/l2d/src/` | 12 rs |
| 文档契约 | `docs/architecture/core-contracts.md`, `directory.md` | 核心契约 |

> 注意：主分支上 **无** `web_api/`、`supervisor/`（D 批次内容），只有第一波 Rust 骨架。

## 三、子分支（feature/node-d-d3-d4）继续干的部分

### 范围
`refactor/elegance..feature/node-d-d3-d4` 的 **3 个提交**：
1. `e91d4695` **D2 批次**：SettingsService（settings_to_view 脱敏 / apply_patch 三态 / plan_atomic_write）+ SecurityContext（Origin/Content-Type 校验）+ 首次配置闭环（P0C）+ WS bridge
2. `b89ea49d` **D3+D4 批次**：models API（6 端点 + registry 原子写回 + 路径穿越双重防护）+ 命令注册表（6+2 动作 + typed invoke 走 core 仲裁）+ 前端动作面板
3. `5ba4e8c0` **housekeeping**：WSL 迁移路径修复 + 归档后清理（CI Rust 化 / 删失效测试与脚本 / clippy 修复）

### 子分支上的关键文件
| 层 | 文件 |
|---|---|
| supervisor | `crates/live2d-ai-desktop/src/supervisor.rs` + `supervisor/turn.rs` + `handlers.rs` |
| web_api | `crates/live2d-ai-desktop/src/web_api/`（mod/dispatch/security/ws/ws-connection/ws-events/supervisor_slot/chat.js/app.js） |
| settings | `crates/live2d-ai-runtime/src/settings/`（patch/view/patch_tests/settings_tests） |
| models | `web_api/models_routes/`（mod/registry/handlers/dto） |
| commands | `web_api/command_registry.rs` + `commands_routes.rs` |
| 计划文档 | `docs/plans/node-d-api-contract-2026-08-28.md`, `node-d-d3-rendering-route.md`, `node-d-py-asset-migration.md` |

### 子分支已落地门禁
```
cargo test --workspace   668 passed
cargo fmt --all --check  ✅
cargo clippy -D warnings ✅
pytest tests/            22 passed, 1 skipped
```

### 子分支待继续（尚未做）
| 待办 | 说明 |
|---|---|
| D3.2 后置 | ZIP 上传、background_image 注册表、activate 后 supervisor 热重载 |
| P1WS-3 | 服务端读 client 帧（tungstenite 阻塞 I/O 结构性限制） |
| 5 个 P1 WS 事件 | tool_action/error（需 AppEvent 新变体）、audio_status/mouth_level（需 supervisor 信号源）、model_status（需 D3 激活路径） |
| 清理项 | dispatch.rs current_epoch_unused 参数、cli usage 提示 |
| 节点 D 定向复审 | 待用户裁决后决定合并策略 |

## 四、给高级 agent 的审查建议

1. **先审主分支 Rust 骨架**（`refactor/elegance` 的 crates/ + xtask/），确认第一波重构质量；
2. **再审子分支 D 批次**（3 提交），重点：安全红线（密钥不进 GET/日志/WS/导出、loopback-only、Origin 校验、命令 invoke 必经 core 仲裁）、supervisor 双闩锁/epoch 语义、SettingsService 三态；
3. **决策确认**：主分支冻结的 36 提交中，Py 旧功能演进（soullink 等）是否需回看决策；Android/Py 归档是否影响 Rust 设计（动作工具语义、persona 结构）；
4. **合并策略**：裁决通过后，feature 分支如何并入主分支（squash/merge/rebase）需裁决。
