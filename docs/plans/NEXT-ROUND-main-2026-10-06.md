# 下一轮清单（**在 main 上开工**）· 2026-10-06 补齐轮交接

> 本轮报告：[`../audit/2026-10-06-gaps-round/ROUND-REPORT.md`](../audit/2026-10-06-gaps-round/ROUND-REPORT.md)。
> 本文只写**没做完的**，每条给「现状 / 可执行规格 / 验收判据」。纪律不变：回源码复核（行号会腐烂）、
> **不许伪造绿灯**、测试只增不减、共享单写者文件串行。

## P0 · 只有维护者能做

| # | 事项 | 现状 | 规格 / 判据 |
| --- | --- | --- | --- |
| **M1** | **轮换手机锁屏口令**（F-0616-01） | 树里已删净、扫描器已补这一类、台账已脱敏；但**历史提交**（含公开远端的 `v0.1.0-rc.1` 根提交）里那份原文还在 | 换掉那台手机的锁屏口令。要「从远端抹掉」必须**重写历史 + force-push**（本轮**未授权、未做**）；只改树不轮换 = 没修 |
| **M2** | 维护者肉眼/听音清单 | `docs/verification/v0.2.0-checklist.md` 仍未勾；本轮把「真实音频链路」**自动化了**（WS 帧 + blob `<audio>` + `currentTime` 前进），可自动的部分不再需要人工 | 在 `http://127.0.0.1:18080/app/` 上逐条落勾；重点是**舞台像素级**、读屏器、真机 Windows |
| **M3** | 舞台 WebGPU canvas 像素与拖动手柄 | 无头 swiftshader 下舞台区截图恒白 ⇒ 探针 `3d`/`6c-pixel`/`5g` 判 **manual-only**（**没算成 pass**） | 真机肉眼：四套主题的舞台底色 = 该套 `stage` 色；背景库拖动排序能改顺序 |

## P1 · 工程债（可立即开工）

| # | 事项 | 现状 / 规格 | 判据 |
| --- | --- | --- | --- |
| **E1** | `main.dart` 1411 行拆分 | **17 条源码扫描守卫直接读 `lib/main.dart`**。先按本轮建好的 `test/support/dart_library.dart`（库 + parts 并集读取）把守卫升级，再按顶层类边界 `part` 拆 | `flutter analyze` 0 issue、`flutter test` 只增不减、Dart >800 再降；守卫不得改判据 |
| **E2** | `display_prefs.dart` 1169 行 | `DisplayPrefs` 是**单个 972 行类**，类体不能跨 `part` ⇒ 只能做**类分解**（这是「改」不是「搬」，要单独论证行为等价） | 同上 + 逐字段变更检测回归（`display_prefs_style_change_detection_test` 系列）全绿 |
| **E3** | `>500` 棘轮顶格 | Rust `>500` 现为 **44/44**（本轮从 48 收紧） | **新增任何 >500 行的 src 文件必须同 commit 拆掉**，否则 `code-stats --check` 直接红 |
| **E4** | 产物预算 | `build/web` **50 MiB**（预算 35）· `dist` **5.4 MiB**（预算 4）。新增项：字体镜像 **2.7 MiB**（离线红线的代价）；`canvaskit` 占了产物大头 | 二选一：真减（如按需裁剪 canvaskit 变体）或**明文改预算 + 理由**并写进 `xtask` 报告口径 |
| **E5** | 台账仍未关闭的 6 条 P1 | 题干与落点见 [`../audit/2026-10-05-ledger/README.md`](../audit/2026-10-05-ledger/README.md)：`F-0001-01`（四族零落盘日志）· `F-0002-01`（首次配置路径 watcher 不装）· `F-0002-02`（slot 假绿灯）· `F-0006-03`（同一把锁两种口径 + 工厂代码在锁内）· `F-0013-01` / `F-0644-01`（未知 Mod id 静默丢弃） | 每条要有**回源码复核** + 「现在它能红」的自证 |
| **E6** | D6 假绿灯续 | `F-0184-01`：`stripCommentsAndStrings` 在 `test/` 下有 8 份副本，其中 1 份已漂移 | 合一到 `test/support/`，并证明漂移那份的能力被保留 |
| **E7** | CI 在真 runner 上首跑 | 本机无 runner；nightly 的 `cargo build` 是否缺 `libasound2-dev` 未验证 | 首次 nightly 原始日志；红则修 |
| **E8** | 文档减量账 | 台账入库后口径要重算（`docs/audit/**` 排除） | 用「不含 `docs/audit/**`」那一行判定；**归档不减总量** |
| **E9** | 探针稳健性 | ① `audio-c` 2 次里 1 次 blocked（媒体元素已销毁/还没建）；② 像素阈值只对**默认黑主题**成立 | ① 加宽采样窗口或用 `Media` 域事件；② 四主题各自给阈值并写清来源 |
| **E10** | Flutter 升级漂移 | 引擎回落表（`font_fallback_data.dart`）版本号/字族切分一变，`scripts/font_fallback_mirror.sh --check` 就会红 | 红 = **该重跑生成脚本**的信号，不是 bug；升级 Flutter 的那一轮必须跑它 |

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
| **R1** | 移除死树 `-Ai` worktree | 已冻结为 0 脏、保档在 `/home/skystar/backup-2026-10-05/`；**本会话的 cwd 就是它**，故未删。命令：`git -C /home/skystar/Live2D-Ai-fe worktree remove --force /home/skystar/Live2D-Ai` + `git branch -d mod/persona-polish` |
| **R2** | `dev/integrity` / `dev/node-p1-p2` 裁决 | bundle 已覆盖，可降级为档案 |
| **R3** | 是否推 `origin`（N18） | 本地 `main` 领先 `origin/main` **128** 提交；历史口径「不推远端」，需维护者明确解除才推 |
| **R4** | Gitleaks（N19） | 本轮仍不补（多一个第三方 action = 多一条供应链面）；要补单独一轮 |