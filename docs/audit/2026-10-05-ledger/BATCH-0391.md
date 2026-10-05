# BATCH-0391 · ⭐⭐ **三个 API 的 `null` 有三种含义，且被逐个点名**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 背景库的**字节水合**与逐图编辑器（B0390 留）

## 跑的命令（全部只读）
```
grep -nE "字节|读回|_reading|pending|decodeDataUrlBytes" lib/settings/sections/appearance_background.dart | head -8
sed -n '108,120p' appearance_background.dart ; sed -n '72,80p' appearance_background.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **「可空」不是一种设计，是每次都要说清的那种**
> 「为什么不直接用 `BackgroundImage.copyWith`：它的 `clearStyle` 是**三项一起清**的开关，
> 而逐图编辑器要能**只清一项**；**`copyWith(opacity: null)` 又表达不了「清掉」**
> （**那里的 `null` 是「不改」**）。                                                     // :109-112
> 「**dataUrl 原样带走**：**丢了它这张图会退回「字节还没读回来」，缩略图与渲染一起消失，
> 直到下一次水合。**」                                                               // :113-114
> `final bool **hydrating**;`  // 「**背景库字节是否还在水合（IndexedDB 读回中）**」  // :75-76
> `static … maybeOf(…)  // 「**没有外壳注入时返回 null**」                        // :77-78
```
三个可核点：
1. ⭐⭐⭐ **三个 `null` 语义被分开点名**：
   | API | `null` 的含义 |
   |---|---|
   | `clearStyle` | **三项一起清的开关**（不是逐项） |
   | `copyWith(opacity: null)` | **「不改」** |
   | 逐图编辑器要表达的 | **「清掉」** |
   ⇒⇒ **同 B0349 的可空性家族，但这里更进一步：不同 API 的可空含义不同** ⇒⇒ **三个都得说**
2. ⭐⭐ **而 `dataUrl` 被单独点名**，因为**它不是样式参数、是字节** ⇒⇒
   **丢了它的后果被写出来**（「缩略图与渲染一起消失，直到下一次水合」）⇒⇒ **后果而非只是「要保留」**
3. ⭐⭐ **`hydrating` 是显式的 bool** ⇒⇒ **「还没读回来」与「真的空」被区分**
   ⇒⇒ **B0325「降级不能压过错误」/ B0362「空 vs 错误两个字段」的同族** ⇒⇒ 而 `maybeOf` 的
   「**没有外壳注入时返回 null**」写在文档上 ⇒ 「空态被定义」族

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
> **「可空」不是一种设计，是每次都要说清的那种。**
> ⇒⇒ **B0349 我记的是「可空 = 有一个可推导的默认」；本批补的是它的另一半 ——
> 同一个文件里三个可空参数，含义各不相同，而**每一个都被点名了**。**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `AppShell._currentBackground` 本体 · IndexedDB 读回的**失败分支**（`hydrating` 卡在 true 时会怎样 · **未核**）
2. `evaluate_mode` 函数本体 · `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `LiveRegionThrottle.feed` 本体 ·
   `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
