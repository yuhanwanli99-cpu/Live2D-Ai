# BATCH-0408 · ⭐⭐⭐ **这个类刻意不包含「谁造成了这一轮」** ⇒ 那条八层判据与五个信号**正交**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `UiSignals` **定义本体**（挂了几十批的未核实项）

## 跑的命令（全部只读）
```
sed -n '/class UiSignals/,/^}/p' lib/state/ui_state_tracker.dart   # ⇒ 零命中（不在这个文件）
grep -rn "UiSignals" lib/ --include=.dart | grep -vE "deriveUiPhase|signals" | head -4
sed -n '52,82p' lib/state/ui_phase.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一个挂了几十批的未核实项结清**（0 条新发现）
> 「派生 [UiPhase] 所需的**全部**信号。全是布尔/枚举 —— **没有 `BuildContext`、没有 Stream、
> 没有时间**。**「打断窗口」由调用方折算成 `interrupted` 布尔（定时器是副作用，不属于纯函数）**。」 // :53-57
> `wsConnected`「`WsStatus == connected`」· `turnActive`「**`POST /api/v1/chat` 已受理**且
> **本轮尚未收口**」· `voiceActive`「收到 **`voice_started`、未收到 `voice_ended`**」·
> `interrupted`「处于 **`new_epoch` 后的短窗口内**」· `errorActive`「有未关闭的错误
> （`error` 帧 / `turn_state: failed`）」                                                  // :66-79
```
五个可核点：
1. ⭐⭐⭐ **每个信号的定义都是**线路级**的、不是 UI 级的**：
   | 信号 | 定义 | 它记的是 |
   |---|---|---|
   | `wsConnected` | `WsStatus == connected` | **连接状态** |
   | `turnActive` | POST **已受理** ∧ **未收口** | **一次往返的两端** |
   | `voiceActive` | `voice_started` ∧ ¬`voice_ended` | **一对事件** |
   | `interrupted` | `new_epoch` 后的短窗口 | **一个事件** |
   | `errorActive` | 有未关闭的错误 | **错误的有无** |
   ⇒⇒⇒⭐ **⇒ 五个信号没有一个是「界面现在该显示什么」** ⇒⇒ **而 `deriveUiPhase` 才做那一步**
2. ⭐⭐⭐⭐⭐ **而这个类刻意**不包含「谁造成了这一轮」**——它只记录「有没有在跑」，
   **「是谁」由 `settleTurn(stopped:)` 那条链回答**（B0304–B0313 核过的八层判据）
   ⇒⇒⇒ **⇒ 两者是**正交**的，不是同一件事的两种写法** ⇒⇒ **而这个正交是我此前一直默认、没核过的**
3. ⭐⭐⭐ **「打断窗口」被逐字推到调用方**，理由是「**定时器是副作用，不属于纯函数**」
   ⇒⇒⇒⭐ **而这一句同时做了两件事**：**① 保住纯度 ② 点名「谁该负责那个定时器」**
   ⇒⇒⇒ **⇒ 而「责任在调用方」写在**被推出去的那一方**的注释里**，不是推出去的地方
4. ⭐⭐ **全是布尔/枚举 ⇒ 可 `const` 构造** ⇒⇒ **⇒ 每次派生得到的是「一个可比较的值」**
   ⇒⇒ **⇒ 而这让「状态是否变了」可以按值比较**（B0326 的 `presetStatus` 族）
5. ⭐⭐ **三条纯度约束同时写明**（无 `BuildContext` · 无 `Stream` · 无时间）⇒⇒ **同 B0332 核的那三条**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的收获是第 2 点**：
> **「有没有在跑」与「是谁造成的」被刻意分在两个地方。**
> ⇒⇒ **前者是状态（可比较、可派生、可测试）**；**后者是收口时的判据（一次性、需要上下文）**
> ⇒⇒⇒ **⇒ 而「打断窗口」被推到调用方，正是这个分离能成立的技术前提**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` **谁来填**
   （`ui_state_tracker.dart:80` 是唯一的 `deriveUiPhase` 调用点，**它怎么组装这五个信号未读**）·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
