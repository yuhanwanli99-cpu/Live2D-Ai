# BATCH-0322 · ⭐ 同一个失效**被两端各记一次**：回调侧说「覆盖层会永远停在 loading」、retry 侧说「宿主停在旧的 error 覆盖层上」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 舞台的 **retry 路径**

## 跑的命令（全部只读）
```
grep -rn "retry" lib/live2d/*.dart | grep -vE "^.*///"
sed -n '530,560p' lib/live2d/live2d_stage.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **B0290 那个「挂上去」的对称面**
```dart
/// 错误后重建 iframe。「重试」：重建 iframe（并**重置错误态**）。
/// 宿主只需要 `GlobalKey<Live2DStageState>.currentState?.retry()`，**不必知道舞台内部是怎么重挂的**。
void retry() {
  final previous = _bridge;  _bridge = null;
  unawaited(_renderSubscription?.cancel());
  if (previous != null) { previous.removeListener(_onBridgeChanged); unawaited(previous.destroy()); }
  _hostError = null;              // :546  ⭐ 重置错误态
  _lastReportedPhase = null;      // :547  ⭐⭐ 「重建后要**重新上报**阶段
                                  //        （否则**宿主停在旧的 error 覆盖层上**）」
  setState(() => _generation++);  // :549  重建由一个整数驱动
}
```
四个可核点：
1. ⭐ **旧桥的处置完整**：取消订阅 → 摘监听 → **`destroy()`**
   ⇒ ⇒ 与 **B0290 的 `_attach`（「挂上去」）对称** ⇒ **挂与拆是一对**
   ⇒ ⇒ 而用的是 **`destroy()` 而非 `dispose()`** ⇒ **命名给了线索**（语义是否不同**未核**）
2. ⭐⭐ **`_lastReportedPhase = null` 且理由写明**：「否则**宿主停在旧的 error 覆盖层上**」
   ⇒ ⇒ ⭐ **同一个失效被两处独立记录**：`:147-148` 从**回调侧**（B0238 核过）
   「宿主拿不到阶段变化 ⇒ `StageHost` 的加载覆盖层会**永远停在 loading**」·
   `:547` 从 **retry 侧**（「宿主停在旧的 error 覆盖层上」）
   ⇒ ⇒⇒ **同一条纪律的两端**（与 B0321 的 501、B0305–B0311 的判据**同形**：
   **一个已知的失效，被记在它能被看见的地方**）
3. ⭐ **窄接口 + 宽内部**：宿主只需一个 `GlobalKey` 调 `retry()`，**不必知道舞台内部怎么重挂**
   ⇒ ⇒ 同 `swapModel` 家族（B0299）
4. ⭐ `_generation++` ⇒ **重建由一个整数驱动** ⇒ **Widget 树不必知道 iframe 变了**

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
   （⚠ **新增**：`destroy()` 与 `dispose()` 的语义差别**未核**）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~595 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
