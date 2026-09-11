# 交接：渲染档位落地 + 文档一致性治理（2026-09-08）

> 本轮会话交接。目标：**代码审查 + 核心链路增强 + UI 增强 + 文档更新**。
> 对本项目而言是「通用人形皮套 AI 接入一体化平台」（非单一桌宠应用）。
> 回滚锚点链：`15d0c018`（HEAD）→ `0e0b9601`/`86940b22`/`c6840a5a`/`ab668260` → base `63b1e542`。
> run 分支：`piagent/run/20260908-201956-lp8l`（未 push）。建议合并进 `main` 后删除该分支。

---

## 1. 本轮完成（按 commit）

### `ab668260` — 核心链路增强·l2d 渲染档位（T01，主代理）
- 新增 `crates/l2d/src/renderer/tier.rs`：`RenderTier` 枚举（`DEFAULT=Tier4096` / `side()` /
  `required_max_texture_dimension()` / `from_side()`），附 4 条单测。
- `renderer/mod.rs`：导出 `RenderTier`（`pub mod tier` + `pub use tier::RenderTier`）。
- `renderer/gpu.rs`：`init_headless_device(tier)` 按档位构造 `max_texture_dimension_2d`（仅 16384
  档需显式升高）；新增 `GpuContext::headless_with_tier(tier)`，保留旧 `headless()` 向后兼容
  （offscreen / benchmark 调用点不受影响）。

### `c6840a5a` — 核心链路增强·wasm 档位接线（T02，主代理）
- `l2d-wasm-demo/src/web/surface.rs`：`BridgeState` 新增 `tier: RenderTier`（默认 `DEFAULT`）；
  `canvas_pixel_size` 增 `tier` 参数并以 `tier.side()` 为上钳（`min`），避免超大离屏 target 超
  sampler 上限/显存预算；两处调用点（初始 setup + resize handler）接入。
- `l2d-wasm-demo/src/main.rs`：`stage-config` arm 解析 `tier`（4096/8192/16384，非法忽略，
  用 edition 2024 let-chain）。

### `86940b22` — UI 增强·渲染档位下拉 + UI 锁测试（T03，主代理代子代理落地）
> 子代理 T03 因上下文长度限制未能产出 commit；主代理亲自落地该任务的**高价值部分**。
- `web_api/index.html`：外观与舞台新增「渲染档位」下拉 `id=appRenderTier`（4096/8192/16384）。
- `web_api/app.js`：`state.appearance.tier`（默认 4096）+ `els.appRenderTier` +
  `postStageConfig()` 追加 `cfg.tier` + `bindAppearance()` 本地记忆 `dsh.app.tier` 并即时应用。
- `web_api/tests_html.rs`：`index_html_template_contains_model_and_appearance_dom` 追加锁
  `#appRenderTier` 存在与 `app.js` 消费（`els.appRenderTier`）。

### `0e0b9601` — 文档更新 + doc-实现一致性门禁（T04，子代理）
- `README.md`：「3 个 Mod」→「5 个 Mod」（2 处，与 `main.rs` `AVAILABLE_MOD_FACTORIES` 实态对齐）。
- `docs/architecture/renderer-texture-tier-adr.md`：§6 状态更新为「接口已落地、真机验收仍属外部」。
- `crates/live2d-ai-desktop/src/main.rs`：`AVAILABLE_MOD_FACTORIES` 改 `pub` + 新增 3 条一致性
  门禁测试：`mod_count_is_five` / `mod_factory_ids_match_expected` / `mod_factory_ids_are_unique`
  （锁 Mod 计数=5 + 工厂 id 与文档集合一致 + 唯一）。

### `15d0c018` — fmt 修正（主代理集成）
- `cargo fmt --all` 修正 T04 集成时 main.rs 的超宽数组字面量（子代理提交未跑 fmt）。

---

## 2. 当前门禁状态（全绿）

- `cargo fmt --all --check`  干净
- `cargo clippy --workspace --all-targets -- -D warnings`  干净
- `cargo test --workspace --all-targets`  全绿（排除 §3 的 flaky 测试）
- `cargo test --doc --workspace`  通过
- `cargo run -p xtask -- rust-ratio`  **95.27%**（≥95%）

### 测试增量
| 域 | 新增测试 |
|---|---|
| `l2d` tier | `default_is_4096` / `side_maps_each_tier` / `required_max_dimension_equals_side` / `from_side_parses_known_and_rejects_unknown` |
| `live2d-ai-desktop` | `mod_count_is_five` / `mod_factory_ids_match_expected` / `mod_factory_ids_are_unique` |
| `web_api` | `index_html_template_contains_model_and_appearance_dom`（追加 `#appRenderTier` 断言）|

---

## 3. 踩过的坑 / 重要发现

1. **子代理依赖主代理任务会阻塞调度**：T03 `depends_on: T02`（主代理）。dispatch 只调度 child
   任务，无法满足依赖 → 需把 T02 置为「已完成且已在 run 分支」后将 T03 依赖置空再派发。
2. **子代理上下文/输出长度限制会导致静默失败**：T03 做了大量 read/bash 后以 `error: length`
   中止、无 commit。涉及大改（如 app.js 减重）的任务，子代理易超上下文，建议拆小或主代理亲自做。
3. **子代理提交未必 fmt 干净**：T04 的 main.rs 数组超宽，集成后 `cargo fmt --check` 报警，
   由主代理 `cargo fmt --all` 修复。（凡集成子代理 commit，务必复核 fmt。）
4. **`live2d-ai-runtime` 曾存在 flaky 集成测试**：`tool_call_becomes_tool_action_event`
   （mock HTTP 服务 + async 管道时序）。**根因已定位并修复**：`OpenAiClient` 的 reqwest 客户端
   池机会复用 mock 服务器已 `connection: close` 的半关闭连接，下一个 TTS/LLM 请求读到
   hyper::Error(IncompleteMessage) 而误判 TurnStatus::Failed。修法：给 client 加
   `.pool_max_idle_per_host(0)` 禁空闲复用。**实证 50/50 通过**（原 ~50% 抖动）；
   并复核该测试在 main 上同样抖动，与本轮的 l2d 改动无关。
   此修复是我（接手后的主代理）在 `52280d25` 落地的。
5. **wasm crate 有既存 clippy warning**（`net.rs` 两处 `collapsible_if`）——只在
   `--target wasm32` 下暴露，不在 native workspace 门禁内；本轮未动，非回归。

---

## 4. 未做完 / 后续工作

### 4.1 技术债处置（接手后的主代理已于 2026-09-08 续作）
- [x] **flaky 测试加固**：`tool_call_becomes_tool_action_event` 已修（见 §3.4；commit `52280d25`）。
- [~] **app.js 减重**：安全瘦身 `943 → 921` 行（commit `280a25db`；删死代码 + 压缩冗余注释 +
      renderAll 用 setIf 统一赋值），`node --check` 通过、desktop 489 测试全绿。
      **仍距 spec-v2 预算 ≤760 差 ~161 行**，为真实功能代码（配置面板 25+ 函数），
      按「触核心逻辑即停」原则未强制删除。若要继续压，需独立重构轮并先建行为基线（tests_html 可兜底）。

### 4.2 外部验收项（需真机 / 浏览器 / 真实端点，不 fake 视为完成）
- [ ] **S3 原生 16384 离屏 target 实例**：需带独立 GPU 环境实测 16384 能否建立 + 给出典型卡安全档。
- [ ] **S4 档位视觉验收**：浏览器 `/render`，切换档位后离屏 target 清晰度/描边变化 + 回退不 crash。
      wasm 受浏览器 GPU limits 约束，16384 档一般不可达（上限取决于浏览器）。
- [ ] **F6 端到端**：真实 LLM/TTS → 浏览器发消息 → 出声 → 口型动。

### 4.3 与既有 roadmap 的衔接
- 纹理档位（ADR §3）已从「文档先行、代码未落地」推进到「接口已落地、真机验收仍属外部」，
  是 [future-roadmap-2026-09.md](../plans/future-roadmap-2026-09.md) R1「纹理档位实现」与
  R2「S3/S4 真机验收」的分界线。
- doc-实现一致性门禁补上了 R1「行/文件一致性门禁」项，防「3 Mod vs 5 Mod」类漂移再发生。

---

## 5. 关键文件索引
| 域 | 路径 |
|---|---|
| 渲染档位常量 | `crates/l2d/src/renderer/tier.rs`（新增）|
| native limits | `crates/l2d/src/renderer/gpu.rs`（`headless_with_tier` / `init_headless_device`）|
| wasm 档位 | `crates/l2d-wasm-demo/src/web/surface.rs`（`BridgeState.tier` / `canvas_pixel_size`）、`src/main.rs`（`stage-config`）|
| 前端档位下拉 | `crates/live2d-ai-desktop/src/web_api/{index.html,app.js}`（`#appRenderTier`）|
| UI 锁测试 | `crates/live2d-ai-desktop/src/web_api/tests_html.rs` |
| Mod 一致性门禁 | `crates/live2d-ai-desktop/src/main.rs`（`AVAILABLE_MOD_FACTORIES` + 3 测试）|
| ADR | `docs/architecture/renderer-texture-tier-adr.md` |
| 规格依据 | `docs/design/web-ui-spec-v2.md`（§3 舞台/外观 token）、`docs/plans/web-ui-redo-plan.md` |

---

## 6. 回滚
- 整体回退：`git reset --hard 63b1e542`（回到本轮 base，丢弃本轮全部提交）。
- 单步回退：按 §1 锚点链逐个 `git revert` 即可（每 commit 均为最小、独立语义）。
