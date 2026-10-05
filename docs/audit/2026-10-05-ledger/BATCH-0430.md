# BATCH-0430 · ⭐⭐⭐⭐ **三种失败全被翻译成「空表」** —— 而外面那行 `length == 0` 接的是**最后一层**

Phase 4 · **证伪**（B0429 留的：`fetchPresetLabels` 自己失败时返回空表还是抛）

## 跑的命令（全部只读）
```
grep -rn "fetchPresetLabels" -A 14 lib/ --include=.dart | head -16
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**「把失败翻译成值」的完整实现**（0 条新发现）
```dart
// lib/settings/preset_labels.dart:113-123
Future<PresetLabelTable> fetchPresetLabels() async {
  try {
    final res = await http.get(Uri.base.resolve('/actions/preset_labels.json'));
    if (res.statusCode != 200) **return PresetLabelTable.empty;**          // ① 非 200
    return PresetLabelTable.parse(res.body);
  } catch (_) {
    **return PresetLabelTable.empty;**                                       // ② 抛（网络 **+ 解析**）
  }
}
```
五个可核点：
1. ⭐⭐⭐⭐⭐ **而三种失败全部被映射成「空表」**：
   | 失败 | 处置 |
   |---|---|
   | ① HTTP 非 200 | `PresetLabelTable.empty` |
   | ② 网络抛（`catch (_)`） | `PresetLabelTable.empty` |
   | ③ **解析失败**（`parse` 抛） | **被同一个 `catch` 接住** ⇒ `empty` |
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 而调用方（B0429 核的 `_loadPresetLabels`）因此**不需要 `try`**、**只需要判 `length == 0`**
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒⇒ ⇒ 这是一个「**把失败翻译成值**」的完整实现**
   （B0392 核的「降级不能压过错误」**同一族**、**且在同一个函数里做完**）
2. ⭐⭐⭐⭐⭐ **而 B0429 我记的那条 `if (table.length == 0) return;` 现在看清了**：
   **它不是多余的 —— 它挡的是第 ③ 种（解析失败 ⇒ 空表）**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ ⇒ 「空结果也算失败」这个判据**在这里是有用的**、不是冗余**
3. ⭐⭐⭐ **`catch (_)` 不区分异常类型** ⇒⇒ **同 B0398 / B0399 核的 Dart 侧一致风格**
4. ⭐⭐⭐⭐ **而 `Uri.base.resolve('/actions/preset_labels.json')`** ⇒⇒⭐⭐⭐
   **⇒⇒ 「相对于 base」而不是拼 host** ⇒⇒⇒ **⇒ 同 B0321「主键不用 `*`」同一族** ——
   **⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 「路径是相对的、base 由宿主决定」⇒⇒⇒ ⭐⭐⭐ **⇒⇒⇒ ⇒⇒ 「跟着部署走」而不是「写死一个 host」**
5. ⇒ ⇒⭐⭐⭐⭐⭐ **而本批最大的收获是一个结论**
   > **「把失败翻译成值」与「在调用方判空值」是**一对** ——
   > **只有两者都在，「失败」才不需要在每一处被处理** ⇒⇒⇒⭐⭐⭐
   > **⇒⇒⇒ ⇒⇒ 而本仓这**两者都在**（`fetchPresetLabels` 翻译 + `_loadPresetLabels` 判空）**
   > **⇒⇒⇒ ⇒⇒ ⇒ 而 B0429 我看到 `length == 0` 时**只以为它是防空表**，
   > **⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ 现在知道它接的是**三层失败漏下来的最后一层****

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `PresetLabelTable.parse` 的**本体**（它自己有没有「坏 JSON ⇒ 空表」的第二层保护 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
