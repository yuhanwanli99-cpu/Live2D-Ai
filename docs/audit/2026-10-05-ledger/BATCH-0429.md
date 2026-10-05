# BATCH-0429 · ⭐⭐⭐⭐ **「为什么可以静默」是降级注释里最少被写、却最重要的一句**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— **两个 best-effort 读取**（B0329 引用过头注，**本体**未核）

## 跑的命令（全部只读）
```
sed -n '29,52p' lib/app/shell_admin.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**三个降级判据**（0 条新发现）
```dart
/// 表在服务端静态路由 `GET /actions/preset_labels.json`（与渲染面读的
/// `presets.json` 同一棵树）。**取不到 → 空表，调试面板回落显示稳定 id。**     // :31-32
Future<void> _loadPresetLabels() async {
  final table = await fetchPresetLabels();
  if (!mounted || **table.length == 0**) return;      // :35  ⭐ 空结果**也**当失败
  _presetLabels = table; _refresh();
}
/// **失败静默：保持 `null`，不误报红字；端点自己的 403 `mod_disabled` 仍是兜底真源。** // :40-41
Future<void> _refreshVoiceModState() async {
  try { final mods = await _modsApi.list(); … } catch (_) { /* 见上：读不到不拦。 */ }
}
```
四个可核点：
1. ⭐⭐⭐⭐ **而 `_loadPresetLabels` 的降级是「换一种显示」**（空表 ⇒ **调试面板回落显示稳定 id**）
   ⇒⇒⇒⭐⭐ **⇒ 而「**稳定** id」这个词是承重的** ⇒⇒⇒ **⇒ 它是给开发者看的、且标签表挂掉时仍然可用**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ ⇒ 这与 B0428 的判据同源：「给谁看」决定「怎么降级」**
   ⇒⇒⇒⇒ **⇒⇒⇒ 而注释自己写明主体是「**调试面板**」⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒ 所以降级到 id 是对的**
2. ⭐⭐⭐⭐⭐ **而 `_refreshVoiceModState` 写出了本仓最关键的一句降级说明**：
   「**失败静默：保持 `null`，不误报红字**；**端点自己的 403 `mod_disabled` 仍是兜底真源**」
   ⇒⇒⇒⭐⭐⭐ **⇒ 「不误报」= 「不把不知道说成知道」**（B0342「别假装有话说」）
   ⇒⇒⇒⭐⭐⭐⭐ **⇒⇒⇒ 而后半句更值钱**：
   **「读不到」不是唯一真源 · 端点自己会回 403** ⇒⇒⇒ **⇒⇒ ⇒ 两个来源**分工明确**：
   **启动时读一次（best-effort）· 每次点按钮由端点现判（权威）**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒ ⇒⇒ 这解释了为什么启动读**可以**静默** ⇒⇒⇒⭐⭐⭐⭐
   **⇒⇒⇒⇒⇒ 「静默」有依据、不是省事** ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒⇒⇒ （与 B0425 那条「一半的降级写了理由」形成对照）**
3. ⭐⭐⭐ **而 `if (table.length == 0) return;`** ⇒⇒⭐⭐ **⇒ 拿到**空表也不刷新**（避免用空表覆盖旧表）**
   ⇒⇒ **⇒ 与 B0415 核的「`/env` 失败**沿用旧值**」同形** ⇒⇒⇒⭐
   **⇒⇒⇒ ⇒⇒ 而这一次是 `length == 0` 而非异常 ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒ 第三个降级判据出现：空结果也算失败**
4. ⇒ ⇒⭐⭐⭐⭐⭐ **可提炼**
   > **「为什么可以静默」是降级注释里最少被写、却最重要的一句** ——
   > **它把「我漏了信息」与「信息在别处」这两件事区分开** ⇒⇒⇒⭐⭐
   > **⇒⇒⇒ ⇒ 而「信息在别处」**必须点名那个别处**（本例：`mod_disabled`）**
   > **⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒ 否则「静默」就与「放弃」无法区分**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `fetchPresetLabels()` 的**本体**（它自己失败时返回空表还是抛 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
