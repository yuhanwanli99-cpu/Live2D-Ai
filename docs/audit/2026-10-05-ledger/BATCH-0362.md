# BATCH-0362 · ⭐ **清空整库没有确认框**，而它的成功文案带一个「只有懂行的人才会想到」的残留提示

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `memory_panel` 的**清空整库**（该面板最破坏性动作）

## 跑的命令（全部只读）
```
grep -nE "clear|清空" lib/settings/mods/memory_panel.dart | head -8
sed -n '427,446p' lib/settings/mods/memory_panel.dart ; grep -n "_clear\b" lib/settings/mods/memory_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **一个不对称，如实记账而不记发现**
```dart
Future<void> _clear() async {
  await _send('清空', 'clear', memoryCommandArgs(sessionId: widget.ctx.activeSessionId),
    (ModCommandResult result) {
      final bool **residue = result.result['residue'] == true**;
      return '已清空记忆库：清掉 ${memoryCountText(result.result['removed'])} 条，'
             '现存 ${memoryCountText(result.result['records'])} 条'
             '${residue ? '。**提示词里仍留着上一轮注入的记忆块**，停用 Mod 会按既…' : ''}';
    });
}
… onPressed: canAct ? () => unawaited(_clear()) : null,     // :663
```
三个可核点：
1. ⭐⭐⭐ **`residue` 是服务端返回的标志，而 UI 对它有专门的一句话**
   → 「**提示词里仍留着上一轮注入的记忆块**，停用 Mod 会按既…」
   ⇒ ⇒ **「清空了库，但本轮提示词里还有残留」** —— **这是一个只有懂行的人才会想到的残留**
   ⇒ ⇒ **同 B0355 的 `restarted` / `enabled`**（**服务端标志 → 前端专门文案** ⇒ **又一次「反馈含后果」**）
2. ⭐⭐ **成功文案是三段且带数字**：「清掉 **N** 条，现存 **M** 条」+ residue 分支
   ⇒ ⇒ **数量被报告**（不是「已清空」三个字）
3. ⭐⭐ **而 `clear` 没有确认框**（`:663` 直接 `onPressed`）
   ⇒ ⇒ **与 B0236 核过的 `_delete` 形成不对称**：**单条删除有确认**（且确认框**引用那条正文**）
   **⇒ 而「清空整库」这个更破坏的动作没有**
⇒ ⇒ **我为什么把它记在账本里、而不是记成发现**
| 可能的理由 | 我能否核 |
|---|---|
| 记忆库是**可重建的缓存**，不是不可恢复的数据 | **不能** —— 仓库里没有这句话 |
| 记忆是**用户自己写的**，所以清空是他的本意 | **不能** —— 没有这句声明 |
| 「加了确认反而会训练用户无脑点确认」 | **不能** —— 没有这句声明 |
⇒ ⇒ ⭐ **按我自己的规矩（拿不出证据就不进 FINDINGS），这**不记发现**，
**但它值得「产品决策者看一眼」** —— 因为 `:437` 那句 residue 提示本身**证明了后果并不显然**
⇒ ⇒ 而若将来补一句「记忆库可由导入重建，故清空不设防」，**这个不对称就立刻变成一个有理由的决定**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体 ·
   `PersonaCardFilePicker` typedef 注释 · **`residue` 在 Mod 侧怎么算出来的**
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1810 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
