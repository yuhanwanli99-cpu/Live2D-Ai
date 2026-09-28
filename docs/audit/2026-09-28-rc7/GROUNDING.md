# R7-0 · rc.7 波次接地与提示词对账（2026-09-28）

> **作用**：`IMPL-PROMPTS-0.2.0-seal-2026-09-27.md` 里的 **Stage C（C1/C2/C3）与 Stage D** 写于 rc.5 之前，
> 其中引用的**行号/文件名已漂移**。本文是**编排者**在 rc.6 收口后重测的对账，作为 rc.7/rc.8 的施工输入。
> **纪律**：行号是快照，动手前**回源码重读**；本文只提供**当前**实测值。

## 0. rc.6 收口基线（本文件的测量基线）

- 提交 **`e1e05eb3`**，本地 tag **`v0.2.0-rc.6`**（另有开工前还原点 `checkpoint/rc5-pre-rc6`）。
- 门禁：cargo test **1457/0**、doc **3**、fmt clean、clippy **0**、rust-ratio **97.3263% PASS**；
  flutter analyze **0 issue**、flutter test **1370**（rc.5 为 1272，+98，测试文件 93 → 101，**删除 0**）；
  build web ✓（`main.dart.js` 晚于所有 `.dart`）；canvaskit gstatic **0/0**；`ignite.sh --check` **四项 ok**。

## 1. Stage C/D 提示词里已漂移的数字（**以本表为准**）

| 提示词原文（rc.5 前） | **2026-09-28 实测** | 说明 |
|---|---|---|
| `appearance_section.dart` **1489**（「超 ≤500 且不满足 ≤1000 豁免带」） | **709** | rc.6 已把背景域抽到 `appearance_background.dart` |
| —（新文件，提示词里没有） | `appearance_background.dart` **1551** | **新的超长文件**（rc.6 新建，头注已如实标既有债） |
| `dev_tools_section.dart` **1874** | **1874**（未变） | 仍是最大单文件 |
| `main.dart` **1238** | **1329** | rc.6 的索引同步 + 水合标志 |
| `display_prefs.dart` **1002** | **1157** | rc.6 的四档 + 逐图 + enabled |
| `shell_prefs.dart`（未列） | **500**（正好上限） | rc.6 改水合落点 |
| `live2d_bridge.dart`（未列） | **547**（有豁免头注） | rc.6 加 `sendStageBg` 去重 |
| `display_prefs_test.dart` **882** | **892** | 仍超 800 |
| `memory_panel_test.dart` **852** | **852**（未变） | 仍超 800 |
| —（新文件） | `app_shell_layout_test.dart` **749**、`action_scales_wiring_test.dart` **737**、`persona_panel_test.dart` **649** | 均在 800 内 |
| Stage C3 原文说 `appearance_section.dart` 1039 行 | 已过时两轮 | 见上 |

**C3 的拆分对象因此变成**：`dev_tools_section.dart`(1874)、`appearance_background.dart`(1551)、
`main.dart`(1329)、`display_prefs.dart`(1157) —— 四个都超出 `≤1000` 豁免带；
`appearance_section.dart`(709)、`live2d_bridge.dart`(547) 只需豁免头注（已有）。

## 2. rc.7 波次（按文件零重叠切分，**本版实际排期**）

| 波次 | 内容 | 独占文件 | 状态 |
|---|---|---|---|
| **R7-0** | Stage C/D 提示词重新接地（**本文**）+ AGENTS「当前版本」对齐 rc.6 | docs | ✅ 本文 |
| **R7-a** | 审计主发现 **F-0005-2**：每个 `text_delta` 重建整棵 AppShell 子树（含折叠态设置分区） | `app/app_shell.dart`、`main.dart`、`chat/**` | 并行中 |
| **R7-b** | **4 条假绿灯**改可失败断言（F-0005-1/3/6/7）+ 判别力自证 | `ui/message_bubble.dart` + 4 个既有测试文件 | 并行中 |
| **R7-c1** | 正确性：`F-0007-1`(本地失败不回落相位) / `F-0008-1`(心跳看门狗) / `F-0012-1`+`F-0003-2`(自检成败靠文案猜) | `chat/**`、`api/**`、`settings/sections/*` | 待排 |
| **R7-c2** | `F-0001-1`(错误横幅不开面板) / `F-0004-1`(记忆面板不随会话桶) | `app/**`、`settings/sections/memory_panel*` | 待排 |
| **R7-c3** | `F-0017-1`(后端 performance 段零解析) / `F-0021-1`(行内码字体回退，红线 L) | `settings/sections/llm_section.dart`、`ui/**` 行内码渲染 | 待排 |
| **R7-d** | Stage C1 文档单一化（AGENTS 以 main 版为底 + 变更历史补 rc.2/3/4 + v0.3.0 矛盾） | `AGENTS.md`、`docs/**` | 待排 |
| **R7-e** | Stage C3 结构拆分 + **W4 五项裁决** | `settings/sections/**` | 待排（依赖 c 组收口） |
| **R7-f** | Stage C2 仓库卫生（**只有编排者能做**）+ rc.7 收口 | git 破坏性操作 | 待排 |

## 3. 关于「是否拆 rc.8」的**预先裁决**（按 `ORCHESTRATOR-PROMPT` §3.3）

- **rc.7 = 正确性与诚实性**：R7-a + R7-b + R7-c1/c2/c3（及 R7-d 文档单一化，若余量允许）。
- **rc.8 = 结构与仓库卫生**：R7-e（C3 拆分 + W4 裁决）+ R7-f（C2 仓库卫生）+
  剩余「其余按序」的 P2/P3 发现（见 `TRIAGE-0.2.0-audit-45-2026-09-28.md`）。
- 理由：C3 要动 4 个超长文件（1800+ 行），与「正确性修复」混在一版里，出问题时**无法区分**
  「是重构弄坏的还是修 bug 弄坏的」——而本项目口径是「**宁可多一版，不要把不相关的风险挤进同一版**」。
- **两版都必须各自完整收口**（门禁 + 肉眼 + 审查记录 + tag），不留半版。
