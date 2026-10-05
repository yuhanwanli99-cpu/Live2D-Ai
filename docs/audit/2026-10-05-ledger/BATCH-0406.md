# BATCH-0406 · ⭐⭐⭐ **上限被「冻结」，而「不写任何持久化」被写成红线** —— 两种「有界」可接受性不同

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 渲染面**观测缓冲**（有界增长的一类）

## 跑的命令（全部只读）
```
sed -n '186,205p' lib/live2d/render_events.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一个可迁移的判据**
> 「本段只做**纯逻辑**（**不 import Flutter**）：观测记录、**有界环形缓冲**、preset 请求记录……
> 单例 hub（`ChangeNotifier`）住在 `settings/sections/director_observer_section.dart`；
> **观测缓冲只住内存，不写任何持久化（**本栏的红线**）。」                        // :186-189
> 「**观测缓冲的默认上限（阶段 5 §2 B 栏冻结：上限 200）**」                       // :190-191
> `const int kObserverBufferCapacity = 200;`
```
四个可核点：
1. ⭐⭐⭐⭐ **上限被「冻结」在决策文档里**（「阶段 5 §2 **B 栏冻结**：上限 **200**」）
   ⇒⇒ **一个数有出处、有栏位、有它被定下来的那次** ⇒⇒ **同 B0242「阈值写在决策点」/ B0365「clamp 写在协议上」**
   ⇒⇒⇒⭐ **而「冻结」是一个状态词** ⇒⇒ **它告诉后来者「这个数可以调，但改它要动决策文档」**
2. ⭐⭐⭐ **而「不写任何持久化」被写成**红线**、不是实现细节**
   ⇒⇒ **观测数据（用户操作、会话内容）的留存边界被提升到红线级**
   ⇒⇒⇒ **同 B0307 那条同族**（「什么不进产物」是一条纪律）
3. ⭐⭐⭐⭐ **两种「有界」，可接受性完全不同**：
   | | 实现 | 丢了之后 | 可接受性 |
   |---|---|---|---|
   | **观测缓冲** | **有界环形**（覆盖最旧） | 没人察觉 | ✅ |
   | **会话列表** | `maxSessions` × `maxMessages` **夹持** | 用户会说「我的历史呢」 | ⇒ **必须**如实说明（B0395 核的「已达上限」） |
   ⇒⇒⇒⭐ **判据是「丢了之后用户会不会察觉」** ⇒⇒ **同一个「有界」目标、两种手段**
   ⇒⇒⇒ **⇒ 这解释了 B0405 为什么两层手段不同**
4. ⭐⭐ **纯逻辑（不 import Flutter）⇒ 可单测** · **单例 hub 的位置被点名**（又一次「指路」）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 3 点**：
> **「有界」不是一个决定，是两个** ——
> **「用什么手段达到有界」取决于「丢了之后用户会不会察觉」**
> ⇒⇒ **察觉不到的 ⇒ 环形覆盖（最省、零打扰）**；**察觉得到的 ⇒ 夹持 + 一句「已达上限」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `kObserverBufferCapacity = 200` 的**落点**（环形缓冲本体 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~450）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
