# BATCH-0422 · ⭐⭐⭐ **那个论证是三对一** —— 而大多数「不拆文件」的注释只写代价那一侧

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `dev_tools_section` 的**头注**（本仓最大的面板文件 ~1780）

## 跑的命令（全部只读）
```
sed -n '1,26p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一个三对一的论证**（0 条新发现）
> 「「Mod」「诊断」「模型库」「开发模式」四个分区的实现。
> **合在一个文件里：它们共享同一套「列表 + 动作 + 内联结果」的骨架，
> 拆成四个文件只会让同一段列表渲染代码出现四遍。每个类都短、职责单一，
> 找起来靠类名即可。**」                                                            // :1-7
```
四个可核点：
1. ⭐⭐⭐⭐ **而它给出的是「**重复四遍**」这个具体的代价** ⇒⇒ **不是「不够整洁」**
   ⇒⇒⇒⭐ **⇒ 而「每个类都短 · 职责单一 · 靠类名找得着」是**代价那一侧的三个反驳**
   ⇒⇒⇒⇒⭐⭐ **⇒ 合起来是一句「**我知道这样不好，但三处抵消了两处**」**
   ⇒⇒⇒⇒⇒ **⇒⇒ 这与 B0393 核的「≤1000 豁免，理由写在这里」是同一动作**：
   **⇒⇒⇒⇒ **⇒ 而这一条更进一步 —— 它连「反驳那一侧」都写了**
2. ⭐⭐⭐ **「同一套「列表 + 动作 + 内联结果」的骨架」被抽名了** ⇒⇒ **三段式骨架**
   ⇒⇒ **⇒ 同 B0230 核的「列表 + 动作 + 内联结果」（记忆面板）** ⇒⇒⇒⭐
   **⇒ 而「抽出一个名字」本身就让它可被复用** ⇒⇒⇒
   **⇒⇒ 而这是我第一次在本仓看到三段骨架被**命名**并跨文件复用**
3. ⚠ **import 区的顺序是乱的**（`'../mods/mod_panel.dart'` 在 `'director_observer_section.dart'` **之前**）
   ⇒⇒ **⇒ 而 Dart 有 `directives_ordering` lint** ⇒⇒⇒⚠ **⇒ 但我未核 lint 配置**
   ⇒⇒⇒ **⇒ 按「无证据不记发现」不记 FINDINGS** ⇒⇒ **⇒ 只记为一条待核观察**
4. ⭐ **`show PresetStatus, kDefaultExpressionIntensity, kDefaultPresetIntensity`** ⇒⇒
   **⇒ 三个具名导入、只为了让整个 `live2d_stage.dart` 不进依赖图** ⇒⇒⇒⭐ **依赖最小化**（B0409 族）

⇒ ⇒⭐⭐⭐ **而本批最值钱的是第 1 点**
> **「合在一个文件里」的论证是三对一**：**代价（重复四遍）对上三处反驳**
> （**类短 · 职责单一 · 靠类名找**）⇒⇒⇒⭐⭐
> **⇒ 而大多数「不拆文件」的注释只写代价那一侧** ⇒⇒⇒⭐
> **⇒⇒ 写上反驳那一侧，这个决定才可以被重新评估** ⇒⇒⇒⭐⭐
> **⇒⇒⇒ （而 B0393 核的「豁免理由」只写了理由那一侧 —— **⇒ 两条合起来才是完整形状**）**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `analysis_options.yaml` 的 lint 集合（**`directives_ordering` 是否启用 · 未读** ⇒ 上面第 3 点待此结清）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
