# BATCH-0407 · ⭐⭐⭐ **「有界」的第二种手段：不是丢最旧，而是**先折叠** —— 而理由被量化了**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `ObserverBuffer` 本体（B0406 留）

## 跑的命令（全部只读）
```
grep -rn "kObserverBufferCapacity" lib/ test/ --include=.dart
sed -n '250,276p' lib/live2d/render_events.dart ; sed -n '133,140p' lib/settings/sections/director_observer_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**「有界」有两种手段**（0 条新发现）
> 「**有界环形缓冲：只保留最近 `capacity` 条**（第 `capacity+1` 条挤掉最旧）。
> **纯逻辑、无 IO**：容量回归（**cap=200**）与「**按类型过滤**」都在 **VM 上单测**。」          // :250-252
> 「**阶段 5 维护者补丁（D44）**：stage-clock 是 **30ms 的连续信号**，不是事件。
> **逐条入环会让 200 条容量在约 6 秒内被时钟灌满**，**action_cue / ack 全被挤掉**、
> **B 栏实际不可用**。这里按 `(type, seq)` **折叠**：同一段只保留最新一条。」                 // :265-270
> `if (record.type == 'stage-clock') { _records.removeWhere((r) => r.type == record.type && r.seq == record.seq); }`  // :271-275
> 消费侧：`_trim<T>(list) { while (list.length > kObserverBufferCapacity) { list.removeAt(0); } }`  // :136-139
```
五个可核点：
1. ⭐⭐⭐⭐ **「有界」的第二种手段**：
   | 手段 | 什么时候用 | 理由 |
   |---|---|---|
   | **挤掉最旧** | 记录之间**同等重要** | 「只保留最近 N 条」 |
   | ⭐ **先按 `(type, seq)` 折叠** | **有一类记录是连续信号、会淹掉其它** | **B0406 判据的第二次、且更锋利的应用** |
   ⇒⇒⇒⭐ **⇒ 「有界」不是「丢得多」，是「先决定谁有资格留在环里」**
2. ⭐⭐⭐ **而理由被量化了**：「**200 条容量在约 6 秒内被时钟灌满**」⇒⇒ **30 ms × 200 = 6 秒**
   ⇒⇒⇒ **⇒ 这个数是算出来的** ⇒⇒ **同 B0329「两个理由都不是性能」那一族：结论带计算**
3. ⭐⭐⭐ **而「B 栏**实际不可用**」被写出来** ⇒⇒ **不是「不好看」，是「这一栏的功能没了」**
   ⇒⇒⇒ **⇒ 「不可用」比「难用」更能推动一次修补** ⇒⇒ **同 B0348「已知缺口」的写法**
4. ⭐⭐ **折叠的键是 `(type, seq)`** ⇒⇒⭐ **「同一段」是可判定的** —— `seq` 由渲染面给 ⇒⇒ **不是靠猜「这两条算同一段」**
5. ⭐⭐ **而消费侧对另一组列表用**同一个上限**做同样的裁**（`:136-139`）
   ⇒⇒ **上限被复用**（B0409 核的「共用一个上限」族）
⇒ ⇒ ⚠ **一处按 B0375 的判据检查后不记发现**：`ObserverBuffer({this.capacity = …}) : **assert(capacity > 0)**`
   ⇒⇒ **Dart 的 `assert` 在 release 下被去掉、而 `capacity` 是构造参数** ⇒⇒ **看起来是弱防护**
   ⇒⇒ **但**：**生产侧唯一的构造用缺省 200** ⇒⇒ **「capacity = 0」不可达** ⇒⇒ **按「无证据不记发现」不记**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
> **「有界」的第一问不是「留多少条」，是「谁有资格留在环里」。**
> ⇒⇒ **B0406 的判据（丢了会不会被察觉）在这里被反过来用**：
> **stage-clock 之所以特殊，是因为它挤掉的那 200 条**是被察觉的**（ack 消失了）**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `requestAnimationFrame` **暂停语义**
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
