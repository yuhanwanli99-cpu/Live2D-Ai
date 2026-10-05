# BATCH-0335 · ⭐ 「**不允许各自判断**」是可核的；而六个分支被**两侧夹击**锁住

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `deriveUiPhase` 的**全部消费者**（B0332 那条禁令的遵守情况）

## 跑的命令（全部只读）
```
grep -rn "deriveUiPhase|UiPhase\." lib/ --include=*.dart | grep -v "ui_phase.dart"
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **「唯一真源」是结构性的**
```
lib/state/ui_state_tracker.dart:80   UiPhase get phase => deriveUiPhase(signals);   ← **全仓唯一调用点**
lib/ui/chat_panel.dart:356/360/364  (phase == UiPhase.thinking || phase == UiPhase.speaking) ×3
lib/ui/streaming_indicator.dart:46  if (widget.phase == UiPhase.thinking) …
lib/ui/state_pill.dart:58–88         **六个变体逐个** → UiPhaseView(offline/idle/thinking/speaking/interrupted/error)
```
四个可核点：
1. ⭐⭐⭐ **`deriveUiPhase` 在 `lib/` 里只被调用一次**（`ui_state_tracker.dart:80`）
   ⇒ ⇒ **「唯一真源」是结构性的，不是约定**
   ⇒ ⇒⇒ **B0332 那句「不允许各自判断」因此是可核的**：
   **没有任何消费者自己去读 `signals`**
2. ⭐⭐ **三个消费者都读 `phase`**（`chat_panel` · `streaming_indicator` · `state_pill`）⇒ **它们看到同一个值**
3. ⭐⭐⭐ 而 `state_pill.dart:58-88` 对**六个变体逐个写了视图、**没有 `_` 兜底**
   ⇒ ⇒⇒ **加一个新的 `UiPhase` 变体 ⇒ 这张表编译不过**
   ⇒ ⇒⇒ **P24「让新增成员必然失败」的又一例**
4. ⭐⭐ **两侧夹击成立**：**加变体 ⇒ 穷举表编译不过**（B0335）**+ 加了但没给它信号 ⇒ 可达性测试变红**（B0334）
   ⇒ ⇒⇒ **「加一个新相位」这个动作被前后各堵一次**

⇒ ⇒ **0 findings**；⇒ ⭐ 而 ④ 意味着：**这两个文件（B0333 的函数 · B0334 的测试 · 本批的表）
共同守着同一件事，而它们互相不知道对方存在** ⇒ ⇒ **「可审查性」在这里是三份冗余**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读（`sig(...)` 构造器的默认值）· `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~460）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
