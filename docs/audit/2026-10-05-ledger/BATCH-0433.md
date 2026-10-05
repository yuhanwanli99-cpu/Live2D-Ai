# BATCH-0433 · ✅ **+1 条 P3：一句「可被断言、却没人断言」的自述**

Phase 4 · **证伪**（B0432 留的：那条说法有没有对应的断言）

## 跑的命令（全部只读）
```
grep -rn "semantics|Semantics|bySemanticsLabel|matchesSemantics" test/*.dart | grep -iE "devmode|导演|observer|语义树|semantics" | head -6
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**F-0433-01 (P3)**
> `dev_tools_section.dart:1296-1298`「**只在 devMode 下渲染** —— off 时**整块不在语义树**（本 if 块一起消失）」

**全仓唯一断言语义树的是** `test/audio_bar_test.dart:173-183`
（`tester.ensureSemantics()` + `find.bySemanticsLabel('主音量')` · 主音量滑杆）
⇒⇒ **没有任何测试断言「devMode 关闭时导演可观测整块不在语义树」**

### 四个可核点
1. ⭐⭐⭐⭐ **这句话今天为真**（`if (devMode)` 不构建该 widget）⇒⇒ **⇒ 但它是唯一一处把
   「无障碍树里没有这个节点」当成**可断言事实**写下来的地方，而没有对应断言**
2. ⭐⭐⭐⭐⭐ **⇒ 按 P24「让变更必然失败」**（B0242 字体 `fieldCount` · B0335 穷举表 ·
   B0333「相位只被派生一次」= 同一原则）：**把 `if (devMode)` 改成
   `Opacity(opacity: devMode ? 1 : 0)`（一种常见的「可见性重构」）⇒ 没有任何东西变红**
   ⇒⇒ 而后果是**读屏用户读到整块导演观测面板**（A/B/C/D 四栏 + 事件流 + TTS 文本）
   ⇒⇒ **回归防护缺口、不是当前缺陷 ⇒ P3**
3. ⭐⭐ **修法是一条测试**（注释已把该断言的形状写好了）⇒⇒ 与 `audio_bar_test.dart` 同一手法
4. ⭐⭐⭐⭐⭐ **⇒ 而它与 B0418 核的「可核对的自我描述」标准正好对照**：
   **那一句是「可核对的自我描述」；这一句声称了一个可断言的事实、却没有断言**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ **它差的那一步，正是 B0418 在同仓里演示过的那一步**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. **是否存在一条用 `find.byType(DirectorObserverSection)` 而不用语义树的等价断言**（**未逐个通读 `test/`**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
