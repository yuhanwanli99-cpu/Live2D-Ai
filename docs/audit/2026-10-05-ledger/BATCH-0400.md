# BATCH-0400 · ⭐⭐⭐ **三层三种处置** —— 而这**修正了我 B0399 的措辞**：「合并」只在入口层成立

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `ChatSessionStore.fromJson`（B0399 留下的问题）

## 跑的命令（全部只读）
```
sed -n '/factory ChatSessionStore.fromJson/,/^  }/p' lib/chat/chat_session.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**策略是分层的，而我的 B0399 只在一个层次上成立**（0 条新发现）
```dart
if (raw is! Map) **return ChatSessionStore.empty();**                        // 层① 整体不是 Map
// **版本门禁：不认识的版本整体丢弃而不是尽力解析**——半懂不懂地[读]
// 一份未来格式，比重来一遍更危险（**可能把新字段写没了**）。
if (json['version'] != kChatStoreVersion) **return ChatSessionStore.empty();**   // 层② 版本不认识
for (final item in rawSessions) { final s = ChatSession.fromJson(item);
  if (s != null) sessions.add(s); }                                        // 层③ **逐条容错**
activeId: sessions.any((s) => s.id == activeId) ? activeId : null,          // 层④ 指向不存在的会话
// 「`activeId` 指向一个不存在的会话（**存储被外部改坏**）→ **当作没选**」
store._trimSessions(); for (final s in store.sessions) { ChatSessionStore._trimMessages(s); }  // 层⑤ 读时也夹持
```
五个可核点：
1. ⭐⭐⭐⭐ **版本门禁的第二个判据**（B0399 我只找到「不可恢复」那一个）：
   「不认识的版本**整体丢弃**而不是尽力解析 —— 半懂不懂地[读]一份未来格式，**比重来一遍更危险
   （可能把新字段写没了）**」⇒⇒⭐ **理由是**写回**：解析一份未来格式再写回去 ⇒ **会抹掉新字段**
   ⇒ ⇒⇒ **而它与「不可恢复」是**独立的两个理由** —— 即使旧格式**完全可解析**，也整体丢弃**
2. ⭐⭐⭐ **而版本对时，逐条容错**：坏的那一条**被跳过**，好的留下 ⇒⇒ **不是整体丢弃**
3. ⭐⭐⭐ **`activeId` 是第三种处置**（「指向不存在的会话（**存储被外部改坏**）→ **当作没选**」）
   ⇒⇒ **三个坏法、三种处置** ⇒⇒ **而「外部改坏」这个来源被点名**（同 B0399 的 `raw.isEmpty` 判据）
4. ⭐⭐ **`_trimSessions()` + `_trimMessages()` 在反序列化后被调用** ⇒⇒ **B0398 核的「双重夹持」**
   **不只是写入时** ⇒⇒ **读入的旧档也会被裁** ⇒⇒⭐ **一个外部塞进来的大档不会撑爆存储**
5. ⭐ **层①②返回 `empty()`、层③④⑤继续** ⇒⇒ **粗粒度失败整丢、细粒度失败局部丢**

⇒ ⇒⭐⭐⭐ **而这修正了 B0399 的措辞**
| | B0399 我写的 | 本批看到的 |
|---|---|---|
| 层次 | 「三种『没有』合并成一个值，合并是对的」 | **在 `loadChatSessions` 那一层成立** |
| 更细 | — | **到了 `fromJson` 那一层，它们被分成三种处置** |
⇒⇒⇒⭐ **分层才是真相：「合并」发生在粗粒度入口，「区分」发生在细粒度解析**
⇒⇒⇒ **⇒ 我 B0399 那条判断**只在一个层次上成立** ⇒⇒ **而我当时没有说清是哪个层次**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `ChatSession.fromJson(item)` 单条的容错粒度（它自己吞什么 · **未读**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
