# BATCH-0336 · ⭐⭐ **三个同形表达式本来就该一起变** ⇒ 这**限定了我自己的 P26-b**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `chat_panel` 的**发送/停止按钮**（P26-b 的形状）

## 跑的命令（全部只读）
```
sed -n '350,372p' lib/ui/chat_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **对 P26-b 的一次自我限定**
```dart
// 一轮进行中时，**同一个位置变「停止」**——**避免用户以为发了没反应**。   // :353
_RoundActionButton(
  icon:      (phase == thinking || phase == speaking) ? Icons.stop_rounded : Icons.arrow_upward_rounded,
  tooltip:   (phase == thinking || phase == speaking) ? '停止本轮' : '发送（Enter）',
  onPressed: (phase == thinking || phase == speaking) ? onStop : onSend,
),
```
四个可核点：
1. ⭐⭐⭐ **三处相同条件是「同一个按钮的三个字段」，不是三个独立子句**
   ⇒ ⇒ **不存在「被合并成一个」的对象** ⇒ **它们变一致才是对的**
   ⇒ ⇒⇒ ⭐⭐ **这是 B0235 那条 P26-b 的**反向情形**：那里的危险是
   「三个**本应不同**的子句被看起来一样地合并」；**这里的三个同形表达式本来就该一起变**
   ⇒ ⇒⇒ ⭐⭐⭐ **P26-b 需补一个限定**：
   > **「同形」本身不是问题，「同形却本应不同」才是。**
2. ⭐⭐ **morph 而不是新增按钮，理由是用户心智模型**（「**避免用户以为发了没反应**」）
   ⇒ ⇒ **不是「省一个控件」** ⇒ ⇒ **P1 家族**
3. ⭐ **`tooltip` 也跟着变**（`停止本轮` / `发送（Enter）`）⇒ **三处一致**
   ⇒ ⇒ 而 `tooltip` **带上了快捷键**（`发送（Enter）`）⇒ **把键位写进提示**
4. ⭐ **`onPressed` 跟着换**（`onStop` / `onSend`）⇒ ⇒ **动作与相位严格对应**
   ⇒ ⇒ 而 B0304 已核 `onStop` 在 controller 侧是 `_finishTurn(stopped: true)` + `api.stopChat()` ⇒⇒ **闭合**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的真正产出是对我自己一条模式的限定** ——
**P26-b 是从 B0235 单点归纳出来的，缺了这里的反向情形**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~455）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
