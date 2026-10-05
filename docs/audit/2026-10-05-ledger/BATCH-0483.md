# BATCH-0483 · ✅ **那句注释就是 B0328 那条缺陷的成因** —— 而「重置」是**两件事**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 舞台**错误覆盖层的重试**（B0328 核过那层 widget，**它读什么**未核）

## 跑的命令（全部只读）
```
grep -nE "errorText|_loadError|retry|重试" lib/live2d/live2d_stage.dart | head -6
sed -n '530,552p' lib/live2d/live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一个动作重置两件东西，而两件都必要**（0 条新发现）
```dart
// lib/live2d/live2d_stage.dart:533-550
/// 错误后重建 iframe（也适用于渲染端热更新后的重试）。
/// 「重试」：重建 iframe（并重置错误态）。
/// 宿主只需要 `GlobalKey<Live2DStageState>.currentState?.retry()`，
/// **不必知道舞台内部是怎么重挂的**。
void retry() {
  final previous = _bridge;
  _bridge = null;
  unawaited(_renderSubscription?.cancel());
  if (previous != null) {
    previous.removeListener(_onBridgeChanged);
    unawaited(previous.destroy());
  }
  _hostError = null;
  // 重建后要**重新上报**阶段（否则**宿主停在旧的 error 覆盖层上**）。
  _lastReportedPhase = null;
  setState(() => _generation++);
}
```

### 五个可核点
1. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 「重建后要**重新上报**阶段（否则宿主停在旧的 error 覆盖层上）」**
   ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 那句注释就是 B0328 核过的那条缺陷的**成因，一句话** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ **⇒「重置」不是一件事、是两件**
   （`_hostError` + `_lastReportedPhase`）—— **只重置前者就会复现那条缺陷** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
2. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而 `retry()` 的**契约写在头注里**（`:535-537`）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   **⇒⇒ 「**宿主只需要 `currentState?.retry()`、不必知道舞台内部是怎么重挂的**」** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ **⇒ 这与 B0305 核的 `calling: [_saveAndRefetch]`**同族：对外一句、内部不外泄** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐ **⇒ 而拆桥那一段是**有界的四步**（置 null · cancel 订阅 · `removeListener` · `destroy`）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
4. ⭐⭐⭐ **⇒ 而 `unawaited(...)` 出现两次（`cancel` / `destroy`）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
   ⇒⇒ **⇒⇒⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ ⇒⇒⇒⇒ ⇒⇒ **⇒ 「不等」被显式写出来了**（不是忘了 await、而是标明了「不等」）** ⇒⇒⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐⭐
5. ⇒ ⇒⭐⭐⭐⭐⭐ **⇒ ⇒ 而 `_generation++` 是**这一批的第三个「让下一次上报重新发生」的机制**（另两个是 `_lastReportedPhase` 的清空与 `_lastReportedKey`）** ⇒⇒⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 1 点**
> **「重建后要重新上报阶段（否则宿主停在旧的 error 覆盖层上）」= B0328 那条缺陷的成因，一句话**
> **⇒⇒⇒ ⇒⇒⇒ ⇒ ⇒「重置」不是一件事、是两件** —— **只重置 `_hostError` 就会复现那条缺陷**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `_generation` 的**读点**（`didUpdateWidget` 侧 · **未核**）· `wake_gate_open` 本体 ·
   `clean_transcript` 尾部 · `lower_eq` / `is_separator` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（**⚠ 覆盖行数是移动靶**）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
