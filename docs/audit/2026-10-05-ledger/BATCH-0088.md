# BATCH-0088 · `appearance_background.dart` 的结构真相 —— **0 条新发现**（一次高危假发现被我拦下）

Phase 1 · 域覆盖 · 前端 `settings/sections/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/sections/appearance_background.dart` — 1551（读 8-45 头注全段 + 定点 275-330 / 388-415 / 925-974 / 1194-1305）
2. `shell/flutter/lib/settings/sections/appearance_section.dart` — （定点 211：`_BackgroundBlock` 的构造点）

## 跑过的命令（全部只读）
```
grep -n "^class [A-Z]" appearance_background.dart          # 公开类枚举
grep -n "onAddPattern|onRemoveItem|onRemoveMany|onPreviewItem|onReorderItem" appearance_background.dart
sed -n '388,415p' appearance_background.dart ; sed -n '925,974p' appearance_background.dart
grep -n "_BackgroundBlock(" appearance_background.dart ; grep -rn "_BackgroundBlock" shell/flutter/lib/ shell/flutter/test/
sed -n '8,45p' appearance_background.dart ; ls -la sections/ | grep appearance
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批：0 条新发现，但**拦下一次高危假发现**
### 我差一步就记下「1551 行背景 UI 从未被构造」
推理链（**每一步都「有依据」，但结论是错的**）：
1. `grep -n "_BackgroundBlock("` 只命中 **1 处** = 自己的构造声明（:276）⇒ 我判「无构造点」；
2. 类名带下划线前缀 ⇒ **私有** ⇒ 我判「文件外无法构造」；
3. 两者相加 ⇒ 「**这个私有类从未被实例化 ⇒ 整块背景 UI 是死代码**」——
   若成立，这是本审计最重的一条（1551 行、AGENTS 豁免上限的两倍、且**界面功能整体不可达**）。
**真相**：`appearance_background.dart:44` 末行 —— **`part of 'appearance_section.dart';`**
⇒ 它是 `appearance_section.dart` 的 **`part` 文件**，**共享整个 library 的私有命名空间**
⇒ `appearance_section.dart:211` 正在构造 `_BackgroundBlock(`。
⇒ **我犯的错是「把「文件私有」当成「库私有」**。这是**规则 3 的第 4 次同族**，
也是**最险的一次**（结论的分量最大、证据看起来最硬）。
⇒ **补充规则 3 变体 4**：**在断言「某类/某符号从未被使用」之前，必须确认文件是
`part` 还是独立库**——`part` 文件里的私有名**对整个 library 可见**，
`grep` 同一文件找不到构造点**不构成**「死代码」的证据。

### 顺带核到：文件头注本身是「诚实标注既有债」的又一样本（正面）
`:22-44`：
- 「# 行数（**如实标注：超 1000，是既有债**）」—— 现状 **1551 行**，
  并给出**规模来源**（`appearance_section.dart` 原本 **1489 行**）与**为何不现在拆**
  （Stage B 口径是「**只搬不改**……抽取那一刻**不许**顺手拆小——那正是
  『搬运 + 重构同时做』的经典事故面」）；
- 且**给出了 Stage C3 的拆分表**：四类内容、每个的**当前实测行数**、**建议去处文件名**
  （`appearance_background_library.dart` / `appearance_background_style.dart`）——
  **「接手的照它拆」**；
- **实测**：`ls sections/ | grep appearance` ⇒ 仓库里**只有** `appearance_background.dart`
  与 `appearance_section.dart` ⇒ **拆分尚未执行**（两个目标文件都不存在）
  ⇒ 头注没有把「计划」写成「已完成」✔（又一次对照 F-0040-01 那种漂移）

### 另一条正面：同文件 :927-929 的无障碍理由
> 「`ReorderableListView` 而不是 `Wrap`：拖动排序是内建能力（自带拖动手柄、**键盘可达**、
> `onReorder` 回调），自己用 `LongPressDraggable` 拼一个**既不完整又没有无障碍**。」
⇒ 选型理由**点名了它换来的是什么**（键盘可达），与 B0028 那条（`ReorderableListView.builder`
的真实工作集）同属「按框架自带能力而非自拼」的判断。

## 未核实项
1. `appearance_background.dart` 16 个类里仍只碰了 6 个
   （`_MoreOptions` 725 / `_ImageTile` 1464 / `_AlignPad` 1491 / `_UsageBar` 1109 /
   `_RowNotice` 1342 / `_ItemStyleEditor` 1366 未读）
2. **第二处「同形」（回调生产未接上 ⇒ 功能静默不可达）** **仍未找到**：
   `_BackgroundBlock` 的 5 个可空回调（`onAddImage` / `onClearLibrary` / `onPickStageImage` /
   `onClearStageImage` / `onRemoveItem` / `onAddPattern`）**在 `appearance_section.dart:211` 的构造
   实参尚未逐个核** ⇒ **B0089 第一件事**（这次我知道该去哪个文件、该看哪一行）
3. `appearance_background.dart:46-275` 未读
4. `settings_controller.dart:120-322` / `:372-409` 未读
5. `background_store_web.dart` 余 ~190 行未读；其余 Mod 面板（4 个 / 2284 行）未读

## 本批新增
**0 条**（+ 拦下一次高危假发现 + 两条正面样本）
