# BATCH-0452 · ⭐⭐⭐⭐ **可空的三个条件，本仓三条都在** —— 而消费者就在 B0355

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `ModConfigResult`（服务端标志 → 客户端文案那一端）

## 跑的命令（全部只读）
```
grep -rn "class ModConfigResult" -A 16 lib/api/mods_api.dart | head -20
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**把 B0349 那条可空观察补成三条**（0 条新发现）
```dart
// lib/api/mods_api.dart:156-170
class ModConfigResult {
  const ModConfigResult({ required this.ok, this.restarted = false, this.enabled, });
  final bool ok;
  /// 配置生效需要服务端重启该 Mod。
  final bool restarted;
  /// 保存后该 Mod 是否仍启用；**`null` = 服务端没回这个键**。
  final bool? enabled;
}
```
### 四个可核点
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 三个字段 = 界面需要的三个答案**
   ⇒⇒ **⇒⇒⇒ 与 B0355 核的 `_okMessage`（`restarted` · `enabled == false` · 其它）**逐条对上**
2. ⭐⭐⭐⭐⭐⭐ **⇒⇒⇒ 「可空」说清了的**三个条件**，本仓**三条都在**：
   | # | 条件 | 本仓的落点 |
   |---|---|---|
   | ① | 真的是 `bool?`（不是 `bool` + 别的） | `:169` `final bool? enabled;` |
   | ② | **文档写出 `null` 的含义** | `:168` 「**`null` = 服务端没回这个键**」 |
   | ③ | **调用方按那个含义分支** | **B0355 的 `enabled == false`（显式比 `false`）** |
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 三个条件**同时**满足** ⇒⇒⇒⭐⭐⭐⭐⭐
3. ⭐⭐⭐⭐⭐ **⇒⇒⇒ 而 `!enabled` 与 `enabled == false` 的差别在这里是实质的**：
   **⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ 若写成 `!enabled`，`null` 会被当成 `true`**
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ **而写 `!enabled` 的人**看不出自己错在哪** ⇒⇒⇒⭐⭐⭐⭐⭐
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ ⇒ ⇒ 「显式比 `false`」这个写法是**三值语义逼出来的**、不是风格偏好** ⇒⇒⇒⭐⭐⭐⭐⭐
4. ⭐⭐⭐ **⇒⇒ 而 `restarted = false` 是**有缺省的**、而 `enabled` **没有缺省** ⇒⇒⭐⭐
   **⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 一个「默认 false 是常态」+ 一个「默认无（要问服务端）」—— 两种缺省被区分开了**

⇒ ⇒ **0 findings**；⇒ ⭐⭐⭐⭐⭐ **⇒ 而本批最值钱的是第 2 点**
> **一个可空字段要同时满足三个条件才「说清了」**：
> **① 真的是 `bool?` ② 文档写出 `null` 的含义 ③ 调用方按那个含义分支**
> **⇒⇒⇒ 而本仓这三条都在** ⇒⇒⇒⭐⭐⭐⭐⭐
> **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ B0349 我只记了「可空 = 有一个可推导的默认」——
> 这一批补的是「**可空的三个条件**」** ⇒⇒⇒⭐⭐⭐⭐⭐
> **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒ 「`null` 的含义」被写下来，
> 这不是注释的礼貌，而是**调用方分支的前提****

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~440 / `chat_panel` ~440）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
