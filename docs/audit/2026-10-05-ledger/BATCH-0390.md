# BATCH-0390 · ⭐⭐⭐ **B0389 那条链闭合了** —— 按钮**按来源条件存在**，「当前项」**只在一处判**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 壳背景的**来源选择**（B0389 留）

## 跑的命令（全部只读）
```
grep -rln "backgroundSource" lib/ | head
grep -rn "backgroundSource" lib/ --include=.dart | grep -vE "display_prefs.dart" | head -5
grep -nE "背景来源|SwitchListTile" lib/settings/sections/appearance_background.dart
sed -n '296,306p' appearance_background.dart ; sed -n '320,326p' appearance_background.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0389 的链闭合**（0 条新发现）
```dart
/// 「**来源 = 舞台那张**」时的**那两个按钮**（换 / 清）。                    // :298
final VoidCallback? **onPickStageImage**;
final VoidCallback? **onClearStageImage**;                                // :299-300
…
// 来源是背景库时，「当前」= **运行时轮播索引**指向的那一项（**外壳注[释]里
// 不再是「永远第 0 项」**）；来源是舞台那张时 = 舞台那张图。
// **判据只在外壳一处（`AppShell._currentBackground`），这里只消费。**       // :322-325
…
label: '**背景来源**' … onChanged: (int v) => onChanged(prefs.copyWith(backgroundSource: v)),  // :354/367-368
```
两个可核点：
1. ⭐⭐⭐ **那两个按钮**都是可空**、且只在「来源 = 舞台那张」时存在**
   ⇒ ⇒ **B0389 记的「一个必然无效的按钮」在结构上被消除了** ⇒⇒
   **「条件成立才给回调」**（同 B0326 的 `onOpenSessions` 可空族）
2. ⭐⭐⭐ **而「当前是哪一项」的判据只在一处**（`AppShell._currentBackground`），面板**只消费**
   ⇒ ⇒ **同 B0335「相位只被派生一次」** ⇒⇒ **而注释还记着「外壳注释里**不再说『永远第 0 项』**」**
   ⇒ ⇒⇒ ⭐ **连注释都被同步修过** ⇒⇒ **「代码改了、注释也改」是同一次改动的两半**
   （同 B0212 记的那个形状，但那次是「正文改、注释留旧」—— **这次是两边都改**）
⇒ ⇒⭐⭐ **B0389 那条链现在完整**：
**默认朝向换过来**（B0389：不是加提示 · 是换默认）·
**按钮按来源条件存在**（本批）· **当前项判据集中**（本批）
⇒ ⇒ **三处合起来 = 那个缺陷的三种失效面各自被一条规则挡住**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 的**本体**（它怎么按来源取值 · `:614` 那个 `!=` 判断的另一侧）
2. `evaluate_mode` 函数本体 · `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `LiveRegionThrottle.feed` 本体 ·
   `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
