# BATCH-0399 · ⭐⭐ **三种「没有」收敛到同一个值** —— 而我判断这个合并是**正确的**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `loadChatSessions` 的**读取失败分支**（B0398 留）

## 跑的命令（全部只读）
```
sed -n '/ChatSessionStore loadChatSessions/,/^}/p' lib/app/browser_io.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **一次「该不该合并」的判断，而我的答案是「该」**
```dart
ChatSessionStore loadChatSessions() {
  try {
    final String? raw = web.window.localStorage.getItem(kChatSessionsKey);
    if (raw == null || raw.isEmpty) **return ChatSessionStore.empty();**     // ① 键不存在 / 空串
    return ChatSessionStore.fromJson(jsonDecode(raw));                       // ② 解析与反序列化**都在 try 内**
  } catch (_) { **return ChatSessionStore.empty();** }                        // ③ 坏档
}
```
四个可核点：
1. ⭐⭐⭐ **「空」有三个来源而它们收敛到同一个值**：**键不存在** · **键是空串** · **JSON 坏了**
   ⇒ ⇒ **而「键不存在」与「键坏掉」不是一回事** ⇒⇒ **同一个 `empty()`**
   ⇒ ⇒⇒ ⭐ **而我判断这个合并是**正确的**，理由：**存档是缓存**（对话仍在内存/服务端）、
   **坏掉的存档没有可恢复的内容** ⇒⇒ **「降级成空」与「降级成默认」是同一种正确选择**
   ⇒ ⇒⇒ **且 B0398 已核写侧的静默理由写着「不该影响正在进行的对话」** ⇒⇒ **两侧口径一致**
   ⇒ ⇒⇒ **按 B0362/B0392 的判据（文档缺口不算发现、且要有理由）⇒ 不记发现**
2. ⭐⭐ **一个 `catch` 覆盖两类异常**（读失败 + 解析失败）⇒⇒ **不区分** ⇒⇒ **而这与 B0398 核的
   `saveChatSessions` 的 `catch (_)` **同一风格** ⇒⇒ **Dart 侧不区分异常类型是全仓一致的写法**
3. ⚠ **它的代价**：**真的 bug（空指针之类）也会被吞成「空存档」**
   ⇒⇒ **按「无证据不记发现」，我只把它记为一条**口径观察**，不进 FINDINGS**
   ⇒⇒ **可核的边界**：若要区分，需要 `catch` 里先试 `jsonDecode` 再试 `fromJson`，**两处分开的错误文案**
   —— **而这属于改进、不属于缺陷**（**用户可察觉的后果都是同一个：没有历史**）
4. ⭐ **`if (raw == null || raw.isEmpty)` 把「空串」也归为空** ⇒⇒ 防御性判据
   ⇒⇒ **而写侧 `jsonEncode` 不会产生 `""`** ⇒⇒ **它防的是「外部编辑 / 旧版本残留」**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的收获是「一次判断被写下来」**：
> **「三种『没有』合并成一个值」既可能是疏忽、也可能是对。**
> ⇒⇒ **区分它们的办法只有一个：问「它们可不可区分、有没有不同的正确处置」**
> ⇒⇒ **这里两者都不可区分、正确处置也相同** ⇒⇒ **合并是对的** ⇒⇒ **不记发现，但把判断写进账本**
> ⇒⇒⇒ **B0388 那条判据的第三次应用**（上两次是「那张表自称为什么」/「计数不等于判据」）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
