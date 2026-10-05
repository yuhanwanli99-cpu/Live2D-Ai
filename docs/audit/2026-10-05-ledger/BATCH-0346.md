# BATCH-0346 · ⭐ 三条变更路径的成功文案**都回答同一个问题**：「它什么时候生效 / 有什么副作用」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `memory_panel` 的**导入**（该面板最后一个未核动作）

## 跑的命令（全部只读）
```
grep -nE "导入|'import'|_import" lib/settings/mods/memory_panel.dart
sed -n '325,352p' lib/settings/mods/memory_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **三条路径的共性浮现**
```dart
Future<void> _import() async {
  final String text = _importController.text.trim();                    // :326
  if (text.isEmpty) { … _message = '导入失败：请先输入要记住的内容'; return; }   // :327-333
  await _send('导入', 'import',
    memoryCommandArgs(sessionId: widget.ctx.activeSessionId, extra: {'text': text}),
    (ModCommandResult _) => '已导入一条记忆（**下一条命中它的用户话，本轮就会**…）');   // :335-341
  if (mounted) _importController.clear();                               // :342
}
```
四个可核点：
1. ⭐⭐ **成功文案说的是「什么时候生效」**（**下一条命中它的用户话，本轮就会…**）
   ⇒ ⇒ **同族**：B0330 的「已更新这条记忆（**id 不变**）」（**副作用**）·
   B0331 的「关闭开关会**还原成主链 `system_prompt`**」（**还原成什么**）
   ⇒⇒⇒ **三条变更路径的文案都回答「它什么时候生效 / 有什么副作用」**
2. ⭐⭐ **三条路径共用一个 `_send`** ⇒ ⇒ 而 **B0230 已核它有重入闩 `_busy`**
   ⇒ ⇒ **导入 / 编辑 / 删除不可能只漏掉一条的闩** ⇒ **共用的代价，也顺带共给了保护**
3. ⭐⭐ **本地校验在发命令之前**，且提示**是具体的**（「请先输入要记住的内容」）
   ⇒ ⇒ 与 B0330 编辑路径**同形**（先判空、再发）⇒ ⇒ **同一面板里的两条路径写法一致**
4. ⭐ **`if (mounted) _importController.clear();` 在 `_send` 之后** ⇒ ⇒ **成功才清**
   ⇒ ⇒ 与 **B0287「有理由才清」**同族（写进 `.env` 之后就没有理由留着）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的价值是「共性浮现」**：
**同族的三处（W0330 / W0331 / 本批）我此前是分别核的，
而它们连起来才显出一条纪律：变更的反馈必须包含「后果」，而不只是「成功」。**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 的**定义本体**未读 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~510 / `message_bubble` ~450 / `chat_panel` ~440）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
