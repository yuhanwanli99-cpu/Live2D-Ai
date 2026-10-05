# BATCH-0238 · ⭐ 舞台的 ack 契约：**每个回调的文档都写「缺了它会怎样」** ⇒ P1 覆盖到 API 面

Phase 1 · 域覆盖 · 前端 `.dart`（47/218）—— `lib/live2d/live2d_stage.dart` 的桥接回调面

## 跑的命令（全部只读）
```
grep -nE "stage-ack|stageAck|ack|首帧|已加载|loaded" live2d_stage.dart
sed -n '152,170p' live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **P1 的一种新落点：写在 API 契约里**
```dart
/// 每收到一条**新的** `stage-ack` 回调一次。                        // :154
/// 宿主用它把「渲染面实际生效的缩放」显示出来。**缺了它**，舞台角标的百分比
/// 只能等用户按一次放大/缩小才有值 —— **首屏会一直显示一个「—」，看起来像坏了**
/// （**2026-09-11 在真浏览器里就是这么显示的**）。                   // :155-159
final ValueChanged<StageAckEvent>? onAck;
```
三个可核点：
1. ⭐⭐ **AGENTS rc.5 ⑧ 记的那个真缺陷**（「缩放读数永远是 `—`／没人读渲染面首帧的 `stage-ack`」）
   **在这里被逐字引用，并带着症状与日期** ⇒ ⇒ 修复**在位**，且**为什么需要它**写在**签名旁边**
2. ⭐ 「每收到一条**新的**」⇒ **去重**（不是每帧都回调）⇒ ⇒ 宿主端不会收到重复 ack
   ⇒ 与 B0132/B0136 的「不刷屏」纪律同族
3. ⭐ 而 `:161-165` 把「**不维护镜像状态**」写成了**协议级**条款：
> 「渲染面**事件级 ack**（协议 §7：四条 ack + `segment-ended`，**O13 冻结名**）……
> 前端**不**据此维护镜像状态、也不做每帧状态流（V8）——`presetStatus` 那份显示快照
> **是唯一例外，且它同样只由 ack 驱动**。」

⇒ ⇒ **协议名是冻结的**（契约）· **前端明确不建镜像**（不推断）· **唯一的例外被点名，且它也由 ack 驱动**
⇒ ⇒ **B0232 的发现在回调层被重述了一遍** ⇒ ⇒ **两处独立地说了同一句话**

### ⭐ 而这一整面是 **P1 的一种新落点**
本文件的四个回调（`onError` / `onAck` / `onRenderEvent` / `onReady`）**每一个的文档都写了「缺了它会怎样」**
⇒ ⇒ 所以**只读签名 + 文档**的调用方，就能知道自己该负责什么
⇒ ⇒ **纪律不只活在「决策旁边的注释」里，它活在 API 契约里**

## 未核实项
1. `live2d_stage.dart` 余 ~620 行未读（桥接生命周期本体 · 事件队列 · retry 重建 · 值变化的 diff）
2. `director_observer_section.dart` 余 ~650 行未读（四栏本体 · 状态读取 · 错误呈现）
3. `message_bubble.dart` 余 ~490 · `chat_panel.dart` 余 540 · `error_banner.dart` 余 45 · `persona_panel` 余 ~560 · `memory_panel` 余 ~590 未读
4. 前端 `.dart` 仍 171 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
7. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
