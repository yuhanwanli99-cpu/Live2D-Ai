# BATCH-0607b 落盘（极简）· Dart 界面层三巨头：两个已越过 500 上限，第三个是聚合入口

## 编号说明
`BATCH-0607.md` 已被**另一批**占用（`ignition-precheck.sh`，37 行，早于本轮）
⇒ 本批改落 `BATCH-0607b.md`（沿用 B0632 的先例）。

## 命令（只读）
```
sed -n '1,12p'  shell/flutter/lib/settings/sections/dev_tools_section.dart
grep -n "^class " 同上
sed -n '1,10p'  shell/flutter/lib/settings/sections/appearance_background.dart
grep -n "^class " 同上
```

## `dev_tools_section.dart`（1874 行）
```
:1 /// 「Mod」「诊断」「模型库」「开发模式」四个分区的实现。
:3 /// 合在一个文件里：它们共享同一套「列表 + 动作 + 内联结果」的骨…
:4 /// 拆成四个文件只会让同一段列表渲染代码出现四遍。每个类都短、
:5 /// 职责单一，找起来靠类名即可。
```
**类清单（15 个）**：
```
AdminRow 33 · AdminEmpty 106 · ModelsSection 144 · _ImportModelField 266(+State 276)
ModsSection 429 · _ModConfigTile 575(+State 599)   ← 单个 State 占 599→1038 = 440 行
DiagnosticsSnapshot 1039 · DiagnosticsSection 1047 · DeveloperSection 1199
DebugPanels 1341(+State 1385)
```
**⇒⇒⇒⇒⇒ 三点**
1. **`:3-5` 给出了「合在一个文件」的显式理由**（共享同一套列表渲染骨架）
   ⇒⇒ **⇒⇒⇒⇒⇒⇒ 判据：大文件不必然是债**；**要问「它的边界能不能被说出口」**。
2. **⇒⇒⇒⇒⇒⇒ 但这个理由只覆盖「四个分区同骨架」**，
   而 `_ModConfigTileState` **一个人占 440 行**（`ModsSection` 的内部实现）
   ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒ 判据：「骨架共享」能解释同层聚合，解释不了单个 widget 内部的 440 行**
   ⇒⇒ 该 State 是本文件的真正重心。
3. `DiagnosticsSnapshot 1039` 与下一类 `DiagnosticsSection 1047` **相距 8 行**
   ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据：行号间距能告诉你「哪些是实体、哪些是壳」**
   （B0586「按行号间距读结构」的用法）。

## `appearance_background.dart`（1551 行）
```
:1 /// 「外观与互动」分区的**背景域**（2026-09-28 · Stage B · R6-b 第 0 步抽…
:3 /// # 这个文件是怎么来的
:5 /// `appearance_section.dart` 一度把「外观 / 舞台单图轮播 / 舞台与口型 / …
:6 /// 四个域全装在一个文件里（**1489 行**，超 ≤500 上限且不满足 ≤1000…
:7 /// 本文件是它的 **`part`**（不是独立库：私有 widget 名与分区共用一个…
:9 /// 承载**壳背景**那一整块：来源与选图、背景库管理与预览、逐图样…
:10 /// 铺法 / 位置 / 不透明度 / 遮罩 / 模糊、壳背景轮播。
```
**类清单（12 个）**：`BackgroundRuntimeScope 60` · `_CommitSliderField 163` ·
`_BackgroundBlock 275` · `_MoreOptions 725` · `_LibraryManager 799` ·
`_StyleOverrideRow 1036` · `_UsageBar 1109` · `_LibraryRow 1157` ·
`_RowNotice 1342` · `_ItemStyleEditor 1366` · `_ImageTile 1464` · `_AlignPad 1491`

**⇒⇒⇒⇒⇒⇒⇒ 关键一条**：**`:5-6` 自己写着「1489 行，超 ≤500 上限且不满足 ≤1000」**
⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据：抽文件时把「原文件超了上限」写进新文件头注，
等于把违规记录留在现场** —— 与 B0500「一条禁令要带后果」**同族**，
**是本仓少见的自证式治理**。

**⇒⇒⇒⇒⇒⇒⇒⚠ 同时暴露一条**：该文件是 `appearance_section.dart` 的 **`part`**
⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒ 下一批要核的就是宿主 `appearance_section.dart` 现在多大**
⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒⇒ 判据：抽文件的头注要写「拆完后宿主还剩多少」——
本仓这两处头注都没交代**。

## 三巨头的共同形态
1. **都 >500 行**（1874 / 1551 / 1344）⇒⇒ 而 500 上限按 AGENTS 是 **Rust-only**
   （B0007 / B0569 已核）⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 无违规**
2. **两个有「为什么合 / 为什么拆」的头注** ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒ 本仓文档纪律的高点**
3. **⇒⇒⇒⇒⇒⇒⇒⇒⇒ 唯一可疑处**：拆文件后**宿主的体量**没在任何头注里交代

## 未核
`appearance_section.dart` 当前行数与剩余域（下一批）·
`main.dart`（1344 行）的头注与结构 · `_ModConfigTileState` 那 440 行

## 补核：宿主 `appearance_section.dart` = **709 行**（本批当场核完）
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒ 1489 → 709 + 1551（part）⇒ 拆完后宿主仍有 709 行**
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒⇒ 判据成立**：**抽文件只解决了「一个域太胖」，
没解决「宿主仍大于 500」** ⇒⇒ 而 500 是 Rust-only（Dart 不受约束）
⇒⇒⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ 所以这不构成违规，构成一个「是否值得继续拆」的判断题**
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒⇒⇒ 本审计不下这个判断**（Dart 无行数门禁，
而 709 行的宿主已经把四个域拆成四个 part —— 见下）。
⇒⇒⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒⇒ 下一批核 `appearance_section.dart:1-12` 的头注**：
它是否也交代了「拆完还剩哪些域 / 为什么这几个够小」。

## 编号纪律补充（本次撞车的收获）
⇒⇒⇒⇒ 我按 STATE 头递增到 BATCH-0607，**而 `BATCH-0607.md` 早于本轮就存在**
（内容是 `ignition-precheck.sh`）⇒⇒ **⇒⇒⇒⇒⇒ 根因：STATE 头是「我以为的下一个」，
而文件名是「历史上已占用的集合」** ⇒⇒ **⇒⇒⇒⇒⇒⇒⇒⇒ 判据（强化 B0574 的「落盘前先 ls」）：
`ls` 之后若发现已存在，**不要改 STATE 的编号去迁就文件**，
**也不要覆盖**；改落 `b` 后缀并在本文件头注明撞车事实**（B0632 先例）。
