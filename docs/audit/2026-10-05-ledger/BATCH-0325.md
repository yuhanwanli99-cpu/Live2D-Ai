# BATCH-0325 · ⭐ 同一段文字、两种强调 —— 而**强调的切换条件就是文案的分支条件**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 桶降级说明的**呈现**

## 跑的命令（全部只读）
```
sed -n '570,586p' lib/settings/mods/memory_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **「同一句话、状态决定强调」的第四例，且最干净**
```dart
InlineNotice(
  message: memoryBucketNotice(ctx.activeSessionId),                       // :577
  severity: ctx.activeSessionId == null                                   // :578
      ? NoticeSeverity.**warning** : NoticeSeverity.**info**,
  dense: true,                                                            // :580
),
```
四个可核点：
1. ⭐⭐ **它放在面板顶部的运行态那块，且用 `InlineNotice`**
   ⇒ ⇒ **B0227 核过**：「错误横幅的外形已**收敛到 `InlineNotice` 一处**」⇒ ⇒ **又一次 P2 的汇合落点**
2. ⭐⭐ **同一段文字、两种强调**：无会话（**全局桶降级**）⇒ **warning**；有会话 ⇒ **info**
   ⇒ ⇒ ⇒ **这是 B0222 那条线的第四例**，而**这一次最干净**：
   **`severity` 的切换条件（`activeSessionId == null`）与 B0246 的文案分支条件是同一个**
   ⇒ ⇒ **判据与文案同源** ⇒ ⇒ 不存在「文字说降级、颜色说不降级」的可能
3. ⭐ **`dense: true`** ⇒ ⇒ 它是**辅助信息**、在版面上**给「注入开关」让位**
4. ⭐ 紧随其后的注释记着**一次用户裁决**：「一句话说清边界（**用户裁决：不要把契约全文塞进面板**）」
   ⇒ ⇒ **又一个「规则来自用户裁决」的记录**（与 B0185 的 `effectiveBackground`、
   B0215 的 `kChatHistoryLimit` 同族）

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~595 / `memory_panel` ~540 /
   `persona_panel` ~515 / `message_bubble` ~460 / `chat_panel` ~465）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
