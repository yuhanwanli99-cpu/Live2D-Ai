# 阶段2 推进计划（Wave 2 = W7 + 欠账 A–D + flaky 定名 + TTS 端到端补证）

> 落盘人：顶层管理与代码审查（2026-09-22）。工作树 `/home/skystar/Live2D-Ai-l1`（分支 `mod/l1-product`，HEAD `d2bdd4fd`）。
> **规格真源**：`docs/plans/RESEARCH-actions-director-audit-2026-09-21.md`（只读，不得改）。
> **任务真源**：`docs/plans/IMPL-PROMPTS-actions-performance-round.md`（W7 块）。
> **调度参考**：`docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md`（§2 归属 / §4 门禁 / §5 核验 / §10 上版续作）。
> 本文件 = 阶段2 的范围冻结 + 编排 agent 提示词 + git 门控协议。

---

## 0. 一句话结论

Wave 0 / Wave 1 已交付并由上一轮维护者独立验收；**阶段2 = Wave 2（W7 幅度下发通道整修）**，连同 Wave 1 转来的 **4 条欠账 A/B/C/D**、**flaky 定名**、**TTS 端到端补证**。
**W9 / W10 / W11 从「下一波」改为「暂缓待重排」**：2026-09-22 的表演层 v0（`[performance]`）已改变「表演由谁驱动」的主路由（`performance-layer-v0.md` §6/§7：表演层 = 主路由，director `staging_*` = 遗留回退），W9「单一驱动者」的目标部分已被其门控实现，需维护者重排后才放行。

---

## 1. 工作区事实（2026-09-22 20:34 实测）

| 项 | 事实 |
| --- | --- |
| 工作树 / 分支 | `/home/skystar/Live2D-Ai-l1`，`mod/l1-product`，HEAD `d2bdd4fd` |
| 未提交量 | `git status --porcelain` = **189** 条（137 modified + 52 untracked），全部 WIP |
| 冻结时间 | `find shell/flutter crates -newermt "2026-09-21 22:35"` = **0 命中** ⇒ 自上一轮交接后无人再改，W7 确实未开工 |
| 服务 | 在跑：pid 138，`./target/debug/live2d-ai-desktop --web --http-port 18080`，`GET /` → **302** |
| **TTS** | `GET http://127.0.0.1:8080/v1/models` → **200**（上一轮为 000）⇒ 端到端 PCM 证据**本次必须取证** |
| 前端产物 | `main.dart.js` = 2026-09-21 22:20:39（0 个 `.dart` 更新） |
| 渲染面产物 | `dist/index.html` = 2026-09-21 22:19:00（0 个 `.rs` 更新） |
| flutter | 不在非交互 PATH；真身 `/home/skystar/flutter/bin/flutter` |

### 1.1 治理发现（必须先处理）

这 189 条未提交横跨 **至少 4 个程序**：L1 产品化（五 Mod + 会话）、记忆摘要 P1-5、动作/表演回合 Wave 0–1、表演层 v0。**没有任何 checkpoint 提交**。
⇒ 无法把「W7 引入的改动」与「历史 WIP」在 diff 上分开。**git 门控必须先落一个基线 checkpoint**（见 §4）。

### 1.2 行号漂移（W7 块写于 2026-09-21，已漂移；一律按符号定位）

| 块内行号 | 当前实际 | 说明 |
| --- | --- | --- |
| `live2d_stage.dart:96` | `live2d_stage.dart:159` | `final Map<String,double>? actionScales;`（字段存在，但见下行） |
| `main.dart:709-717` | `main.dart:732` `Live2DStage(` | **构造点当前未传 actionScales** ⇒ 死参数成立 |
| `live2d_stage.dart:205-207` | `:268-269` | `didUpdateWidget` 比较 + 下发 |
| `live2d_stage.dart:287-294` | `~:355` | `_attach` 首帧/重建补发 |
| `shell_prefs.dart:99-114` | `:38 _scheduleActionScalesSync` / `:43 _syncActionScalesNow` / `:75` 直发 / `:92 force:true` | 产品路径 |
| `shell_settings.dart:254-261` | `:255` | 临时覆盖直发 |
| `live2d_bridge.dart:205` | 仍在 | 「钳 [0.2, 2.5]」→ 2.2（D④） |
| `dev_tools_section.dart:1596-1601` | 文件共 1792 行 | 按文案定位 |
| `preset_labels.dart:32` | 仍在 | `kExpressionPresetIds` 定义点 |

文件实测行数：`live2d_stage.dart` 629 / `main.dart` 880 / `shell_prefs.dart` 265 / `action_scales_sync.dart` 79 / `live2d_bridge.dart` 442 / `dev_tools_section.dart` 1792 / `director_panel.dart` 345 / `preset_labels.dart` 123。

---

## 2. 阶段2 范围（冻结）

### 2.1 必做

1. **W7 块**（`IMPL-PROMPTS` 的「复制给 Worker W7」整段，逐字分发给 worker）。
2. **编排补充 A①–D④ + ⑤–⑨**（Wave 1 转来的 4 条欠账，本轮必须结清）。
3. **flaky 定名**：20 × `--no-fail-fast`，与 W7 并行、不共享文件。
4. **TTS 端到端补证**：真实 PCM 帧 + `turn_state=completed`（此前因 8080 不在线未取证，现已在线）。

### 2.2 明确不做

- W9（单一驱动者）/ W10（动作协议字段化）/ W11（导演 AI 侧）——待维护者重排（缘由见 §0）。
- 未被点名文件、任何 git 写操作、修改 `RESEARCH` 文件。
- 不碰基座独占文件 `topics.rs` / `mod_registry.rs` / `supervisor.rs` / `desktop/src/main.rs`。

### 2.3 W7 文件归属（含 A–D 授权扩项）

`lib/main.dart`、`lib/live2d/live2d_stage.dart`、`lib/app/shell_prefs.dart`、`lib/live2d/action_scales_sync.dart`、`lib/app/shell_settings.dart`、`lib/settings/sections/dev_tools_section.dart`（仅文案段）、新建 `test/action_scales_wiring_test.dart`；
**A–D 追加授权**：`lib/settings/mods/director_panel.dart`（仅点名的三处文案 + 通道标签处）、`lib/live2d/live2d_bridge.dart`（**仅 :205 一行注释**）。

---

## 3. 阶段2 完成的定义（验收判据）

1. W7 四条必做 + A/B/C/D 四条 **闭环 diff**（逐条）。
2. `flutter analyze` 无问题 + `flutter test` 全过 + 前端产物重建**三证据**。
3. Rust 全量门禁绿（基线 + 收口两组，`--no-fail-fast` + tee 原始输出）。
4. 浏览器实测：临时幅度覆盖在换主题/调音量后**仍在**；「恢复产品设置」确实写下去。
5. flaky 20 次结果（未复现则给风险清单）。
6. TTS 端到端：`text_delta` + 真实 PCM + `turn_state=completed` + `error` 帧 0。
7. 报告四件套齐全（文件 / 测试名 / 门禁原始数字 / 未决问题），自述无原始输出者退回。

---

## 4. git 门控协议（管理者执行，非 worker）

| 门 | 时机 | 动作 |
| --- | --- | --- |
| **Gate 0** | 编排者交回「基线门禁全绿」证据后 | 在 `mod/l1-product` 落一个 **checkpoint 提交**，冻结 Wave 0/1 + 表演层等历史 WIP，使 W7 diff 可独立审阅（方案 A：单条 checkpoint；方案 B：按程序拆 3–4 条主题提交） |
| **Gate 1** | W7 交回 + 我独立核验通过后 | 选择性 `git add` W7 文件集合 → 提交 `fix(w7): 幅度下发通道收敛 + 欠账 A–D` |
| **Gate 2** | 阶段2 全部验收后 | 决定是否把 `mod/l1-product` 推进到 `main`（需用户点头；不 push / 不 tag / 不 bump，除非明确授权） |

worker 与编排者**一律禁止** git 写操作；他们的交付物是「可被直接 `git add` 的文件集合 + 原始门禁证据」。

---

## 5. 编排 agent 提示词（维护者直接粘贴给编排者）

```text
【阶段2 续作指令 · Wave 2 = W7 + 欠账 A–D + flaky 定名 + TTS 端到端补证】

你仍是本回合的**编排者**（不是执行者）。下面是 2026-09-22 的最新事实与放行范围，直接执行。
规格真源（只读，不得修改）：/home/skystar/Live2D-Ai-l1/docs/plans/RESEARCH-actions-director-audit-2026-09-21.md
任务真源：/home/skystar/Live2D-Ai-l1/docs/plans/IMPL-PROMPTS-actions-performance-round.md（W7 块 = 文件内「===== 复制给 Worker W7 =====」整段，逐字分发给 worker，不要转述）
调度参考：/home/skystar/Live2D-Ai-l1/docs/plans/ORCHESTRATOR-PROMPT-actions-performance-round.md（§2 文件归属 / §4 门禁 / §5 核验 / §10 上一版续作）
阶段计划：/home/skystar/Live2D-Ai-l1/docs/plans/STAGE2-WAVE2-plan-2026-09-22.md

【0. 工作区与红线】
- 工作区 = /home/skystar/Live2D-Ai-l1（git worktree，分支 mod/l1-product，HEAD d2bdd4fd，189 条未提交）。
- **绝对不要碰** /home/skystar/Live2D-Ai（另一个 worktree、另一条分支）。
- **git 由维护者门控**：你与所有 worker 一律禁止 git worktree/checkout/switch/stash/reset/commit/clean/add/tag/push。
  你的交付物是「可被维护者直接 git add 的文件集合 + 原始门禁证据」，不是提交。
- 禁止不带 --check 的 cargo fmt --all；禁止仓库级 dart format；不改 RESEARCH 文件；密钥/.env/请求体不进任何报告。
- 需要动基座独占文件（topics.rs / mod_registry.rs / supervisor.rs / desktop/src/main.rs）→ 停下问维护者。

【1. 开工基线（你自己做，先于一切 worker；红则立刻停并上报）】
1.1 先对当前 WIP 跑一遍全量门禁（--no-fail-fast + 2>&1 | tee 原始输出到 /tmp/stage2-baseline-*.log）：
     cargo test --workspace --all-targets
     cargo test --doc --workspace
     cargo fmt --all -- --check
     cargo clippy --workspace --all-targets -- -D warnings
     cargo run -p xtask -- rust-ratio          # 必须 >= 95%
     cd shell/flutter && flutter analyze && flutter test
     （flutter 不在非交互 PATH：export PATH="$PATH:/home/skystar/flutter/bin"）
     目的：把「W7 之前就存在的红」与「W7 引入的红」分开。基线红 → 停止，不派 worker，先回报。
1.2 记录：git status --porcelain | wc -l；main.dart.js / dist/index.html 的 mtime；改动文件的 mtime 归因。
1.3 环境事实（2026-09-22 20:34 实测，直接用，不必重新发现）：
     - 服务在跑：pid 138，./target/debug/live2d-ai-desktop --web --http-port 18080，GET / → 302。
     - **TTS 已在线**：GET http://127.0.0.1:8080/v1/models → 200。=> §8 端到端那一条本次**必须取证**。
     - 产物：main.dart.js 2026-09-21 22:20:39（0 个 .dart 更新）；dist/index.html 2026-09-21 22:19:00（0 个 .rs 更新）。
     - dev_mode 需要时内存期 PATCH 打开（不要写盘）。

【2. 本阶段范围（冻结）】
必做：(a) W7 块（逐字分发）；(b) 编排补充 A①–D④ + ⑤–⑨（Wave1 转来的 4 条欠账）；
      (c) flaky 定名（20 次，与 W7 并行、不共享文件）；(d) TTS 端到端补证。
不做（不得顺手）：W9/W10/W11；未被点名文件；任何 git 操作；不改 RESEARCH。
  说明：2026-09-22 表演层 v0（[performance]）已把「表演主路由」改成性能层（performance-layer-v0.md §6/§7），
  W9/W10/W11 需维护者重排后另行放行。

【3. 并发与文件所有权（铁律：禁止两个 worker 同时改同一文件）】
- W7 = 单个 worker，独占下面文件；不得再派人碰它们。
- flaky 探针**只跑命令、不改任何文件**，可与 W7 并行。
- W7 独占：shell/flutter/lib/main.dart、lib/live2d/live2d_stage.dart、lib/app/shell_prefs.dart、
  lib/live2d/action_scales_sync.dart、lib/app/shell_settings.dart、
  lib/settings/sections/dev_tools_section.dart（仅文案段）、新建 test/action_scales_wiring_test.dart。
- A–D 追加授权（只改点名行/段）：lib/settings/mods/director_panel.dart、lib/live2d/live2d_bridge.dart（仅 :205 一行注释）。

【4. 编排补充（除 W7 块原文外逐条追加）】
  A① 「叠加基础表情」开关文案写实：「打开后每次点手势都会把当前基础表情**重新起算**（2.6s 重新计时）」。
     不改 _applyGesture 语义、不动旧 Face→Gesture 回归（维护者已裁 (i)）。
  B② ttl 显示（live2d_stage.dart）与通道标签（director_panel.dart）改成**优先用标签表、取不到表才回落
     kExpressionPresetIds**；不删常量（preset_labels.dart 定义点已写明兜底理由）。贴两处改后 diff。
  C③ director_panel.dart 三处「遗留回退旁路 / 不是产品主路径」文案改成实情（表演层=主路由，staging=回退），
     译文案与 performance-layer-v0.md §7 一致。
  D④ 三处陈旧注释：live2d_stage.dart 的「0.75 / 1.4 / 1.0」（现 ~:157）、shell_prefs.dart:31（同）、
     live2d_bridge.dart:205「钳 [0.2, 2.5]」→ 2.2。**live2d_bridge.dart 只改这一行注释**。
  ⑤ W7 的改动仍限本任务范围，不要顺手扩大（尤其不要拆 dev_tools_section.dart）。
  ⑥ W7 改了 Dart => 收口时由你串行跑一次 flutter build web --release --base-href /app/ --no-web-resources-cdn
     并出三证据；W7 不碰 l2d-wasm-demo => 不需要 trunk build。
  ⑦ W7 与 W9 都碰 main.dart / live2d_stage.dart => 仍须串行（W9 留在 Wave 3）。
  ⑧ 报告逐条给 A/B/C/D 的**闭环 diff**（本轮必须结清）。
  ⑨ 不为 flaky 另开 worker；不为让测试变绿改测试期望。

【5. 行号漂移警告（重要）】
W7 块里的行号是 2026-09-21 快照，**已漂移**；一律按符号/文案定位，并在报告里附「块内行号 -> 当前实际行号」对照。实测：
  - live2d_stage.dart 共 629 行：actionScales 字段 :159（块写 :96）；didUpdateWidget :268；_attach 补发 ~:355；applyActionScales :476。
  - main.dart 共 880 行：构造点 Live2DStage( 在 :732（块写 :709-717），**当前未传 actionScales**（正是 W7 第 2 条要处置的）；_settings.addListener ~:371。
  - shell_prefs.dart 共 265 行：_scheduleActionScalesSync :38；_syncActionScalesNow :43；产品直发 :75；force 补发 :92。
  - shell_settings.dart：临时覆盖直发 :255。
  - dev_tools_section.dart 共 1792 行：按文案定位。
  - live2d_bridge.dart:205 的「[0.2, 2.5]」已确认仍在。

【6. flaky 定名（与 W7 并行）】
  for i in 1..20: cargo test -p live2d-ai-desktop --bin live2d-ai-desktop -- --test-threads=1 --no-fail-fast
    输出 2>&1 | tee /tmp/flaky-$i.log；同时另一终端跑 flutter test + flutter build web 制造并发窗口。
  * 禁止把输出过滤成只剩 test result: 行（上次就是这样丢掉用例名）。
  20/20 绿 => 记「未复现」+ 风险清单（点名 supervisor/tests_stall.rs、web_api/ws/connection.rs）；
  一旦复现 => **立刻停、Wave 2 暂停**，把失败用例名 + 原始输出交维护者。

【7. 收口门禁（你串行亲跑，全部 --no-fail-fast + tee 原始输出）】
Rust：cargo test --workspace --all-targets / --doc / fmt --check / clippy -D warnings / rust-ratio >= 95%
Flutter：analyze + test + flutter build web --release --base-href /app/ --no-web-resources-cdn
三证据：① main.dart.js mtime 晚于最后一个被改 .dart；② grep -c 'gstatic.com/flutter-canvaskit' build/web/main.dart.js == 0；
        ③ ./scripts/ignite.sh --check --port 18080 四项全 ok（服务留着别停）。
任一红 => 不进下一阶段；把原始输出发回负责人，修好重跑整组。

【8. 独立核验（不采信自述，逐条贴原始输出）】
1) W7 新建 test/action_scales_wiring_test.dart 的四条断言名 + 「改主题后临时值仍在」的断言原文；
2) 浏览器实测（用 ORCHESTRATOR §1.1 配方）：设明显偏离的临时值 -> 回「外观与互动」换主题/调音量 ->
   回 Developer 面板读值 + 渲染面 HUD scale_diag 行，证明临时值仍在；再点「恢复产品设置」证明产品值写下去（两条原始读数）；
3) 端到端（TTS 已在线，本次必做）：POST /api/v1/chat「下午好」+ 挂 /ws/state，确认 text_delta + **真实 PCM 音频帧**
   + turn_state=completed + error 帧 0；POST /api/v1/settings/test/llm 读 body 的 ok；
4) 产物 mtime 对照（.dart vs main.dart.js；无 Rust/wasm 改动则说明无需 trunk）；
5) flaky 20 次逐次 passed/failed 与日志路径。

【9. 报告格式（给维护者，他会转给审查方逐条复核）】
按 ① 基线门禁 ② W7 ③ A/B/C/D 闭环 diff ④ flaky ⑤ 端到端 ⑥ 未决/未取证 分节；
每节含：改动文件清单 / 新增或修改的测试名 / 每条门禁的实际输出数字 / 未决问题。
凡自述而无原始输出的结论一律退回。禁止「应该没问题」「大概通过」。

【10. 红线不变】
不碰 /home/skystar/Live2D-Ai；禁止一切 git 写操作（含 add/commit）；
不改 RESEARCH-actions-director-audit-2026-09-21.md（规格真源只读）；
需要动 topics.rs / mod_registry.rs / supervisor.rs / desktop/src/main.rs 先停下问维护者；
密钥 / .env 内容 / 请求体不得写进任何报告或日志。
```

---

## 6. 下一阶段预览（不在本轮放行，仅供维护者决策）

| 阶段 | 候选内容 | 前置 / 待决 |
| --- | --- | --- |
| 阶段3（= 原 W9） | 退役「拉 latest.preset_id」前端通道，统一到 action_cue | 大部分已被表演层 v0 的门控覆盖；需维护者确认是否还需 W9，或只保留「删死代码 + 文档」 |
| 阶段4（= 原 W10） | 动作协议字段化（body/head/expression 三字段 + 同类 add + hold + 事件回执 + 音频时钟） | 与表演层 v0 的 speak/cues schema 需合并成唯一契约；先重写 RESEARCH §10 的目标 |
| 阶段5（= 原 W11） | 导演 AI 侧输出 schema / 手册同源 / 只切不改字 / 两条件唤醒 / 会话 baseline | 表演层 v0 已实现 speak+cues；W11 需重定范围（合并而非重做） |
| 债务 | dev_tools_section.dart 1792 行拆分；WIP 的 git 历史整理 | 与 W7 冲突，W7 收口后单独排 |

---

## 7. 决策记录（维护者 2026-09-22 确认，Gate 0 执行依据）

| # | 决策 | 内容 |
| --- | --- | --- |
| 1 | 阶段2 定义 | 按**工程最优雅**取：阶段2 = **W7 + 欠账 A–D + flaky 定名 + TTS 端到端补证**（唯一未被表演层 v0 改写的独立缺陷面）。W9 判定为**被表演层 v0 门控取代**，不单独立项；W10/W11 的目标契约（三字段 / stage-clock）**并入** `performance-layer-v0.md` 的 `speak/cues` 契约后，作为阶段3+ 重新立项（先重写目标，再放行）。 |
| 2 | Gate 0 checkpoint | 方案 **B：按程序拆主题提交**（L1 产品化 / 记忆 P1-5 / 动作回合 Wave 0–1 / 表演层 v0），使 W7 diff 可独立审阅。 |
| 3 | 最终归属 | **保持 worktree 隔离**：只提交，不合并到 `main`，不 push / 不 tag / 不 bump。 |

### 7.1 方案 B 的执行口径（诚实前置）

189 条未提交横跨 4 个程序，`main.dart` / `settings/view.rs` / `mod_registry.rs` 等**被多个程序先后改过**，按文件无法完全切干净。Gate 0 按以下顺序执行：

1. 以**文件归属为主**做主题分组（能干净归组的先切）；
2. 交叉文件（多程序共改）**归入其最后一次被改动的程序**，并在 commit body 里写明「本条含 N 个前序程序的连带改动」；
3. 若某条主题混入过多，**降级为合并提交**，不为「凑主题」做逐 hunk 拆分（那是把可读性换成历史美观，不划算）；
4. **提交前把拟定的分组清单报给维护者确认**，通过后再落。

### 7.2 收到 worker 回报后的固定动作

1. 核对基线门禁原始输出 → 全绿则执行 Gate 0（方案 B，先报分组清单）。
2. 核对 W7 收口门禁 + A/B/C/D 闭环 diff + 三条新测试 + 产物 mtime。
3. 独立复核：亲自跑一次关键门禁；必要时无头浏览器复现「临时覆盖不被冲掉」。
4. 通过 → Gate 1 提交 W7；不通过 → 退回并给出具体失败点。

---

## 8. 暂停处置裁决（维护者独立核验后，2026-09-22）

### 8.1 核验结论（我亲跑/亲读，不采信自述）

| 项 | 结论 |
| --- | --- |
| flaky 根因 | **确认，且是测试端缺陷**：`tests_ws.rs:131-142` 的 `do_client_handshake` 读到 `\r\n\r\n` 后把整个 head 缓冲**丢弃**；`ws/connection.rs:134-139` 在 101 之后**立即**写 subscribe_ack，TCP 合并时被一并丢掉 → 只能读到 10s 后的 heartbeat。产品协议未破。 |
| W7 代码在盘 | 已落盘且**逻辑正确**：我跑 `flutter test test/action_scales_wiring_test.dart` = **+8 All tests passed**；`onClearScales` 全链路接通（shell_settings → dev_tools_section → 按钮）。 |
| W7 是否可收口 | **否**：`flutter analyze` = **4 issues（红）**（新测试文件 :24 未用 `dart:async` import；:147/:153 三处 `unnecessary_underscores`）。报告未跑 analyze，故未发现。⇒ 当前树**不可 checkpoint**。 |
| W7 是否碰 Rust | 否（`find crates -newermt "2026-09-22 20:45"` = 空）⇒ 无需 trunk build。 |
| RESEARCH | 未改（mtime 仍 2026-09-21 22:34:15）。 |
| 上一版提示词的缺陷 | 我给的 flaky 命令把 `--no-fail-fast` 放在 `--` **之后** → libtest 拒绝、20/20 假红。正确：`--no-fail-fast` 属 **cargo** 参数，必须在 `--` **之前**。已勘误。 |

### 8.2 裁决：并行推进，合并收口

1. **W7 收口 worker（Dart）**：清 4 个 analyze 问题 → 复核 A①–D④ 闭环 → 全量 Flutter 门禁 + 构建三证据 + 浏览器实测。
2. **flaky 修复 worker（Rust 测试端）**：保留握手残余字节（或换 `tungstenite::client`），修全部同类调用点，补**确定性回归**，50 次全量 bin 验证。
3. 两者**文件不相交**（Dart vs `tests_ws.rs`），可并行；**合并收口门禁在两方都 settle 后由编排者串行跑一次**（含 Rust 全量），不得边跑边改。

### 8.3 Gate 0 口径修订（重要）

W7 的改动与历史 WIP 在 7 个文件里**同文件交织**，且**没有 pre-W7 快照**（W7 开工前未备份）⇒ 方案 B 无法把 W7 完全剥离。执行口径：
- 主题提交「动作/表演回合（Wave 0–2）」**并入 W7 的 diff**，commit body 注明「本条含 Wave 2（W7）」；
- 纯 W7 新建的 `test/action_scales_wiring_test.dart` 可单独成条；
- **落提交前把分组清单报维护者确认**（沿用 §7.1.4）。
