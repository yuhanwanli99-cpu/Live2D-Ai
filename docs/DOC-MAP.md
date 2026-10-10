# DOC-MAP · 文档地图与生命周期

> 立档：**2026-10-01** · 状态：**活** · 真源：本文件
> 用途：回答两件事 —— **「这个问题看哪份文档」** 与 **「这份文档还活着吗」**。
> 真源优先级：**源码 > AGENTS.md > docs/architecture/* > docs/plans/*（活）> docs/legacy/*（历史，仅留证）**。

## 1. 真源地图（按问题查）

| 我想知道 | 看这份 | 备注 |
|---|---|---|
| 项目定位 / 分层 / 红线 / 门禁 | `AGENTS.md`（仓库根，唯一真源） | 工作树只有 `/home/skystar/Live2D-Ai` |
| 核心契约 / 目录约定 | `docs/architecture/core-contracts.md`、`directory.md` | |
| 主链逐环与出处 | `docs/architecture/core-chain-baseline.md` | |
| 表演协议 v1（动作 / cue / preset） | `docs/architecture/performance-protocol-v1.md` | 唯一真源 |
| AI-Vtuber 对照（导演层讨论） | `docs/research/调研结果-AI-Vtuber对照-2026-10-07.md` | 2026-10-07；只借鉴行为，不复制 GPL 源码 |
| 导演层同类对照（N.E.K.O. + 12 项目） | `docs/research/调研结果-导演层同类对照-2026-10-07.md` | 2026-10-07；N.E.K.O. 快照 `a3c82b5a`；判据/证据纪律见文内 |
| 外部输入（弹幕 / 礼物） | `docs/external-input.md` | B 站抓取在 Win sidecar，不在主仓 |
| 语音转写 | `docs/voice-input.md` | |
| Mod 产品链路 / 许可 | `docs/architecture/mod-product-chain.md`、`mod-community-license.md` | |
| 已**删除** Mod（local-llm / wallpaper / pet-desktop） | `docs/architecture/ARCHIVED-mods.md` | §1.4/§2.4/§3.1 删除记录（W2-A，2026-10-01）；**禁止挂回** |
| TTS 是核心（不是 Mod） | `docs/architecture/tts-is-core.md` | |
| 渲染算法 / 纹理档位 | `docs/architecture/renderer-*.md`、`mask-*.md` | |
| 逐版发布说明 | **`docs/releases/v0.2.4-rc.1.md`（当前）**、`v0.2.3-rc.2.md`、`v0.2.3-rc.1.md`、`v0.2.2.md`、`v0.2.1-rc.1.md`、`docs/releases/v0.2.0*.md`（更早） | 版本四处以源码为准 |
| **当前执行计划** | `docs/plans/TASKS-2026-10-07.md` | 去臃肿计划已做完（本文同级） |
| **D1 休眠资产裁决 + 红线修订（W2）** | `docs/audit/2026-10-01-debloat/W2/D1-IMPACT-BRIEF.md` | 删/移边界、5 处主链触点、**红线修订 1/2**；`W2-A` = `d140604f`、`W2-B` = `03765bd3`（**两段均已执行**，见下 §3「结构性变更」） |
| **0.2.0 阶段报告（欠账 / 事故 / 口径更正）** | `docs/audit/2026-10-01-debloat/PHASE-REPORT-0.2.0.md` | 与发布说明互指；**已入库**（`e8da68c0`，与发布说明同一次提交） |
| 接手快照 | `docs/plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md` | |
| 前端审计账本（封口） | `docs/audit/2026-09-28-frontend-nightly/` | 45 条为准 |
| 全库审计账本 | `docs/audit/2026-10-05-ledger/`（**已入库**，2026-10-06） | 旧 `AUDIT-REPO/`（未跟踪）已并入此目录（1,014 份 `.md`）；树外参考副本 `/home/skystar/audit-ref-2026-10-06/`（不再运行） |
| 依赖 / 许可 / 出处 | `docs/architecture/dependencies.md`、`SOURCES.md`、`CREDITS.md` | |
| 部署 | `docs/headless-deploy.md` | |
| 历史（Python / Android 时代 + 94 份已收口计划） | **已移出工作树**（2026-10-06 E8 文档减量）——取回见 [REMOVED-docs-index-2026-10-06.md](REMOVED-docs-index-2026-10-06.md) | **勿当现网**；git 历史逐字保留 |

## 2. 生命周期三分（不是二分）

1. **活**：被引用、结论未过时 → 留在原处，本文件 `1 可查到。
2. **历史留证**：**移出工作树**（`git rm`，2026-10-06 起的口径）——证据在 git 历史里，
   逐份登记进 [REMOVED-docs-index-2026-10-06.md](REMOVED-docs-index-2026-10-06.md)。
   **不再使用「搬进 `docs/legacy/`」**：搬运不减行数（见 §3），只把「活 / 历史混放」换成另一种混放。
3. **保留正文 + 作废清单**：正文一行不删，顶部一节逐条标「已被推翻」（现行例：`architecture/director-rfc.md`）。

**新增文档必须先归入三类之一**；不允许「无人引用、结论过期、却留在原地」的第四态。

## 3. 现状盘点（**2026-10-06 E8 之后**实测）

**测得时点 = 2026-10-06，工作树 @ `main`（E8 落盘 + 本轮交接文件）**（下次复算请重跑下面命令并更新此时间戳）
**口径与复算命令**（「.md」只数文件；「行数」= 把命中文件全部 `cat` 后的行数；含未跟踪）：

```bash
find docs -name '*.md' -type f -exec cat {} + | wc -l                              # docs 合计（含 audit）
find docs -name '*.md' -type f -not -path 'docs/audit/*' -exec cat {} + | wc -l    # docs 合计（**判 D5 以这条为准**）
#   ⚠ 口径差：`find|cat|wc -l` 数的是**换行符**，比 xtask 的 `physical_lines`（末尾残行计 1）少 6 行
#     ——**预算判据一律以 `code-stats --only docs` 的 xtask 值为准**（本表 D5 三行即该口径）
find docs/audit -name '*.md' -type f -exec cat {} + | wc -l                        # audit 自身
cargo run -p xtask -- code-stats --check                                          # 机器判据（含 docs 预算行）
ls docs/plans/*.md | wc -l                                                        # plans 顶层份数（判据 ≤25）
```

| 目录 | .md（盘上） | 行数 | 备注 |
|---|---:|---:|---|
| `docs/architecture` | 35 | 10,517 | 契约真源 |
| `docs/plans` | 39 | 8,539 | **顶层 22（判据 ≤25 ✓）** + `parallel-mods/` 17 |
| `docs/research` | 23 | 8,937 | 调研（结论仍被引用，本轮未动） |
| `docs/verification` | 13 | 3,060 | E9 证据目录 + 门禁基线 |
| `docs/releases` | 16 | 2,679 | |
| `docs/design` | 2 | 2,600 | 旧 JS 规格已随 E8 移出 |
| `docs/examples` | 2 | 465 | |
| `docs/development` | 2 | 124 | |
| `docs/legal` | 3 | 45 | |
| `docs/screenshots` | 0 | 0 | |
| `docs/` 顶层 `*.md` | 8 | 2,396 | 含**本文件** + `REMOVED-docs-index-2026-10-06.md` |
| `docs/audit` | 1,014 | 75,154 | **审计过程产物，只增不减**（本轮两次快照：75,148 → 75,154） |

**D5 口径（必读）**：`docs/audit/**` 是过程产物、入库后仍在增长，因此

| 口径 | 行数 | 判据 |
|---|---:|---|
| docs 全量（**含** `docs/audit/**`） | **114,523** | 只作快照披露（同上：会随新文档上升） |
| docs 全量（**不含** `docs/audit/**`） | **39,385** | **PLAN §5 的 `≤45,000` 以这条为准 → PASS（余量 5,615）**（下同：数字随新文档上升，**以 `--only docs` 实时值为准**） |
| 其中 `docs/audit/**` 自身 | **75,154** | 不计入预算 |

> **两个口径差 6 行（别当成矛盾）**：　`find | cat | wc -l`（本文件 §3 的目录行）数**换行符**；xtask `physical_lines`（**预算判据**）把**无末尾换行的残行也算 1 行**。
> 两者差额 = **无末尾换行的 `.md` 文件数**（本轮实测 6）——所以 `find` 口径会比 xtask 口径**少几个行**，这是**口径差**，不是数据不一致。
> **判 D5 只认 xtask 口径**（`cargo run -p xtask -- code-stats --check --only docs`）。

> **归档不减少 docs 总行数**（在 `docs/` 内搬运 + 每份 +2 行 banner）⇒ 减量只能来自**移出工作树**。
> 2026-10-01 那轮「94 份计划 → `docs/legacy/plans/`」把顶层压到 24，但**总行数一行没减**；
> 2026-10-06 E8 把只具历史留证价值的 **139 份 / 31,509 行**移出工作树，
> 才真正把「不含 audit」从 **70,762 → 39,273**（移出 31,509 行；其余为活文档改写净额 **+20**）。
> **该 39,273 是 E8 落盘提交 `7918974` 的值**；之后每加一份活文档都会让它上升（本轮交接文件 +111 行即为一例）。

**E8 移出清单与取回**：[REMOVED-docs-index-2026-10-06.md](REMOVED-docs-index-2026-10-06.md)
（`docs/legacy/**` 108 份、`docs/design/legacy/**` 9 份、2026-08「节点 C」/原生壳验证 10 份、
已收口计划 12 份）。**活文档已改链，历史文档刻意不改链**（改历史引用 = 篡改记录）。

**六个文档级问题**（2026-10-01 立档；下为 2026-10-06 状态）：
1. ~~`docs/plans/` 115 份里 94 份是历史候选，与活文档混放~~ → **已收口**（2026-10-01 归档 → 2026-10-06 移出）；
2. ~~**AGENTS.md 两份并存**~~ → **已收口**：唯一真源 = 本工作树 `AGENTS.md`；
3. ~~`CHANGELOG.md` 停更却仍被当版本线~~ → **已收口**：顶部停更头注写明版本真源 = `docs/releases/*.md` + 代码版本三处；
4. ~~`docs/releases/v0.3.0.md` 是未发布草案~~ → **已标作废**；
5. ~~`AUDIT-REPO/`（未跟踪）~~ → **已收口**：已入库为 `docs/audit/2026-10-05-ledger/`（1,014 份）；
6. ~~全仓 111 个 `.md` 引用 `docs/plans/`，移动必须同步改链~~ → **已收口**：2026-10-01 改链 209 处/74 文件；
   2026-10-06 E8 改的是**活文档层**，历史层按 §4 规则保留原样。

## 4. 归档规则 / 保留清单（**2026-10-06 E8 起**）

**规则**：
- 结论被现行实现推翻 → 第 3 类（保留正文 + 顶部作废节）；
- 任务已收口且不再被引用 → **移出工作树**（`git rm` + 登记进 REMOVED 索引）；
- 同主题有更新版本 → 旧版**移出工作树**，新版留在 `docs/plans/`；
- 只具历史留证价值 → **移出工作树**（不搬 `docs/legacy/`——搬运不减行数）。

**「移出」的四步**（缺一步就是脏删）：
1. `git rm`（被删文件必须在 git 历史里逐字可取回——**未提交过的文件不许删**）；
2. 登记进 [REMOVED-docs-index-2026-10-06.md](REMOVED-docs-index-2026-10-06.md)：路径 / 文件数 / 行数 / 理由 / 取回命令；
3. **活文档改链**（`README.md` · 本文件 · `AGENTS.md` · `architecture/*` · 根文档 · `crates/*/README.md`）；
4. **历史文档不改链**（`CHANGELOG.md` · `releases/*` · 已收口 `plans/*` · `research/*` · `verification/*`）——
   保留原样是**刻意**的：死链 = 「这份文档写于移出之前」的信号。

**保留在 `docs/plans/` 的 22 份（2026-10-06 实测）**：
`AUDIT-PROMPT-whole-repo-2026-10-06` · `AUDIT-PROMPT-whole-repo-2026-10-06-B` · `DECISION-artifact-budget-2026-10-06` ·
`DECISION-display-prefs-2026-10-06` · `HANDOFF-2026-09-28-rc7-and-whole-repo-audit` ·
`HANDOFF-2026-10-06-e1-e5-debt-round` · `HANDOFF-2026-10-06-team-round` · `HANDOFF-2026-10-06-e8-docs-and-guards-round` · `HANDOFF-2026-10-06-team-round-2` ·
`IMPL-PROMPTS-actions-performance-round` · `IMPL-PROMPTS-debloat-round-2026-10-01` ·
`NEXT-ROUND-main-2026-10-06` · `ORCHESTRATOR-PROMPT-actions-performance-round` ·
`ORCHESTRATOR-PROMPT-debloat-round-2026-10-01` · `PLAN-actions-voice-memory-2026-09-15` ·
`PLAN-debloat-and-closeout-2026-10-01` · `PLAN-frontend-redesign-2026-09-27` ·
`PLAN-visual-substance-2026-09-27` · `PROMPT-frontend-nightly-audit-2026-09-27` ·
`RESEARCH-actions-director-audit-2026-09-21` · `TRIAGE-0.2.0-audit-45-2026-09-28` · `future-roadmap-2026-09`

**2026-10-01 那轮归档的历史记录**（保留作证据；命令已不再可用，因为 `docs/legacy/` 已移出）：
94 份 `git mv` + 209 处改链；还原点 tag `checkpoint/pre-doc-archive` = `dde6b855`、
归档完成点 `checkpoint/docs-archive-done` = `4285af8b`。**回滚方式已改为**
`git checkout 42c5825 -- docs/legacy`（取回整目录）。中间态 `b9eff54e` 缺
`docs/README.md` 的链接修正，**不要**当回滚点。

## 5. 命名与新增规范

- 计划 `PLAN-<主题>-<YYYY-MM-DD>.md`；调度 `ORCHESTRATOR-PROMPT-<主题>-<日期>.md`；
  提示词 `IMPL-PROMPTS-…`；交接 `HANDOFF-<日期>-<主题>.md`；审计 `docs/audit/<日期>-<主题>/`；发布 `docs/releases/v<版本>.md`。
- 每份新文档头部必须有：**立档日期 + 状态（活 / 历史 / 作废）+ 真源指向**。
- 一份文档只讲一个主题；超过 300 行就拆。

## 6. 维护节奏

- **每次发布**：release note 里勾一次「本版新增 / 作废 / 归档的文档」；
- **每 3 个 rc**：重跑本文 §3 的体量统计；`docs/plans` 超过 25 份即按 §4 强制移出一轮；
- **每次移出文档**：登记 REMOVED 索引 + 改活文档链 + 跑 `cargo run -p xtask -- code-stats --check`
  （docs 预算行是**机器判据**，不再靠人眼看）。
