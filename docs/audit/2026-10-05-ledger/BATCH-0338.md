# BATCH-0338 · ⭐ **四条通道**逐条命名；而「**不要写 `Colors.transparent`**」是**那道门禁的一次具体生效记录**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `state_pill` 的**渲染本体**

## 跑的命令（全部只读）
```
grep -n "filledDot" lib/ui/state_pill.dart
sed -n '192,212p' lib/ui/state_pill.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **门禁在这里生效过**
```dart
// **形通道第二维：实心/空心点。**                                    // :193
Container(
  width: 5, height: 5,
  decoration: BoxDecoration(
    shape: BoxShape.circle,
    // `null` = 不填充（只用描边表达「静止」）。
    // **不要**写 `Colors.transparent`：那是**裸色字面量**，
    // **设计令牌门禁会拦**（颜色只能来自 ColorScheme 或 AppPalette/AppColors）  // :197-200
    color: view.filledDot ? view.tone : null,
    border: Border.all(color: view.tone),                             // :202
  ),
),
```
四个可核点：
1. ⭐⭐ **四条通道被逐条命名，且顺序是「形状维在色之前」**：
   **① 图标**（B0337：每视图一个）· **②「形通道第二维：实心/空心点」**（本批）· **③ 文字** ·
   **④ 色**（B0337：分三档）⇒ ⇒⇒ **「颜色只是其中一维」被写成了一句编号**
2. ⭐⭐⭐ **`null` 而不是 `Colors.transparent`**，理由是**门禁会拦**
   ⇒ ⇒ ⭐⭐ **B0204/B0205 核过的 `design_tokens_lint_test.dart`（扫裸色/裸字号）**
   **在这一行有一次具体的生效记录** ⇒ ⇒ **门禁不是抽象规则，它在这一行拦住过一个写法**
3. ⭐ **`border` 恒定** ⇒ ⇒ **空心点仍有轮廓** ⇒ ⇒
   「**静止由『不填充』表达，而不是『没有东西』**」⇒ ⇒ **与 B0245「不假装」同族：空心不是缺席，是另一种状态**
4. ⭐ **实心/空心直接编码「在不在动」**：`offline`·`speaking` **实心**（连接中 / 说话中）
   vs `idle`·`thinking`·`interrupted`·`error` **空心** ⇒ ⇒
   **两个「有事情在动」的相位实心、四个「静止」的相位空心** ⇒ ⇒ **这一维是二值、可立刻分辨**

⇒ ⇒ **0 findings**；⇒ ⭐ 而 ② 的价值在于：**我核过那道门禁，但从未见过它生效；
而这里有一次「它拦了什么」的记录** ⇒ ⇒ **门禁的存在与门禁的用法，是两件事**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~455）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
