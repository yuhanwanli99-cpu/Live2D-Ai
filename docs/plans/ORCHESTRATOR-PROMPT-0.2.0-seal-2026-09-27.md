# 调度提示词（Orchestrator）：0.2.0 封口四阶段（rc.5 → rc.6 → rc.7 → 0.2.0 末版）

> 本文是**编排者**（你）的执行手册。实现提示词在 docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md，
> 规划与证据在 docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md（**开工前必读**）。
> 审计原始复现：shell/flutter/test/zz_audit_tmp_test.dart（15 条复现，全部当前通过 = 缺陷稳定可复现）。

---

## 0. 你的角色与目标

维护者 2026-09-27 口径：
1. **0.2.0 线继续开 rc.x**，最后收在 **0.2.0 末版**；
2. **导演层接线的整套 Live2D 表演 + TTS 过滤 = 本版本重要资产**，必须守护；
3. **背景图片透传**以 shalldie/vscode-background @ eef5ddb（v3.1.0）为短期目标；
4. 然后**清理技术债 + 项目管理**；最后发布 0.2.0 末版。

你的职责：**只调度、只做 git 破坏性操作、只做独立核验**。业务代码由 worker 改；
你不要替 worker 写实现（否则没人做独立核验）。

---

## 1. 环境事实（先读，别踩）

| 事实 | 值 |
|---|---|
| 规划 worktree | /home/skystar/Live2D-Ai（分支 mod/persona-polish @ 4e421993，**2026-09-14 旧基线**） |
| 它和 main 的关系 | **是 main 的祖先**：落后 84 提交、不领先任何提交 |
| 0.2.0 线 HEAD | main = e4f139a8 **v0.2.0-rc.4**（2026-09-26） |
| origin/main | 2d492447 v0.2.0-rc.1（09-14）——本地 main 领先远端 85 个未推提交 |
| crates 差距 | main 比本 worktree 多 150 文件 / +39908 行 |
| 未提交工作区（本 worktree） | 133 项；113 files +5349/-2055；**crates/ 零改动** |
| 与 main 的 Flutter 重叠 | **29** 个文件 |
| 门禁现状 | flutter analyze 0 issue；flutter test **970 通过**（含 15 条临时 = 真实 955） |
| 仓库卫生 | **24** 个 worktree、**1** 个 stash、docs/releases/v0.3.0.md 与 rc.4 版本线矛盾 |

**最容易踩的三个坑**：
1. **不要在本 worktree 直接开干**——它是旧基线，且这份未提交重设计是唯一副本。
2. **不要用 git checkout/stash/reset 搬重设计**——会丢工作区。用 patch/复制（见 §2 开工前）。
3. **重放会覆盖资产接线**（main.dart 的 _applyDirectorCueForSeq、live2d_stage.dart 的 applyPreset 等）——
   所以 A1 必须先架守护断言。

---

## 2. 开工前（你亲自做，不派给 worker）

~~~~
1) 建新 worktree：
   cd /home/skystar/Live2D-Ai
   git worktree add /home/skystar/Live2D-Ai-fe main -b feat/frontend-redesign

2) **保档重设计（关键：未跟踪文件不在 git diff 里，必须单独抓）**：
   cd /home/skystar/Live2D-Ai
   git diff > /tmp/redesign-tracked.patch
   mkdir -p /tmp/redesign-untracked
   git status --porcelain | awk '$1=="??"{print $2}' \
     | grep -E '^(shell/flutter|docs/)' \
     | while read p; do mkdir -p "/tmp/redesign-untracked/$(dirname "$p")"; cp -r "$p" "/tmp/redesign-untracked/$p"; done
   # 自检：/tmp/redesign-untracked 下应至少有 lib/data/、background_item.dart、background_logic.dart、
   #       background_patterns.dart、group_card.dart、shell_slideshow.dart 与 6 份未跟踪文档

3) 把 15 条复现也保档（zz_audit_tmp_test.dart 在 git status 里是 ??，会被第 2 步一起抓走；确认它在了）。
4) 记录 main 的基线数字：在 /home/skystar/Live2D-Ai-fe 上跑一遍
   cargo test --workspace --all-targets / --doc / fmt --check / clippy -D warnings / xtask rust-ratio
   与 cd shell/flutter && flutter analyze && flutter test —— **这是 Stage A 的对照基线**。
~~~~

---

## 3. 波次编排（并发铁律：文件零重叠才可并发）

### Stage A · 0.2.0-rc.5

| 波次 | 任务 | 并发性 |
|---|---|---|
| A0 | §2 开工前（你） | 单独 |
| A1 | 资产守护网（新文档 + 新测试文件） | **必须先于 A2** |
| A2 | 重放 + 解 29 冲突 | **独占，绝不可并发** |
| A3a / A3b / A3c | P0 修复 / 死代码与文档 / 复现转正 | 文件已切分，可 3 路并行（测试文件若重叠，先让 A3a、A3b 收口） |
| A4 | rc.5 门禁 + 肉眼 + 发布说明（你或收口 worker） | 单独 |

**冲突热点（A2 放行前确认没有别人在改）**：shell/flutter/lib/main.dart、live2d/**、
settings/display_prefs.dart、settings/sections/**、app/app_shell.dart、app/shell_prefs.dart。

### Stage B · 0.2.0-rc.6

| 波次 | 任务 | 并发性 |
|---|---|---|
| B0 | B4 偏离说明（docs-only） | 任意时刻可并行 |
| B1 | imageFit 四档 + tileSize | 先行 |
| B2 | 轮播索引 + 管理/预览分离 | **等 B1**（都碰 display_prefs.dart） |
| B3 | 逐图样式覆盖 | **等 B2** |

### Stage C · 0.2.0-rc.7

| 波次 | 任务 | 并发性 |
|---|---|---|
| C1 | 文档单一化（AGENTS.md / v0.3.0.md） | 可与 C2 并行（docs-only） |
| C2 | 仓库卫生（worktree / stash / 未跟踪文档） | **只有你能做**（破坏性 git） |
| C3 | appearance_section 拆分 + W4 裁决 | **等 C1** |

### Stage D · 0.2.0 末版

D1 发布（你执行，worker 只做校验）。

---

## 4. 子 worker 的调用规范

- 每个 worker 一块提示词（IMPL-PROMPTS 里整块粘贴），**不要把多块塞给同一个 worker**。
- 每个 worker 开工前必须读规划里对应那一节；提示词里已写明，你只需确认。
- worker **不得**做 git 破坏性操作；需要建/删 worktree、打 tag 时**回报给你**，由你做。
- worker 回报必须含：改动文件清单 / 测试名 / **门禁原始数字** / 未决问题。缺任一项就打回。
- 同一波次不要在同一棵工作树上并发改同一个文件；每个任务块末尾都写了【文件归属】。

---

## 5. 集成与门禁（每个波次收口时你亲自跑，串行）

~~~~
cd /home/skystar/Live2D-Ai-fe/shell/flutter
export PATH="$HOME/flutter/bin:$PATH"
flutter analyze                          # 必须 0 issue
flutter test                             # 必须全绿；计数 ≥955 且不含 zz_
flutter build web --release --base-href /app/ --no-web-resources-cdn
cd /home/skystar/Live2D-Ai-fe && ./scripts/ignite.sh --check    # 四项全 ok

# 只有碰过 Rust 才跑（本路线预期不碰 crates/**）：
cargo test --workspace --all-targets
cargo test --doc --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p xtask -- rust-ratio         # ≥95%
~~~~

**产物新鲜度**（本项目踩过两次）：
- 有 .dart 改动 → build/web 必须重建，且 main.dart.js 的 mtime **晚于**最后一个被改的 .dart；
- 有 crates/l2d-wasm-demo/** 改动 → trunk build，且 dist/ 的 mtime 晚于最后一个被改的 .rs。

---

## 6. 独立核验（不采信自述）

每个阶段收口时逐条自测并贴原始输出：
1. **资产守护断言**：A1 的新测试全绿；且**故意**注释掉 main.dart 里一处 _applyDirectorCueForSeq 调用，
   确认断言**变红**（证明它不是空转），再还原。**这条是整轮最重要的核验。**
2. **P0 回归**：数据丢失那条——模拟 store 读失败，确认 prefs **原样**、retainOnly **未被调用**；
   拖动那条——[A,B,C] 向下拖一格得到 [B,A,C]。
3. **测试计数**：确认 zz_audit_tmp_test.dart 已被删除且计数没有掉。
4. **grep 死代码**：kShellSurfaceAlpha 全仓零引用。
5. **肉眼**：Win 浏览器按 A4 的 checklist 走一遍（背景刷新仍在 / 动作 / 语音 / 口型 / 导演可观测）。

---

## 7. 失败与边界

- **撞到未裁决项就停**：表演层「说话权归属」（V12/Q1）**不得自行裁决**，写清问题交维护者。
- **两行意图不兼容**（重设计 vs main 的新能力）→ 停，记录，不要发明第三种语义。
- **门禁红** → 不许把红说成「已知问题」蒙过去；修或明确记录为阻塞项。
- **任何需要改后端才能完成的背景能力**（舞台分区背景、舞台模糊、正文帧带 sentence_seq）→ **只记录**，
  写成未来工作，不要在本轮实现。

---

## 8. 最终交付（给维护者）

1. 四个 rc 的交付报告：改了什么 / 门禁数字 / 肉眼结论 / 未决问题。
2. 资产守护断言的**红-绿双向**演示结果。
3. 仓库卫生前后对比（worktree 数、stash、未跟踪文档）。
4. 0.2.0 末版的 release note + tag + 推送结果。

---

## 9. 启动提示词（维护者直接粘贴给编排者的那一段）

~~~~
你是本轮的**编排者**（Orchestrator），不要自己写业务实现。
必读（按顺序）：
  1) docs/plans/PLAN-0.2.0-seal-and-cleanup-2026-09-27.md
  2) docs/plans/IMPL-PROMPTS-0.2.0-seal-2026-09-27.md
  3) docs/plans/ORCHESTRATOR-PROMPT-0.2.0-seal-2026-09-27.md（本文）
背景一句话：0.2.0 线（main = v0.2.0-rc.4）要继续开 rc.x，先守护「导演层接线的 Live2D 表演 + TTS 过滤」资产，
把 09-27 的前端重设计（长在 09-14 旧基线、112 个文件未提交）重放到 0.2.0 线上，再做背景透传追平
shalldie/vscode-background，然后清技术债，最后发 0.2.0 末版。
第一步（你亲自做，不要派）：按本文 §2 建新 worktree + 保档重设计 patch 与未跟踪文件 + 记录 main 基线门禁数字。
第二步：派 A1（资产守护网）。A1 收口**之后**才派 A2（重放，独占，不可并发）。
第三步：A2 收口后，A3a/A3b/A3c 三路并行（注意测试文件顺序）。
每个波次收口你亲自跑门禁并做 §6 的独立核验（尤其「故意弄红」那条）。
硬约束：worker 禁止 git 破坏性操作；源码 ≤500 行；禁止仓库级 dart format。
遇到未裁决项或两行意图不兼容 → 停下记录，不要自己发明语义。
~~~~
