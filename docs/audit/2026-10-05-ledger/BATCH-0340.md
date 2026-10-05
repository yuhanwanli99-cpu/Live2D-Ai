# BATCH-0340 · ⭐ 输入区是「**形态即纪律**」的正面对照；而提示与行为**逐字同源**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_panel` 的**输入区**（剩余 ~455 行的关键面）

## 跑的命令（全部只读）
```
grep -nE "onSubmitted|onKeyEvent|Enter|IME|composing|TextInputAction" lib/ui/chat_panel.dart
sed -n '330,350p' lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **正面对照 B0300**
```dart
TextField(
  controller: input, minLines: 1, maxLines: 4,
  textInputAction: **TextInputAction.send**,                  // :347
  onSubmitted: (_) => onSend(),                              // :348
  decoration: const InputDecoration(hintText: '说点什么…'),
),
```
四个可核点：
1. ⭐⭐⭐ **这是 B0300（rc.5 ⑫「**毫无功能的占位 UI**」）的**正面对照****
   ⇒ ⇒ B0300 核过那轮**清掉了无功能的占位**，而**这一处的按钮不是占位**：
   它**外观**像发送（B0300 圆/上箭头）、**多行输入时变成停止**（B0336 已核）、
   **且真的有 handler**（`:348 onSubmitted` → `onSend` → controller 的 `send()`；
   B0130 / B0192 已核它有 `_sending` / phase 门）
   ⇒ ⇒⇒ **形态即纪律**：形状**在编码它做的事**，而行为**与形状同时变**
2. ⭐⭐ **`minLines: 1 / maxLines: 4` ⇒ 多行只增长不提交**
   ⇒ ⇒ **提交键只有输入区右下角那一个 + Enter** ⇒ ⇒ **形态决定了「在哪里能提交」**
3. ⭐⭐ **`TextInputAction.send` ⇒ 平台软键盘的「发送」键与 Enter 同一行为**
   ⇒ ⇒ **桌面 Enter 与移动端软键盘键** ⇒⇒ **不是「桌面能用、移动端不能用」那类不一致**
4. ⭐⭐ **提示与行为**逐字**同源**：`:348 onSubmitted` 文档写着「**发 Enter**（软键盘的发送键等价」）
   ⇒⇒ **提示（`:362`「发送（Enter）」）与行为引的是同一句文档** ⇒⇒ **不是两张表**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的收获是「对照」而非「新发现」**：
**我需要知道 B0300 清理之后剩下的是什么形状的东西** ⇒ 而答案是「**形状 = 行为的编码**」

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~450）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
