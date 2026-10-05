# BATCH-0455 · ⭐⭐⭐⭐ **「键不出现」被两个来源共用**（而这是对的）—— 且 **`putTri` 是三态上线路的唯一一处**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 三态补丁的**序列化本体**（B0454 留的下半）

## 跑的命令（全部只读）
```
sed -n '/toJson()/,/^  }/p' lib/api/settings_models.dart | head -18
grep -n "void putTri" -A 10 lib/api/settings_models.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0454 那一格的两侧都对上了**（0 条新发现）
```dart
// lib/api/settings_models.dart:75-85
void putTri<T>(Map<String, Object?> out, String key, Tri<T>? field) {
  switch (field) {
    case null:            //  ⭐ 键**不出现**
    case TriKeep<T>():    //  ⭐ 键**不出现**
      break;
    case TriClear<T>():   out[key] = **null**;        // 显式清空
    case TriSet<T>(:final T value): out[key] = value;  // 设为目标值
  }
}
```
### 四个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 「键不出现」这一个 wire 形状被**两个来源**共用**（`null` 没给 / `TriKeep` 明确保持）
   ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ 而这是**正确的**：从服务端看，「不修改」与「保持」本来就**无法也不必**区分
   ⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒⇒ 它的正确性依赖**服务端把「键不出现」也当作保持** ⇒⇒⇒⭐⭐⭐⭐⭐
   ⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 而这也澄清了 B0454 那句话的适用范围**：
   **「两种『没有』必须分开」是**针对 wire 形状**的**、而 `null` / 不出现 那一侧是**第三态**、不是第四态****
2. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 而 `putTri` 是一个顶层泛型函数、`Tri<T>` 是封闭类型** ⇒⇒ **三态的编码**只有一处****
3. ⭐⭐⭐⭐⭐ **⇒⇒⇒⇒ 每一类 patch（`LlmSettingsPatch` / `TtsSettingsPatch` / …）只调 `putTri`**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒ 「三态怎么上线路」这个决定**不在任何一个 patch 类里**、而在一个共享 helper 里**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ 这是 B0335「判据只被派生一次」在**序列化**那一侧的同形** ⇒⇒⇒⭐⭐⭐⭐⭐
4. ⭐⭐⭐⭐⭐ **⇒⇒⇒⇒ 「如果每个 patch 类各写一遍 switch，『三态怎么编码』就有 N 份」** ⇒⇒⇒⭐⭐⭐⭐⭐

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 1 点**
> **「键不出现」被两个来源共用，而这是对的** ——
> **⇒⇒⇒ 从服务端看，「不修改」与「保持」本来就**无法也不必**区分**
> **⇒⇒⇒⇒ 而这也澄清了 B0454 那句话的适用范围**：
> **「两种『没有』必须分开」是**针对 wire 形状**的 · 而 `null` / 不出现那一侧是**第三态**、不是第四态**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._current_background` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `Tri` 的三个构造子（`TriKeep` / `TriClear` / `TriSet`）**本体未读**
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~440 / `chat_panel` ~440）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
