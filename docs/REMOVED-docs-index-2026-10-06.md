# 已移出工作树的文档索引（E8 文档减量）

> 立档：**2026-10-06** · 状态：**活** · 真源：本文件
> 用途：回答「我在历史文档里看到一个链接，文件没了怎么办」。

## 1. 口径（一句话）

**历史留证 ≠ 必须占工作树行数。** git 历史就是那份证据；工作树只留**现在还要用**的文档。
依据：`docs/DOC-MAP.md` §2 生命周期三分 + `PLAN-debloat-and-closeout-2026-10-01.md` §5 D5
——「归档（在 `docs/` 内搬运）不减少 docs 总行数 ⇒ 减量只能来自**真删 / 真合并**」。
所以本轮把只具历史留证价值的文档**移出工作树**（不是改写、不是压缩），全部可**逐字取回**。

## 2. 本轮移出清单（**139 个文件 / 31,509 md 行**）

### 2.1 历史留证类（127 文件 / 28,896 行）

| 路径 | 文件 | md 行 | 为什么移出 |
|---|---:|---:|---|
| `docs/legacy/**` | 108 | 25,193 | DOC-MAP 第 2 类「历史留证」：Python/Android 双端时代文档 + **2026-10-01 归档的 94 份已收口计划**（每份顶部本就标「历史，勿当现网」）。归档只解决「活/历史混放」，不解决行数 |
| `docs/design/legacy/**` | 9 | 584 | 旧 **JS 前端**设计规格与预览图（前端已于 2026-09-11 删除，规格标的对象不存在） |
| `docs/design/web-action-trigger-archive.md` | 1 | 122 | 已删除的旧 JS 动作触发 UI 的归档说明 |
| `docs/verification/node-c-*.md`（6 份） | 6 | 2,050 | 2026-08「节点 C」原生壳（托盘 / 拖动 / X11 / WSLg）验证记录——**该子系统已于 2026-10-01 物理移出**（`architecture/ARCHIVED-native-shell.md`） |
| `docs/verification/desktop-platform-smoke-2026-08-26.md` | 1 | 32 | 同上（原生壳平台能力实测） |
| `docs/verification/pc-preview-smoke.md` | 1 | 24 | PC preview 时代（Python 壳）冒烟清单 |
| `docs/verification/rust-bakeoff-mocari.md` | 1 | 891 | **未被选中的**候选（mocari）的 bakeoff 报告；结论与选型理由留在仍在树的 `rust-bakeoff-decision.md` + `rust-bakeoff-ayagami.md` |

### 2.2 已收口计划类（12 文件 / 2,613 行）——PLAN §5 的第二条判据（`docs/plans` 顶层 ≤25）

| 路径（`docs/plans/`） | md 行 | 为什么移出 |
|---|---:|---|
| `AUDIT-PROMPT-whole-repo-2026-09-28.md` | 356 | 已被 10-06 版**取代**（10-06 版开头就写明「前身」） |
| `BACKLOG-0.2.0-closeout-2026-09-28.md` | 82 | 0.2.0 已收口 |
| `HANDOFF-2026-09-21-actions-performance-round.md` | 152 | 动作/表演轮已收口（rc.4 已发布） |
| `HANDOFF-2026-09-21-actions-performance-wave0-1.md` | 357 | 同上 |
| `HANDOFF-2026-09-21-assistant-memory-presets-tts-clean.md` | 257 | 同上 |
| `HANDOFF-2026-09-22-performance-layer.md` | 109 | 同上（rc.4 表演协议 v1 已落地） |
| `HANDOFF-2026-09-27-stageB-plan-and-status.md` | 100 | Stage B 已收口 |
| `IMPL-PROMPTS-0.2.0-seal-2026-09-27.md` | 367 | 提示词已用尽（0.2.0 已发布） |
| `ORCHESTRATOR-PROMPT-0.2.0-closeout-2026-09-28.md` | 250 | 同上 |
| `ORCHESTRATOR-PROMPT-0.2.0-seal-2026-09-27.md` | 190 | 同上 |
| `PLAN-0.2.0-seal-and-cleanup-2026-09-27.md` | 334 | 0.2.0 封口计划已完成 |
| `NEXT-ROUND-main-2026-10-05.md` | 59 | 已被 `NEXT-ROUND-main-2026-10-06.md` 取代 |

结果：`docs/plans` 顶层 **33 → 21**（PLAN §5 判据 `≤25` **达标**）。

**合计：139 文件 / 31,509 md 行**（`docs` 不含 `docs/audit/**` 的口径：**70,762 → 39,273 行**；
本轮新增的**本索引 +81 行**、活文档改写净 **−72** 行；**PLAN §5 的 `≤45,000` 达标，余量 5,727**）。

## 3. 取回（逐字，不需要重建）

移出前的最后一个提交 = **`42c5825`**（本轮删除提交的父提交），它**逐字包含上表全部 127 份**。

```bash
# 单份
git show 42c5825:docs/legacy/plans/PLAN-V1.md > /tmp/PLAN-V1.md
# 整目录（docs/legacy、docs/design/legacy）
git archive 42c5825 docs/legacy docs/design/legacy | tar -x -C /tmp/recover
# 枚举本轮移出的全部路径（把 <删除提交> 换成实际 hash）
git show --name-status <删除提交> | awk '$1=="D"{print $2}'
```

## 4. 引用处理规则（**刻意不对称**）

| 引用所在层 | 处理 | 理由 |
|---|---|---|
| **活文档**（`docs/README.md` · `docs/DOC-MAP.md` · `AGENTS.md` · `docs/architecture/*` · `docs/legal/*` · 根文档 · `crates/*/README.md`） | **已改链**到本文件 | 活文档点得到，才叫活文档 |
| **历史文档**（`CHANGELOG.md` · `docs/releases/*` · `docs/plans/HANDOFF-*`、`IMPL-PROMPTS-*`、`ORCHESTRATOR-*` 等已收口计划 · `docs/research/*` · `docs/verification/*` 剩余各份） | **保留原样**，不改链 | 它们是**当时的记录**；改链 = 篡改证据（与 DOC-MAP「台账历史引用刻意不改」同一条纪律）。看到死链即为「这份文档写于移出之前」的信号，按 §3 取回 |

## 5. 门禁（本轮起有机器守门）

- **行数**：`xtask code_stats` 报 `docs ≤ 45,000 行（不含 docs/audit/**）`，`--check` 有独立判据，
  与 `RATCHET_DART_800` / 产物预算同一套「预算 + 内/超」口径。
- **审计账本不计入**：`docs/audit/**` 是过程产物、只增不减（2026-10-06 实测 1,014 份 / 75,154 行），
  计入它会掩盖真实减量。

## 6. 本轮**没有**做（如实登记）

- 未改写任何被移出文件的内容（移出 = 纯删除，git 里逐字可查）；
- 未动 `docs/audit/**`（过程产物，只读）；
- 未动研究类 `docs/research/**`（结论仍被引用，未被取代）；
- 未做「真合并 / 压缩改写」——不存在把 25k 行历史压成 2k 行的诚实做法。
