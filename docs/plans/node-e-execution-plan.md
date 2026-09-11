# 节点 E 执行计划（打勾追踪 / 子代理派发要点）

> **本文件是节点 E 的唯一状态源**：主 Agent 每完成一个子任务就在此打勾。
> 每个任务原子化、单文件/双文件、验收明确；子代理返回触发下一步派发。
> **状态：节点 E 全部完成 ✅**（tag `node-e-final`，commit `3c264676`）。

## 子代理派发要点（每次派发必读）

1. **模型**：Laguna S 2.1 FREE——只能用作编码 Agent，任务书必须自包含（含全部背景事实与精确改法），不依赖它会调研。
2. **任务原子化**：一个子代理只做 1-4 个文件的明确改动，给出精确的修改点与验收命令；复杂任务加时间盒（"先写后验，调研 ≤2 分钟"）。
3. **文件集零重叠**才并行派发；有依赖关系的任务串行（前一个返回并 review 通过后才派下一个）。
4. **主 Agent 纪律**：只编排、派发、code review、打勾；不亲手改重要节点代码；不轮询/确认子代理状态（返回会后台通知）；子代理返回后**独立复核门禁**（不轻信报告）再打勾。
5. **验收门禁**（每个子任务返回后主 Agent 复核）：`cargo test --workspace --all-targets` 全绿、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、关键端点 curl 冒烟。
6. **每完成一个节点**：commit + tag（`node-e-*`）保证可回滚。

## 已完成（全部有 commit/tag 证据）

- [x] **E0** 边界裁决 ADR（tag `node-e-e0`）
- [x] **E1a** 入口文档 Rust 化（tag `node-e-e1a`）
- [x] **E1b** 大文件豁免确认（不强拆）
- [x] **E2** WASM 渲染纵切：`wasm_assets.rs`（`/models/*`、`/render/*`，curl 验证 200；tag `node-e-e2`）
- [x] **E4a** Mod SDK crate `live2d-ai-mod-system`（tag `node-e-e4`）
- [x] **E4b** host ModRegistry（tag `node-e-e4b`）
- [x] **E5** External Input Mod（tag `node-e-e5`，commit `c0cd2416`）：
  - E5-T1 修 lib.rs 测试（`53766c00`）/ E5-T1b clippy lint（`a23a536e`）/ E5-T2 host 接线（`ff3d69d5`）/ E5-T3 `POST /api/v1/external/chat`（`0f6aee8d`；loopback+Origin+json 校验，curl 200/415/405）
- [x] **E3** Web 产品 UI（tag `node-e-e3`，commit `dae4bab0`）：
  - E3-T1 65/35 布局 + 8 分组抽屉骨架（`e7999452`；47 原 id 全保留）/ E3-T2 app.js 抽屉逻辑 + capability 驱动（`0e198481`）/ E3-T3 chat.js 审计零改动（`32699b70`）
- [x] **E6** Mod 管理 UI（tag `node-e-e6`，commit `9efe8905`）：
  - E6-T2 前端面板（`c6f0e120`）/ E6-T1b 后端 `/api/v1/mods` API（`2da66d71`；e2e：disabled→enable→running→disable→restart 全链路）。注：E6-T1 首派（`cfb1ae69`）中途耗尽零落盘，由 T1b 接替。
- [x] **E7** Pet/Director Mod（tag `node-e-e7`，commit `48eea2ca`）：
  - E7-T1 director crate（`5dc44d21`；白名单动作序列 performance sequence，4 测试）/ E7-T2 pet-desktop crate v1 骨架（`e50d42e0`；3 测试，无 GUI 依赖）/ E7-T3 3 工厂注册 + e2e（`67add2a8`）
- [x] **E-final** 文档一致性（tag `node-e-final`，commit `3c264676`）：README/directory.md 与 3 Mod 实态对齐（`21b92a79`）。

## 最终验收（✅ 全部通过）

- [x] `cargo test --workspace --all-targets`：18 组 test result ok、0 failed；`--doc` 7 组 ok；fmt/clippy 干净。
- [x] Rust 占比 **95.80%**（48219/50332，门槛 95% PASS）。
- [x] tag 链完整可回滚：e0 / e1a / e2 / e4 / e4b / e5 / e6 / e7 / final（+pre-implementation）。
- [x] 3 个 Mod e2e 验证：external-input（say 主链路 + HTTP 端点）、director（白名单动作序列）、pet-desktop（v1 骨架）；`/api/v1/mods` enable/disable/restart 全链路。
- [x] Web UI：65/35 布局 + ⚙ 设置抽屉 8 分组（含 Mod 管理）+ `/render` + `/models/*`。
- [x] 入口文档与实态一致。

## 遗留（节点 F 候选）

- pet-desktop 真实悬浮窗/点击穿透/托盘接线（v1 骨架已就位，窗口生命周期后置）。
- README CLI 模式段落（`--chat`/`--pet-mode`）与"窗口接线后置"状态仍有轻微张力（文档子代理已标注）。
- ModRegistry `dispatch_event` → per-Mod `on_event` 的细粒度路由（E4b v1 为占位 worker）。
- 视觉验收由人类负责。
