# BATCH-0395 · ⭐⭐ **四个常量，四个被解释过的数** —— 而**「别名不是第二套配额」被写明了**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 背景**预算**（B0394 留的「两套预算」）

## 跑的命令（全部只读）
```
grep -nE "stagePlaylist|预算|kStage" lib/settings/display_prefs.dart | head -8
sed -n '62,96p' display_prefs.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**「两套预算」实际上是「两套通路、一个上限 + 两个局部上限」**（0 条新发现）
```dart
const int **kStageImageMaxChars = 1500000**;                                     // :68  单张上限
const int **kShellImageMaxChars = kStageImageMaxChars;**                         // :75  ⭐ 别名
const int **kStagePlaylistMaxItems = 16**;                                       // :81  项数上限
const int **kStagePlaylistMaxChars = kStageImageMaxChars;**                      // :92  总长上限
```
四个可核点：
1. ⭐⭐⭐ **`kShellImageMaxChars = kStageImageMaxChars` 是**别名**，且理由逐字写明**：
   「壳背景与舞台背景写在**同一条 localStorage 记录**里，给两份独立上限只会让
   「**整份偏好都写不进去**」更容易触发 ⇒ 所以**只是给壳背景一个可读的别名，不是第二套配额**」
   ⇒⇒⇒ ⭐⭐⭐ **这是「别名」这个概念被解释得最清楚的一处** ——
   **一个常量存在两次的理由不是「方便」，是「让读代码的人以为有两套配额」**
2. ⭐⭐⭐ **而 1500000 这个数**有自己的理由链（`:62-67`）：
   「一旦超配额，**整份偏好都写不进去**（不只是背景图），那会连带丢掉主音量、口型设置，
   **代价远大于「一张图没记住」**」+「**超限不是错误**：本次会话照常生效，只是**不写盘**
   （**UI 会如实说明**）」+「**下游裁剪没有做**——它需要浏览器 canvas，属于**不可测代码**，
   这一轮**刻意不引入**」
   ⇒⇒⇒ **三件事都在**：**为什么是这个上限** · **超限怎么表现** · **以及没做什么、为什么不做**
   ⇒⇒⇒ **最后一条尤其**：**「没做」被写下来并给了理由**（同 B0348「已知缺口」的家族）
3. ⭐⭐ **「16 张」被指向了真正的上限**：「16 张的理由见 [`kStagePlaylistMaxChars`] ——
   **真正的上限是总字符预算**，这一条只挡住『**项数无界**』」
   ⇒⇒ **一个次级上限被指向主上限、并说清自己是次级的** ⇒⇒ **同 B0349「可空 = 有可推导的默认」的对称做法**
4. ⭐⭐ **总长上限的理由独立成节**（`:84-91`「# **为什么必须单独有一条**（2026-09-14，Wave 2 硬约束）」）
   ⇒⇒ **并写了超限时的具体行为**：「**超出时保留前面的、丢掉放不下的**，
   并在界面上**如实说明「已达上限」**，而不是**悄悄把整份设[置丢掉]**」
   ⇒⇒⇒ ⭐ **又是 P31**：**「已达上限」是一句可见的话**，**而代价（丢整份）被提前挡住了**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 2 点的最后一句**：
> **「没做」被写下来并给了理由**（「下游裁剪没有做——需要浏览器 canvas · 属于**不可测代码** · 刻意不引入」）
> ⇒⇒ **而给出的理由是「不可测」** ⇒⇒⇒ ⭐ **这是本仓的一贯判据**：
> **「这一步会引入不可测的代码」** 本身被当成一个**充分的拒绝理由**
> ⇒⇒⇒ **同 B0321「服务端不支持的客户端参数要删掉」、B0361「粘贴是主路径、文件选择要宿主接线」** 的同一族

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. 「**已达上限**」那句话**在哪里说**（`appearance_section.dart` 的播放列表卡？**未核**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
