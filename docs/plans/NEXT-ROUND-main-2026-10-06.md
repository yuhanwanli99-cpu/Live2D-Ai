# 下一轮清单（**在 main 上开工**）· 2026-10-06 补齐轮交接

> **接手先读**：[`HANDOFF-2026-10-06-e1-e5-debt-round.md`](HANDOFF-2026-10-06-e1-e5-debt-round.md)
> （2026-10-06 夜：工作树单一化 + E1 拆分 + **台账 6 条 P1 全清**；含未提交改动清单、门禁数字、
> 待维护者裁决的两件事（E2 / E4）与本轮踩到的 8 个坑）。
>
> 本轮报告：[`../audit/2026-10-06-gaps-round/ROUND-REPORT.md`](../audit/2026-10-06-gaps-round/ROUND-REPORT.md)。
> 本文只写**没做完的**，每条给「现状 / 可执行规格 / 验收判据」。纪律不变：回源码复核（行号会腐烂）、
> **不许伪造绿灯**、测试只增不减、共享单写者文件串行。

## P0 · 只有维护者能做

| # | 事项 | 现状 | 规格 / 判据 |
| --- | --- | --- | --- |
| ~~M1~~ | ~~轮换手机锁屏口令~~ | **已关闭（2026-10-06 夜）**：维护者已轮换那台手机的锁屏口令 ⇒ 远端残留 blob 里的口令**已失效**（残留本身仍在，见 §4 与 [`../audit/2026-10-06-purge/PURGE-RECORD.md`](../audit/2026-10-06-purge/PURGE-RECORD.md)） | 无。要彻底过期旧对象只剩 GitHub Support（非必需） |
| ~~M2~~ | ~~维护者肉眼/听音清单~~ | **已关闭（2026-10-06）** | 维护者确认肉眼/听音验收通过，并指示删掉勾选表 ⇒ `docs/verification/v0.2.0-checklist.md` 已退役删除 |
| ~~M3~~ | ~~舞台 WebGPU canvas 像素与拖动手柄~~ | **已关闭（2026-10-06）**（随 M2 一并由维护者肉眼确认） | 探针里那 4 项 `manual-only` **仍如实标注，没算成 pass** |

## P1 · 工程债（可立即开工）

| # | 事项 | 现状 / 规格 | 判据 |
| --- | --- | --- | --- |
| **E1** | `main.dart` 1411 行拆分 | **17 条源码扫描守卫直接读 `lib/main.dart`**。先按本轮建好的 `test/support/dart_library.dart`（库 + parts 并集读取）把守卫升级，再按顶层类边界 `part` 拆 | `flutter analyze` 0 issue、`flutter test` 只增不减、Dart >800 再降；守卫不得改判据 |
| **E2** | `display_prefs.dart` 1169 行 | `DisplayPrefs` 是**单个 972 行类**，类体不能跨 `part` ⇒ 只能做**类分解**（这是「改」不是「搬」，要单独论证行为等价） | 同上 + 逐字段变更检测回归（`display_prefs_style_change_detection_test` 系列）全绿 |
| **E3** | `>500` 棘轮顶格 | Rust `>500` 现为 **44/44**（本轮从 48 收紧） | **新增任何 >500 行的 src 文件必须同 commit 拆掉**，否则 `code-stats --check` 直接红 |
| **E4** | 产物预算 | `build/web` **50 MiB**（预算 35）· `dist` **5.4 MiB**（预算 4）。新增项：字体镜像 **2.7 MiB**（离线红线的代价）；`canvaskit` 占了产物大头 | 二选一：真减（如按需裁剪 canvaskit 变体）或**明文改预算 + 理由**并写进 `xtask` 报告口径 |
| **E5** | ~~台账仍未关闭的 6 条 P1~~ | **已完成（2026-10-06 夜）**：六条全关（细节与红-绿自证见 [`AGENTS.md`](../../AGENTS.md) 变更历史「2026-10-06 夜（第二轮 / 第三轮）」与台账 README 的「关闭状态」）。**仍未关闭的 P1 = 0 条** | — |
| **E6** | ~~D6 假绿灯续（`F-0184-01`）~~ | **已核实关闭（2026-10-06 夜）**：`stripCommentsAndStrings` 全仓**只剩一个定义点**（`test/support/source_scan.dart`），`test/source_scan_test.dart` 的反复制门禁（含零命中判红）**20 条全过**。NEXT-ROUND 原文的「8 份副本」是 W3-D3 合并前的旧状态 | — |
| **E7** | CI 在真 runner 上首跑 | 本机无 runner；nightly 的 `cargo build` 是否缺 `libasound2-dev` 未验证 | 首次 nightly 原始日志；红则修 |
| **E8** | 文档减量账 | **口径已复核（2026-10-06 夜 · T1）**：`xtask code-stats` 的 docs 行 = `docs/**/*.md`（含 `docs/legacy/`），**不排除 `docs/audit/`** ⇒ 台账入库的 1,014 份 / 75,148 行**全部计入总量**（「归档不减总量」成立）。实测（`cargo run -p xtask -- code-stats`）：**全部 1,279 份 / 143,270 行**；**不含 `docs/audit/**` = 265 份 / 68,122 行**；`docs/audit/` 自身 = 1,014 份 / 75,148 行。PLAN §5 的目标（docs ≤45,000 行、`docs/plans` ≤25 份）按**不含 audit** 口径**仍未达标**：68,122 超 23,122 行；`docs/plans` 现 **46** 份（`docs/legacy/plans` 另有 94 份） | 复算：`cargo run -p xtask -- code-stats`；「不含 audit」行用同口径脚本（`\n` 计数 + 末尾残行计 1）复算——两条数已对上（143,270 与 xtask 逐位一致） |
| **E9** | 探针稳健性 | ① `audio-c` 2 次里 1 次 blocked（媒体元素已销毁/还没建）；② 像素阈值只对**默认黑主题**成立 | ① 加宽采样窗口或用 `Media` 域事件；② 四主题各自给阈值并写清来源 |
| **E10** | Flutter 升级漂移 | **已实测（2026-10-06 夜）**：`scripts/font_fallback_mirror.sh --check` 当前 **PASS**（清单 == 磁盘 == 引擎表全集，21 文件 / 2 815 292 B，逐文件 sha256 相符）。**红 = 该重跑生成脚本**的信号，不是 bug；升级 Flutter 的那一轮必须跑它——本行保留为常驻提醒 |

## P2 · 功能（本轮未碰）

| # | 事项 | 说明 |
| --- | --- | --- |
| **F1** | **R1 正文帧带 `sentence_seq`（朗读高亮）** | 全项目**唯一「不做就永远做不出来」**的一条：音频帧已有 `sentence_seq`，正文帧没有，纯前端无法映射 |
| **F2** | 会话记忆后端 / 动作-语音选型 | 真源 `docs/plans/PLAN-actions-voice-memory-2026-09-15.md`；实施前先改 `AGENTS.md` 的 director 台账措辞 |
| **F3** | R3/R4（舞台图解码状态帧、capabilities 能力发现） | 排障成本相关，可选 |
| **F4** | 离线 CJK 字体回落 | 现在只镜像 5 族（emoji/符号/数学/音乐）；CJK 五族（jp/hk/sc/tc/kr）**约 11.9 MiB** 未镜像 ⇒ 离线时子集外 CJK 是豆腐块 | 若要：要么加镜像（体积代价），要么把「引擎回落表驱动生成」做成可选构建步骤 |

## P3 · 仓库收尾

| # | 事项 | 现状 |
| --- | --- | --- |
| **R1** | ~~移除死树 `-Ai` worktree~~ | **已完成（2026-10-06 夜，方向与原文相反）**：维护者裁「`-Ai` 为主树」⇒ 改为**删除 linked worktree `Live2D-Ai-fe`**、把 `main` 切回 `/home/skystar/Live2D-Ai`（31 G `target/` 与前端产物已迁入，运行态以 `-fe` 为准并入）；A/B 审计台账保运到树外 `/home/skystar/audit-ref-2026-10-06/`（按指示不再运行）。当前 `git worktree list` 只剩 `/home/skystar/Live2D-Ai ... [main]` |
| **R2** | `dev/integrity` / `dev/node-p1-p2` 裁决 | bundle 已覆盖，可降级为档案 |
| **R3** | ~~是否推 `origin`（N18）~~ | **已完成（2026-10-06）**：维护者授权 ⇒ 历史清洗后 force-push `main`、重写后的远端 tag；新 tag `v0.2.1-rc.1` 与 GitHub pre-release 已建。**措辞更正（2026-10-06 夜 · T1）**：原文「删除残留分支 `mainline/1-core-baseline`」与事实不符——**远端** `refs/heads/mainline/1-core-baseline` = `b58b223` **仍然存在**（本机无 token，删不掉）；**本地**那份已于 2026-10-06 夜由 Lead 用 `git branch -d` 删除（连同另外 4 条 0 领先分支，见 R7）。要删远端分支需 GitHub token |
| **R4** | Gitleaks（N19） | 本轮仍不补（多一个第三方 action = 多一条供应链面）；要补单独一轮 |

---

## 4 · 2026-10-06 夜（清洗收尾轮）：新增待办

### 4.1 已关闭 / 已处置（本轮）

- **M1 关闭**：维护者**已轮换那台手机的锁屏口令**。远端 `refs/pull/1/head`（`44da2a4c`）里那个 blob 中的口令**已失效** ⇒ 从「P0 泄密」降级为「技术残留、历史垃圾」（`refs/pull/*` 服务端只读，`DELETE` = 422）。
- **GitHub 凭据**：维护者已删除其 token；本地 `gh` 里那把凭据经 `gh auth status` 判定 **invalid**，本轮据维护者指示执行 `gh auth logout` 从 `~/.config/gh/hosts.yml` 移除（现为 `{}`，`oauth_token` 计数 0）。另查：`~/.git-credentials` 不存在、环境无 `GH_TOKEN`/`GITHUB_TOKEN`、remote URL 无水印式内嵌凭据。
- **清洗备份退役**：`backup-2026-10-06/pre-purge-all-refs.bundle`（**132 448 444 B**，`sha256 c499f8631fadddfc0d30be03c8e882e98e5bcfb2624e429ede0eead1af66a7be`）**已删除**；同目录仅保留 `refs-before.txt`。

### 4.2 新增待办

| # | 事项 | 判据 / 备注 |
| --- | --- | --- |
| **R5** | **本地提交待推**：本轮收口文档是本地提交（workspace 无新 token） | `git rev-list --count origin/main..main` 回 0；新 token 就绪后 `git push origin main` |
| **R6** | **文档收口四项** ① `AGENTS.md` 首屏「当前版本 `0.2.0`」与树（`0.2.1-rc.1`）不一致；② `archive/action-layer-p6` 被 10+ 处引用（`AGENTS.md` / `CHANGELOG.md` / `README.md` / `README.zh-CN.md` / `docs/architecture/core-chain-baseline.md` / `crates/live2d-ai-desktop/src/main.rs`）但**本地与远端都没有该分支**（T1 已按「分支已不存在」改写写面内全部引用，并给出等价取回命令 `git show 98469df^:…` / `git show ef9f428^:…`；`CHANGELOG.md` 与 `crates/**` 不在 T1 写面，留给 Lead）；③ 本轮清单 R3 的措辞「已删除残留分支 `mainline/1-core-baseline`」**与实际不符**：远端此刻仍有 `refs/heads/mainline/1-core-baseline = b58b223`；④ 上一轮台账已入库，指向它的旧路径写法要收口 | 逐条 `git grep` + `git ls-remote` 复核 —— **已收口（2026-10-06 夜 · T1）**：① 版本口径改 `0.2.1-rc.1`（`Cargo.toml` `version` + tag 实测）、`0.2.0` 降为「上一版」；② 写面内 `archive/action-layer-p6` 引用全部改写（`.md` 侧，含 `docs/architecture/**` 与两份 README）+ 等价取回命令；③ 见 R3 行的措辞更正；④ 旧账本路径统一到 `docs/audit/2026-10-05-ledger/`（`docs/DOC-MAP.md` / `docs/README.md` / §4.3） |
| **R7** | **分支清理（13 条非 main）** | **5/13 已完成（2026-10-06 夜，Lead）**：0 领先的 5 条已用 `git branch -d` 删除 —— `feat/frontend-redesign`（28 落后／0 领先；它 = 远端 tag `v0.2.0` 的提交 `a3f2717`，**删了不丢东西**）· `chore/debt-round-2026-10-05`(24/0) · `mainline/1-core-baseline`(187/0，本地；远端 ref 仍在) · `pr-1`(188/0) · `mod/persona-polish`(151/0)；本地现余 **8** 条非 main。保留待裁决：`archive/action-trigger-p5`(193/356) · `archive/full-history-2026-09-11`(193/374) · `android-archive`(193/1) · `dev/integrity`(193/300) · `dev/node-p1-p2`(193/243) · `backup-local-main-before-force`(193/215) · `feature/node-d-d3-d4`(193/215) · `refactor/elegance`(193/215)；判定：`git rev-list --left-right --count main...<b>` 右值为 0 |
| **R8** | **清洗前历史的本地副本仍在**（口令已失效 ⇒ 价值为零，纯占盘）：`backup-2026-10-05/archive-full-history-2026-09-11.bundle`（114 MB，**实测仍含清洗前链**：其 tip `5e455ad6` 在清洗后的库里 `git cat-file -t` 报 fatal，本地重写后同名分支 tip 为 `90e0fac9`）· `Live2D-Ai-LEGACY-FULL-HISTORY.bundle`(114 MB) · `Live2D-Ai-PY-LEGACY.bundle`(100 MB) · `Live2D-Ai-baseline-28de52cf.bundle`(114 MB) · `Live2D-Ai-baseline-incremental.bundle`(13 MB) · `redesign-backup-2026-09-27.tar.gz`(6.8 MB) · `backups/dsh-data-backup-20260821.tar.gz`(134 MB) | 维护者决定删或留；**删前确认已不需要回滚**（`05` 那份是当前唯一的清洗前回滚路径） |

### 4.3 审计安排（本轮已定，规程入库）

- 规程：[`AUDIT-PROMPT-whole-repo-2026-10-06.md`](AUDIT-PROMPT-whole-repo-2026-10-06.md)（v3.2；工作树外另存 `/home/skystar/audit-prompt-2026-10-06.md`）。锁 `main` @ `6be9984` 或其后 1–2 个 docs 提交。
- **第一优先 = 前端设置 + Mod 面**（维护者 2026-10-06 指定）：`Phase 0` 九批（`P0-1` `display_prefs` 字段真源 → 控制器/骨架 → 外观 → dev_tools → Mod 面板 → 主链设置 → 契约面 → 后端对照面 → 设置测试质量），配 **§7.1 用户侧审视**七问 + **设置项三方对账**（死字段 / 假旋钮 / 隐藏开关）。
- 台账 `AUDIT-REPO/`（未跟踪，永不 `git add`），批次从 **`BATCH-1001`** 起（上一轮已占用 0001–0927）。
- Rust 主线顺延 `Phase 1` 第 1 项 = `crates/live2d-ai-runtime/**`（上一轮台账恰在此停住）。