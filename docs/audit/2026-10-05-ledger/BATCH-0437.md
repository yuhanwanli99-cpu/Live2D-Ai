# BATCH-0437 · ✅ **反证结清**（F-0436-01）—— 而「同一种手法」在本仓被用成**两种强度**

Phase 4 · **证伪**（读 `referencedOutside` 的实现）

## 跑的命令（全部只读）
```
grep -rn "bool referencedOutside" -A 14 test/ | head -18
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**反证结清；本条仍 P3、不升级，但多一层落差**
```dart
// test/wiring_test.dart:38-49
bool referencedOutside(String symbol, String ownFile) {
  final sources = Directory('lib').listSync(recursive: true).whereType<File>()
      .where((f) => f.path.endsWith('.dart'))
      .where((f) => !f.path.endsWith(ownFile)).toList();
  for (final file in sources) { if (file.readAsStringSync().contains(symbol)) return true; }
  return false;
}
```
四个可核点：
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 反证结清：它只有 `(symbol, ownFile)` 两个参数、不接受「调用方」**
   ⇒⇒⇒ **⇒⇒ 「被 `ChatController` 用上」确实没有断言支撑** ⇒⇒⇒⭐⭐
   **⇒⇒⇒ ⇒⇒ ⇒ 「建议补第三条断言」成立**、而「删掉多余参数」那条分支**不成立**
2. ⭐⭐⭐⭐⭐ **而它是 `contains(字符串)`、不解析符号 ⇒ 注释里提到也算命中** ⇒⇒ **一行注释就能让 `:77` 变绿**
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 同仓另一条扫源码字符串的测试**（`visual_language_test`，B0381 已核）
   **刻意排除注释**（有反向对照：注释里的 `**` 不该被扫出来）
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ 「同一种手法在同一个仓里被用成两种强度」**，
   而**弱的那一种恰好被写在一条承诺更强的测试名下**
3. ⭐⭐⭐⭐ **而 `!f.path.endsWith(ownFile)` 正确排除了自己那个文件** ⇒⇒ **「在别处被引用」被正确检查**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒ 「意图对、强度弱」—— 这就是 F-0436-01 的准确性质**（不是「测试无效」）
4. ⭐⭐ **而它扫 `lib/`（不含 `test/`）** ⇒⇒ **⇒ 与「必须被接线」的意图一致** ✅

⇒ ⇒ **F-0436-01 仍 P3、不升级**（当前无缺陷）
**⇒ ⇒ 但新增一层落差**：「被文件外引用」**可被一行注释满足**、而名字说的是「被 `ChatController` 用上」
⇒ ⇒⇒ **修法相应地是两处：排除注释 + 改名或点名调用方**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
