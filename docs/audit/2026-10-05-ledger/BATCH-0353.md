# BATCH-0353 · ⭐⭐ **「新消息把上翻的���户拽回底部」这个缺陷，在结构上不存在**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_panel` 的**消息列表方向**

## 跑的命令（全部只读）
```
grep -nE "ScrollController|animateTo|jumpTo|reverse|_scroll" lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **又一次「结构预防」**
```dart
reverse: true,        // :235
// 流式提示：贴在做消息列表下方（**列表是 reverse 的**，视觉上就……）   // :253
```
四个可核点：
1. ⭐⭐⭐ **列表是 `reverse: true` ⇒ 新消息在 `index 0`（视觉底部）**
   ⇒ ⇒ **新消息不需要滚动** ⇒ ⇒ **「新消息把上翻中的用户拽回底部」这个缺陷在结构上不存在**
   ⇒ ⇒⇒ ⭐ **不是因为加了判断，是因为**不需要判断**
2. ⭐⭐ **`:253` 的注释说明了同一件事**：「流式提示：贴在做消息列表下方（**列表是 reverse 的**）」
   ⇒ ⇒ **「下方」在代码里是 index 0** ⇒ ⇒ **注释在解释一个由 `reverse` 带来的便宜**
3. ⭐⭐ 这是**「结构预防」家族的又一例**：
   B0328「两层真相**读同一个插值值**」· B0290「重建后**重放当前状态**」·
   B0335「相位**只被派生一次**」· **本批「列表方向让『底部最新』免费」**
   ⇒ ⇒⇒ **共同形状：不是「处理症状」，是「让症状无法发生」**
4. ⭐⭐ **而它与 B0243 的上界是同一个界面习惯的两半**：
   B0243「超出后**丢最旧的**」保成本（`ListView.builder` 虚拟化）·
   **本批 `reverse: true` 保语义**（底部是最新）⇒⇒ **一个用数据顺序实现 · 一个用上界实现**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批把一个我一直默认「需要处理」的 UX 问题，变成了一次结构选择**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~430）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
