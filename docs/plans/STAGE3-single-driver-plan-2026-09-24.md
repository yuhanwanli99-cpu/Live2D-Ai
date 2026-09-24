# 阶段3 推进计划：单一驱动者 —— 退役 `latest.preset_id` 驱动通道，统一到 `action_cue`（2026-09-24）

> 落盘人：顶层管理与代码审查。工作树 = `/home/skystar/Live2D-Ai-l1`（分支 `mod/l1-product`，
> HEAD `dde21788`，工作树干净）。**Gate 2 = 保持 worktree 隔离**（只提交，不并入 main、不 push/tag/bump）。
> 前置：阶段2 已收口（见 `STAGE2-CLOSEOUT-plan-2026-09-24.md` §7.7）。
> **本文件 = 阶段3 范围冻结 + 编排入口 + 验收判据 + git 门控。**

---

## 0. 一句话 + 裁决

退役前端「拉 `GET /api/v1/mods/director/state` 的 `latest.preset_id`」这条**舞台驱动**通道；
舞台动作只由 WS `action_cue` 驱动；`latest` 降级为**面板只读展示**。

| # | 裁决（维护者） | 内容 |
| --- | --- | --- |
| **D10** | **撤销语义并入 action_cue** | `cues[].preset_id == "none"` 在该句音频开始时**撤销两个槽**（与渲染面 `preset{id:"none"}` 同义）。**不改帧结构**（字段已存在，只是语义化使用）；`cues: []` 仍是「本轮不动」。 |
| **D11** | **performance 开时的中性轮 = noop** | 表演层契约（`performance-layer-v0.md` §2.2）本就是 noop=不动，**保留**；显式撤销只在 **performance 关**（director 规则路径）保留 —— 即把 W4 语义平移到 cue 层。此不对称**必须写进文档**。 |
| **D12** | **保留面板只读** | `state_json.latest` / `GET …/state` 照旧，供 director 面板展示；只是**不再驱动舞台**。 |
| **D13** | 原 W9「禁止改 action_cue 帧结构」仍有效 | 本次只用既有字段的既有取值，不新增字段。 |

---

## 1. 现状（维护者 2026-09-24 亲读源码）

| 通道 | 实现 | 触发 | 问题 |
| --- | --- | --- | --- |
| **A（正路）** | `main.dart` `_directorCues`（:207）← `ActionCueEvent`（:426-429）→ 音频 start 时 `_applyDirectorCueForSeq`（:452-465）→ `applyPreset(cue.presetId)` | WS `action_cue` 帧 | 对 `preset_id=="none"` 已会调 `applyPreset('none')`（**无生产者**） |
| **B（要退役）** | `main.dart` `_applyDirectorPreset`（:484-514）+ `_directorPresetGate`/W4 `DirectorPresetGate`（`live2d_stage.dart:55-101`） | `TextDelta`/`TextFallback`（:421-423），轮末 `TurnStateEvent` 复位（:432-434） | **唯一**显式撤销路径；且 performance 开/关**都跑** ⇒ 与 A 双驱动 |

- 生产者事实：`forward_sentence_ready_to_mods = !engine.performance_enabled()`（`handlers.rs:47-91`）⇒ performance 开时 director **不产** cue；performance 关时性能层为 `None`（`engine.rs:404` 的 `if let Some(perf)`）⇒ 只有 director 产 cue。**A 自身无冲突，冲突来自 B**。
- `DirectorPlan::rule(epoch, None|"none", …)` 今天产**空 plan**（`plan.rs:76-91`）⇒ 中性轮无撤销；`arbiter.apply`（`arbiter.rs:41-50`）**不校验 preset id** ⇒ `none` cue 可直接承载。
- doc 与代码不符：`director-mod-v0.md:47-48` 说状态面「供面板展示」，而代码用它驱动（S6 治理缺口）。

---

## 2. 范围（冻结）

### 2.1 必做

1. **Dart（Worker W9-Dart）**：退役通道 B。
   - 删 `main.dart`：`_applyDirectorPreset()`、`_directorTurnHandled`、`_directorFetching`、`_directorPresetGate` 字段、`TextDelta`/`TextFallback` 的触发分支、`TurnStateEvent` 复位分支。
   - 删 `live2d_stage.dart`：`DirectorPresetGate` + `DirectorPresetAction` + `DirectorPresetDecision`（W4 纯逻辑，被 cue-`none` 取代）。
   - 保留并确认通道 A：`preset_id=="none"` → `applyPreset('none')` → 撤销。
   - 测试：删 `action_cue_test.dart` 中针对 gate 的三条；新增 ①action_cue 含 `none` → 该句撤销一次；②源码级断言 `shell/flutter/lib/` 不存在 `state('director')`+ `applyPreset` 同路径（仿 `action_scales_wiring_test.dart` 的源码扫描先例，写清是哪种断言）；③`grep -rn "mods/director/state" shell/flutter/lib/` 只剩面板读取处（贴原始输出）。
2. **Rust + 文档（Worker W9-Rust）**：撤销平移到 cue 层 + 文档对齐。
   - `plan.rs`：`DirectorPlan::rule(epoch, None|"none", …)` 改为产 `{sentence_seq:1, preset_id:"none", intensity, ttl_ms}`（不再空 plan）；有预设时行为不变。补回归。
   - `lib.rs`：`start()` 里「经只读状态面 latest.preset_id 交给前端」的日志/注释改成实情（latest 仅供面板；驱动走 `ModServices.cues`）；模块头注同步。
   - 文档：`performance-layer-v0.md`（§2/§7 补 `none` 语义 + D11 不对称 + 退役拉取通道）、`director-mod-v0.md`（§0/§18/§34/§39/§47-48/§224/§263-264/§341 与代码一致）、`AGENTS.md`（§27/§275/§294/§450 director 台账）、`app_event.rs:233` 注释、`HANDOFF-2026-09-22-performance-layer.md` §6。
3. **合并收口（编排者）**：Rust 全量 5 条 + Flutter analyze/test + build web 三证据 + `ignite --check` + 两种配置下的	extbf{驱动实测} + `verify_core_chain.py` 回归一次。

### 2.2 明确不做

- 不改 `action_cue` **帧结构**、不改 `ActionCue` 解析字段；不新增帧类型。
- 不动 performance 层的 `speak` 语义 / `[performance]` 配置 / 三态；不裁决 Q1（谁保留 speak）。
- 不改渲染面 `preset` 协议与 `l2d-wasm-demo`（本阶段零 wasm 改动）。
- 不碰基座独占文件：`topics.rs` / `mod_registry.rs` / `supervisor.rs` / `desktop/src/main.rs`（需要动 → 停下问维护者）。
- `tests_p0c` 等测试设计局限、prebuilt binary、首屏 pending —— 阶段2 backlog，不在本阶段。

---

## 3. 文件归属（并发铁律：禁止两人改同一文件）

| Worker | 独占文件 |
| --- | --- |
| **W9-Dart** | `shell/flutter/lib/main.dart`、`shell/flutter/lib/live2d/live2d_stage.dart`、`shell/flutter/test/action_cue_test.dart`（或新建 `driver_policy_test.dart`，二选一并在报告里说明） |
| **W9-Rust** | `crates/live2d-ai-mod-director/src/{plan.rs,lib.rs,tests.rs,tests_staging.rs}`、`crates/live2d-ai-desktop/src/app_event.rs`（**仅注释**）、`docs/architecture/{performance-layer-v0.md,director-mod-v0.md}`、`docs/plans/HANDOFF-2026-09-22-performance-layer.md`、`AGENTS.md` |

两轨无交集 ⇒ 可并行；合并收口由编排者在双方 settle 后**串行**跑。

---

## 4. 验收判据（阶段3 完成的定义）

1. `grep -rn "mods/director/state" shell/flutter/lib/` **只剩面板读取处**；`grep -rn "_applyDirectorPreset\|DirectorPresetGate" shell/flutter/lib/` = 0。
2. `action_cue` 含 `preset_id:"none"` 的 cue → 该句音频开始时**恰好一次** `applyPreset('none')`（Dart 测试 + 原始断言）。
3. **驱动实测（浏览器/WS，两种配置各一次，贴 HUD `preset:` 行 + WS 帧序列）**：
   - 配置①`[performance] enabled=false` + director 启用 → 动作由 `action_cue` 驱动（规则 cue），中性轮出现 `none` cue → 两槽归零；
   - 配置②`[performance] enabled=true` → 动作由 `action_cue` 驱动（表演层 cue），中性轮 noop（不动）——与 D11 一致。
4. Rust 全量门禁绿（`--no-fail-fast` + tee）：workspace all-targets / doc / fmt --check / clippy `-D warnings` / rust-ratio ≥95%。
5. Flutter：`analyze` 无问题 + `test` 全过 + `flutter build web --release --base-href /app/ --no-web-resources-cdn` 三证据（mtime / gstatic 0 / `ignite --check` 四项 ok）。
6. `python3 scripts/verify_core_chain.py --timeout 300 --text 下午好` 18 跳全过（主链无回归）。
7. 文档与注释：`director-mod-v0.md` / `performance-layer-v0.md` / `AGENTS.md` 不再声称「前端拉取驱动舞台」；D11 不对称有明文。
8. 报告四件套齐全；凡是自述无原始输出者退回。

---

## 5. git 门控（管理者执行，非 worker）

| 门 | 动作 |
| --- | --- |
| Gate 0' | 无（阶段2 的 checkpoint 已存在；本阶段从干净 `dde21788` 起） |
| Gate 3 | 收口报告 + 我独立复核通过后：按轨两条提交 —— `refactor(flutter): 退役 latest.preset_id 驱动通道（阶段3）` / `feat(director): 撤销经 action_cue none cue + 文档对齐（阶段3）` |
| Gate 4 | 阶段3 全部验收后再议是否并入 `main`（**需用户点头**；不 push/tag/bump 除非明确授权） |

worker 与编排者**一律禁止** git 写操作。

---

## 6. 风险与未决（诚实标注）

1. **D11 的不对称**：performance 开时中性轮不撤销（靠 ttl ≤5s 自然到点），performance 关时立即撤销。这是两个提供者的契约差异，**不是 bug**，但必须在 UI/文档里说清；若用户要求统一，另立裁决（改动落在 performance 层，属阶段4）。
2. **W4 回归面**：`DirectorPresetGate` 删除后，「同 seq 重复帧只归零一次」这条保护由 `action_cue` 的整表替换语义承担（前端按 seq 存 cue，应用一次即 remove）。需在 Dart 测试里钉住。
3. **规则层撤销的新回归点**：`DirectorPlan::rule` 从「空 plan」变「none cue」会改变 `cues_emitted`/账本计数语义 —— 相关回归（`tests_staging.rs`、director `tests.rs`）要同步核对，别让计数断言变成假绿。
4. **本阶段零 wasm / 零渲染面改动** ⇒ 不跑 `trunk build`；若有人碰了 `l2d-wasm-demo/**`，红线照旧要求重建。
