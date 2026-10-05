# BATCH-0332 · ⭐⭐⭐ **「我以为有的东西」连同「我怎么验证它没有」一起写下来了**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `ui_phase.dart`（140 行）· 发送按钮状态的来源

## 跑的命令（全部只读）
```
wc -l lib/state/ui_phase.dart ; sed -n '1,20p' lib/state/ui_phase.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **本批最好的一段头注**
> 「UI 状态派生：**纯函数、无 web 依赖、无 `BuildContext`**。这是状态呈现的**唯一**真源：
> `StatePill`、舞台角标、聊天面板都读同一个 [`deriveUiPhase`] 的结果，**不允许各自判断** ——
> **6 个开源项目的教训是「状态机层面本来就是同一个值，界面上却长出 6 套视觉」**（**OLV-Web**）。」   // :1-6
> 「**为什么状态要从信号派生而不是等一个 `phase` 帧**：调研曾把 core 的 `Phase`
> 列为「**已在 WS 上的状态源**」。**这不准确**：`web_api/ws/*.rs` 全文**没有 `phase` 投影**
> （**已实测 `grep -rn phase web_api/ws/` 零命中**）。」                                          // :9-14
四个可核点：
1. ⭐⭐ **一个派生、多个消费者、且「不允许各自判断」是禁令**
   ⇒ ⇒ `StatePill` / 舞台角标 / 聊天面板 **三个消费者、一个 `deriveUiPhase`**
   ⇒ ⇒ **禁令 + 外部统计**（6 个项目长出 6 套视觉）⇒ ⇒ P26 族
2. ⭐⭐ **纯函数 + 无 web 依赖 + 无 `BuildContext`**（三条同时写明）
   ⇒ ⇒ 而「无 `BuildContext`」正是**它可被单测**的原因（同 `settleTurn` / `mustReleaseTurnOnWsLoss` 族）
3. ⭐⭐⭐ **而它记录了一次「调研结论被实测推翻」，并给出方法**：
   「调研曾把 core 的 `Phase` 列为『已在 WS 上的状态源』⇒ **这不准确** ⇒
   `web_api/ws/*.rs` 全文**没有 `phase` 投影**（**已实测 `grep -rn phase web_api/ws/` 零命中**）」
   ⇒ ⇒⇒ ⭐⭐ **可提炼**：**否定结果也必须给出方法** —— 不是「没有 phase」，
   而是「**已实测、命令是什么、结果是什么**」
   ⇒ ⇒ 与我 **B0274** 那个「只搜了一次就下结论」**是同一件事的两种做法**
   ⇒ ⇒ **而他们的版本可复跑，我的版本当时不可复跑**
4. ⭐ `:19-20`「界面相位（**枚举里刻意没有 `listening`**）」+「本项目**当前没有输入侧**」
   ⇒ ⇒ **又一个「枚举里刻意没有什么」**（同 B0242 的 `actions` 0–2）

## 未核实项（本批后仍开着 4 条 + 4 条永久）
1. `deriveUiPhase` 的**本体未读**（四个信号如何合成一个相位）· `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
4. `main.rs` 余约 930 行 · `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~520 /
   `persona_panel` ~510 / `message_bubble` ~455 / `chat_panel` ~460）
5. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
