# BATCH-0394 · ⭐⭐ **「只摆一套」也是结构性的** —— 而边界的澄清句用的是**否定式**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 舞台单图轮播 × 壳背景库的**互斥**（B0391 头注引出的）

## 跑的命令（全部只读）
```
grep -nE "stagePlaylist|轮播|stage-bg" lib/settings/sections/appearance_section.dart | head -8
sed -n '231,246p' appearance_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**「来源」这个概念的第三种失效面也有了结构守卫**（0 条新发现）
> 「── 舞台单图轮播（**DEC-2 的分工那一半**）──」                                     // :231
> 「它与上面那块「壳背景轮播」**按来源互斥**：来源 = 舞台那张时只有[这一块]
> （**走渲染面 `stage-bg` 帧**），来源 = 背景库时只有那块（**Flutter 层换图**）。
> **两套列表、两套预算、两条下发通道，同时摆出来用户只会以为它们是一套** ——
> **rc.5 §9.2 记的就是这个重叠**。」                                                   // :232-237
> 「名字分清『舞台单图轮播』vs『壳背景轮播』：这一块管的是**舞台那张的历史列表，
> **不是背景库**（背景库那套预算与它**无关**）。」                                    // :238-240
> `if (**!librarySource**) GroupCard(title: '舞台单图轮播', …)`                   // :241-244
```
四个可核点：
1. ⭐⭐⭐ **互斥被做成了结构**（`if (!librarySource)`）⇒⇒ **不是「都摆出来、让用户自己选」**
   ⇒⇒⇒ **同 B0342「禁用在**输入层**生效」—— 「只摆一套」同样是结构性的**
2. ⭐⭐⭐ **理由是用户会把两套当成一套**（「**同时摆出来用户只会以为它们是一套**」）
   ⇒⇒ **并引了 `rc.5 §9.2`**（又一次「指路」）⇒⇒ **同 B0389 的思路：防的是「用户的合理误解」**
3. ⭐⭐ **「两套列表、两套预算、两条下发通道」被逐一点名** ⇒⇒ **三样都不同 ⇒⇒ 所以**摆在一起时无法从界面上区分**
4. ⭐⭐⭐ **而边界的澄清句用的是否定式**：「这一块管的是**舞台那张的历史列表，不是背景库**」
   ⇒⇒⇒ ⭐ **用「不是」划界比用「是」更难被误读** ⇒⇒ **同 B0324 记忆面板头注「会覆盖 / 清空」也是否定式的思路**
⇒ ⇒⭐ **而「来源」这个概念的三种失效面，现在都有结构守卫**：
| 失效面 | 守卫 | 出处 |
|---|---|---|
| ① 按钮必然无效 | `onPickStageImage`/`onClearStageImage` **可空** | B0390 |
| ② 「当前是哪一项」算错 | 判据**只在外壳一处** | B0390 |
| ③ **两个入口同时出现** | `if (!librarySource)` | **本批** |

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. 「两套**预算**」具体指什么（`appearance_section.dart:235` 提到、**未核**）
2. `AppShell._currentBackground` 本体 · `evaluate_mode` 函数本体 · `UiSignals` 定义本体 ·
   `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1780 / `live2d_stage` ~565 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~420）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
