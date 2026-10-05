# BATCH-0453 · ⭐⭐⭐⭐⭐ **「0 是一个有意义的值」⇒「缺失」就不能落到 0 上** —— 一条可推广的判据

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `settings_models.fromJson`（B0452 那三个条件在它那里成不成立）

## 跑的命令（全部只读）
```
grep -nE "factory .*fromJson|static .*fromJson" -A 6 lib/api/settings_models.dart | head -10
sed -n '134,145p' lib/api/settings_models.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0452 的三个条件升级成四种形态**（0 条新发现）
```dart
// lib/api/settings_models.dart:134-145
bool get **isMaxTokensUnlimited => maxTokens == 0**;                      // ⭐ 0 是**有意义的值**

factory LlmSettingsView.fromJson(Map<String, Object?>? json) {            // ⭐ 参数可空
  final Map<String, Object?> j = **json ?? const <String, Object?>{}**;    // ⭐ 归一
  return LlmSettingsView(
    baseUrl: _str(j['base_url']),
    hasApiKey: j['has_api_key'] == true,
    // **服务端缺失该字段（旧服务端）→ 回落默认；回落成 0 语义正好相反**      // :143  ⭐⭐⭐
    maxTokens: j.**containsKey**('max_tokens') ? _int(j['max_tokens']) : defaultMaxTokens,
```
### 四个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ B0452 那三个条件在这里是**同一个模式的另一种落法**：
   | # | B0452 的三条件 | 本仓另一处的落点 |
   |---|---|---|
   | ① | 类型可表达「没有」 | ⭐ **`json` 参数本身可空**（`Map<String,Object?>?`）并**归一为空 map** |
   | ② | 文档写出「没有」的含义 | ⭐⭐⭐ **注释逐条写**：「服务端缺失该字段（**旧服务端**）→ 回落默认」 |
   | ③ | 调用方按含义分支 | ⭐ **`j.containsKey('max_tokens')`** —— **先问「键在不在」再取值** |
2. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 而第 ③ 条在这里是一个 `containsKey` ⇒⇒ 「**键在不在**」与「**值是什么**」被分开问了**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 「键在不在」与「值是什么」被分开问**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ 而这在 `bool?` 那处是**做不到**的（B0452 只能问「是 null 吗」）**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 「`containsKey` 比 `== null` 更能表达『键在不在』」—— 而这是**表结构**与**对象**的差别**
3. ⭐⭐⭐⭐⭐⭐ **⇒⇒⇒ 而那句注释还说明了**为什么必须分开**：「回落成 **0** 语义**正好相反**」
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 「0 在这一栏是一个**有意义的值**（`isMaxTokensUnlimited`）⇒
   「缺失」**不能**落到 0 上** ⇒⇒⇒⭐⭐⭐⭐⭐
4. ⇒ ⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ 「0 是一个有意义的值」⇒「缺失」就不能落到 0 上** —— **一条可推广的判据**
   ⇒⇒⇒ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ **而它的形状是「先看有没有哪些值是有意义的」** ⇒⇒⇒⭐⭐⭐⭐⭐
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 判据：给一栏选回落值之前，先问「它的取值集合里有没有哪个值本身就有意义」**

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批把 B0452 那三个条件升级成四种形态**：
> **「说清了一个『没有』」的四种落法**：
> **① `bool?` + 文档写 `null` 含义**（B0452 的 `enabled`）· **② 参数本身可空 + 归一**（本批 `json`）·
> **③ `containsKey` 先问「键在不在」**（本批 `max_tokens`）· **④ 回落值与真实值语义相反时必须用 ③**
> **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **「0 是一个有意义的值」⇒「缺失」就不能落到 0 上**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~440 / `chat_panel` ~440）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
