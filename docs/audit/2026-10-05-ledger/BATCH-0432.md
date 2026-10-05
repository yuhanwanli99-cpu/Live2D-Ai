# BATCH-0432 · ⭐⭐⭐⭐ **「判据集中、禁止走私」的**文案版** —— 走私的是文案、不是判据

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— **预设标签的呈现**（B0429–B0431 那条链的界面侧）

## 跑的命令（全部只读）
```
grep -n "presetLabel|labelFor|稳定" lib/settings/sections/dev_tools_section.dart | head -6
sed -n '1243,1250p' lib/settings/sections/dev_tools_section.dart
sed -n '1288,1300p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一个禁令，四个可核点**（0 条新发现）
> 「预设 id → 中文展示名（读 `assets/actions/preset_labels.json`）。
> **缺省空表 = 显示稳定 id（不手写第二套中文标签）。**」                    // :1244-1245
五个可核点：
1. ⭐⭐⭐⭐⭐ **而「**不手写第二套中文标签**」是一句禁令，且它防的是最贵的那类错**：
   ⇒⇒⇒⭐⭐⭐ **两套中文标签 = 两份要同步维护的真相**
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒ 而同步两份的失败模式是「**界面显示了一个渲染面不认识的名字**」**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ 「**判据集中、禁止走私**」的**文案版** —— 走私的是**文案**、不是判据**
2. ⭐⭐⭐⭐ **而「缺省空表」被显式声明为一个合法输入**（`:1210` `presetLabels = PresetLabelTable.empty`）
   ⇒⇒ **⇒ 空表不是「还没加载」而是「永久没有」** ⇒⇒⇒⭐
   **⇒⇒⇒ 而这两种在 B0429/B0430 核的调用侧都表现为 `length == 0`**
   ⇒⇒⇒⭐⭐ **⇒⇒ 而两种在界面上**长得一样**（都是 id）⇒⇒⇒⭐⭐ **⇒⇒ ⇒⇒ 这恰好是对的**：
   **「还没加载」与「表里没有」在这个控件上**不需要区分**，因为**它对用户的意义一样**
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ 与 B0415「降级要只降该降的一处」又一次同族**：
   **⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒⇒ 两个不同原因 ⇒ 同一个显示 ⇒ 而这是有意的、不是省略**
3. ⭐⭐⭐ **而 `clock` 也被注入**（`:1248-1250`「**不在 widget 里写死 `DateTime.now()`**」）
   ⇒⇒ **⇒ 「把不可测的东西变成参数」的**第三处**（timer · throttle clock · **面板时钟**）
4. ⭐⭐⭐⭐ **而「导演可观测」的注释把门控写进**渲染**那一层**：
   「**只在 devMode 下渲染** —— off 时**整块不在语义树**（本 if 块一起消失）」   // :1296-1298
   ⇒⇒⇒⭐⭐ **⇒⇒ 「不在语义树」是**可被测试断言**的说法**（无障碍树里有没有这个节点）
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 而这比「不显示」精确** ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ 「不显示」可以是 `opacity: 0`（仍在树里）**
5. ⭐⭐ **而「数据经单例 `DirectorObserverFeed` 注入（**不改 `shell_settings.dart`**）」** ⇒⇒⭐
   **⇒⇒ 不让数据穿过中间层** ⇒⇒⇒ **⇒⇒ 与 B0409「依赖方向即测试性」同族**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. **「不在语义树」那条断言是否存在**（B0432 第 4 点：**说了、可被断言、但我没找那条测试**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
