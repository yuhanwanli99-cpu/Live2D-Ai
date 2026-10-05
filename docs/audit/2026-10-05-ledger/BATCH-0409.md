# BATCH-0409 · ⭐⭐⭐⭐ **「逻辑」的一半是「时序与归属」** —— 而**纯度与可测性在这里是同一件事**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `ui_state_tracker` 的**信号装配**（B0408 留）

## 跑的命令（全部只读）
```
grep -nE "_interrupted|Timer|new_epoch|interrupted:" lib/state/ui_state_tracker.dart | head -8
grep -nE "interrupted = |_newEpoch" lib/state/ui_state_tracker.dart | head -6 ; sed -n '1,10p' lib/state/ui_state_tracker.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**四个观察被统一了**（0 条新发现）
> 「`ui_phase.dart` 是纯函数，但「**信号从哪来、什么时候清**」**同样是逻辑**，
> **而且是更容易错的一半**：`voice_started` 之后**谁负责把它清掉**？**`new_epoch` 的打断窗口由谁计时**？
> 留在 `main.dart` 里就**永远测不到** —— 而 `main.dart` **必须 `import 'package:web'`**。」   // :5-9
> 「本文件**不碰 `package:web`**：它只消费已经解析好的 [WsEvent] 与 [WsStatus]」            // :10-11
> `typedef TimerFactory = Timer Function(Duration, void Function());` … `_newTimer = timerFactory ?? _defaultTimerFactory;`  // :35-51
> 「于是 `new_epoch` 分支里刚设上的 `_interrupted = true` **立刻被自己抹掉**——」            // :223
```
四个可核点：
1. ⭐⭐⭐⭐⭐ **它把「信号从哪来、什么时候清」称为「逻辑」，而且是「更容易错的一半」**
   ⇒⇒⭐ **⇒ 这纠正了一个常见直觉**（逻辑 = 计算）
   ⇒⇒⭐⭐ **⇒ 逻辑的一半是「时序与归属」，而那一半**不 import 任何东西**
   ⇒⇒⇒⭐⭐ **⇒ 「纯度」与「可测性」在这里被统一成同一件事**
   ⇒⇒⇒ **⇒ 而两个具体的追问被当场写出来**（`voice_started` 谁清？`new_epoch` 谁计时？）
   ⇒⇒⇒⇒ **⇒ 那正是 B0408 推给调用方的那两件事** ⇒⇒ **本批看到的是它的落点**
2. ⭐⭐⭐ **「留在 `main.dart` 里就永远测不到」+「`main.dart` 必须 `import 'package:web'`」**
   ⇒⇒ **⇒ 测试性与依赖方向是同一件事**
   ⇒⇒ **同 B0361「面板不 import `app/browser_io.dart`」· B0328「气泡只依赖纯模型」**
3. ⭐⭐⭐ **定时器是**注入的**（`TimerFactory`、可传可省）⇒⇒
   ⇒⇒⭐ **这正是 B0408 那句「定时器是副作用、不属于纯函数」的**落点** ⇒⇒⇒ **纯度被保住，副作用被参数化**
   ⇒⇒ **同 B0179 核的 `LiveRegionThrottle` 注入时钟** ⇒⇒ **两次都是「把不可测的东西变成参数」**
4. ⭐⭐⭐⭐ **而 `:223` 有一句**自我更正**（「刚设上的 `_interrupted = true` **立刻被自己抹掉**」）**
   ⇒⇒ **同 B0290 核的 `didUpdateWidget` 那处**（B0289→B0290 的形状）⇒⇒ **第三次出现「自我更正型注释」**
   ⇒⇒⇒⭐ **⇒ 而它出现在「谁负责清」这个**被专门命名过**的问题上**
   ⇒⇒⇒⇒ **⇒ 命名过的难点，恰好就是留痕最多的地方**

⇒ ⇒⭐⭐⭐⭐ **而本批的收获是把三次观察统一了**：
> **「逻辑」的一半是「时序与归属」，而那一半不 import 任何东西。**
> ⇒⇒ **⇒ 纯度与可测性在这里是同一件事** ⇒⇒ **⇒ 依赖方向即测试性**
> ⇒⇒⇒ **⇒ 它统一了 B0361（面板不 import browser_io）· B0328（气泡只依赖纯模型）·
> B0179（把时钟做成参数）** ⇒⇒⇒ **⇒ 三次不同的观察、一个共同的根据**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
