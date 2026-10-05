# BATCH-0239 · ⭐ 单例 feed **两端都接线**，且它在拆卸时被**显式清空** ⇒ 那个「代价」只是机制、不是债

Phase 1 · 域覆盖 · 前端 `.dart`（48/218）—— `DirectorObserverFeed` 的**生产端与消费端**

## 跑的命令（全部只读）
```
grep -rn "DirectorObserverFeed" lib/
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**B0233 记的那个「代价」被补上了另一半**
四个可核点：
1. ⭐ **`main.dart` 在四处 push**：`main.dart:319` `onEvent: DirectorObserverFeed.instance.pushRenderEvent`
   （舞台的 render-event 回调**直接接到 feed**）· `:710` `pushStageClock` · `:726` `pushWsEvent(event)` ·
   `:782` `pushPresetRequest`
   ⇒ ⇒ **feed 有真实生产者** ⇒ **这个面板不是装饰**（否则就是 B0237 说的「界面看起来没事」那一类）
2. ⭐ **消费端是四栏、且每栏类型明确**：`Listenable.builder(listenable: …)`（:278）+
   `ObserverBuffer`（:447）· `List<PresetRequest>`（:537）· `List<RenderEvent>`（:538）· `List<TtsSentence>`（:587）
   ⇒ ⇒ 与头注声明的「**四栏**」**逐栏对应**
3. ⭐⭐ **`director_observer_section.dart:313` `DirectorObserverFeed.instance.clear();`**
   ⇒ ⇒ **那个全局可变单例在面板拆卸时被显式清空**
   ⇒ ⇒ **B0233 记的「代价」被补上了另一半：代码顺手清理了它**
4. `render_events.dart:141` 复述了安排：「（`DirectorObserverFeed`）：宿主**仍只写** `onRenderEvent: _direc…`」
   ⇒ ⇒ 与头注「**不改 `shell_settings.dart`**」的声称**一致**（宿主的责任面仍是一个回调）

### ⭐ 由此得到一条（正面模式 **P27**）
> **当一个全局可变的「代价」被写下时，若代码还顺手清理了它，那「代价」就只是「机制」，不是「债」。**
⇒ 本例：单例 feed 是真实成本（`main.dart` 持有一个全局可变句柄，B0233 已核），
但它**在拆卸时被 `clear()`** ⇒ ⇒ **它有主、有边界、会被回收** ⇒ **不必再收敛**。
⇒ ⭐ **推论**：审计里看到「这里用了全局」**不构成缺陷**；
**要看的是「它有没有主人、什么时候被清空」** —— 有这两条，它就是机制。
（⇒ 与 B0166「`(bool, String)` 是**类型**问题」同源：**先问「代价是否被承担」，再问「形态是否理想」。**）

## 未核实项
1. `director_observer_section.dart` 余 ~600 行未读（四栏的呈现本体 · 状态读取 · 错误呈现 · `ObserverBuffer` 的容量策略）
2. `message_bubble.dart` 余 ~490 · `chat_panel.dart` 余 540 · `error_banner.dart` 余 45 ·
   `persona_panel` 余 ~560 · `memory_panel` 余 ~590 未读
3. `live2d_stage.dart` 余 ~620 行未读（桥接生命周期本体 · 事件队列 · retry 重建）
4. `ObserverBuffer` 的**容量上界**未核（一个只被 push 的缓冲区若**无上界**，是「长期运行会累积」那一类）
5. 前端 `.dart` 仍 170 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
