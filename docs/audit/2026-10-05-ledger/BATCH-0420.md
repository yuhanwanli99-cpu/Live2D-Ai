# BATCH-0420 · ⭐⭐⭐⭐ **一个动作里做了三件事** —— 而我差点只看见其中一件

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `_resultEpoch` 的**自增点与对账点**（B0419 留）

## 跑的命令（全部只读）
```
grep -nE "_resultEpoch" lib/ -r --include=.dart | head -8
grep -nE "_resultEpoch\+\+|_resultEpoch =" lib/main.dart
sed -n '1035,1046p' lib/main.dart
sed -n '1096,1152p' lib/main.dart | grep -nE "_llmTesting|_ttsTesting|_section =|_resultEpoch|_adminMessage"
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**一个真问题被提出、被就地关掉**（0 条新发现）
```dart
/// 结果落地前先对一次 [_resultEpoch]：用户在这几秒里切走了分区的话，
/// 这条结果**落在一个他已经离开的分区上**，**看起来会像是「刚测的」**。      // :1036-1038
final int epoch = _resultEpoch;   // ← await **之前** capture
…
if (!mounted || **epoch != _resultEpoch**) return;      // ← 成功支 :1044
…                                                       // ← 失败支 :1053（同一形状，另一方法 :1066/:1077）
…
_section = next;                                        // :1108   唯一写者（B0419 已核）
/// 连通性自检结果被**静默丢弃**（那正是 [_resultEpoch] 想要的效果，…）  // :1146-1147
_resultEpoch++;                                         // :1149   **唯一自增点**
_llmTesting = false;                                    // :1150   ⭐ 逐个复位
```
四个可核点：
1. ⭐⭐⭐⭐⭐ **而我提出的那个问题被就地关掉了**：`if (epoch != _resultEpoch) return;` **发生在
   `setState(_llmTesting = false)` 之前** ⇒⇒ 我担心「标志卡在 `true`」
   ⇒⇒⇒ **但 `_gotoSection` 自己复位了它**（`:1150`）⇒⇒⇒⭐
   **⇒ 而不是「侥幸无害」—— 是同一个动作里做了两件事：作废在途 + 复位标志**
2. ⭐⭐⭐⭐ **而「静默丢弃」被写成目的**：「连通性自检结果被**静默丢弃**（**那正是 `_resultEpoch` 想要的效果**）」
   ⇒⇒⇒⭐⭐ **⇒ 「静默」在这里是**判据**、不是副作用** ⇒⇒ **同 B0407「0 行入环是有意的」那一族**
3. ⭐⭐⭐ **`_resultEpoch++` 与 `_llmTesting = false` 相邻** ⇒⇒⇒⭐ **⇒ 世代推进与标志复位在同一个动作里**
   ⇒⇒⇒ **⇒ 不会出现「世代变了但标志没复位」** ⇒⇒⇒
   **⇒ ⇒ 这是 B0419 那条「清空不足以解决竞态」的另一半**：
   **竞态靠世代号 · 观感靠复位**
4. ⭐⭐⭐ **自增点唯一**（`:1149`）⇒⇒ **⇒ 而它就在「切分区只有一条路」那个函数里**（B0419 已核）
   ⇒⇒⇒⭐ **⇒ 世代的推进与清空不会脱节**

⇒ ⇒⭐⭐⭐⭐ **可提炼：一个动作里做了三件事**
| # | 做什么 | 对付哪一种失效 |
|---|---|---|
| ① | 换分区（`_section = next`） | **显示错位** |
| ② | 推进世代（`_resultEpoch++`） | **在途写入** |
| ③ | 复位标志（`_llmTesting = false`） | **进度条卡住** |
⇒⇒⇒⭐ **三者相邻、同一个 `setState`**
⇒ ⇒⇒⭐⭐⭐ **⇒ 而我今天差一点只看见 ①**（`return` 跳过复位 ⇒ 标志卡住）
⇒ ⇒⇒ **⇒ 若不是追问「它会不会卡在 true」，我会记一条假缺陷** ⇒⇒ **⇒ 「会不会卡住」是廉价的、必须问**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `_ttsTesting` 是否也逐个复位（`:1150` 只见到 `_llmTesting` 一处 · **未核**）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~430）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
