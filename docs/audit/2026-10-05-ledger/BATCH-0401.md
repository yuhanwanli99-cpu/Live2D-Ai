# BATCH-0401 · ⭐⭐ **「必须丢」与「可以补」的界是 `id`** —— 而丢弃的粒度由**代价的不对称**决定

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `ChatSession.fromJson` 单条容错（B0400 留）

## 跑的命令（全部只读）
```
sed -n '/factory ChatSession.fromJson/,/^  }/p' lib/chat/chat_session.dart   # ⇒ 零命中（是 static 不是 factory）
grep -nE "ChatSession.fromJson|static ChatSession\?" lib/chat/chat_session.dart | head -4
sed -n '88,120p' lib/chat/chat_session.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**「分层」的第三层**（0 条新发现）
> 「反序列化。**坏数据返回 `null`（丢弃这一条），绝不抛**。
> 与 **`DisplayPrefs.fromJson` 同一条纪律**：一份被外部改坏 / 版本不匹配的存储
> **不该让整个聊天区打不开**。**丢一条会话的代价远小于丢整个列表。**」            // :90-93
```dart
if (raw is! Map) **return null;**                                            // 形状不对 → 整条丢
final id = _readString(json['id']);
if (id == null || id.isEmpty) **return null;**                               // ⭐ **没 id → 整条丢**
createdAt: _readTime(...) ?? **DateTime.fromMillisecondsSinceEpoch(0)**,     // 时间坏 → 补
updatedAt: _readTime(...) ?? createdAt,                                      //        补
title: _readString(...) ?? **''**,                                            // 标题坏 → 补
for (item in rawMessages) { final m = ChatMessage.fromJson(item); if (m != null) messages.add(m); }  // 消息坏 → 只丢那条
```
五个可核点：
1. ⭐⭐⭐ **纪律被逐字陈述，并给了它自己的判据**：「丢**一条**会话的代价远小于丢**整个**列表」
   ⇒⇒ **丢弃的粒度由代价的不对称决定** ⇒⇒ **不是「宽容」，是「代价小的那侧」**
2. ⭐⭐⭐ **而它点名了同族**：「与 **`DisplayPrefs.fromJson` 同一条纪律**」⇒⇒ **第三次指路到那一处**
   ⇒⇒ **而三处（`DisplayPrefs` / `ChatSessionStore` / `ChatSession`）确实是同一形状**
3. ⭐⭐⭐⭐ **「什么必须丢」与「什么可以补」的界，是 `id`**：
   | 坏在哪 | 处置 | 为什么 |
   |---|---|---|
   | `raw` 不是 Map | **整条丢** | 形状不对 |
   | **`id` 空/缺失** | **整条丢** | ⭐ **没有 id 就无法被 `activeId` 引用、无法被 trim 定位** |
   | `createdAt` 坏 | **补 `epoch 0`** | 可排序即可 |
   | `title` 坏 | **补 `''`** | B0236 的「空记录兜底」族 |
   | 某条 `message` 坏 | **只丢那一条** | 局部坏不拖垮局部 |
   ⇒⇒⇒⭐ **`id` 是这条记录**唯一不可替代**的东西** ⇒⇒ **同 B0347 核的 persona「按 id 定位」是同一条纪律**
4. ⭐⭐ **`createdAt` 的缺省是 `epoch 0`** ⇒⇒ **一个很旧的时间** ⇒⇒ **排序时它会沉到最后**
   ⇒⇒ **一个「猜」出来的缺省、且**方向安全**（旧的沉底、不占前面）**
5. ⭐⭐ **「分层」至此是三层**：`loadChatSessions` 入口整丢 · `ChatSession.fromJson` 记录级丢 ·
   `ChatMessage.fromJson` 消息级丢 ⇒⇒ **每层有自己的丢弃判据**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 3 点**：
> **「必须丢」与「可以补」的界通常就是「那条记录里唯一不可替代的那一栏」。**
> ⇒⇒ **`id` 不可替代 ⇒ 缺它就丢**；**时间/标题可替代 ⇒ 缺它就补**
> ⇒⇒⇒ **而这个判据可以迁移**：**任何「解析失败就丢」的地方，都该先问「它缺的那一栏是不是不可替代的」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `ChatMessage.fromJson` 本体（消息级那层 · **未读**）· `AppShell._currentBackground` 本体 ·
   `evaluate_mode` 函数本体 · `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
