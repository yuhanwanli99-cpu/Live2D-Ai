# DOC-MAP · 文档地图与生命周期

> 立档：**2026-10-01** · 状态：**活** · 真源：本文件
> 用途：回答两件事 —— **「这个问题看哪份文档」** 与 **「这份文档还活着吗」**。
> 真源优先级：**源码 > AGENTS.md > docs/architecture/* > docs/plans/*（活）> docs/legacy/*（历史，仅留证）**。

## 1. 真源地图（按问题查）

| 我想知道 | 看这份 | 备注 |
|---|---|---|
| 项目定位 / 分层 / 红线 / 门禁 | `AGENTS.md`（**-fe 版**） | ⚠ 有两份，旧份在 `-Ai` worktree（说「动作系统已拆除」），**不要用** |
| 核心契约 / 目录约定 | `docs/architecture/core-contracts.md`、`directory.md` | |
| 主链逐环与出处 | `docs/architecture/core-chain-baseline.md` | |
| 表演协议 v1（动作 / cue / preset） | `docs/architecture/performance-protocol-v1.md` | 唯一真源 |
| 外部输入（弹幕 / 礼物） | `docs/external-input.md` | B 站抓取在 Win sidecar，不在主仓 |
| 语音转写 | `docs/voice-input.md` | |
| Mod 产品链路 / 许可 | `docs/architecture/mod-product-chain.md`、`mod-community-license.md` | |
| 已**删除** Mod（local-llm / wallpaper / pet-desktop） | `docs/architecture/ARCHIVED-mods.md` | §1.4/§2.4/§3.1 删除记录（W2-A，2026-10-01）；**禁止挂回** |
| TTS 是核心（不是 Mod） | `docs/architecture/tts-is-core.md` | |
| 渲染算法 / 纹理档位 | `docs/architecture/renderer-*.md`、`mask-*.md` | |
| 逐版发布说明 | **`docs/releases/v0.2.0.md`（当前）**、`docs/releases/v0.2.0-rc.N.md`（历史） | 版本四处以源码为准 |
| **当前执行计划（现在 + 未来）** | `docs/plans/PLAN-debloat-and-closeout-2026-10-01.md` | 本文同级 |
| **D1 休眠资产裁决 + 红线修订（W2）** | `docs/audit/2026-10-01-debloat/W2/D1-IMPACT-BRIEF.md` | 删/移边界、5 处主链触点、**红线修订 1/2**；`W2-A` = `d140604f`、`W2-B` = `03765bd3`（**两段均已执行**，见下 §3「结构性变更」） |
| **0.2.0 阶段报告（欠账 / 事故 / 口径更正）** | `docs/audit/2026-10-01-debloat/PHASE-REPORT-0.2.0.md` | 与发布说明互指；**已入库**（`e8da68c0`，与发布说明同一次提交） |
| 接手快照 | `docs/plans/HANDOFF-2026-09-28-rc7-and-whole-repo-audit.md` | |
| 前端审计账本（封口） | `docs/audit/2026-09-28-frontend-nightly/` | 45 条为准 |
| 全库审计账本 | `docs/audit/2026-10-05-ledger/`（**已入库**，2026-10-06） | 旧 `AUDIT-REPO/`（未跟踪）已并入此目录（1,014 份 `.md`）；树外参考副本 `/home/skystar/audit-ref-2026-10-06/`（不再运行） |
| 依赖 / 许可 / 出处 | `docs/architecture/dependencies.md`、`SOURCES.md`、`CREDITS.md` | |
| 部署 | `docs/headless-deploy.md` | |
| 历史（Python / Android 时代） | `docs/legacy/` | **勿当现网** |

## 2. 生命周期三分（不是二分）

1. **活**：被引用、结论未过时 → 留在原处，本文件 `1 可查到。
2. **历史留证**：移入 `docs/legacy/`，顶部一行标「历史（<日期> 归档，勿当现网）」。
3. **保留正文 + 作废清单**：正文一行不删，顶部一节逐条标「已被推翻」（现行例：`architecture/director-rfc.md`）。

**新增文档必须先归入三类之一**；不允许「无人引用、结论过期、却留在原地」的第四态。

## 3. 现状盘点（2026-10-01 归档后实测）

**测得时点 = 2026-10-01 16:01 UTC，工作树 @ `389cb368`（task-9 之后）**（下次复算请重跑下面命令并更新此时间戳）
**口径与复算命令**（「.md」只数文件；「行数」= 把命中文件全部 `cat` 后的行数；**含未跟踪**）：

```bash
for d in $(find docs -mindepth 1 -maxdepth 1 -type d | sort); do
  printf '%-18s %6s %8s\n' "$d" \
    "$(find "$d" -name '*.md' -type f | wc -l)" \
    "$(find "$d" -name '*.md' -type f -exec cat {} + | wc -l)"
done
find docs -maxdepth 1 -name '*.md' -type f | wc -l          # docs 顶层
git ls-files -- 'docs/*.md' 'docs/**/*.md' | sort -u | wc -l # 跟踪数
ls docs/plans/*.md | wc -l                                   # plans 顶层（归档口径）
find docs -name '*.md' -type f -exec cat {} + | wc -l                            # docs 合计（含 audit）
find docs -name '*.md' -type f -not -path 'docs/audit/*' -exec cat {} + | wc -l   # docs 合计（不含 audit）
```

| 目录 | .md（盘上，含未跟踪） | 行数 | 备注 |
|---|---:|---:|---|
| `docs/plans` | 41 | 8,916 | **顶层 24**（活文档）+ `parallel-mods/` 17 |
| `docs/legacy` | 104 | 25,191 | 顶层 10 + **`plans/` 94（本波归档）** |
| `docs/architecture` | 32 | 9,983 | W2-A 后新增删除/休眠记录 |
| `docs/audit` | 55 | 6,897 | **审计过程产物，会持续增长**；含 09-28 封口账本 21 批 + `2026-10-01-debloat/`（W0/W1/W2 证据，**已全部入库**；复核者仍在写入 ⇒ 数值随时间增长） |
| `docs/research` | 23 | 8,937 | |
| `docs/verification` | — | — | `v0.2.0-checklist.md` **已于 2026-10-06 退役删除**（维护者确认）；新增 `evidence-2026-10-06/` 浏览器验收证据 |
| `docs/releases` | 15 | 2,535 | |
| `docs/design` | 8 | 3,302 | 含 `legacy/` 4 份旧 JS 规格 |
| `docs/legal` | 3 | 45 | |
| `docs/examples` | 2 | 465 | |
| `docs/development` | 2 | 124 | |
| `docs/screenshots` | 0 | 0 | |
| `docs/` 顶层 `*.md` | 7 | 2,388 | 含**本文件**——改本表会使其漂移，故须连时点一起读 |
| **docs 合计** | **312** | **73,479** | **tracked 312 + 未跟踪 0** |

**本轮结构性变更（影响文档口径，与 docs 行数无关但必须知道）**：
- **workspace members 16 → 13**：`live2d-ai-mod-local-llm`（DEPRECATED）、`live2d-ai-mod-wallpaper`（ARCHIVED）、
  `live2d-ai-mod-pet-desktop`（ARCHIVED）三个 crate 已**删除**（W2-A / D1 第一段，2026-10-01）；
  还原点 tag **`checkpoint/pre-d1-dormant` = `d140604f`**；删除记录与恢复步骤见
  `docs/architecture/ARCHIVED-mods.md` §1.4 / §2.4 / §3.1；裁决与红线修订真源见
  `docs/audit/2026-10-01-debloat/W2/D1-IMPACT-BRIEF.md`。
- 原生壳岛（`live2d-ai-desktop` 的 `app/`/`tray`/`repl`/`benchmark`/`model_smoke` + `backend/` + `adapter/`）已**移出**：
  **W2-B = `03765bd3`**（31 文件 ≈8,954 行；**`desktop` 依赖 34 → 25**；**休眠岛行数归零**；退出 **111 条**测试
  = 100 条 ⊆ 被移出目录 + 11 条测已删对象，leader 签署）。机械集合差：`cargo test --workspace --all-targets`
  **1457 → 1302**（主链行为覆盖一条未少）。台账（111 条全名 + 恢复条件）：
  `docs/architecture/ARCHIVED-native-shell.md`；两段合并的欠账/事故/口径更正：
  `docs/audit/2026-10-01-debloat/PHASE-REPORT-0.2.0.md`。

**D5 口径（必读）**：上表「docs 合计」**含 `docs/audit/**`**，而那是**审计过程产物**，只增不减
（实证：`docs/audit/2026-10-01-debloat/GROUNDING.md` 一夜 **84 → 333 行**）。所以 D5 的
「`docs` 69,733 → **≤45,000**」**必须同时给含 / 不含 `docs/audit/**` 两个数**，
否则目标会被过程产物稀释（或反过来被它掩盖真实减量）：

| 口径 | 行数 | 复算 |
|---|---:|---|
| docs 全量（**含** `docs/audit/**`） | **73,479** | `find docs -name '*.md' -type f -exec cat {} + \| wc -l` |
| docs 全量（**不含** `docs/audit/**`） | **66,582** | 上条加 `-not -path 'docs/audit/*'` |
| 其中 `docs/audit/**` 自身 | **6,897** | 两条相减 |

> **归档不减少 docs 总行数**（只在 `docs/` 内移动 + 每份 +2 行 banner + 改链），
> 所以 D5 的减量只能来自**真删 / 真合并**；归档只解决「活 / 历史混放」。
> **判 D5 请以「不含 `docs/audit/**`」那一行为准**：`docs/audit/**` 是过程产物，**入库后仍在增长**
> （本轮两次快照：6,712 → 6,897 行 = **+185**；同一现象见 `GROUNDING.md` 一夜 84 → 333 行）
> ⇒「含 audit」只是**快照**。

> 归档前基线（复核者可对照）：顶层 `docs/plans/*.md` = **118**（跟踪 113 + 未跟踪 5），
> docs 全量 `.md` = **303**（归档只在 `docs/` 内部移动，总量不变）。
> 归档前引用基线原文留在 `docs/audit/2026-10-01-debloat/W0/raw/pre_archive_*.txt`（**只读证据，勿改**）。

**归档结果（2026-10-01，本波）**：`docs/plans` 顶层 **118 − 94 = 24** → 达到 D5 目标（≤25）；
94 份全部落在 `docs/legacy/plans/`，每份顶部一行「历史（2026-10-01 归档，勿当现网）」。

**六个文档级问题**（2026-10-01 立档；下为 S0 收口后状态）：
1. ~~`docs/plans/` 115 份里 94 份是历史候选，与活文档混放~~ → **已收口**：94 份入 `docs/legacy/plans/`，顶层 **24**；
2. ~~**AGENTS.md 两份并存**（`-Ai` 旧版 vs `-fe`/main 新版）~~ → **已收口**：`-fe` 版为唯一真源，`-Ai` 副本在 `AGENTS.md` 首屏被标为过时历史副本；
3. ~~`CHANGELOG.md` 停更，却仍被当版本线之一~~ → **已收口**：顶部停更头注写明版本真源 = `docs/releases/*.md`（最新一份）+ 代码版本三处；
4. ~~`docs/releases/v0.3.0.md` 是未发布草案，与 0.2.0 线矛盾~~ → **已标作废**：顶部「历史草案，未发布」+ 收尾指向 `AGENTS.md` 首屏；
5. ~~`-fe` 的 `docs/README.md` 改动、4 份 2026-10-01 新文档、2 份 09-28 文档、`AUDIT-REPO/`（未跟踪）**仍未提交**~~ → **已提交（2026-10-01）**：归档 = `b9eff54e`，doc-chore = `4285af8b`，复核证据账本 = **`c1717ba5`**（`docs/audit/2026-10-01-debloat/` 的 W0/W1/W2 GROUNDING + raw，67 份），发布说明 + 阶段报告 + 文档指针/计数 = **`e8da68c0`**。**当前仍未跟踪的只剩一处**：`AUDIT-REPO/`（全库审计账本，**永不 `git add`**）。
   **复核注（2026-10-06 夜）**：该账本**已入库**为 `docs/audit/2026-10-05-ledger/`（1,014 份 `.md`，`docs/**/*.md` 总量因此到 1,279 份 / 143,270 行），工作树根**不再有**未跟踪的 `AUDIT-REPO/`；条目 5 的「只剩一处未跟踪」至此作废。新 run 的落盘目录仍用树根未跟踪的 `AUDIT-REPO/`，但**规程里写的 `-fe` 工作树路径已过期**（该 worktree 已删除，唯一工作树 = `/home/skystar/Live2D-Ai`）。
6. ~~全仓 111 个 `.md` 引用 `docs/plans/`，任何移动都必须同步改链~~ → **已改链**：94 个归档名共 **209 处**按各自文件位置改写为正确相对路径（其余补丁：出链重定位 42 ／ 可见文字 22 ／ 归档件内正文引用 78）；`docs/README.md` 相对链接 **105 条 missing=0**。


## 4. 归档规则 / 保留清单 / 候选

**规则**：
- 结论被现行实现推翻 → 第 3 类（保留正文 + 顶部作废节）；
- 任务已收口且不再被引用 → `docs/legacy/plans/`；
- 同主题有更新版本 → 旧版进 `docs/legacy/plans/`，新版留在 `docs/plans/`。

**保留在 `docs/plans/` 的 21 份（活）+ 本波新增 3 份 = 顶层 24 份**：
`AUDIT-PROMPT-whole-repo-2026-09-28` · `BACKLOG-0.2.0-closeout-2026-09-28` ·
`HANDOFF-2026-09-21-actions-performance-round` · `HANDOFF-2026-09-21-actions-performance-wave0-1` ·
`HANDOFF-2026-09-21-assistant-memory-presets-tts-clean` · `HANDOFF-2026-09-22-performance-layer` ·
`HANDOFF-2026-09-27-stageB-plan-and-status` · `HANDOFF-2026-09-28-rc7-and-whole-repo-audit` ·
`IMPL-PROMPTS-0.2.0-seal-2026-09-27` · `IMPL-PROMPTS-actions-performance-round` ·
`ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28` · `ORCHESTRATOR-PROMPT-0.2.0-seal-2026-09-27` ·
`ORCHESTRATOR-PROMPT-actions-performance-round` · `PLAN-0.2.0-seal-and-cleanup-2026-09-27` ·
`PLAN-actions-voice-memory-2026-09-15` · `PLAN-frontend-redesign-2026-09-27` ·
`PLAN-visual-substance-2026-09-27` · `PROMPT-frontend-nightly-audit-2026-09-27` ·
`RESEARCH-actions-director-audit-2026-09-21` · `TRIAGE-0.2.0-audit-45-2026-09-28` · `future-roadmap-2026-09`
（＋本波 `PLAN-debloat-and-closeout-2026-10-01` / `IMPL-PROMPTS-debloat-round-2026-10-01` /
`ORCHESTRATOR-PROMPT-debloat-round-2026-10-01`）

**归档候选 94 份**：清单 `docs/legacy/plans-archive-candidates-2026-10-01.txt`
（**已于 2026-10-01 执行完毕**，94 份全部在 `docs/legacy/plans/`）。

### 4.1 归档执行步骤（**2026-10-01 已执行**）

```bash
# 在 /home/skystar/Live2D-Ai-fe 上；先打还原点（tag 由 leader 打）
git tag checkpoint/pre-doc-archive
mkdir -p docs/legacy/plans
while read -r f; do
  case "$f" in '#'*|'') continue;; esac
  git mv "docs/plans/$f" "docs/legacy/plans/$f"
done < docs/legacy/plans-archive-candidates-2026-10-01.txt
# 改链：全仓 .md 里指向归档件的引用改写成「相对本文件位置」的正确路径
# 校验：grep -rn "docs/plans/<归档名>" docs/ AGENTS.md README.md CONTRIBUTING.md
#        → 0 命中；白名单 = 只读证据（docs/audit/<date>-*/**）、跨 worktree 绝对路径、非 .md 冻结件
```

**执行结果（2026-10-01）**：`docs/plans` 顶层 118 → **24**；94 份归档件各加顶部一行
「历史（2026-10-01 归档，勿当现网）」；改链 **209 处 / 74 个文件**；
`docs/README.md` 相对链接 **missing=0**（105 条，归档完成点上）。

**提交与 tag（下一轮 bisect / 判红前必读）**：
- `b9eff54e` = **归档中间态**（94 份 `git mv` + 改链，**不含** `docs/README.md` 的链接改写）
  → 单看这一刻 `docs/README.md` 有 **18 条断链**，由下一个 commit 补齐；**不要**把它当「归档完成点」判红。
- `4285af8b` = doc-chore，补齐 `docs/README.md` 索引与链接 → **归档完成点**（tip 上 missing=0）。
- **`checkpoint/pre-doc-archive` = `dde6b855`**（归档前还原点）；
  **`checkpoint/docs-archive-done` = `4285af8b`**（归档完成点）。

### 4.2 回滚

- 归档前：`git checkout checkpoint/pre-doc-archive -- docs/`；
- 归档完成点：`git checkout checkpoint/docs-archive-done -- docs/`；
- 或逐个移回：`git mv docs/legacy/plans/* docs/plans/`。
- ⚠ **不要**用 `b9eff54e`（中间态）当回滚点——它缺 `docs/README.md` 的链接修正，回滚后仍留 18 条断链。

## 5. 命名与新增规范

- 计划 `PLAN-<主题>-<YYYY-MM-DD>.md`；调度 `ORCHESTRATOR-PROMPT-<主题>-<日期>.md`；
  提示词 `IMPL-PROMPTS-…`；交接 `HANDOFF-<日期>-<主题>.md`；审计 `docs/audit/<日期>-<主题>/`；发布 `docs/releases/v<版本>.md`。
- 每份新文档头部必须有：**立档日期 + 状态（活 / 历史 / 作废）+ 真源指向**。
- 一份文档只讲一个主题；超过 300 行就拆。

## 6. 维护节奏

- **每次发布**：release note 里勾一次「本版新增 / 作废 / 归档的文档」；
- **每 3 个 rc**：重跑本文 `3 的体量统计；`docs/plans` 超过 25 份即强制归档一轮。
