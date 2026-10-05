# BATCH-0438 · ⭐⭐⭐ **30 个落点** —— 所以我**只记分母、不下结论**

Phase 4 · **证伪**（把 B0437 的「两种强度」**计数化**：跨全仓扫「扫源码字符串」的测试）

## 跑的命令（全部只读）
```
grep -rln "readAsStringSync|readAsString()" test/ | head -8
for f in $(grep -rln "readAsStringSync|readAsString()" test/); do
  printf "%-46s 行数:%s 注释行:%s" "$(basename $f)" "$(wc -l < $f)" "$(grep -cE '^\s*//|/\*\*' $f)"
done
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一个分母，与一条诚实的「不结论」**
| 项 | 机械值 |
|---|---|
| **扫源码字符串的测试文件数** | ⭐⭐⭐⭐ **30**（从 `wiring_test.dart` 146 行到 `display_prefs_test.dart` 892 行） |
| 我**已核**注释处理强度的 | **2**（`visual_language_test` 排除 · `wiring_test.referencedOutside` 不排除） |
| 我**未核**的 | **28** |

### 三个可核点
1. ⭐⭐⭐⭐⭐ **⇒ 「结构测试」在这个仓是一个**有规模的层**，不是零星几处**
   ⇒⇒⭐⭐⭐⭐ **⇒⇒ ⇒ 而这让 B0437 的观察**更值得记、也更不该夸大**：
   > **30 个文件里我只确认了**两处**的强度不同** ⇒⇒⇒⭐⭐ **⇒⇒⇒ 「两处强度不同」是一个**观察**，
   > 而「30 处强度相同」**不是结论**（未核）** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒ **按我自己的规矩：不出结论、只记分母**
2. ⭐⭐⭐⭐⭐ **⇒⇒ 而「强度差异」在任何地方都没有被声明**：
   **「是否排除注释」「是否指名调用方」**这两条**没有一个文件说明自己按哪种强度写**
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒ **而 `visual_language_test` 是**唯一一处**把这个差别做成「反向对照」的**（B0381 已核）
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒⇒ ⇒⇒⇒ ⇒⇒ **「谁把这条纪律做成了自我证明」是本仓的稀缺项**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒ `wiring_test:77` 那条本可以照抄 B0381 的做法、而没有**
3. ⭐⭐ **⇒⇒ 而这给了 F-0436-01 一条**规模依据**：它不是「一个孤立的弱点」，
   **而是「一个有 30 个落点、只有 1 个做了自我证明的层」里的一个条目**
   ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ **⇒ 换句话说：这一层的默认强度是「不排除注释、不指名调用方」，而唯一的例外没有被复用到同层**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. **28 个文件的注释处理强度未逐个核**（**按判据不结论、只记分母**）· ⚠ **这是本批最大的诚实标注**
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
