# BATCH-0427 · ⭐⭐⭐⭐ **「未知」是**被测试固定的行为** —— 而同一个原则在本仓有两种落法

Phase 4 · **证伪**（B0426 留的线索：`ModStatus` 新增变体时有没有测试让客户端变红）

## 跑的命令（全部只读）
```
grep -rn "statusLabel" test/ | head -5
grep -nA 8 "enum ModStatus" crates/live2d-ai-mod-system/src/status.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**线索结清，且是更好的那一种**（0 条新发现）
```dart
test/admin_api_test.dart
:169  expect(mods[0].statusLabel, '已停用');
:172  expect(mods[1].statusLabel, '运行中');
:186  expect(m.statusLabel, **'weird_state'**);        // ⭐ 未知态被**原样透出**
:197  expect((await api.list()).single.statusLabel, **'未知'**);   // ⭐ 未知态被映射成「未知」
```
四个可核点：
1. ⭐⭐⭐⭐⭐ **线索结清：有测试，而且它固定的是「降级行为」本身**
2. ⭐⭐⭐⭐ **而两个未知态、两种兜底，各自被断言**：
   | 位置 | 未知态的表现 | 钉住的性质 |
   |---|---|---|
   | `:186` 单个 `ModInfo.statusLabel` | **原样透出** `'weird_state'` | **不做猜测** |
   | `:197` 列表级 | **映射成「未知」** | **给一个不会误认的字样** |
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ 两者不矛盾**：**「不让用户误认」这个目标（B0426 的理由）两种都满足**
3. ⭐⭐⭐⭐⭐ **而这是 B0242「新增成员必然失败」的**反面、且是更好的一种**：
   > **① 编译期红**（字体门禁的 `fieldCount` · B0242）·
   > **② 运行期显示「未知」**（Mod 状态 · 本批）
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ 而 ② 的适用范围更广**（**跨语言、跨版本时不可能编译失败**）
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ 而 ② 之所以能成立，前提是「**降级路径本身被断言了**」**
4. ⭐⭐⭐⭐ **⇒ 而「未知」这三个字不是一个兜底、它是**被测试固定的一个行为****
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ ⇒ 这也修正了我 B0426 的担心**（我担心「新增变体 ⇒ 客户端要同步改」）
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒ 而实际上：客户端**不需要**同步改也能工作，只是显示得保守**
   ⇒⇒⇒ **⇒⇒⇒ ⇒ 而这正是 B0411 那条「跨语言对齐应被断言守住」的**更实际的一版**：
   **⇒⇒⇒⭐⭐ **跨语言时不可能「编译失败」，所以正确做法是「断言降级行为」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `:186` 与 `:197` 那个「两种兜底」的**分工理由**（为什么一处透出、一处映射 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
