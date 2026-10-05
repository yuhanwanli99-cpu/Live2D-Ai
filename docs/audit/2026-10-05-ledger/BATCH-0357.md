# BATCH-0357 · ⭐ **两个错误源、一处合并、重试清空** —— 这是 B0290「旧覆盖层」的**对面**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 舞台的**宿主错误**链路（`_handleHostError`，B0290 留）

## 跑的命令（全部只读）
```
grep -n "_hostError" lib/live2d/live2d_stage.dart
grep -rn "errorMessage" lib/ --include=*.dart | grep -v live2d_stage.dart
grep -n "stageError" lib/app/app_shell.dart ; grep -rn "stageError" lib/ | grep -v app_shell
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **「清空」的完整清单齐了**
| 位置 | 内容 |
|---|---|
| `live2d_stage.dart:183` | `String? _hostError;` |
| `:236` | `String? get errorMessage => **_hostError ?? _bridge?.errorMessage**;` |
| `:382` | `setState(() => _hostError = message);`（`_handleHostError`） |
| `:546` | `_hostError = null;`（**`retry()` 里**） |
| `main.dart:1319` | `stageError: **_stageKey.currentState?.errorMessage**` |
| `app_shell.dart:697` | `errorMessage: widget.stageError` ⇒ **进 `StageHost` 的覆盖层**（B0328 核过归属） |
四个可核点：
1. ⭐⭐⭐ **两个错误源在**一个 getter** 里合并**（`_hostError ?? _bridge?.errorMessage`）
   ⇒ ⇒ **而优先级就写在 `??` 的顺序里**（宿主的错误优先）
2. ⭐⭐ **`main.dart` 通过 `GlobalKey` 取**（`_stageKey.currentState?.errorMessage`）
   ⇒ ⇒ **与 B0323 核的 `retry()` 走的是同一条 `GlobalKey` 通道** ⇒⇒ **宿主与舞台之间只有这一条**
3. ⭐⭐ **`retry()` 清 `_hostError`** ⇒ ⇒ **「点重试后还挂着旧错」这个缺陷不存在**
4. ⇒ ⇒ ⭐ **这正是 B0290「宿主停在旧覆盖层」的**对面**：
   那次是**相位**会陈旧（⇒ 故要清 `_lastReportedPhase`，B0322 已核）·
   **这次确认**错误消息**不会陈旧**（⇒ 因为 `retry()` 清了它）
⇒ ⇒ **「清空」的完整清单至此齐了**（重试路径上三样东西**各自**被清）：
| 被清的 | 位置 |
|---|---|
| **旧桥** | B0323 `retry()` → `previous.destroy()` |
| **旧相位** | B0322 `retry()` → `_lastReportedPhase = null` |
| **旧错误** | **本批** `retry()` → `_hostError = null` |

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1820 / `live2d_stage` ~580 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~430)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
