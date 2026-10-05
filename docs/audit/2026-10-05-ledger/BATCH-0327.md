# BATCH-0327 · ⭐⭐⭐ **删掉一个通道之前，先把还剩哪些通道列出来** —— 而且**按通道分别算**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 气泡的**角色标签**

## 跑的命令（全部只读）
```
grep -nE "roleLabel|角色标签|ChatRole" lib/ui/message_bubble.dart
sed -n '114,130p' lib/ui/message_bubble.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **本批最好的无障碍样本**
```dart
// ── 角色标签：**2026-09-11 按用户要求去掉可见文字** ──                    // :116
// 用户原话：「把助手和用户这两个不显示在对话或者删除」。                   // :119
// 去掉之后，「谁说的」由**两条本来就存在的通道**表达：
// ① 气泡面色（`bubbleUser` 是强调色淡底、`bubbleAssistant` 是中性面）；
// ② **左右对齐**。**标签只是第三条**，去掉它**不会丢信息**。                // :121-124
// 但**语义标签保留**（下面那个 `Semantics` 的 `label`）：
// **读屏用户没有「左右对齐」这个通道**，去掉会**真的**分不清谁在说话。      // :126-127
Semantics(container: true, label: message.role.label, child: const SizedBox(width: double.infinity)),
```
四个可核点：
1. ⭐ **用户裁决被逐字引用**（「把助手和用户这两个不显示在对话或者删除」）
   ⇒ ⇒ 同 B0325「用户裁决：不要把契约全文塞进面板」族
2. ⭐⭐⭐ **去掉之前，先把还剩哪些通道列出来**：① 面色（**两个具体 token 名**）② **左右对齐**
   ⇒ ⇒ **并明说「标签只是第三条」**
   ⇒ ⇒⇒ ⭐⭐ **可提炼（P30）**：**删掉一个通道之前，先把还剩哪些通道列出来** ——
   而**「它只是第 N 条」这句话本身就是论证**（⇒ 它可被反驳，因为 N 是可数的）
3. ⭐⭐⭐ **而对读屏用户，通道清单不同**：「**读屏用户没有「左右对齐」这个通道**」
   ⇒ ⇒⇒ ⭐⭐ **同一个「谁说的」问题，在视觉通道与无障碍通道上答案不同**
   ⇒ ⇒ **他们按通道分别算了一遍，而不是按「用户」算一遍**
4. ⭐ **`Semantics` 包的是 `SizedBox(width: double.infinity)`**（一个空布局盒）
   ⇒ ⇒ **语义标签挂在不可见的布局节点上** ⇒ ⇒ **「删掉可见的、留下不可见的」不矛盾，
   因为它们服务不同通道**

⇒ ⇒ ⭐ **而这是本审计最好的「无障碍靠推理而不是靠规则」的样本**：
**他们没说「按规范留一个 a11y 标签」（规则）**，
**而是说「这里有这些通道，而读屏用户没有其中一条」（推理）**。

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~595 / `memory_panel` ~540 /
   `persona_panel` ~515 / `message_bubble` ~455 / `chat_panel` ~460）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
