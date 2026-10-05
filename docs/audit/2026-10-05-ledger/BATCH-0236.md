# BATCH-0236 · ⭐ 破坏性路径：**确认框引用那条记录自己的正文**，且**显示与确认共用同一个渲染函数**

Phase 1 · 域覆盖 · 前端 `.dart`（45/218）—— `lib/settings/mods/memory_panel.dart` 的删除 / 清空

## 跑的命令（全部只读）
```
grep -nE "clear|清空|删除|confirm|确认|'delete'" lib/settings/mods/memory_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**两处「安全属性靠汇合保证」**
```dart
final bool? confirmed = await showDialog<bool>(…  title: const Text('删除这条记忆？'),      // :398-401
  content: Text('**删除后不可恢复**：${memoryRecordText(record['text'])}'),                  // :402
  … child: const Text('确认删除') );
if (confirmed != true) return;                                                               // :415
```
四个可核点：
1. ⭐⭐ **确认框引用那条记录自己的正文** ⇒ 用户看到的是「**要删的是哪一条**」，而不只是「确定吗？」
   ⇒ 而 `memoryRecordText` **正是列表显示用的那个函数**（B0187 核过它：**压平空白 + 截断 + 空记录兜底**）
   ⇒ ⇒ ⭐ **显示与确认共用同一个渲染函数** ⇒ **确认框不可能显示出与列表不同的东西**
   ⇒ ⇒ 这是 **P2（私有汇合）用在「安全属性」上**，而不只是用在「一致性」上
2. ⭐ **`if (confirmed != true) return;`**（:377 / :415）⇒ **`null`（对话框被 ESC / 返回键关掉）
   不会被误判为「确认」** ⇒ ⇒ **破坏性动作取的是「null 安全的那个方向」**
3. **两处破坏性动作各有一个确认框**（:352 / :398）⇒ 清空与删除**都**要确认
4. ⭐ 而**清空的失败也被映射成可处置文案**（:14「成功/失败都写**带错误码**的文案」+
   :120-123 `memoryCommandErrorMessage(e, '清空')`）⇒ ⇒ **失败路径与成功路径同样被照顾**
   ⇒ 且它用的是**共享**的映射函数（B0187 核过其测试：「**每个码都带码且可处置**」）⇒ ⇒ **P2 贯穿整个面板**

## 未核实项
1. `memory_panel.dart` 余约 590 行未读（记录行本体 · 编辑对话框 · 导入一条 · 桶降级说明 UI）
2. `persona_panel.dart` 余 ~560 行未读（会话绑定卡片的呈现 · 停用还原 · 坏卡失败处置文案）
3. `message_bubble.dart` 余 ~490 行 · `chat_panel.dart` 余 540 行 · `error_banner.dart` 余 45 行未读
4. `live2d_stage.dart` 余 ~645 · `director_observer_section.dart` 余 ~650 未读
5. 前端 `.dart` 仍 173 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
