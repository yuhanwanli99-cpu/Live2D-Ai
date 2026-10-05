# BATCH-0404 · ⭐⭐⭐⭐ **`version` 的作用是「让读侧整丢我」—— 而它必须总被写出来**

Phase 4 · **证伪**（同一条规则在会话层 / 存储层是否成立 · B0403 留）

## 跑的命令（全部只读）
```
sed -n '/class ChatSessionStore/,/^}/p' lib/chat/chat_session.dart | grep -A 10 "toJson()"
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**规则在最要紧处生效**（0 条新发现）
```dart
Map<String, Object?> toJson() => <String, Object?>{
  '**version**': kChatStoreVersion,                                        // ⭐⭐⭐⭐ 必写
  '**activeId**': activeId,                                                // 必写，null 也写
  'sessions': <Object?>[for (final s in sessions) s.toJson()],            // 必写，空也写
};
```
三个可核点：
1. ⭐⭐⭐⭐ **`version` 永远写** ⇒⇒ **而它就是那个门禁的判据**
   （读侧：`if (json['version'] != kChatStoreVersion) return ChatSessionStore.empty();`）
   ⇒⇒⭐⭐⭐⭐ **⇒ 若它也走「缺省不写」，新存的文件就没有门禁字段 ⇒ 读侧会把「自己刚写的文件」整丢**
   ⇒⇒⇒ **这是「**必需 ⇔ 必写**」这条规则在最要紧处的一次生效**
2. ⭐⭐⭐ **`activeId` 写 `null` 而不是省略**，而读侧
   `activeId: sessions.any((s) => s.id == activeId) ? activeId : null`
   ⇒⇒ **「有 activeId」与「没有」在存储里是两种**（`"activeId": null` vs 缺字段）⇒⇒ **而两者读出来都是 `null`**
   ⇒⇒⇒ **⇒ 与 B0400 记的「三种坏法、三种处置」对齐**：**四种可能的输入、两种可能的输出**
3. ⭐⭐ **`sessions` 哪怕空也写 `[]`** ⇒⇒ **「没有会话」被显式表达而不是省略**
   ⇒⇒ **同 B0349 核的「`post_once` 要写 `null` 而不是省略」那一族**

⇒ ⇒⭐⭐⭐ **而本批把 B0401 那条判据推到了它的边界**
> **「不可替代 ⇒ 必写」在 `version` 上是最强的形式** ——
> **一个字段的作用是「让读侧整丢我」，而它自己一旦缺失，读侧就会整丢包括它自己在内的一切。**
> ⇒⇒⇒⭐ **⇒ 所以这条规则不只是「别丢信息」，还是「别让判据自己消失」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `ChatSession.toJson` 本体（**只核了 `ChatMessage.toJson` 与 `ChatSessionStore.toJson`**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
