# BATCH-0330 · ⭐ 记忆编辑：**第一行注释就是一条真机 `FocusScope` 断言** —— 而它防的是「有人改得更规范」

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `memory_panel` 的**编辑对话框**（B0236 核过删除）

## 跑的命令（全部只读）
```
grep -nE "编辑|TextField|update|'save'|dialog" lib/settings/mods/memory_panel.dart
sed -n '344,396p' lib/settings/mods/memory_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **四个可核点，第一是「真机症状决定惯用法」**
```dart
/// 用 `initialValue + onChanged` 而不是 controller：对话框退出动画期间
/// widget 仍在树上，此时 **dispose controller 会让重建命中已释放的焦点节点**
/// （**真机上表现为一次诡异的 `FocusScope` 断言**）。                      // :348-350
final bool? confirmed = await showDialog<bool>(… TextFormField(key: const Key('memory-edit-field'),
  initialValue: draft, maxLines: 4, autofocus: true, onChanged: (String v) => draft = v) …);
final String text = draft.trim();
if (confirmed != true) return;                                            // :375
if (text.isEmpty) { … _message = '更新失败：**正文不能为空**'; return; }   // :376-381
await _send('更新', 'update', memoryCommandArgs(… extra: {'id': id, 'text': text}),
  (ModCommandResult _) => '已更新这条记忆（**id 不变**）');                   // :383-391
```
四个可核点：
1. ⭐⭐⭐ **两个 Flutter 惯用法之间的选择，由一条**真机症状**决定**
   ⇒ 「`initialValue + onChanged` 而不是 controller」+「**真机上表现为一次诡异的 `FocusScope` 断言**」
   ⇒ ⇒ ⭐ **这条记录防的是「有人改得更规范」** —— 而**B0329 的「代价如实记录」是它的同族**
   （那边记「我们主动放弃了什么」，这边记「另一条路会崩」）
   ⇒ ⇒⇒ **「为什么不用那个更常见的写法」是一类值得单独记的注释**，因为**它会被善意地推翻**
2. ⭐⭐ **`if (confirmed != true) return;` 在校验空之前** ⇒ ⇒ **先看用户是否点了「保存」，再看内容**
   ⇒ ⇒ 而空内容的提示**是具体的**（「**正文不能为空**」）⇒ ⇒ **不是笼统的「更新失败」**
3. ⭐ **按 `id` 定位**（`extra: {'id': id, 'text': text}`）⇒ ⇒ **与头注承诺的「编辑/删除按 `id` 定位」一致**
   （B0236 已核头注）
4. ⭐ **成功文案带一条额外保证**：「已更新这条记忆（**id 不变**）」
   ⇒ ⇒ **顺带说清副作用**（编辑不会改变它属于哪条）⇒ ⇒ **P1 家族**

⇒ ⇒ **0 findings**；⇒ ⭐ 而本批补上了一个此前没单列的注释类别：
**「为什么不用那个更常见的写法」** —— **它和「代价如实记录」一样，都是为了防止「善意地推翻」**

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~515 / `message_bubble` ~455 / `chat_panel` ~460）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
