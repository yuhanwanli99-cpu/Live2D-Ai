# 节点 E 执行计划（打勾追踪 / 子代理派发要点）

> **本文件是节点 E 的唯一状态源**：主 Agent 每完成一个子任务就在此打勾。
> 每个任务原子化、单文件/双文件、验收明确；子代理返回触发下一步派发。

## 子代理派发要点（每次派发必读）

1. **模型**：Laguna S 2.1 FREE——只能用作编码 Agent，任务书必须自包含（含全部背景事实与精确改法），不依赖它会调研。
2. **任务原子化**：一个子代理只做 1-2 个文件的明确改动，给出精确的修改点与验收命令；禁止开放式调研型任务书。
3. **文件集零重叠**才并行派发；有依赖关系的任务串行（前一个返回并 review 通过后才派下一个）。
4. **主 Agent 纪律**：只编排、派发、code review、打勾；不亲手改重要节点代码；不轮询/确认子代理状态（返回会后台通知）；子代理返回后先 review 再派下一批。
5. **验收门禁**（每个子任务返回后主 Agent 复核）：`cargo test --workspace --all-targets` 全绿、`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`。
6. **每完成一个节点**：commit + tag（`node-e-*`）保证可回滚。

## 已完成（有 commit/tag 证据）

- [x] **E0** 边界裁决 ADR（tag `node-e-e0`）
- [x] **E1a** 入口文档 Rust 化（tag `node-e-e1a`）
- [x] **E1b** 大文件豁免确认（不强拆）
- [x] **E2** WASM 渲染纵切：`wasm_assets.rs`（`/models/*`、`/render/*`，curl 验证 200；tag `node-e-e2`）
- [x] **E4a** Mod SDK crate `live2d-ai-mod-system`（trait/factory/registry/services/status/topics；tag `node-e-e4`）
- [x] **E4b** host ModRegistry `mod_registry.rs`（生命周期+事件有界 worker；tag `node-e-e4b`）
- 验证：全 workspace 测试 687 passed、fmt/clippy 干净（E4b 时点）。

## 进行中（已派出，等待返回）

- [x] **E5-T1**（子代理 `53766c00`，已返回+review 通过）：修 `crates/live2d-ai-mod-external-input/src/lib.rs` 失败测试——`say_external` 改走 `services.say_tx.say`，删冗余 `say_handle` 字段。验收：`cargo test -p live2d-ai-mod-external-input` 3 测试全绿（217 行）。
- [x] **E3-T1**（子代理 `e7999452`，已返回+review 通过）：重写 `web_api/index.html + style.css`——65/35 布局（左 Live2D 右对话）、右上 ⚙、`#settings-drawer` 8 分组抽屉骨架、47 个原 id 全保留（grep 验证）、响应式 ≤900px 单列（html 184 行 / css 117 行）。遗留小瑕疵：CSS `#chatHint` 选择器与实际 id `chat-hint` 不匹配（旧文件遗留），E3-T2 顺手修。

## 待派任务队列（原子化，依赖关系标注）

- [x] **E5-T2**（子代理 `ff3d69d5`，已返回+review 通过）：host 接线——Cargo.toml 加 external-input 依赖；main.rs 加 `AVAILABLE_MOD_FACTORIES` 静态注册（`#[expect(dead_code)]`，E5-T3 消费）；mod_registry 加 `say_source` 字段 + `set_say_source()` + `make_services` 接收 say。441 desktop 测试绿；本 3 文件 clippy 干净。⚠ 发现 E5-T1 遗留 `field_reassign_with_default` lint（workspace clippy 失败）→ 已派 E5-T1b 修复。
- [x] **E5-T1b**（子代理 `a23a536e`，已返回+review 通过）：修 E5-T1 遗留 clippy lint——tests 改结构体字面量构造，去 `mut`。3 测试绿 + workspace clippy 干净 + fmt 干净。
- [x] **E5-T3**（子代理 `0f6aee8d`，已返回+独立复核通过）：`external_routes.rs` 新文件（POST /api/v1/external/chat：loopback+Origin 同源+json 校验，405/415/403/400/503 全覆盖，≤2000 字符）+ mod.rs 两处接线。curl 冒烟 200/415/405；workspace 0 failed、fmt/clippy 干净（主代理独立复核一致）。
- [x] **E5 验收**：commit `c0cd2416` + tag `node-e-e5`。
- [x] **E3 验收**：commit `dae4bab0` + tag `node-e-e3`（E3-T1/T2/T3 返回并 review 后提交，前端 4 文件 + plan 文档）。
- [ ] **E5-T3**（依赖 E5-T2）：新文件 `web_api/external_routes.rs`——`POST /api/v1/external/chat`（body `{"text": ...}` → `say()`），`dispatch.rs` 加路由，external-input 未启用时 404。验收：curl 验证 + 全测试绿。
- [x] **E3-T2**（子代理 `0e198481`，已返回+review 通过）：app.js 抽屉逻辑（lazyLoadDrawer/setDrawer/toggleDrawer/closeDrawer/renderGroups）+ capability 驱动（基于 dto.rs 真实 AppInfo 字段，不猜测能力位）+ style.css `#chatHint`→`#chat-hint` 修复。node --check OK，452 行。
- [x] **E3-T3**（子代理 `32699b70`，已返回+review 通过）：chat.js 全量引用审计——8 静态 id 全在、actions-card 动态挂载自包含、空引用均有 guard，**零改动**（499 行不变）。node --check 过。E3 前端三块齐。
- [ ] **E3-T3**（依赖 E3-T2）：`web_api/chat.js` 适配新 DOM（对话面板在新布局下的选择器核对）。验收：自查 + 不破坏现有 WS/发送逻辑。
- [ ] **E5 验收**（依赖 E5-T3）：主 Agent 复核全量门禁 → commit + tag `node-e-e5`。
- [ ] **E3 验收**（依赖 E3-T3）：主 Agent 复核（curl 首页 200 + 结构自查）→ commit + tag `node-e-e3`。
- [ ] **E6**（T2 前端完成，T1b 后端进行中）：Mod 管理 UI。
  - [x] **E6-T2**（子代理 `c6f0e120`，已返回+review 通过）：app.js `renderMods`/`modAction`/MOD_STATUS/MOD_ACTIONS + 抽屉 3 处接线；index.html `#grp-mods` 骨架（静态编译语义提示）。500 行 node --check 过；按契约先行（后端未就绪行内降级）。注意：E6-T1 首派（`cfb1ae69`）中途耗尽零落盘，已派 E6-T1b 接替。
  - [ ] **E6-T1b**（子代理 `2da66d71`，进行中）：mods_routes.rs + ServerContext 挂 ModRegistry + main.rs 装配 + 前置路由（时间盒任务书）。
  - [x] **E6-T1b**（子代理 `2da66d71`，已返回+独立复核通过）：mods_routes.rs（GET list + POST enable/disable/restart，403/404/405/415 全覆盖）+ ServerContext.mod_registry + cli_entry 装配 + 前置路由。e2e curl 全链路（disabled→enable→running→disable→restart）+ 门禁全绿（主代理独立复核一致）。
  - [x] **E6 验收**：commit `9efe8905` + tag `node-e-e6`。
- [ ] **E7**（进行中）：Pet/Director Mod。
  - [x] **E7-T1**（子代理 `5dc44d21`，已返回+review 通过）：director crate（326 行）——TurnStarted 触发白名单过滤动作序列（performance sequence 演示）+ ActionFinished 记录 + 4 测试绿（含非法动作过滤验证）。
  - [ ] **E7-T3**（进行中，子代理见下）：两工厂注册 AVAILABLE_MOD_FACTORIES + workspace 成员 + manifest 启用 + e2e。
  - [x] **E7-T2**（子代理 `e50d42e0`，已返回+review 通过）：pet-desktop crate v1 骨架（242 行）——settings schema 3 字段 + Voice 事件 voice_active 状态翻转 + 3 测试绿，无 GUI 依赖，根 Cargo.toml 还原干净。小瑕疵：头注"节点 E E6"笔误（应为 E7），验收时容忍或 E7-T3 顺带修。
  - [ ] **E7-T3**（依赖 T1+T2）：两工厂注册进 AVAILABLE_MOD_FACTORIES + manifest 默认启用 + e2e 验证。
  - [ ] E7 验收：全量门禁 + commit + tag `node-e-e7`。
- [ ] **E7**（依赖 E5 Mod 架构）：pet-desktop / director Mod（含 performance sequence 能力）——届时再原子化拆分。

## 最终验收（全部完成后）

- [ ] `cargo test --workspace --all-targets` + `--doc` 全绿；fmt/clippy 干净；Rust 占比 ≥95%（xtask `rust-ratio`）。
- [ ] E0-E7 各节点均有 commit+tag；入口文档（README/AGENTS/directory）与实态一致。
- [ ] 汇报：3 个 Mod 完成、Web UI 可用（65/35 + 抽屉 + wasm 展示 + Mod 管理）、Mod 可 enable/disable 且失败不影响主链路。
