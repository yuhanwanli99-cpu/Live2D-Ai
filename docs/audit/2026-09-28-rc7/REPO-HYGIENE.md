# 仓库卫生前后对比（Stage C2 · **只有编排者能做的破坏性 git 操作**）· 2026-09-28

> **口径**（维护者 2026-09-28）：个人小项目、产物存本地即可；**任何破坏性 git 操作前先建 tag**；
> 「本地」≠「可以丢」。因此本清单里**每一条删除都先说清「东西还在哪」**。
> 前置：审计账本**已先入库**（`docs/audit/2026-09-28-frontend-nightly/`，rc.6 前的 docs-only 提交），
> 所以「未跟踪清零」不会误删唯一副本。

## 0. 前后对比

| 指标 | 前 | 后 | 说明 |
|---|---|---|---|
| **worktree 数** | **25**（含主开发线 `-fe`） | **3** | 22 个「已并入 main 且工作树干净」的已移除 |
| **本地分支数** | **31** | **12** | 删掉 19 个已并入 main 的分支（`git branch -d`，内容仍在 main 历史里） |
| **stash** | **1**（`mod/persona-polish: hold: pre-integration dirt`） | **0** | **先导出补丁再 drop**：`docs/legacy/stash-persona-polish-pre-integration-2026-09-28.patch`（58 行） |
| **未跟踪项** | 5（`AUDIT/`、`AUDIT-B/` + 本轮 3 个新文件） | **0** | 审计账本已入库 ⇒ 删掉根目录重复的 `AUDIT/`/`AUDIT-B/`；本轮新文件已提交 |
| **rc 线 tag** | 3（rc.1 / rc.4 / rc.5，**rc.2 / rc.3 / rc.6 缺**） | **6+1**（rc.1–rc.6 全 + `checkpoint/rc5-pre-rc6`） | 见 §3 |
| 磁盘 | 147G 已用 **140G（48M 可用）** | **103G（37G 可用）** | 见 §4（一次 ENOSPC 事故的处理） |

## 1. 移除的 22 个 worktree（每个都是「已并入 main」+「工作树干净」）

`mod/integrate-0.2.0-rc.2`、`mod/l1-product`、`mod/pg-director`、`mod/pg-external`、`mod/pg-memory`、
`mod/pg-persona`、`mod/pg-voice`、`mod/stabilize`、`mod/director-rfc`、`mod/memory-v0`、`mod/pet-desktop-v1`、
`mod/voice-sidecar-v1`、`mod/wallpaper-wire`、`mod/w3-{director,external,memory,persona,pet,voice,wall}`、
`mod/wave2`、`mod/wave3`

判据（三条**同时**满足才移除）：① `git branch --merged main` 命中；② `git status --porcelain` 为空；
③ 分支内容可从 main 到达。**命令**：`git worktree remove <path>`（不带 `--force` —— 脏的一律拒绝，这是护栏）。

**保留的 3 个 worktree 与理由**：

| worktree | 分支 | 脏项 | 为什么留着 |
|---|---|---|---|
| `/home/skystar/Live2D-Ai-fe` | `feat/frontend-redesign` | 0 | **唯一开发线** |
| `/home/skystar/Live2D-Ai` | `mod/persona-polish` | **136** | 旧基线；其脏副本是 09-27 前端重设计的**原始源**（已重放并在 rc.5 提交，但**脏改动本身未入库**）；且它是**当前会话的工作目录**。**不做删除** —— 需要维护者明确裁决（先出补丁还是保留）。 |
| `/home/skystar/Live2D-Ai-product` | `mod/product-grade` | 1 | 有 1 份**唯一**未跟踪文档 `docs/plans/PRODUCT-L1-GOALS-2026-09-15.md`，与 `-fe` 同名文件 **cmp 不同**（不是重复副本）⇒ 不删，交维护者裁决。 |

## 2. 保留的 12 个分支

`main`、`feat/frontend-redesign`（开发线）、`mod/persona-polish`、`mod/product-grade`（上面两个脏 worktree 的分支）
+ **9 个未并入 main 的**：`android-archive`、`archive/action-trigger-p5`、`archive/full-history-2026-09-11`、
`backup-local-main-before-force`、`dev/integrity`、`dev/node-p1-p2`、`feature/node-d-d3-d4`、`refactor/elegance`
（+`main`）。理由：**未并入 = 内容不在 main 历史里**，删掉就真丢了 ⇒ 一律保留，等维护者逐个裁决。

## 3. tag 补齐（本轮的「还原点」纪律）

```
$ git tag -a v0.2.0-rc.2 91c670aa   # = "release: v0.2.0-rc.2（Wave 1 三轨合成）"
$ git tag -a v0.2.0-rc.3 1e789cb6   # = "release: v0.2.0-rc.3（Wave 2 五轨合成 + FACTORIES 5→6 + 基座）"
$ git tag -l 'v0.2.0-rc.*'
v0.2.0-rc.1  v0.2.0-rc.2  v0.2.0-rc.3  v0.2.0-rc.4  v0.2.0-rc.5  v0.2.0-rc.6  (+ rc.7)
```

**顺序很关键**：**先打 tag 再删 worktree**（rc.2/rc.3 那两个 release 提交恰好就是 `-integrate`/`-wave2` 的 HEAD）。
另：rc.6 之前补了 `checkpoint/rc5-pre-rc6`（= `5ef879f4`）。

## 4. 磁盘事故记录（**如实**：本轮真的撞上过 ENOSPC）

跑 rc.7 收口门禁时 `flutter build` 报 **`ENOSPC: no space left on device`**（`/dev/sdd` 147G 用到 **140G，仅剩 48M**），
构建与点火服务被 SIGTERM 杀掉。排查与处理：

| 处理 | 释放 | 安全性论证 |
|---|---|---|
| `rm -rf /home/skystar/Live2D-Ai-l1/target` | ~35G | 该 worktree 的分支 `mod/l1-product` **已并入 main** 且 HEAD 就是 `e4f139a8`(=main/rc.4) ⇒ target 是**纯构建产物**，可重建；**源码一行未动**，worktree 随后也按 §1 正常移除 |
| `rm -rf /tmp/flutter_tools.*` | ~1.5G | flutter 工具链的**临时目录**（被 SIGTERM 杀掉的构建残留） |
| 删 /tmp 里本轮自己的备份文件 | 微量 | 校验任务已完成 |

**未触碰**：`CosyVoice 3.0`(20G)、`ComfyUI`(16G)、`miniconda3`(11G)、`-fe/target`(18G，门禁需要)、任何 git 对象与工作树源码。
处理完 `df` = **103G 已用 / 37G 可用**；随后**重跑**全部门禁并全部通过（见 `docs/releases/v0.2.0-rc.7.md` §0）。

> 教训（写进纪律）：**24 个 worktree 的 `target/` 是磁盘黑洞**（`-l1` 一个就 35G）。
> 「清 worktree」不只是整洁问题，**它同时也是磁盘可用性问题** —— 建议后续每次 `cargo build` 前先确认 worktree 数。
