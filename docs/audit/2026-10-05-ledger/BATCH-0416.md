# BATCH-0416 · ⭐⭐⭐⭐ **零个 `setState`** —— 「唯一出口」是**结构级**的，不是约定级的

Phase 4 · **证伪**（按 B0415 的教训反向自查：`_refresh()` 是不是唯一的界面刷新入口）

## 跑的命令（全部只读）
```
grep -cn "setState(" lib/app/shell_admin.dart
grep -n "_refresh\b" lib/app/shell_admin.dart | head -4
grep -n -A 2 "setState(" lib/app/shell_admin.dart | grep -vE "setState\(|--|^\s*$" | head -4
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**「唯一出口」是结构级**（0 条新发现）
```
grep -c "setState("  lib/app/shell_admin.dart   ⇒ **0**        ← 全文件零处
grep -n "_refresh"   lib/app/shell_admin.dart   ⇒ :22 :37 :50 …（十几次）
/// :4  「这里只有『**哪个回调接哪个 API**』以及落地时的 `_refresh()`，**没有新逻辑**。」
```
四个可核点：
1. ⭐⭐⭐⭐ **全文件零个 `setState(`，而 `_refresh()` 被调十几次** ⇒⇒ **⇒ 刷新只有一个出口**
   ⇒⇒⇒⭐ **⇒ 「唯一真源」在这里是**结构性的**（B0335 核的相位派生同形）**
2. ⭐⭐⭐⭐ **而 `:4` 的头注把这个文件的自述写成「**没有新逻辑**」** ⇒⇒
   **⇒ 这是一句可核对的自我描述** ⇒⇒⇒ **可核对的方式正是「本文件里没有 `setState`、
   也没有第二套派生」** ⇒⇒⇒⇒ **⇒ 这句话的**结构**与它的**内容**一致**
   （**⇒ 一个自述若无法被机械核对，它就只是姿态；这一句可以**）
3. ⭐⭐ **文件开头是「哪个回调接哪个 API」的对照表** ⇒⇒ **形态 = 接线表 + 落地**
   ⇒⇒ **⇒ B0414 核的「依赖方向即测试性」在这里也成立**（这层不含任何判定逻辑）
4. ⇒ ⇒ ⭐⭐ **而这一批是 B0415 那条教训的**第一次成功应用**：
   > **B0415 我发现「我只看了一条原则的一半」⇒ 本批我按那个教训去查「刷新是不是唯一出口」**
   ⇒⇒ **⇒ 答案是「是、而且强到不需要 `setState`」**
   ⇒⇒⇒⭐ **⇒ 而如果我不查，账上会继续写着「刷新入口未核」**

⇒ ⇒⭐⭐⭐⭐ **而本批最值钱的是第 1 点的可提炼形式**：
> **「唯一出口」有两种强度**：
> **① 约定级**（大家都记得调那个函数）· **② 结构级**（**根本没有别的出口**）
> ⇒⇒ **零个 `setState` = ②** ⇒⇒⇒⭐⭐ **⇒ 而这解释了 B0415 那两次降级为什么能被静态读出来**：
> **⇒ 因为全部状态变化都经由那一个函数，而那个函数只做「通知」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
