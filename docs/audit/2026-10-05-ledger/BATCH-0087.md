# BATCH-0087 · `appearance_background.dart` —— ⭐ **F-0034-01 升级为在线 P2**

Phase 1 · 域覆盖 · 前端 `settings/sections/`（1551 行，前端最大未审文件）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `shell/flutter/lib/settings/sections/appearance_background.dart` — 1551
   （**结构枚举 16 个类** + 定点 388-415 / 957-961 / 1194 / 1263 / 1301）

## 跑过的命令（全部只读）
```
grep -n "^class |^/// |^// ---" appearance_background.dart        # 结构枚举（读到类声明行）
grep -n "styleChanged|onStyleChanged" appearance_background.dart
grep -rn "onItemChanged" shell/flutter/lib/                      # 三形态全覆盖
sed -n '388,415p' appearance_background.dart ; sed -n '957,961p' appearance_background.dart
grep -n "styleOverride|perItemStyle|itemStyles" display_prefs.dart   # 独立样式存储？零命中
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0034-01 P3「潜在」→ P2「在线」**（本批 0 条新缺陷）
六跳接线全部核到行（见 FINDINGS）：`onItemChanged`（:393-398，**生产代码**）
→ `onStyleChanged` 派生（:957-961）→ 按钮渲染条件（:1263/:1301）→ `onChanged`
→ `shell_prefs.dart:146` 的总闸 `if (next == widget.prefs) return;`
→ `display_prefs.dart:970` 的 **id-only `sameAs`**。
⇒ **只改逐图样式 ⇒ id 不变 ⇒ `==` 判等 ⇒ 总闸早退 ⇒ 什么都不发生。**
配套核验：`DisplayPrefs` **无独立样式存储**（三个候选名零命中），
而样式**住在 `BackgroundImage` 内**（B0072 原文已核）⇒ 没有第二个字段能判出差异。
**与 F-0036-01（口型/缩放静默不刷新）同一族。**

## ⭐ 方法论教训（第三次同族，且最贵）
我打「潜在」标签时核的是**按钮的渲染条件**，没核**那个回调值在生产里是否非空**。
⇒ **规则 3 补一个变体**：「条件依赖某值」时必须追到**赋值点**并确认生产路径非空；
只读条件表达式 = 只证明「它**可能**是 null」。
**方向警告**：这类错误也会**低估**（本批），而**低估更危险** ——
它让一个在线缺陷以「潜在」的名义长期留在账本里。

## 未核实项
1. `appearance_background.dart` 其余 ~1500 行未读（16 个类里只碰了 6 个：
   `_BackgroundBlock`(275) / `_MoreOptions`(725) / `_LibraryManager`(799) / `_UsageBar`(1109) /
   `_LibraryRow`(1157) / `_ImageTile`(1464) / `_AlignPad`(1491) 未读）
2. `appearance_background.dart:1-60`（头注）与 `:60-163`（`BackgroundRuntimeScope`）未读
3. §5.3 第 2 条的**规格原文**未读（本批只引了代码里的「§5.3 第 2 条」标注）
4. `settings_controller.dart:120-322` / `:372-409` 未读
5. `background_store_web.dart` 余 ~190 行未读
6. 其余 Mod 面板（4 个 / 2284 行）未读；`app_shell.dart` 全文未读

## 本批新增
**0 条新缺陷** ｜ **1 条既有发现升级：F-0034-01 P3→P2 且「潜在」→「在线」**
