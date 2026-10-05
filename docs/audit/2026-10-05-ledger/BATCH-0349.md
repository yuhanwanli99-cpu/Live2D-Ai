# BATCH-0349 · ⭐⭐⭐ **`bool?` 可空 override** —— 默认由推导给出、用户的决定覆盖它

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `message_bubble` 的**思考折叠区状态**

## 跑的命令（全部只读）
```
sed -n '194,212p' lib/ui/message_bubble.dart
grep -n "initState|initiallyExpanded|_expanded|onlyReasoning" lib/ui/message_bubble.dart | sed -n '3,12p'
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一种我没在别处见过的 UI 写法**
```dart
bool? _expandedOverride;                                              // :332
bool get _expanded => _expandedOverride ?? widget.onlyReasoning;      // :334
…
onTap: () => setState(() => _expandedOverride = !_expanded),          // :349
```
四个可核点：
1. ⭐⭐⭐ **`bool?` 而不是 `bool`，默认值来自 `widget.onlyReasoning`**
   ⇒ ⇒ **用户不碰时，默认值随消息变化**（正文从空变成有 ⇒ 折叠区自动收起）
   ⇒ ⇒⇒ ⭐ **而如果默认存在 state 里（`bool`），它会在第一条消息上被钉死**
   ⇒ ⇒⇒⇒ **可空性就是「默认是**推导**出来的、不是**存**下来的」**这句话的实现
2. ⭐⭐ **`_expandedOverride` 的类型就是那条规则**：
   `null` = 跟随消息 · `true`/`false` = 用户说了算
   ⇒ ⇒ **「用户的决定」与「推导出的默认」在同一字段里区分** ⇒⇒ **不需要两个字段、不需要同步**
3. ⭐⭐ **「正文为空时默认展开」的理由被写了**（`:304`「那种情况思考就是本轮唯一内容」）
   ⇒ ⇒ 同 **B0313** 那条「思考就是本轮唯一内容」⇒⇒ **两处引用同一个理由**
4. ⭐ **位置在正文之上**，理由是「**思考先于答案发生，顺序与模型一致**」
   ⇒ ⇒ **版面顺序 = 事件顺序**

⇒ ⇒ **0 findings**；⇒ ⭐ **而 ① 是一种我没在本仓别处见过的写法**：
> **用「可空性」把「推导的默认」与「用户的决定」放进同一个字段。**
> ⇒ ⇒ **它同时拿到两件事**：**默认会随输入变化** + **用户的覆盖不会被回弹**
> ⇒ ⇒ **同族**：`settleTurn` 的可空 `stopped`（B0313）· `onDismiss`/`onAck` 的可空回调（B0245/B0297）
⇒ ⇒ **而这三处的共同点是：可空在这里不是「也许没有」，而是「有一个可推导的默认」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~440）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
