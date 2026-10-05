# BATCH-0334 · ✅ 优先级**被测试锁住**；而取舍的措辞**住在测试名里，不在函数里**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `deriveUiPhase` 的**测试**（B0333 留）

## 跑的命令（全部只读）
```
grep -rn "deriveUiPhase" test/
sed -n '60,95p' test/ui_phase_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **B0333 的未决项有答案了**
```dart
('**打断中、voiceActive 还残留 → interrupted（不是 speaking）**',
  sig(ws: true, interrupted: true, voice: true), UiPhase.interrupted),      // :61-64
('打断中、本轮还开着 → interrupted', sig(ws: true, interrupted: true, turn: true), …),
// **voice 先于 turn。**                                                          // :70
('说话中且本轮未收口 → speaking', sig(ws: true, voice: true, turn: true), UiPhase.speaking),
('只受理、未出声 → thinking',     sig(ws: true, turn: true),                UiPhase.thinking),
…
group('**每个相位都可达（没有死枚举值）**', () {                            // :84
  expect(<UiPhase>{ … 6 组信号 … }, UiPhase.values.toSet());                 // :87-94
```
五个可核点：
1. ⭐⭐⭐ **优先级被测试锁住**，而 **测试名把取舍写成了一句话**：
   「**打断中、voiceActive 还残留 → interrupted（不是 speaking）**」
   ⇒ ⇒⇒ ⭐⭐⭐ **取舍的措辞住在测试名里，不在函数里** ⇒ ⇒
   **这是可审查性的另一种分布：不在注释里，在断言的名字里**
   ⇒ ⇒ **B0333 那条「函数上无注释」的担忧因此得到缓解**：**理由没丢，只是换了地方**
2. ⭐⭐ **两个信号冲突的情形被专门造出来测**（`interrupted + voice` · `interrupted + turn`）
   ⇒ ⇒ **冲突才是优先级真正被检验的地方** ⇒ ⇒ 而这里**造了两次**
3. ⭐⭐ **「voice 先于 turn」被写成一行注释 + 一条用例**（`speaking` vs `thinking`）⇒ ⇒ **同族也处理了**
4. ⭐⭐ **「每个相位都可达（没有死枚举值）」** + `expect(reachable, UiPhase.values.toSet())`
   ⇒ ⇒ **一个集合相等断言** ⇒ ⇒ **枚举里加一个新值而不给它出路 ⇒ 测试变红**
   ⇒ ⇒⇒ **与 B0204/B0207 的 `fieldCount` 族同形**（P24「让新增成员必然失败」）
5. ⇒ ⇒ **B0332 头注「枚举里刻意没有 `listening`」因此有了另一半保证**：
   不只是「刻意少一个」，而是「**剩下的每个都有出路**」⇒ ⇒ **少与多两侧都被守住**

⇒ ⇒ **0 findings**；⇒ ⭐ **而 B0333 那个「优先级是否被有意论证」现在可以回答**：
**是 —— 它被测试名论证了，只是不在函数上。**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读（`sig(...)` 那个构造器的默认值）· `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~460）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
