# BATCH-0232 · `live2d_stage.dart`：**真源搬家时，它的语义也从「本地计时器」变成了「渲染面 ack 快照」**

Phase 1 · 域覆盖 · 前端 `.dart`（42/218）—— `lib/live2d/live2d_stage.dart`(666) 的头注与结构

## 跑的命令（全部只读）
```
wc -l lib/live2d/live2d_stage.dart ; sed -n '1,20p' lib/live2d/live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **一次「搬家」顺带修对了一个语义**
> 「[`PresetStatus`] 的**唯一真源**已搬到 `preset_status.dart`（阶段4d）：
> 它现在是「**渲染面 ack 的显示快照**，**不是本地计时器**」。这里 **re-export**，
> **既有调用方**（`settings/sections/dev_tools_section.dart` 等）**一行都不用改**。   // :18-20

四个可核点：
1. ⭐ **真源被搬到一处**（P2）⇒ 且**新家被点名**
2. ⭐⭐ **而它的语义同时被修对了**：「现在是『**渲染面 ack 的显示快照**』，**不是本地计时器**」
   ⇒ ⇒ **这个 UI 状态曾经是「本地猜的」**（一个本地计时器），
   **现在改成「读渲染面的回执」** ⇒ ⇒ **UI 不再自己发明状态**
   ⇒ ⇒ 与 **B0116** 的 `shouldPreserveStage`（把本地启发式换成对真实属性的 `mapEquals`）、
   **B0136** 的 epoch 闩锁**同族** ⇒ **「不要推断，去读真源」这条纪律在舞台上也被应用了**
3. ⭐ **搬家是兼容做的**：「这里 **re-export**，既有调用方……**一行都不用改**」⇒ ⇒ **P2 带迁移方案**
4. 且**点名了受益的调用点**（`dev_tools_section.dart` 等）⇒ ⇒ 又一处**指路而非复述**

### 顺带：**平台条件 import 这个模式在本仓出现了第二次**
```dart
import 'live2d_host_stub.dart'
    if (dart.library.js_interop) 'live2d_host_web.dart' as host;     // :12-13
```
⇒ ⇒ 与 B0186 核的 `stage_pointer_interceptor{,_web,_stub}` **同一形状**
⇒ ⇒ 所以「**平台边界用条件导出**」是本仓的**约定**（至少两处独立使用），而不是某一处的权宜

### ⭐ 由此得到一条关于「私有汇合」的观察
> **把真源搬到一处，常常伴随「那个值的语义也变对了」** ——
> **搬家的副产品往往比搬家本身更值钱。**
⇒ 本例：搬 `PresetStatus` 的副产品是「**它从本地计时器变成了 ack 快照**」⇒ **UI 停止发明状态**。
⇒ ⇒ 而这次搬家**没有波及其调用方**（re-export）⇒ ⇒ **成本 ≈ 0，收益 = 语义修正**

## 未核实项
1. `live2d_stage.dart` 余 ~645 行未读（桥接生命周期、`render_events` 消费、预设状态的应用）
2. `director_observer_section.dart`(668) 未读
3. 三个面板余面（`persona_panel` ~560 · `message_bubble` ~500 · `memory_panel` ~600）+ `chat_panel` 540 + `error_banner` 45 未读
4. 前端 `.dart` 仍 176 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
