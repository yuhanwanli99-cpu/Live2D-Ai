# BATCH-0360 · ⭐⭐ **这是 B0310 那段「弯路记录」的结论侧**；而读屏身份由**同一对 API** 承担

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `message_bubble` 的**系统行**（B0313 核过它替空气泡，**外观**未核）

## 跑的命令（全部只读）
```
grep -nE "ChatRole.system" lib/ui/message_bubble.dart
sed -n '60,68p' lib/ui/message_bubble.dart ; sed -n '425,440p' lib/ui/message_bubble.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一份决策记录的两半相接**
```dart
// 「模型整轮没有返回文字」是一条**事实陈述**，不是角色说的话。把[它写成]
// assistant 气泡就是**重犯 2026-09-11 那个错（伪造台词）**；把它**删掉**就是
// 2026-09-12 那个错（**界面上什么都没有，与坏了无法区分**）。
// **第三条路**：居中、小字、弱色的**系统行**——**看得见，且一眼看出不是角色说的话**。  // :60-63
if (message.role == ChatRole.system)
  return _SystemRow(text: message.text, color: colors.contentMuted);   // :64-66
…
Semantics(container: true,
  // **读屏也要听得出这是系统提示**，而不是角色说的话（`excludeSemantics` 后由本 label 独占播报）。
  label: '${ChatRole.system.label}提示：$text', excludeSemantics: true,   // :431-433
  child: Center(child: Text(text, textAlign: TextAlign.center, style: …bodySmall…color: color)))
```
四个可核点：
1. ⭐⭐⭐ **两个被否的方案各自带日期与症状**
   （**2026-09-11 伪造台词** / **2026-09-12 什么都没有、与坏了无法区分**）
   ⇒ ⇒ **这是 B0310 那段「弯路记录」的**结论侧** —— 那里记了两个错法，**这里记了「所以选了第三条」**
   ⇒ ⇒⇒ **一份完整的决策记录被分成了两半存在**，而两半都能查到
2. ⭐⭐ **第三条路的判据是两条**：「**看得见**」+「**一眼看出不是角色说的话**」
   ⇒ ⇒ 与 B0325「**降级不能压过错误**」同族（**在场** + **可区分**）
3. ⭐⭐ **视觉三条（居中 / 小字 / 弱色）都与气泡不同** ⇒⇒ 「不是角色说的话」**在版面上就成立**
   ⇒ ⇒ 且是 **B0337「同一句话、状态决定强调」的对面**：这里**强调永远弱** ⇒⇒ 因为它**永远是系统行**
4. ⭐⭐ **`Semantics(label: '系统提示：$text', excludeSemantics: true)`**
   ⇒ ⇒ **读屏里也带「系统提示」这个身份**
   ⇒ ⇒ ⇒ **B0327「视觉上删掉角色标签」与本处「语义上补上系统身份」用的是同一个机制**
   （`Semantics` + `excludeSemantics`，与 B0348 同款用法：把内容折进一个 label）

⇒ ⇒ **0 findings**；⇒ ⭐ **本批把 B0310 与 B0327 两处接成了一条**：
**「错了两次」·「所以走第三条」·「第三条在读屏里也必须说清它是第三条」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1810 / `live2d_stage` ~580 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~435 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
