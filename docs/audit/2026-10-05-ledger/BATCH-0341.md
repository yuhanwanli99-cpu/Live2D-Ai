# BATCH-0341 · ⭐⭐⭐ **代码主动声明了自己的一个组件是「装饰性」的** —— 这在 UI 代码里几乎从不被说

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_panel` 的 `_ListenButton`（按住说话）

## 跑的命令（全部只读）
```
grep -nE "class _ListenButton|onPressStart|onPressRelease|pttActive|GestureDetector|onLongPress|onTap" lib/ui/chat_panel.dart
sed -n '384,420p' lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一次极诚实的自陈**
> 「- 不支持 / 未接线 → **禁用并说明**，不摆一个按不动的入口。」            // :384
> 「实现用 [`GestureDetector`] 的 **down/up/cancel** + **计时器**判定「点按还是按住」，
> **而不是 Material 的 `onLongPress`（它 500ms 才触发，200ms 的按会什么都不做）**。」  // :386-387
> 「视觉仍是 Material 按钮（文字按钮，项目口径），但**指针由外层 `GestureDetector` 独占
> （[`AbsorbPointer`]）—— 所以按钮的 `onPressed` 只是「看起来可点」**。」         // :388-389
> `dispose() { _holdTimer?.cancel(); … }`                                     // :413-417
四个可核点：
1. ⭐⭐⭐ **「按钮的 `onPressed` 只是『看起来可点』」被明说了**
   ⇒ ⇒ 一个 Material 按钮的 `onPressed` **被故意留空/无用**，因为真正的指针处理
   **由外层 `GestureDetector`（`AbsorbPointer`）独占**
   ⇒ ⇒⇒ ⭐ **而它没有把这伪装成「一个正常的按钮」**
   ⇒ ⇒ **同 B0000 ⑫「毫无功能的占位 UI」的正面对照**（B0300 核过那轮清理 · B0340 核过对照面）
2. ⭐⭐ **两个手势靠计时器区分，理由是 `onLongPress` 的 500ms 门槛**
   ⇒ ⇒ 「**200ms 的按会什么都不做**」⇒ ⇒ **一个具体的用户动作被点名** ⇒ ⇒ **理由是手感的量化**
3. ⭐ **`down/up/cancel` 三个事件**（`cancel` 在列）
   ⇒ ⇒ **拖走/被打断也会收尾** ⇒ ⇒ **收尾路径被显式枚举**
   ⇒ ⇒ 与 B0232 那两处 `mounted` 检查**同族**
4. ⭐ **`dispose()` 取消 `_holdTimer`** ⇒ ⇒ **异步资源在死亡路径上有明确处置**
   ⇒ ⇒ 与 B0290「`!mounted` 时归还 transport」**同族**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的收获是「一种罕见的自陈」**：
**一个组件明说自己的一半是装饰性的** —— **因为不说的代价是「下一个读代码的人会以为 onPressed 是活的」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~445）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
