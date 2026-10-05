# BATCH-0380 · ✅ **B0000 ⑦ 的修法有回归** —— 而第二个测试文件的头注写着「**被守的缺陷**+ 日期」

Phase 4 · **证伪** —— 把 B0379 的问法用在 B0000 ⑦（切主题后舞台不变色）上

## 跑的命令（全部只读）
```
grep -rln "applyPrefs|stageColor|syncShellStageBg" shell/flutter/test/ | head -4
grep -rn "stageColor|applyPrefs" shell/flutter/test/ | head -6
sed -n '1,14p' test/asset_guard_stage_attach_resend_test.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**B0000 ⑦ 的修法有回归**（0 条新发现）
四个相关测试文件：`asset_guard_stage_attach_resend_test.dart` · `stage_color_motion_test.dart` ·
`action_scales_wiring_test.dart` · `display_prefs_test.dart`

### ① ⭐⭐⭐ 而 `asset_guard_stage_attach_resend_test.dart` 的头注是**一份带日期的「被守的缺陷」清单**
> 「A1 资产守护网（A2/A6 接触点）：**iframe 重建后的自愈补发**。」              // :1
> 「# **被守的缺陷**（rc.4 实测修复，2026-09-14；**重放最易覆盖掉的就是这几行**）」  // :3
> 「「Win 侧换背景图后舞台仍纯黑」的根因之一：stage-bg 是**独立于 sync 的通道**，
> 旧实现只在「偏好变化」时发一次 ⇒ iframe 还没就绪 / 错误后重建**那张图永久丢掉**。
> 修法是 `_attach` **每次挂桥都补发一次**（`live2d_stage.dart:355-356`）。
> **同类纪律还有 sync 的 `stageColor` 与 `actionScales`** —— 重建后旧帧随旧桥销毁，
> **必须重发**（`:347-354`）。」                                                 // :5-12
⇒ ⇒⭐⭐⭐ **「被守的缺陷」作为一个测试文件的小节标题** ⇒⇒ **同 B0313 核的那条（`stop()` 排序的注释）**
—— **纪律写在「防它不守」的那一处** ⇒⇒⇒ **而这里还带**日期 + **症状**
⇒ ⇒⇒ **而「重放最易覆盖掉的就是这几行」是一句我此前没见过的担心**：
**⇒ 它防的不是「改错」，是「重做时不小心省掉」**

### ② ⭐⭐⭐ 而它明确区分了「已有断言」与「需要新断言」——**并说清已有那条测不到什么**
> 「# **为什么需要「新的」断言（已存在 vs 新增）**
> `test/live2d_bridge_test.dart:92` 已有**行为断言**，但它测的是 `Live2DBridge.sendStageBg`
> 这一条（**桥有没有照发**）。它**测不到**……」                                  // :10-14
⇒ ⇒ **存在断言 ≠ 覆盖到位** ⇒⇒ **这里说清了缺的是哪一层** ⇒⇒ **同 B0335「结构性保证」的分层意识**
⇒ ⇒⇒ ⭐ **而这一整段是「**为什么需要新测试**」的论证** —— 而不是我常见的那种「补个测试」

### ③ ⭐ 而断言本身是**结构扫描**（`RegExp(r'stageColor\s*:\s*widget\.stage…')`）⇒⇒
**它守的是「重发时用的是 `widget.stage…` 而不是缓存」** ⇒⇒ **一个具体写法的回归**
⇒⇒ **同 B0315 核的 `chat_notice_test.dart:150-167`**（结构测试钉「不许绕过判据自己判」）

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批最值钱的是 ① 里的那句话**：
> **「重放最易覆盖掉的就是这几行」**
> ⇒⇒ **它防的不是「改错」，是「重做时不小心省掉」** ⇒⇒ **而这与「修法有回归」是两种不同的防**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
