# BATCH-0417 · ⭐⭐⭐ **「为什么不拆」有两条理由，分属两个世界** —— 而我此前只见过第一个世界

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `shell_admin` 的**形态与归属**（B0416 留）

## 跑的命令（全部只读）
```
sed -n '1,20p' lib/app/shell_admin.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0416 的机制解释 + 一条新的可迁移观察**（0 条新发现）
> 「管理面接线（**`part of` 组合根**）……从 `main.dart` **原样搬出**的行为边界之一 ——
> 这里只有「哪个回调接哪个 API」以及落地时的 `_refresh()`，**没有新逻辑**。
> **状态字段仍留在 `main.dart` 的 `_ShellRootState` 里**：**Dart 私有是**库级**的**，
> **且其中几个被 `test/transient_results_test.dart` 的源码扫描钉住**。」              // :1-7
```
四个可核点：
1. ⭐⭐⭐ **它是 `part of` 组合根、不是独立类** ⇒⇒ **⇒ 所以 `setState` 只能是 `_ShellRootState.setState`**
   ⇒⇒⇒⭐ **⇒ B0416 的「零个 `setState`」现在有了**机制解释**（不是纪律、是结构）**
2. ⭐⭐⭐⭐⭐ **而「状态字段仍留在 `_ShellRootState`」有两条理由，各不相同**：
   | # | 理由 | 属于哪个世界 |
   |---|---|---|
   | ① | 「**Dart 私有是库级的**」⇒ **拆成独立协作者就够不到那些私有字段** | ⭐ **语言约束** |
   | ② | 「**其中几个被 `test/transient_results_test.dart` 的源码扫描钉住**」⇒ **拆走字段会让那条测试变红** | ⭐⭐ **工具链约束** |
   ⇒⇒⇒⭐⭐ **⇒ 两条理由一条来自语言、一条来自测试**
3. ⭐⭐⭐ **而「用 `part` + 扩展方法而不是独立协作者」被指到 `main.dart` 头注**（又一次「指路」）
4. ⭐⭐ **「**原样搬出**」被写明**（2026-09-13 rc.3 N1）⇒⇒ **搬移是**一次声明过的动作**、不是渐进的漂移**
   ⇒⇒ **同 B0393 核的「代码改了、注释也改」是同一种自觉**

⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是一条可迁移观察**
> **「为什么不拆成独立类」的理由有两类，而我此前只见第一类：**
> **① 语言约束**（`private` 是库级 ⇒ 拆了就够不到）·
> **② 工具链约束**（**测试逐字扫字段名 ⇒ 拆了就变红**）
> ⇒⇒ **② 的形状是：「这个字段名被一条测试的 `contains` 钉住了」**
> ⇒⇒⇒⭐ **⇒ 与 B0297 核的「转向消费要搜 `onAck:` 而不是 `onAck`」同源**：
> **结构测试会在代码之外创造约束** ⇒⇒⇒⇒ **⇒ 而这类约束**不会出现在任何设计文档里**

⇒ ⇒⚠ **而这带出一个可核的观察**：本仓有**两条**结构测试、**两种钉法**
| 钉法 | 位置 | 钉住什么 |
|---|---|---|
| **方法调用** | `chat_notice_test.dart:150-167` | `controller.contains('settleTurn(')`（B0315 已核） |
| ⭐ **字段名** | `transient_results_test.dart`（**本批只见其名、未读断言体**） | `_ShellRootState` 的**字段名** |

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `test/transient_results_test.dart` 的**断言体**（它钉住了 `_ShellRootState` 的**哪几个字段** · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
