# BATCH-0403 · ⭐⭐⭐ **「不可替代 ⇔ 必写；可替代 ⇔ 缺省不写」** —— 而这**为 5 MB 配额服务**

Phase 4 · **证伪**（把 B0401 那条判据**反方向**用一次：写出去的字段与读进来的必需字段对不对得上）

## 跑的命令（全部只读）
```
sed -n '/Map<String, Object?> toJson/,/^  }/p' lib/chat/chat_message.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**往返闭合，且「少存」是有理由的**（0 条新发现）
```dart
Map<String, Object?> toJson() => <String, Object?>{
  '**role**': role.name,                          // 读侧**必需** ⇒ 必写
  '**text**': text,                               // 读侧**必需** ⇒ 必写
  if (epoch != null) 'epoch': epoch,              // 读侧可缺省 ⇒ 缺省不写
  if (failed) '**failed': true,                   // 只写 true，不写 false
  if (unfinished) 'unfinished': true,             // 同上
};
```
五个可核点：
1. ⭐⭐⭐ **读侧必需的两个字段、写侧必写**（`role` / `text`）⇒⇒ **往返闭合** ⇒⇒
   **这不是偶然，它是「**必需 ⇔ 必写**」这条规则的两半**
2. ⭐⭐⭐⭐ **而「只存非默认」是为 5 MB 配额服务的**：
   ⇒⇒ **localStorage 里每条消息只存「非默认」的部分**
   ⇒⇒⇒ 而 **B0398 核过**「**配额满不是假设**……用 `maxSessions` × `kMaxMessagesPerSession` **双重夹持**」
   ⇒⇒⇒⭐⭐ **⇒ 这个序列化设计直接服务于那个硬约束** ⇒⇒
   ⇒⇒⇒⭐ **「少存」不是为了省事，是为了让「5 MB 配额」这条硬约束**可被夹持住**
   ⇒⇒⇒ **而我此前只从「夹持」那一侧看过**（B0398）⇒⇒ **现在知道「存」这一侧也是按同一个预算设计的**
3. ⭐⭐ **`if (failed) 'failed': true`** ⇒⇒ **只写 `true`、不写 `false`**
   ⇒⇒ 而读侧 `raw['failed'] == true` **把缺字段解释为 `false`** ⇒⇒⭐ **两侧是同一个约定**
4. ⭐⭐⭐ **`epoch` 用 `if (epoch != null)` 而不是 `if (epoch != 0)`**
   ⇒⇒ **「没有 epoch」与「epoch 是 0」是两种状态** ⇒⇒ **同 B0349「可空 = 有可推导的默认」**
   ⇒⇒ **与 B0402 核的读侧 `rawEpoch is int ? rawEpoch : null` 严格对称**
5. ⇒⇒⭐ **整条纪律（B0401）**在读写两侧都成立**：
   > **不可替代的 ⇔ 必写；可替代的 ⇔ 缺省不写。**
   ⇒⇒⭐ **这比单看一侧更强** ——
   **单看读侧只知道「缺了会怎样」；两侧都看才知道「存的时候就已经省了」**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的价值是「反方向用同一条判据」**：
> **B0401 我在读侧提炼了一条判据；B0403 在写侧验它成立。**
> ⇒⇒ **一条只在单侧成立的判据是「读得通的借口」，两侧都成立的判据才是纪律**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `ChatSession.toJson` / `ChatSessionStore.toJson` 的**同样规则**（**未核** ——
   ⚠ **而 B0398 核的「双重夹持」是靠 `maxSessions` × `kMaxMessagesPerSession`，不是靠字段省略** ⇒ **两处机制不同**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
