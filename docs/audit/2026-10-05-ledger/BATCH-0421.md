# BATCH-0421 · ⭐⭐⭐⭐⭐ **那条禁令自带它的成立条件** —— 形状是「判据」，不是「规矩」

Phase 4 · **证伪**（`_ttsTesting` 是否也逐个复位 · B0420 留）

## 跑的命令（全部只读）
```
sed -n '1145,1160p' lib/main.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**答案 + 一条我此前没见过的禁令形状**（0 条新发现）
> 「**调用方：[_gotoSection]。不要在别处随手调** —— 它会让**进行中的**连通性自检结果
> 被**静默丢弃**（那正是 [_resultEpoch] 想要的效果，
> **但只有在「用户已经离开那个分区」时才成立**）。」                                  // :1145-1148
```dart
void _clearTransientResults() {
  _resultEpoch++;                  // ① 世代
  _llmTest = null;      _llmTesting = false;     // ② 结果 + 标志
  _ttsTest = null;      _ttsTesting = false;     // ③ **同上（B0420 未核的一处：确实也清）**
  _adminMessage = null;                           // ④
  _stageImageMessage = null;  _stageImageFailed = false;    // ⑤ 结果 + **失败态**
  _shellImageMessage = null;  _shellImageFailed = false;    // ⑥ 同上
  _modelOverrideMessage = null; _modelOverrideFailed = false;  // ⑦ 同上
}
```
四个可核点：
1. ⭐⭐⭐⭐⭐ **而那句「不要在别处随手调」写清了调用的**前提条件**：
   「那正是 `_resultEpoch` 想要的效果，**但只有在「用户已经离开那个分区」时才成立**」
   ⇒⇒⇒⭐⭐⭐ **⇒ 这是一条「同一动作在别的场景下就是 bug」的判据**
   ⇒⇒⇒ **⇒ 而它不是写「别乱调」，是写「**在什么条件下它才对**」**
2. ⭐⭐⭐⭐ **而「调用方：`[_gotoSection]`」被逐字写出** ⇒⇒⇒⭐⭐
   **⇒ 同 B0379 核的「唯一调用点」是同一种写法，而这一处更强**：
   **那处写的是「这里被调用」· 这处写的是「不要在别处调」**
   ⇒⇒⇒ **⇒ ⇒ 「不要在别处调」是**双向**的约束（正面写正面 + 背面写背面）**
3. ⭐⭐⭐ **九个字段一次清完**（含 `epoch`）⇒⇒ **⇒ B0419 核的那个逐字段断言
   （`'$flag 没有复位'`）现在有了对象清单**
4. ⭐⭐ **四个 `…Failed` 标志也被清** ⇒⇒⇒⭐ **⇒ 它们是 B0340 核的「三态」里的 `error` 侧**
   ⇒⇒ **⇒ 一次清完「结果 + 标志 + 失败态」**

⇒ ⇒⭐⭐⭐⭐⭐ **而本批最值钱的是第 1 点**
> **「不要在别处调」这条禁令**自带它的成立条件**：
> **同一动作在「用户已经离开」时是对的，在别的场景下就是错的** ⇒⇒⇒⭐⭐
> **⇒ 而这条禁令的形状是「**判据**」而不是「规矩」** ⇒⇒⇒⭐⭐⭐
> **⇒⇒ ⇒ 与 B0305 核的「谁造成了这一轮」是同一个家族**：
> **⇒⇒⇒ **一个动作的对错取决于它发生在谁造成的场合** ⇒⇒⇒
> **⇒⇒⇒ ⇒ 而把这句话写在函数头上，下一个想复用它的人就会先问「我这里也是那个场合吗」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `_clearTransientResults` 的**后半段**（`:1160` 之后是否还有字段 · **未读**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
