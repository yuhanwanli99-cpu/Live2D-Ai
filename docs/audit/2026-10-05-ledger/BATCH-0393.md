# BATCH-0393 · ⭐⭐⭐ **一段注释的唯一目的：防止「下一个人按文件名做出一个不可逆的错误判断」**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `appearance_section.dart` 的**两条记录**

## 跑的命令（全部只读）
```
sed -n '1,22p' shell/flutter/lib/settings/sections/appearance_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**两段记录，第二条是「进行中的豁免」**（0 条新发现）
> 「# **文件名的来历**（原名 `actions_section.dart`，2026-09-11 改名）」                  // :6
> 「**这个文件从来就不是「动作」分区**——它的枚举项一直叫「外观与互动」……只因为当年它多带了
> 一组**只读的「动作记录」**，文件名被叫成了 actions……」                               // :8-10
> 「用户裁定 LLM **无工具、只做对话**后，动作子系统整个移出成品……文件名改为
> `appearance_section.dart` **以名实相符**。
> **留这段是因为「下一个人很可能再按旧名误判一次」**：`actions_section.dart` 一出现就让人以为
> **删掉它不影响外观设置，实际会整套删掉主题与口型**。」                                // :12-16
> 「# **行数（≤1000 豁免，理由写在这里）**：2026-09-28（Stage B · R6-b 第 0 步）：背景域那
> **1011 行**抽到了 `appearance_background.dart`，本文件 **1489 → 约 700 行**。
> **仍超 ≤500**……」                                                                    // :17-22
三个可核点：
1. ⭐⭐⭐ **那段注释的目的被逐字写出**：「**下一个人很可能再按旧名误判一次**」
   ⇒⇒ ⇒⭐⭐ **它防的不是「改错」，是「一个看似合理的判断」**
   ⇒⇒⇒ **而最坏后果被写到最坏**：「**实际会整套删掉主题与口型**」⇒⇒ **不是「有点影响」，是「整套没」**
   ⇒⇒⇒ **同 B0380「重放最易覆盖掉」的思路** ⇒⇒ **两者都不是防「改错」，是防「一个看似合理的判断」**
   ⇒⇒⇒ **与 B0370「删掉一个通道之前先把还剩哪些通道列出来」同族** ——
   **而那一条的对象是「一个通道」，这一条的对象是「一次删除决定」**
2. ⭐⭐ **「以名实相符」被写成改名的理由** ⇒⇒ **而理由不是「更好听」，是「名与实一致」**
3. ⭐⭐ **豁免的**进展**被记着**：`1489 → 约 700` ⇒「**仍超 ≤500**」⇒ **豁免至 1000**
   ⇒⇒⇒ ⭐ **「豁免」是一个进行中的状态、不是一个永久标记** ⇒⇒ **而每次豁免都记着「减了多少、还差多少」**
   ⇒⇒ **同 B0329「代价如实记录」**（那里记代价、这里记进度）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是第 1 点**：
> **「为未来的误判预先写下后果」** —— 与「为未来的修改写下理由」是两种注释，
> **而后者更常见** ⇒⇒ **前者只在「误判会不可逆」时才被写下来**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `appearance_section` 剩下约 700 行里**那四块字段编排**（是否还有第二次可抽 · **未核**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
