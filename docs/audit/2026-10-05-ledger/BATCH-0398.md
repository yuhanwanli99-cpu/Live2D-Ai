# BATCH-0398 · ⭐⭐⭐ **「不引入无法回归的代码」是第二次出现** —— 它是本仓的**判据**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `browser_io.dart`（B0395 的搜索把它带出来的）

## 跑的命令（全部只读）
```
sed -n '75,112p' lib/app/browser_io.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**两个 claims 都成立**（0 条新发现）
> `saveChatSessions`：「**失败静默：无痕模式 / 配额满都会抛**」                            // :78
> 「**配额满不是假设**：localStorage 常见上限 **5 MB**，而 `ChatSessionStore` 用
> **`maxSessions` × `kMaxMessagesPerSession` 双重夹持**把体积压在配额内。即便如此，
> 写失败也只丢「**这次之后的新记录**」，不该影响正在进行的对话。」 `catch (_) {}`      // :80-92
> `pickImageDataUrl`：「**不做缩放（刻意的）：裁剪要走 canvas，而 canvas 只能在浏览器里跑、
> 本仓库的 `flutter test` 覆盖不到 —— 本轮不引入无法回归的代码**。代价是「**大图不写盘**」，
> 由调用方按 `kStageImageMaxChars` **如实告知用户**。」                                  // :98-101
> 「返回 **取消 / 成功 / 读失败** 三态：取消（`dataUrl==null && error==null`）不该弹东西；
> 读失败（`error!=null`）必须说实话 —— 以前读失败与取消**不可分**，用户看到的就是
> 『**点了选图没反应**』。」                                                             // :102-107
五个可核点：
1. ⭐⭐ **「静默」的两条理由被分级**：**无痕模式（预期的）** / **配额满（兜底的）**
   ⇒⇒ **两者都静默、但理由不同** ⇒⇒ **同 B0287「有理由才清」的思路（写入侧）**
2. ⭐⭐ **B0262 核过的「双重夹持」在这里被点名**（`maxSessions` × `kMaxMessagesPerSession`）
   ⇒⇒ **B0143 的裁剪 + B0215 的 `kChatHistoryLimit` + 本条 = 三层**
3. ⭐⭐⭐⭐ **「本轮不引入无法回归的代码」** ⇒⇒⇒ **与 B0395 的「下游裁剪……属于**不可测代码**、刻意不引入」**
   是**同一个判据的第二次出现** ⇒⇒⇒⭐ **它是本仓的判据，不是这一次的选择**
   ⇒⇒⇒ **而这次的代价被逐字写下、且**指给了调用方**（「由调用方按 `kStageImageMaxChars` 如实告知用户」）
4. ⭐⭐⭐ **三态由两个可空字段编码**（`({String? dataUrl, String? error})`）⇒⇒ **取消 = 两者皆 null**
   ⇒⇒⇒ **这正是 B0340 那个坑的形状** ⇒⇒⇒ **而注释明说「以前读失败与取消**不可分**、
   用户看到的就是『**点了选图没反应**』」** ⇒⇒⇒ **一个已修的坑的**症状**被写下来了**
5. ⭐ **`ColorScheme`-free**：本文件是「组合根专有：需要 `package:web`」⇒⇒ **B0361 那个注入边界的对侧**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 3 点**：
> **「这一步会引入无法回归的代码」在这个仓库里被当成一个**充分的拒绝理由**，而且**用了两次**。
> ⇒⇒ **B0395（背景裁剪）· B0398（选图缩放）** ⇒⇒ **两次的代价都被逐字写下、并且都指给了下游**
> ⇒⇒⇒ **「拒绝一个改进」与「记下它的代价」成对出现** —— 而**只有后者被反复做时，前者才站得住**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
