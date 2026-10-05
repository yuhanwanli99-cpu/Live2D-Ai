# BATCH-0363 · ⭐⭐ **B0362 那个不对称在 Mod 侧是有理由的** —— `clear` **刻意只清一层**

Phase 1 · 域覆盖 · **Mod 侧**（`live2d-ai-mod-memory`）—— B0362 留下的「两契约的另一半」

## 跑的命令（全部只读）
```
grep -rn "residue" crates/live2d-ai-mod-memory/src/
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **上一批的含糊被消掉了**
| 位置 | 内容 |
|---|---|
| `mod-memory/src/commands.rs:12` | 命令契约表：`| clear | {session_id?} | {ok, records, cleared, **residue**} |` |
| `commands.rs:203` | 「**只清 JSONL**，**不写** `persona.system_prompt`（残留按既有 `strip_residue` 语义…）」 |
| `commands.rs:216 / 240-252` | `let residue = self.prompt_has_residue(...)` ⇒ if residue {「仍在（按既有 `strip_residue` 语义…）」} ⇒ `"residue": residue` |
| `lib.rs:478` | `pub fn strip_residue(&mut self) -> bool`（**既有机制**） |
| `lib.rs:751` | `let cleaned = self.strip_residue();`（停用时走它） |
| `tests.rs:601 / 610` | 「// 清空**只动 JSONL**：**提示词一字不改**」+ `assert_eq!(result["residue"], true, "**如实报告提示…**")` |
| `commands_tests.rs:563` | `shutdown_clears_only_its_own_slot_and_global_re…`（**停用清理也有测试**） |

四个可核点：
1. ⭐⭐⭐ **这不是「没设防」，是「刻意只清一层」** —— `clear` **只清 JSONL、不写 `system_prompt`**，
   而**这个决定被三处记录**（命令头注 · 实现注释 · 测试断言）并**被测试钉住**
   ⇒ ⇒⇒ **B0362 的不对称在 Mod 侧有理由**：**清空不是「全清」，是「清一层 + 如实说另一层还在」**
   ⇒ ⇒⇒ **而 UI 把这件事告诉了用户**（B0362 的 residue 文案）
   ⇒ ⇒⇒ **整条链是：清一层 + 报另一层 + UI 说清**
2. ⭐⭐ **命令响应契约在头注里逐字段列出**（含 `residue`）⇒ 又一次「契约写在能被查到的地方」
3. ⭐ **`strip_residue()` 是既有语义**，而 B0362 的 UI 文案正说「**停用 Mod 会按既…**」
   ⇒ ⇒ **两侧引用同一个既有机制**（又一次「指路」）
4. ⭐ **停用时的清理也有测试**（`commands_tests.rs:563`「只清自己那一份 + 全局那份」）

⇒ ⇒ **0 findings**；⇒ ⭐⭐ **而本批最大的收获是**修正我 B0362 措辞里的一个含糊**：
我写「值得产品决策者看一眼」，而现在可以说：
> **机制是有理由的；缺的只是 UI 侧一句「这个动作只清一层」的注释。**
⇒ ⇒ **这不是设计缺陷，是文档缺口** —— 而**文档缺口按我的判据不足以记成发现**（P3 的门槛是「读者会因此做错事」；
此处读者**不会**做错事，因为 residue 文案已经说了）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体 ·
   `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1810 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
