# BATCH-0235 · ⭐ 气泡的塌陷规则：**三个例外逐条命名 +「别跟着一起删」** ⇒ P26 的第二种（更锋利的）形态

Phase 1 · 域覆盖 · 前端 `.dart`（44/218）—— `lib/ui/message_bubble.dart` 的失败态与塌陷判定

## 跑的命令（全部只读）
```
grep -nE "失败|failed|重试|复制|未收尾" lib/ui/message_bubble.dart
sed -n '78,100p' message_bubble.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **P26 的第二种、更锋利的形态**
```dart
// 三个**例外**都照旧显示（各有 UI 语义，**别跟着一起删**）：        // :78
// ① 流式中（要有「…」占位，**表示还在等**）；
// ② 失败轮（危险色 + 「重试」，**用户唯一的恢复入口**）；            // :80
// ③ 只有思考、没有正文（**思考就是本轮唯一内容**，见 `onlyReasoning`）。 // :81
if (message.text.trim().isEmpty && !message.streaming && !failed && !onlyReasoning) {
  return const SizedBox.shrink();                                   // :85
}
```
四个可核点：
1. **塌陷规则显式**：空 ∧ ¬流式 ∧ ¬失败 ∧ ¬只有思考 ⇒ 才 `shrink`
   ⇒ ⇒ **失败轮永远不会被塌成「什么都没有」**
2. ⭐ **三个例外各被命名，并写明各自的 UI 语义**（「表示还在等」/「**用户唯一的恢复入口**」/
   「**思考就是本轮唯一内容**」）⇒ ⇒ 它们的**差别**是可见的
3. ⭐⭐ **注释直接对下一次重构说话**：「**别跟着一起删**」
   ⇒ ⇒ 它防的不是「有人觉得这三条差不多」——**恰恰相反，它防的是「有人因为这三条看起来像而把它们合并」**
4. 失败态用**语义调色板槽位**（`dangerSurface` / `danger`），不新造颜色（与 B0229 行内码底色同纪律）

### ⭐ 由此把 P26 补全为两种形态
| 形态 | 防的错误重构 | 本例 |
|---|---|---|
| P26-a（B0229） | 「这**不是微优化**」⇒ 有人想把它「优化」掉 | 富文本分支 |
| **P26-b（本批，更锋利）** | **规则相似处最容易的错误重构是「合并它们」** ⇒ **注释的任务是让「它们哪里不同」变得可见**，而不是让「它们看起来一样」 | 三个 `!x` 例外 |
⇒ ⇒ **推论**：`if (a && !b && !c && !d)` 这类**同形多子句**，是最容易被「顺手化简」的地方；
⇒ **给每个子句写一句「它对应什么用户可见的东西」**，比写一句总说明**更抗重构**。

## 未核实项
1. `message_bubble.dart` 余约 490 行未读（角色标签 · 流式光标 · 复制按钮 · `未收尾` 行 · `onlyReasoning` 展开）
2. `memory_panel.dart` 余 ~600 · `persona_panel.dart` 余 ~560 未读
3. `chat_panel.dart` 余 540 · `error_banner.dart` 余 45 未读
4. `live2d_stage.dart` 余 ~645 · `director_observer_section.dart` 余 ~650 未读
5. 前端 `.dart` 仍 174 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
