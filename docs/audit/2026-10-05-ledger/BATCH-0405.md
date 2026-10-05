# BATCH-0405 · ⭐⭐⭐ **写侧三件套完整** —— 而**每层做的事不同**（这是 B0404 的答案）

Phase 4 · **证伪**（`ChatSession.toJson` · B0404 留的最后一处）

## 跑的命令（全部只读）
```
sed -n '/Map<String, Object?> toJson/,/};/p' lib/chat/chat_session.dart | head -14
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**写侧三件套齐了**，且 **「缺省不写」被限定在消息层**（0 条新发现）
```dart
// ChatSession（会话层）—— **五项全写**
'**id**': id,                                              // 读侧缺它就丢这条（不可替代）
'title': title,                                           // 读侧缺它补 `''`（可替代）
'createdAt': createdAt.**toIso8601String()**,              // ⭐ **转字符串，不转 epoch 毫秒**
'updatedAt': updatedAt.toIso8601String(),
'messages': [for (final m in messages) m.toJson()],
// ChatSessionStore（存储层）—— version / activeId / sessions 三项全写
// ChatMessage（消息层）—— role / text 必写 · epoch/failed/unfinished **缺省不写**
```
四个可核点：
1. ⭐⭐⭐ **「缺省不写」被**限定在消息层**、并有理由**：
   ⇒⇒ **`role` / `text` 是每条消息的主体**（一条消息没有正文就没有意义）
   ⇒⇒ **而 `epoch` / `failed` / `unfinished` 是大多数消息都没有的附加状态**
   ⇒⇒⇒⭐ **⇒ 同一份文件里，两层用了两种策略** ⇒⇒⇒ **而判据是统一的**：
   > **「可替代 ⇔ 缺省不写；不可替代 ⇔ 必写」** —— **在消息层三项可替代、在会话层五项全不可替代**
   ⇒⇒⇒ **⇒ 「全部必写」与「缺省不写」**不是两种风格，是同一条规则在两层上的两个结论**
2. ⭐⭐⭐ **`createdAt` / `updatedAt` 写成 `toIso8601String()`** ⇒⇒ **可读、且带时区**
   ⇒⇒ **而读侧 `_readTime(...) ?? DateTime.fromMillisecondsSinceEpoch(0)`**（B0401 核过）
   ⇒⇒⇒⭐ **⇒ 两侧格式不对称（写 ISO 串 / 读 `DateTime`）** ⇒⇒ **而这是**格式与类型**的对称，不是**格式的对称**
3. ⭐⭐ **而 `title` 全写、即使它常是空串** ⇒⇒ **而 B0401 核的读侧是 `_readString(...) ?? ''`**
   ⇒⇒ ⇒ **「title 常为空」是常态，但仍然写** ⇒⇒ **⇒ 「常为空」不等于「可省略」**
4. ⭐⭐⭐⭐ **而这修正了我 B0404 留下的那个 ⚠**：我写「**两处夹持机制不同**」——
   ⇒⇒ **那是对的、而且正是本批的结论**：
   | 层 | 缩减手段 |
   |---|---|
   | **消息层** | **不写非必需字段**（省字符） |
   | **会话/存储层** | **`maxSessions` × `maxMessages` 双重夹持**（丢整条） |
   ⇒⇒⇒ **⇒ 两层各用各的手段，而**目标同一个：让 5 MB 装得下**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 4 点**：
> **同一个预算约束，在不同层用不同手段达成** ——
> **消息层「不写没用的」· 会话层「丢不下的」** ⇒⇒ **而这比「全用一种手段」更省、也更难写坏**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
