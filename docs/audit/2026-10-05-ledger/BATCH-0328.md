# BATCH-0328 · ⭐ **「两层必须同时变」被做成了「两层读同一个值」**；而第四个「弯路记录」记的是**重复**而非缺失

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 舞台底「两层真相」

## 跑的命令（全部只读）
```
sed -n '556,580p' lib/live2d/live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **又一个「让约束由结构保证」**
```dart
// 舞台底有**两层真相**：渲染面画的那层（iframe 里 canvas 的 `background-color`）
// 与**这一层**（iframe 还没加载完、或**渲染面拒绝了这个颜色时的兜底面**）。
// **两层必须同时变**，否则会看到「**底下的面已经到终色、上面的 iframe 还在旧色**」。
// **所以这里也读同一个插值值**（P1-3）。                                  // :558-564
AnimatedBuilder(animation: _stageColorMotion, … color: _displayedStageColor ?? appPaletteOf(context).stage),
```
四个可核点：
1. ⭐⭐ **两层读同一个插值值**（`_stageColorMotion` / `_displayedStageColor`）
   ⇒ ⇒ **「两层必须同时变」不是纪律，是它们共用一个数据源**
   ⇒ ⇒⇒ ⭐ **又一个「让约束由结构保证」的样本**（同 `_attach` 每次重放 · `stage-ack` 唯一写者 ·
   `settleTurn` 纯函数 · `swapModel` 的 `Future<bool>`）
2. ⭐⭐ **而失效被逐字写出来**：「**底下的面已经到终色、上面的 iframe 还在旧色**」
   ⇒ ⇒ **一个视觉上可辨的中间态** ⇒ ⇒ **可被认出来，就能被报告**
3. ⭐ **`?? appPaletteOf(context).stage` 的兜底有名字**（主题的「舞台底」槽位）
   ⇒ ⇒ **不是硬编码颜色**（同 B0229 的 `surfaceContainerHighest` 纪律）
4. ⭐⭐ **第四个「弯路记录」**（`:572-580`）—— 而**这次的失效是「重复」而非「缺失」**：
   > 「过去这里另有一套「渲染面加载中 N%」徽标 + 「渲染面错误 + 重试」……而 `StageHost` **同样**画了一套
   > 「模型加载中 N%」幕布 + 「模型加载失败 + 重试」。**两套叠在舞台同一块区域上，
   > 用户会同时看到两个重试按钮**、**加载时看到两条不同措辞的进度**。」
   ⇒ ⇒ **两套都在工作、但用户同时看到两个** ⇒ ⇒ **「多」和「少」一样是缺陷**
   ⇒ ⇒ 而修法写成**归属声明**：「加载态与错误态**不在这里画**」（2026-09-11 修的双覆盖层）

⇒ ⇒ **0 findings**；⇒ ⭐ **四个「弯路记录」至此的失效类型**：
**伪造**（B0310-①）· **什么都不显示**（B0310-②）· **兜底反被咬**（B0314）·
**重复**（本批）⇒ ⇒ **它们覆盖了「假、缺、错、多」四种**

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~540 /
   `persona_panel` ~515 / `message_bubble` ~455 / `chat_panel` ~460）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
