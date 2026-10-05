# BATCH-0333 · ⭐⭐⭐ `deriveUiPhase` **七行** —— 而它的顺序是**策略**，且**与 B0305 那条判据同族**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `ui_phase.dart` 的**派生本体**（B0332 留）

## 跑的命令（全部只读）
```
sed -n '/^UiPhase deriveUiPhase/,/^}/p' lib/state/ui_phase.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **七行就是全部策略**
```dart
UiPhase deriveUiPhase(UiSignals s) {
  if (s.errorActive) return UiPhase.error;        // ①
  if (!s.wsConnected) return UiPhase.offline;     // ②
  if (s.interrupted)  return UiPhase.interrupted; // ③
  if (s.voiceActive)  return UiPhase.speaking;   // ④
  if (s.turnActive)   return UiPhase.thinking;   // ⑤
  return UiPhase.idle;                            // ⑥
}
```
四个可核点：
1. ⭐⭐⭐ **优先级本身就是策略，而它是一条严格链**
   ⇒ ⇒ **每个相位只由**一个**信号决定**（不是组合判断）
   ⇒ ⇒ ⭐⭐ **于是「谁优先」只需要读一遍这七行，不需要一张真值表**
2. ⭐⭐ **顺序可解释，且是「信息量递减」**：`error` 最前（**有错时别的都不重要**）→
   `offline`（**没连接时「思考中」是假的**）→ `interrupted` → `speaking` → `thinking` → `idle`
   ⇒ ⇒ **最需要被看见的状态排在最前**
3. ⭐ **`errorActive` 压过 `!wsConnected`** ⇒ ⇒ **断连时报「错误」而非「离线」** ⇒ ⇒ **而这需要一个理由**
   ⇒ ⇒ **头注里那句「6 个项目长出 6 套视觉」（B0332 已核）正是为此**
   ⇒ ⇒ ⭐ **一个七行的函数 + 一段外部统计 = 一套不会长歪的 UI**
4. ⭐⭐ **`interrupted` 在 `voiceActive` 之前** ⇒ ⇒ **「被中断」压过「正在说」**
   ⇒ ⇒⇒ **这是 B0305–B0311 那条「谁造成了这一轮」判据的同族**：
   **原因优先于现象** ⇒ ⇒ 而**它出现在一个纯函数里，而函数上没有一行注释解释这个顺序**
   ⇒ ⇒ ⚠ **诚实标注**：**这个顺序是否被有意论证过，我未核**（`ui_phase.dart` 头注只讲了「谁读它」，
   **没讲「为什么是这个顺序」**）⇒ ⇒ 但**读代码的人能直接看到** ⇒ ⇒ **可审查性不受影响**

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. ⭐ **新增**：`deriveUiPhase` 的**优先级顺序是否被有意论证**（`UiSignals` 的定义与 `deriveUiPhase` 的测试
   **未读**）· `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~460）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
