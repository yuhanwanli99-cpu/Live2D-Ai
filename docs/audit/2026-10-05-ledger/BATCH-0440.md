# BATCH-0440 · ⭐⭐⭐ **自我证明的第三种形态最强** —— 而本批**挑错了一次文件**（如实记）

Phase 4 · **证伪**（第三个实例：B0243 核过的那个历史裁剪有没有自我证明）

## 跑的命令（全部只读）
```
grep -nE "^\s*test\(|group\(|expect\(|reason:" test/session_baseline_test.dart | head -14
   # ⇒ **挑错了**：`session_baseline` 是关于 **ActionCue / baseline cue** 的，不是历史裁剪
grep -rln "kChatHistoryLimit|maxSessions|maxMessagesPerSession" test/ | head -3
grep -rn "kChatHistoryLimit|maxMessagesPerSession" test/ | head -4
sed -n '205,226p' test/chat_session_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ⚠ 本批的过程注记（**挑错文件**）
> 我按文件名挑了 `session_baseline_test.dart`，**得到的是 `ActionCue` 相关断言** ⇒⇒ **它与 B0243 核的
> 「历史裁剪」无关** ⇒⇒ ⇒ **⇒⇒ 而这次不是「零命中」、是「有命中但主题不同」**
> ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ 而这比零命中更危险**：零命中会让我去换文件，**「有命中但主题不同」会让我以为核到了**
> ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ 「文件名可以指向另一个主题」** ⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ 而 B0316 那条规矩（「没找到 ≠ 不存在」）**在这里要扩写成「找到了 ≠ 是它」**

## ★ 本批产出：**自我证明的第三种形态**
```dart
// test/chat_session_test.dart:207-211
test('**消息上限与视图层一致（两处不一致会出现「看得见但重开就没了」）**',
  // `kChatHistoryLimit` 在 `ui/chat_panel.dart`（视图层裁剪）。
  // **这里不 import 它**（那会把 Flutter 拖进纯逻辑测试），改为**钉住数值**：
  // **改动任何一边都会让这条红，逼着人同时看两处。**
  expect(ChatSessionStore.kMaxMessagesPerSession, 500);
// test/keyboard_test.dart:158
expect(kChatHistoryLimit, 500);
```
四个可核点：
1. ⭐⭐⭐⭐⭐ **⇒⇒⇒⇒ 第三种形态、而它最强**：
   | | 形态 | 自证方式 | 防的是什么 |
   |---|---|---|---|
   | B0381 | 扫源码 | **合成坏样例** | **扫描器不工作** |
   | B0439 | 真 widget | **`findsNothing`** | **多画了一层** |
   | **本批** | ⭐ **跨文件数值 pin** | ⭐⭐⭐ **「改任何一边都会让这条红」** | ⭐⭐⭐⭐ **两个真相悄悄分叉** |
   ⇒⇒⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 而这一种最强** ——
   **⇒⇒⇒⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 因为它防的不是「测试本身失效」，是「**两处各自都对、但对不上**」**
2. ⭐⭐⭐⭐ **而测试名把「不一致的后果」写出来了**：「**两处不一致会出现「看得见但重开就没了」**」
   ⇒⇒ **⇒ 后果是**用户可感知的**（看得见、但重开就没）⇒⇒⇒⭐⭐
   **⇒⇒⇒ ⇒⇒ ⇒ ⇒ ⇒ 而这不是「参数不一致」这样的抽象说法** ⇒⇒⇒ **⇒⇒⇒ ⇒⇒ ⇒ ⇒ ⇒ 它给了一个具体的症状**
3. ⭐⭐⭐⭐⭐ **而「不 import、改为钉住数值」是一个**刻意的方法选择 + 理由 + 代价**都被写明**：
   - 理由：「**那会把 Flutter 拖进纯逻辑测试**」（同 B0409「依赖方向即测试性」）
   - 代价：「**逼着人同时看两处**」⇒⇒⇒⭐⭐ **⇒⇒⇒ ⇒ 代价是「麻烦」、收益是「两处一定一起改」**
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ 「知道代价是什么、还是选它」** —— **这与 B0307 核的 `web/surface/input.rs` 那条是同一个自觉**
4. ⇒ ⇒⭐⭐⭐⭐⭐ **⇒⇒⇒ 而第三种是 B0411「跨语言对齐必须被断言守住」的**仓内版****
   ⇒⇒⇒⭐⭐⭐ **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ **而这两条在同一个仓里都存在**（跨语言：两边各钉一个数；
   仓内：两个文件各钉一个数）⇒⇒⇒⭐⭐⭐⭐⭐
   **⇒⇒⇒ ⇒⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒⇒ ⇒ **⇒⇒⇒⇒⇒ ⇒ ⇒⇒⇒⇒⇒ ⇒ 「同一个原则，跨语言与跨文件各有一个实例」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. **其余结构测试的自我证明强度未核**（B0438 的分母仍在）·
   `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `ModConfigResult` 定义
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~455）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token · `_finishTurn` 幂等 · GitHub secret scanning）
