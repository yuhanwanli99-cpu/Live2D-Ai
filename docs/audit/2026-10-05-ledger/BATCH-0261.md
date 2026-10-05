# BATCH-0261 · ⭐ 复制按钮复制**原文**，而它隐藏的理由**引用了「一句一单元」**（0 条新发现）

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `message_bubble.dart` 的复制动作

## 跑的命令（全部只读）
```
grep -nE "Clipboard|复制" message_bubble.dart
sed -n '264,292p' message_bubble.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 「一句一单元」在**第四个层次**上落地
```dart
// ── 底部动作条：复制（+ 失败时的重试） ──                    // :264
// 只在**非流式**时出现：流式期间文本还在变，复制到的会是**半句话**
// （按「**一句一单元**」的口径，**半句话本身也不该被当成一轮的产出**）。   // :266-268
if (!message.streaming && (message.text.isNotEmpty || failed))
  _BubbleActions(text: **message.text**, onRetry: failed ? onRetry : null, …)
```
三个可核点：
1. ⭐ **复制的是 `message.text`（原文），不是渲染后的 span**
   ⇒ ⇒ 与 B0229 核的「纯文本**不进** `Text.rich`」**同源** ⇒ **显示与复制两条路都取原文**
   ⇒ ⇒ 复制到的**正是模型说的话** —— 用户要的就是这个
2. ⭐⭐ **隐藏一个 UI 能力的理由，引用的是本仓自己的语音规则**（「一句一单元」）
   ⇒ ⇒ 这是该红线**第四个**落地层次：
   **提示侧**（B0116 `DISCIPLINE_TEMPLATE`）· **装配侧**（B0132 `sentence.rs` 不许硬切）·
   **上界侧**（B0243 `kChatHistoryLimit`「丢最旧的」）· **界面侧**（本批：流式中不给复制）
   ⇒ ⇒ **同一条约定在四个层次上各自落地、且各自引用它**
3. ⭐ 另两处正面：`kMessageBubbleSurfaceKey` 带注释「**测试按它读约束，不靠 widget 类型猜**」
   ⇒ ⇒ **稳定的测试钩子 + 说明它为何存在**（P1/P3）；
   阅读宽度 `480` 的理由是**人因**：「中文一行超过 ~45 字之后，眼睛回找行首就开始费劲」⇒ ⇒ **数字带人因理由**

## 未核实项
1. 面板余面：`dev_tools_section` ~1845 · `live2d_stage` ~615 · `memory_panel` ~560 ·
   `persona_panel` ~520 · `message_bubble` ~470 · `chat_panel` ~480 · `error_banner` ~15
2. `ErrorResponse` 定义未读；`MutatingCheckError::as_message()` 未读
3. **两侧措辞同步无机制**（B0260 的敞口）· `chat_session.dart` 的 `maxSessions=50` 施加点未读
4. 前端 `.dart` 仍 181 未读（**机械值**）；Mod crates 42 未读；`mod-system` 余 8 文件未读
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **iframe 重建后宿主是否主动发 `clear`**（B0251 留）—— 未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
