# BATCH-0431 · ⭐⭐⭐⭐ **「降级成 id」不是 UI 决定，而是数据层的判据**

Phase 4 · **证伪**（B0430 留的：`PresetLabelTable.parse` 有没有自己的保护）

## 跑的命令（全部只读）
```
grep -n "PresetLabelTable.parse" -A 12 lib/settings/preset_labels.dart | head -16
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**四级保护 + 逐级剥离**（0 条新发现）
```dart
static PresetLabelTable parse(String source) {          // :86
  try {
    final decoded = jsonDecode(source);
    if (decoded is! Map) **return empty;**              // :89  ① 顶层不是 Map
    final raw = decoded['labels'];
    if (raw is! Map) **return empty;**                  // :91  ② 没有 labels / 不是 Map
    raw.forEach((key, value) {
      if (key is! String || value is! Map) **return;**  // :94  ③ **逐条**：形状不对 → 跳过这一条
      final zh = value['zh'];
      if (zh is! String || zh.trim().isEmpty) **return;**// :96  ④ **逐条**：没有非空 zh → 跳过这一条
      …                                                  // :98 `channel` 那一行本批截断（未核）
```
五个可核点：
1. ⭐⭐⭐⭐⭐ **而 `parse` 自己有四级保护** ⇒⇒ **⇒ B0430 那个问题的答案是「有、而且是最细的那一级」**
2. ⭐⭐⭐⭐ **而降级是**逐级剥离**、不是「一坏全空」**：
   | 层 | 坏什么 | 结果 |
   |---|---|---|
   | ① | 顶层不是 Map | **整表空** |
   | ② | 没有 `labels` / 不是 Map | **整表空** |
   | ③ | **某一条**形状不对 | ⭐ **只跳过那一条** |
   | ④ | **某一条**没有非空 `zh` | ⭐ **只跳过那一条** |
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ ③④ 与 B0402 核的 `ChatSessionStore.fromJson`「逐条容错」完全同型**
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 而 ①② 与 B0400 核的「版本不认识 ⇒ 整丢」同型**
   ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ 「粗粒度整丢 / 细粒度跳过」的两层结构，第三次出现**
3. ⭐⭐⭐⭐⭐ **而 ④ 的判据是「`zh` 是**非空**字符串」** ⇒⇒⭐⭐
   **⇒ 中文标签是**必需**的 · 英文（`en`）不是**
   ⇒⇒⇒⭐ **⇒⇒ 与 B0401 核的「**不可替代的那一栏**」同一个判据**（缺它就丢那条）
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ 而 B0430 核的「调试面板回落显示稳定 id」在这里有了机制解释**：
   **⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ 「没有 `zh`」正是「只能显示 id」的那种情况** ⇒⇒⇒⭐⭐⭐⭐
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ 降级形态与数据缺口是**同一个判据****
4. ⇒ ⇒⭐⭐⭐⭐⭐ **而本批最值钱的是第 3 点的副产物**
   > **「没有它就降级成 id」**不是一条 UI 决定，而是**数据层的一个判据**（`zh` 非空）
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒ 「降级形态由数据形状决定、不由界面决定」**
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ 而这是比「判空值」更深一层的约定** ⇒⇒⇒⭐⭐⭐
   **⇒⇒⇒ ⇒⇒ ⇒ ⇒⇒ ⇒ 它在解析时就决定了「这条能不能被显示成中文」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `parse` 的 `:98` 之后（`channel` 那一行与 `PresetLabel` 的字段 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
