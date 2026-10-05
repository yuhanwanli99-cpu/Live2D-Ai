# BATCH-0378 · ⭐⭐ **等回执才说真话** —— 而「不发数值」这条让**真值只有一个来源**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 舞台**缩放**（B0000 ⑧ 记过它「永远是 `—`」⇒ 修后呈现未核）

## 跑的命令（全部只读）
```
grep -nE "zoom|scale|读数" shell/flutter/lib/live2d/live2d_stage.dart | head -8
sed -n '460,486p' live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **「真值只有一个来源」被协议本身保证**
```dart
/// 协议 v1 stage-zoom（in / out /reset）。                                 // :462
/// 渲染面自己算缩放（**±10%，clamp 0.5..2.0**），所以**不发数值**；
/// 应用后的真值从 [`lastAck`] 取（`scale`/`offsetX`/`offsetY`）。            // :464-465
Future<void> sendStageZoom(String dir) async {
  await _bridge?.sendStageZoom(dir);
  _captureAck();                                                         // :469
}
```
四个可核点：
1. ⭐⭐⭐ **「不发数值」是协议的选择** ⇒ ⇒ **宿主没有任何地方能算出「缩放后的值」**
   ⇒ ⇒⇒ **真值只有渲染面一个来源** ⇒⇒ **而显示层只能读 `lastAck`**
   ⇒⇒⇒ ⭐ **这与 B0299 核的 `swapModel` 是同一族**（`Future<bool>` + **等到渲染面 `loaded` 回执**才说成功）
   ⇒⇒⇒ **而这里更进一步**：`swapModel` 至少还有一个「本地已登记」的状态，
   **而 `stage-zoom` 连那个都没有**（**协议层就没有本地值**）
2. ⭐⭐ **而 clamp 写在头注里**（「±10%，clamp 0.5..2.0」）⇒ ⇒ **边界在协议文档上，不在代码里**
   ⇒ ⇒ **宿主不需要知道边界** ⇒ ⇒ **同 B0364「界面的边界 = Mod 声明的边界」的另一应用**（**渲染面侧**）
3. ⭐⭐ **B0000 ⑧ 那个缺陷的修法在这里可见**：「缩放读数永远是 `—`（**没人读渲染面首帧的 `stage-ack`**）」
   ⇒ ⇒ **修法就是 `_captureAck()`** ⇒⇒ **读数从「本地算」改成「读回执」** ⇒⇒ **与第 1 点同源**
4. ⭐ **而 `swapModel` 的头注仍带着那句**：「调用方拿 `false` 时应**如实说『已登记，但舞台未确认』**」
   ⇒ ⇒ **又一个 P31**（错误/未完成状态要指向用户能做的事 · B0352 立）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批把三条线接成了一条**：
> **「不自己算」是让「真值只有一个来源」的结构前提**
> ⇒ B0299（`swapModel` 等 `loaded`）· B0378（`stage-zoom` 不发数值、只读 `lastAck`）·
> **B0000 ⑧ 的修法（读数改读回执）** 全部是同一条纪律的三个应用

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `_captureAck` / `lastAck` 的**实现本体**（首帧回执怎么进 `_captureAck`）——
   B0000 ⑧ 的回归测试**是否存在**（**`stage-ack` 读数零命中那一批之后未再核**）
2. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~570 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
