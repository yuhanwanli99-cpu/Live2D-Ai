# BATCH-0389 · ⭐⭐⭐ **「修法不是加提示，而是把默认换过来」** —— 一句可以照搬的判据

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 壳背景的**图片来源**（rc.5 ③ 的后续演进）

## 跑的命令（全部只读）
```
grep -nE "syncShellStageBg|effectiveShellImage|stageImage|一份真相" dev_tools_section.dart   # ⇒ **零命中**
grep -rln "syncShellStageBg" lib/ ; grep -rn "syncShellStageBg" lib/ | head -6
sed -n '150,172p' lib/settings/display_prefs.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**「零命中」照例不是结论**（换文件问 ⇒ 换成 `display_prefs.dart` 命中）（0 条新发现）
> 「# **为什么换掉 `syncShellStageBg`**（**2026-09-27 修一个真缺陷**）              // :152
> 原来的形态是「壳跟随舞台」，默认 `true`。于是：
> - 用户去**背景库**加图 → 壳却去读 [stageImage]（**从没设过 → `null`**）
>   → **加了 3 张图，界面一点变化都没有**；
> - 而 UI 在那个状态下**把图库藏起来、只留「添加图片」按钮**，点下去还会提示
>   「**已加进背景库**」—— **一个必然无效的按钮**。
> 那是本项目 **P4 明令禁止的「静默失效」**。修法**不是加提示，而是把默认换过来**：
> **背景库是唯一真相，舞台那张图降级成一个可选来源**。
> 铺法 / 位置 / 透明度 / 模糊 / 遮罩 / 过渡在**两种来源下都生效**——
> **它们是渲染参数，不是图片来源**。」                                             // :153-167
四个可核点：
1. ⭐⭐⭐ **症状被分成两层写**：①「加了 3 张图，界面一点变化都没有」**②「一个必然无效的按钮」**
   ⇒⇒ **第二层比第一层严重**（前者是困惑 · 后者是骗人）⇒⇒ **而两层都被修**
2. ⭐⭐⭐⭐ **修法是「换默认」而不是「加提示」**，理由逐字写明
   ⇒ ⇒⇒⭐⭐ **这是本审计见过最清晰的一次「为什么不给它加个提示」**：
   > **加提示处理的是「用户可能困惑」；换默认处理的是「用户本来就该成功」。**
   ⇒ ⇒⇒ **「提示」是给不可避免的失败兜底；「换默认」是让失败根本不该发生**
   ⇒ ⇒⇒ **B0353「让缺陷无法发生」的又一处** —— **而且它被当成了判据写下来**
3. ⭐⭐ **而且升级到项目级禁令**：「那是本项目 **P4 明令禁止的「静默失效」**」
   ⇒⇒ **不是个人判断，是引用一条已存在的红线** ⇒⇒ **又一次「指路」**
4. ⭐⭐ **「渲染参数 ≠ 图片来源」被单独说明**（铺法/位置/透明度/模糊/遮罩/过渡**两种来源下都生效**）
   ⇒⇒ **换默认之后要防止「参数跟着来源走」的连带问题** ⇒⇒ **先说清哪些不变**
⇒ ⇒⭐ **而这也是第四个「弯路记录」**（B0310 ×2 · B0314 · 本批），
⇒ ⇒ **其失效类型是**「默认朝向错了」** ⇒ ⇒ **四类至此**：伪造 · 什么都不显示 · 兜底反被咬 · 默认朝向错

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `backgroundSource` 的**两个来源在 UI 上怎么选**（`display_prefs.dart:150` 说它在类型层，**选择控件未核**）
2. `evaluate_mode` 函数本体 · `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 ·
   `settings_models.dart` 的 `fromJson` 体 · `LiveRegionThrottle.feed` 本体 ·
   `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
