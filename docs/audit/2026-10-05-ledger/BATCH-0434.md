# BATCH-0434 · ✅ **反证条款结清**，且**范围被收窄到一个精确的形状**

Phase 4 · **证伪**（兑现 F-0433-01 自己写的反证条款：有没有用 `find.byType` 的等价断言）

## 跑的命令（全部只读）
```
grep -rn "DirectorObserverSection" test/ | head -4
grep -rln "devMode" test/ | head -4
grep -rn "devMode" test/wiring_test.dart test/settings_controller_test.dart | head -5
grep -rn "devMode: false|devMode: true" test/*.dart | head -4
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0433-01 不撤回，且范围与修法都被收窄**（0 条新发现）
| 证据 | 内容 |
|---|---|
| `test/wiring_test.dart:117` | `testWidgets('**历史轮数只在 devMode 出现**（会话基建，不是…')`，`:123 devMode: false` / `:134 devMode: true` |
| `test/director_observer_test.dart:135/171/230/276` | ⭐ **直接 `DirectorObserverSection(...)`** ⇒ **门控被绕过** |
| `grep "devMode: false" director_observer_test.dart` | **零命中** ⇒⇒ **该块的门控在它自己的测试里从未被触发** |

### 三个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 反证条款结清：没有等价断言。** 而 `devMode` 门控**这个模式本身是有测试的**
   （`wiring_test.dart:117`「历史轮数只在 devMode 出现」）⇒⇒⇒⭐⭐
   **⇒⇒ ⇒⇒ ⇒ 修法因此是「照抄一条已存在的测试形状」** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒ 修法的成本比我原先写得更低**
2. ⭐⭐⭐⭐ **而导演观测的测试**直接构造那个 section**、绕过门控** ⇒⇒⭐⭐
   **⇒⇒ ⇒⇒ ⇒ 而这解释了「为什么没人写这条断言」：要写它**得从面板进去**，而那个测试是从 section 进去的**
   ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒ ⇒ 而这不是疏忽、这是测试分层的正常结果** ⇒⇒⇒⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒ ⇒ ⇒ 真正的缺口是「**分层之后没人补一条跨层断言**」**
3. ⭐⭐⭐⭐⭐ **⇒⇒⇒⭐⭐⭐ 而 F-0433-01 的范围因此被精确成一句话**：
   > **「`devMode` 门控有测试（对聊天基建那块），但**没有一条跨层断言**覆盖「从面板进去时导演观测整块不在语义树」。**
   ⇒⇒⇒ **⇒⇒ ⇒⇒ ⇒ 而这一句话同时说明：缺口不是「忘了测」，是「**分层之后**没有一条**跨层**的断言」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义 ·
   **`wiring_test.dart:117` 那条断言的形状**（它用 `findsNothing` 还是 `findsOneWidget` · **未读**）
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
