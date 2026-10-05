# BATCH-0397 · ⭐⭐ **`reason` 有五个值（含 `'added'`）** —— 而 UI 的 switch 只写四个，**因为穷举完整靠外层分支保证**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `appendToStagePlaylist` 的**本体**（B0396 留）

## 跑的命令（全部只读）
```
sed -n '1085,1120p' lib/settings/display_prefs.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**四态按固定顺序判定，失败一律「列表原样不动」**（0 条新发现）
```dart
if (stageImage 空)              → { playlist: **current**,        added: false, reason: '**empty**' }
if (dataUrl.length > kStageImageMaxChars)      → { playlist: **current**, added: false, reason: '**item_too_large**' }
if (current.length >= kStagePlaylistMaxItems)  → { playlist: **current**, added: false, reason: '**limit_reached**' }
int total = 0; for (final item in current) { total += item.length; }              // :1102-1105
if (total + dataUrl.length > kStagePlaylistMaxChars) → { playlist: **current**, added: false, reason: '**budget_exceeded**' }
return { playlist: <String>[...current, dataUrl], added: true,  reason: '**added**' };
```
四个可核点：
1. ⭐⭐ **四个失败态按固定顺序判定**：空 → 单张太大 → **张数**满 → **总长**满
   ⇒⇒ **先便宜的判据在前** ⇒⇒ 而 **UI 侧（B0396）的 switch 顺序与之一致** ⇒⇒ **两边同序** ⇒⇒ **文案与判据同源**
2. ⭐⭐⭐⭐ **`reason` 有**第五个值 `'added'`** ⇒⇒ **成功也是一个 reason**
   ⇒⇒⇒ **类型是「结果」不是「错误」** ⇒⇒⇒ **同 B0335「六个相位全可达（含 `idle`）」的家族** ——
   **成功被建成一个具名状态，而不是「没有错误」**
   ⇒⇒⇒⭐ **而 B0396 的 switch 只写了四个失败分支 + `_`** ⇒⇒⇒ **因为调用方在 `if (!result.added)` 里**
   ⇒⇒⇒⇒ **穷举完整靠**外层分支**保证，不靠 switch 穷举** ⇒⇒
   ⇒⇒⇒ **同 B0342「条件成立才给回调」、B0349「可空 = 有可推导的默认」的思路：靠结构保证，不靠自觉**
3. ⭐⭐⭐ **每个失败分支都返回 `playlist: current`（原样不动）**
   ⇒⇒ **不是「只在末尾统一处理」** ⇒⇒ 而 **B0395 注释说的正是这个**（「超限时列表**原样不动**」）
   ⇒⇒⇒ ⭐ **注释与实现是同一件事的两半**（B0393「代码改了、注释也改」的正面例）
4. ⭐ **`total` 是现算的**（`for` 累加）⇒⇒ **不是某个缓存字段** ⇒⇒ **不依赖外部维护的计数**
   ⇒⇒ **同 B0371「不依赖一个需要被清掉的字段」的思路**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 2 点**：
> **「成功」被建成一个具名状态，而 UI 只穷举失败分支** ——
> **这不是遗漏，是**调用方先在 `if (!added)` 里分了流** ⇒⇒⇒
> **而这比「在 switch 里写五个分支」更好**：**五个分支会诱使人把 `added` 写成空处理**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
