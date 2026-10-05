# BATCH-0402 · ⭐⭐⭐ **判据在消息层成立** —— 而「不可替代的栏」从一个变成**三个**，理由各不相同

Phase 4 · **证伪**（检查 B0401 那条可迁移判据在第三层成不成立）

## 跑的命令（全部只读）
```
sed -n '/static ChatMessage? fromJson/,/^  }/p' lib/chat/chat_message.dart
grep -rln "class ChatMessage" lib/ | head -2
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**判据成立，而且是它最锋利的一次应用**（0 条新发现）
```dart
if (raw is! Map) return null;
if (rawRole is! String) return null;
ChatRole? role; for (final c in ChatRole.values) { if (c.name == rawRole) role = c; }
if (role == null) **return null;**                            // ⭐⭐⭐ 角色认不出 → **丢整条**
if (rawText is! String) return null;                           // ⭐⭐ 正文必须是字符串
return ChatMessage(role: role, text: rawText,
  **epoch: rawEpoch is int ? rawEpoch : null**,                // ⭐ 三个标记**全部可缺省**
  failed: raw['failed'] == true, unfinished: raw['unfinished'] == true);
```
五个可核点：
1. ⭐⭐⭐⭐ **未知 `role` ⇒ 丢弃整条，而不是「退化成 user / assistant」**
   ⇒⇒⇒⭐ **理由在别处已可核**：
   - **B0327 核过** `message_bubble.dart:431` 的 `Semantics(label: '${ChatRole.system.label}提示：$text')`
     ⇒ **角色决定这条在界面上被怎么称呼**
   - **B0360 核过** `if (message.role == ChatRole.system) return _SystemRow(…)`
     ⇒ **角色决定这条走哪条渲染分支**
   ⇒⇒⇒⇒ **「猜一个角色」= 把一条内容送到错的渲染分支** ⇒⇒⇒ **而送错分支比丢掉更糟**
2. ⭐⭐⭐ **「不可替代的栏」从一个变成三个**，且**理由各不相同**：
   | 栏 | 为什么不可替代 | 出处 |
   |---|---|---|
   | `id` | 无法被 `activeId` 引用 / 无法被 trim 定位 | B0401 |
   | **`role`** | **无法被正确渲染**（决定哪条分支 + 读屏怎么称呼） | **本批** |
   | `text` | 那是一条消息**之所以存在的部分** | 本批 |
3. ⭐⭐ **而 `epoch` / `failed` / `unfinished` 三个全部可缺省** ⇒⇒
   **它们是「附加状态」、不是「身份」** ⇒⇒ **同 B0247 核的「失败轮标记」**（那些是后加的标记）
4. ⭐⭐ **`for (candidate in ChatRole.values)` 而不是 `ChatRole.values.byName(...)`**
   ⇒⇒ **前者不抛、后者抛** ⇒⇒ **不抛的版本与「坏数据不抛」同一条纪律**
5. ⭐ **`raw['failed'] == true` 而不是 `as bool?`** ⇒⇒ **缺字段与 `false` 同义** ⇒⇒ **「没标过失败」= 「不是失败」**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
> **B0401 那条判据（「界 = 唯一不可替代的那一栏」）在这里最锋利** ——
> **因为「猜一个角色」不会立刻错，它会让一条用户消息被画成系统提示、而系统提示被当作用户消息**，
> **两者都还「看起来正常」** ⇒⇒⇒ **这类错误不会报错、只会被忽略** ⇒⇒⇒ **所以这里只能靠「丢」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
