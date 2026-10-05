# BATCH-0229 · ⭐ 富文本分支：**纯文本确实不进 `.rich`**，且理由写的是**行为差异**不是性能

Phase 1 · 域覆盖 · 前端 `.dart`（39/218）—— `lib/ui/message_bubble.dart` 的正文渲染（其余面板本体顺延）

## 跑的命令（全部只读）
```
grep -n "Text.rich|Text(|hasChatMarkdown|parseChatMarkdown|SelectableText" lib/ui/message_bubble.dart
sed -n '445,470p' lib/ui/message_bubble.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**测试名 ↔ 实现理由是同一件事的两面**
```dart
/// 正文：**有记号就走 Markdown，没有就当纯文本**。
///
/// 走捷径那条**不是微优化**：`SelectableText` 与 `SelectableText.rich` 在选择
/// 与复制行为上**并不完全一致**（后者会把 **span 边界带进选区**），
/// 纯文本消息**没有任何理由付这个代价**。                        // :445-449
…
if (!hasChatMarkdown(text)) { return SelectableText(text, style: base); }   // :461-462
```
四个可核点：
1. ⭐ **门是 `hasChatMarkdown(text)`** ⇒ 纯文本走 `SelectableText`、有记号才走 `.rich`
   ⇒ ⇒ B0187 那条测试「**纯文本消息不进 `Text.rich`**（选择/复制行为保持原样）」**在实现里成立**
2. ⭐ **理由是「行为差异」而非性能**：`.rich` 会把 **span 边界带进选区** ⇒ ⇒ **他们知道确切机制**
3. ⭐ 且**预先挡掉了最可能的错误重构**：「**走捷径那条不是微优化**」
4. 另：行内码底色**复用语义槽位** `surfaceContainerHighest`（「『比面板稍微突出一点』的**语义槽位**），
   **不新造颜色**；**浅色主题下它也自动是深一档的灰**）⇒ **tokens P2 纪律的又一处内联应用 + 带理由**

### ⭐ 由此提炼一条（正面模式 **P26**）
> **在决策点写「这不是微优化」+ 写出「若不这么做」的具体代价。**
> 后来者要改，就得**先推翻那条具体的代价**，而不是推翻一个「风格偏好」。
⇒ 本例的具体代价是「**span 边界进选区**」—— 一个**可核对的行为事实**，
而不是「性能更好」这种无法反驳的说法。

## 未核实项
1. `message_bubble.dart` 余约 500 行未读（角色标签 · 流式光标 · 失败态 · 复制按钮 · 各 role 的外形）
2. `memory_panel.dart`(694) · `persona_panel.dart`(608) **本体未读**（列表与编辑删除 · 导入链 UI）
3. `chat_panel.dart` 余 540 行 · `error_banner.dart` 余 45 行未读
4. `director_observer_section.dart`(668) · `live2d_stage.dart`(666) 未读
5. 前端 `.dart` 仍 179 未读；Mod crates 19/61；`mod-system` 余 8 文件未读
6. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
