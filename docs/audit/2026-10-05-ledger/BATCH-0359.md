# BATCH-0359 · ⭐ **同一个标志既驱动「失败轮」外观、也驱动「重试」可用性**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_panel` 的**断线/失败重试**接线

## 跑的命令（全部只读）
```
grep -nE "onRetryConnection|onRetryLast|ensureConnected" lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **一个标志、两种用途**
```
:53   this.onRetryLast,
:97   final **VoidCallback?** onRetryLast;
:243  onRetry: **m.isPlaceholder ? onRetryLast : null**,
```
三个可核点：
1. ⭐⭐⭐ **重试只对 placeholder 消息开放** ⇒ ⇒ **非失败的消息拿不到重试**
   ⇒ ⇒ ⇒ **按钮与状态由**同一个标志**驱动** ⇒ **不可能出现「有重试但没失败」或「失败了但没重试」**
2. ⭐⭐ **而它与 B0235 核的 `failed` 视觉处理同源**：
   B0235 核过 `final bool failed = message.isPlaceholder;`（`:69`）⇒⇒
   **`m.isPlaceholder` 同时决定「这是不是一条失败轮」与「这条能不能重试」**
   ⇒ ⇒ **同 B0335「相位只被派生一次」是同一形状**（**一个标志 ⇒ 视觉 + 动作**）
3. ⭐ **`onRetryLast` 是可空的** ⇒ ⇒ 「**有失败轮但没有重试动作**」是**合法状态**（宿主没接线时）
   ⇒ ⇒ 又一「空态被定义」族（B0326 的 `onOpenSessions` · B0325 的 `onDismiss`）
⇒ ⇒ **0 findings**；⇒ ⭐ **而本批补上了一个此前只是「视觉」的动作接线**：
**失败轮的「重试」不是第二套判断，而是同一个 `isPlaceholder` 的第二个用途**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体 ·
   `onRetryLast` 在 `main.dart` 的接线体
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1810 / `live2d_stage` ~580 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
