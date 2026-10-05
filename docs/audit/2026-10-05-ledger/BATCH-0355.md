# BATCH-0355 · ⭐⭐ **成功有三个文案、两个带后果** —— 而那两个后果是**服务端返回的标志**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `_ModConfigTile` 的**保存结果分支**（B0354 留）

## 跑的命令（全部只读）
```
grep -nE "onSaveConfig|设置失败|已保存|_saving" lib/settings/sections/dev_tools_section.dart | sed -n '1,12p'
sed -n '726,760p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **那条「反馈含后果」的第五例、且是最详的一例**
```dart
final result = await save(widget.mod.id, _buildConfig());          // :729
setState(() { _saving = false; _messageIsError = !result.ok;
  _message = result.ok ? _okMessage(result) : '保存失败：服务端返回 ok=false'; });   // :731-735
if (result.ok) unawaited(_loadState());   // 「热更新（Wave 3）：配置改了 → **立刻重取运行态**…」  // :737
on ApiException catch (e) { … _message = '保存失败：$e';   // 「带上错误码：用户要拿界面上的码去日志里搜（**项目错误契约**）」  // :744-747
…
String _okMessage(ModConfigResult r) {
  if (r.restarted)  return '已保存，**服务端已重启**';
  if (r.enabled == false) return '已保存，**Mod 已停用**';
  return '已保存'; }                                          // :755-759
```
五个可核点：
1. ⭐⭐⭐ **成功有三个不同文案，其中两个带后果**（`restarted` ⇒「服务端已重启」·`enabled==false` ⇒「Mod 已停用」）
   ⇒ ⇒ **同 B0330「id 不变」· B0331「还原成主链 system_prompt」· B0346「下一条命中它的用户话」的第五例**
   ⇒ ⇒ 而**这一例最详**（三个分支）
2. ⭐⭐ **`restarted` 与 `enabled` 是服务端返回的标志，而 UI 真的区分它们**
   ⇒ ⇒ **不是「保存成功」一句话** ⇒⇒ **服务端多返回的字段，前端用上了**
   ⇒ ⇒ **这也是 B0322「同一份 API 在两处的两种正确处理」的另一例**
3. ⭐⭐ **错误分支带上错误码**（`'保存失败：$e'` ⇒ `ApiException.toString` 含码）
   + 注释**引用了项目错误契约**（B0159「拿界面上的码去日志里搜」）⇒ ⇒ **又一次「指路」**
4. ⭐⭐ **成功后 `unawaited(_loadState())`**，理由是「**让「字段跟着变」可见**」
   ⇒ ⇒ **不是「刷新一下」，是「让用户看到配置真的生效了」**
5. ⭐ **三个分支都复位 `_saving`** ⇒ ⇒ **重入闩不会卡在 true**（B0230 的 `_busy` 族）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批补齐了那条纪律的第五例**：
**「变更的反馈必须包含后果」** —— 记忆编辑 · persona 还原 · 记忆导入 · **Mod 配置（本批，三分支）**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 的**定义**（`restarted`/`enabled` 从哪来）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1820 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~430）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
