# 仓库卫生报告 · 2026-10-05 债轮（Lead 执行）

> 执行者：Lead（破坏性 git 只由 Lead 做）。工作树：`/home/skystar/Live2D-Ai-fe`
> 分支：`chore/debt-round-2026-10-05`（本轮），目标 `main` 走 `--ff-only`。
> 纪律：**先保档、再删除**；任何删除前先 `readlink -f` 校验解析后的绝对路径。

## 0. 一页结论

| 项 | before | after |
|---|---|---|
| worktree | 3（`-Ai` / `-fe` / `-product`） | **2**（`-Ai` 冻结档 / `-fe` 开发线） |
| 本地分支 | 12 | **11**（删 1 条已合并的 `mod/product-grade`） |
| 死树脏项 | **136** | **0**（已冻结为 `4e421993` 干净基线；改动与未跟踪全部保档） |
| 磁盘可用 | 24G | **33G**（回收 8.7G：`-product/target` 3.6G + `-Ai/target` 4.9G） |
| 未覆盖的遗留分支 | 1（`archive/full-history-2026-09-11`） | **0**（已打 bundle，`bundle verify` = complete history） |

## 1. 做了什么（逐条 + 证据）

### 1.1 保档（先做，全部校验非空）

目录 `/home/skystar/backup-2026-10-05/`：

| 文件 | 内容 | 校验 |
|---|---|---|
| `-Ai-tracked.patch` (519 KB) | 死树 113 个跟踪文件的改动（`git diff`） | 11,891 行；`git apply --reverse --check` **静默通过** ⇒ 与当时脏树逐字一致 |
| `-Ai-untracked.tar.gz` (6.7 MB) | 死树全部未跟踪文件（`git ls-files -o --exclude-standard`） | 35 个文件；含 `zz_audit_tmp_test.dart`（15 条一次性复现）与 `docs/design/assets/visual-substance-2026-09-27/` |
| `-Ai-SNAPSHOT.md` (6.5 KB) | 死树 HEAD + `git status --short` 全量 | 136 项清单 |
| `PRODUCT-L1-GOALS-2026-09-15.md` (1.9 KB) | `-product` 唯一的未跟踪文档（**main 上没有**） | 36 行 |
| `archive-full-history-2026-09-11.bundle` (114 MB) | 唯一未被既有 bundle 覆盖的遗留分支 | `git bundle verify` = *The bundle records a complete history* |

### 1.2 worktree：删 `-product`（3.8G）

- 证据：`mod/product-grade` **已并入 main**（`git merge-base --is-ancestor` = true），落后 main 66、领先 0；
  工作树只剩 1 个未跟踪文档（已保档）。
- 命令：`git -C /home/skystar/Live2D-Ai-fe worktree remove --force /home/skystar/Live2D-Ai-product`
  → `git branch -d mod/product-grade`（Deleted branch mod/product-grade (was 2c64733a)）。

### 1.3 死树 `-Ai`：从「136 脏项」冻结为「干净基线」

- `-Ai` 的工作树落后 main **123** 提交，其 136 项改动里**只有 2 项 main 上没有**，且都已保档（§1.1）。
- 动作：`git checkout -- .` + `git clean -fd`（**不带 `-x`**，故 `.env`/`live2d-ai.toml`/`mods.json`/`assets/models/` 等 ignore 项全部保留，已实测仍在）。
- 结果：`git status --porcelain` = 0；HEAD 仍 `4e421993`（`mod/persona-polish`）。
- **为什么不直接删这棵树**：它是 Lead 会话的工作目录，删掉会让本会话的相对路径全部失效。
  结论：**保留为「冻结档」**，并在 §3 给出可安全移除的条件与命令。

### 1.4 构建产物回收（8.7G）

- `rm -rf /home/skystar/Live2D-Ai/target`（4.9G，死树的旧构建产物，可再生）。
- `-product/target`（3.6G）随 worktree 一并移除。
- 磁盘：`/dev/sdd` 可用 **24G → 33G**。

## 2. 分支决策表（全部 11 条，逐条给「保留/删除」与理由）

| 分支 | 最后提交 | HEAD | 并入 main？ | 决定 | 理由 |
|---|---|---|---|---|---|
| `main` | 2026-10-02 | fd2747ed | — | 保留 | 集成分支（本轮 ff 目标） |
| `chore/debt-round-2026-10-05` | 2026-10-02 | fd2747ed | 是（起点） | 保留 | **本轮开发线** |
| `feat/frontend-redesign` | 2026-10-02 | fd2747ed | 是 | 保留 | 文档记载的开发线；与 main 同点 |
| `mod/persona-polish` | 2026-09-14 | 4e421993 | 是 | 保留 | `-Ai` 冻结档所检出；删它必须先移除该 worktree |
| `android-archive` | 2026-08-26 | fa48c0dd | 否 | 保留 | `ANDROID_ARCHIVE_POINTER.md` 指向它；bundle **COVERED** |
| `archive/action-trigger-p5` | 2026-09-10 | f17484bb | 否 | 保留 | 文档记载的动作层归档位；bundle **COVERED** |
| `archive/full-history-2026-09-11` | 2026-09-12 | 5e455ad6 | 否 | 保留 | 历史重置前的**完整历史**唯一副本；本轮补 bundle **COVERED** |
| `backup-local-main-before-force` | 2026-09-01 | ed28485f | 否 | 保留 | 与另两条同点；bundle **COVERED** |
| `feature/node-d-d3-d4` | 2026-09-01 | ed28485f | 否 | 保留 | 同上 |
| `refactor/elegance` | 2026-09-01 | ed28485f | 否 | 保留 | 同上；`origin/refactor/elegance`(47848ed4) 也在 bundle 内 |
| `dev/integrity` | 2026-09-07 | 78040b87 | 否 | 保留 | 与 `docs/releases/v0.3.0.md` 版本线矛盾有关，待下轮裁决；bundle **COVERED** |
| `dev/node-p1-p2` | 2026-09-02 | e12e5d86 | 否 | 保留 | 同上；bundle **COVERED** |
| ~~`mod/product-grade`~~ | 2026-09-15 | 2c64733a | 是 | **已删** | 已并入 main、worktree 已移除、唯一未跟踪文档已保档 |

**删分支为什么不省空间**：删除分支不会回收对象（对象仍被 reflog/其他 ref 引用，除非 `gc` 且过期）。
遗留分支的实害是「列表噪音」而不是磁盘 ⇒ 本轮只删**已合并且内容无唯一性**的那一条，
未合并的 pre-history-reset 分支**一律保留**，并把「是否已有 bundle 覆盖」写进上表供将来裁剪。

## 3. 遗留事项（交给下一轮 / 维护者）

1. **`-Ai` 冻结档可移除的条件**：本会话结束后，若确认 `/home/skystar/backup-2026-10-05/` 保档可用，则可
   `git -C /home/skystar/Live2D-Ai-fe worktree remove --force /home/skystar/Live2D-Ai` +
   `git branch -d mod/persona-polish`（分支已并入 main）。**本轮不做**（会打断正在进行的工作）。
2. **`dev/integrity` / `dev/node-p1-p2`**：与 `docs/releases/v0.3.0.md`（自标历史草案）的版本线矛盾相关，
   需要一次裁决：确认「v0.3.0 未发布」后，这两条可降级为纯档案或删除（bundle 已覆盖）。
3. **`origin/` 远端 ref 落后**：本地 `main` 领先 `origin/main` **124** 提交（远端停在 `v0.2.0-rc.1`）。
   是否推送属维护者口径（历史上明确「不推远端」），本轮不擅自推送。
4. **`AUDIT-REPO/`（942 文件，未入库）**：审计账本目前**只存在于 `-fe` 工作区**，未跟踪、无备份。
   建议下一轮决定：入库（`docs/audit/`）或归档到 `/home/skystar/backup-*`。（本轮未动，避免与 CI 报告目录冲突。）

## 4. 复现命令

```bash
git worktree list
git branch -vv
git -C /home/skystar/Live2D-Ai status --porcelain | wc -l   # 期望 0
ls -la /home/skystar/backup-2026-10-05/
git bundle verify /home/skystar/backup-2026-10-05/archive-full-history-2026-09-11.bundle
df -h /home/skystar | tail -1
```
