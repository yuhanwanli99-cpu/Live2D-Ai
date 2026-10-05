# BATCH-0361 · ⭐⭐⭐ **「绝不偷偷改成全局导入」**；而 `package:web` 边界是**第三次**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `persona_panel` 的**两个作用域 / 会话块**（B0347 核了派发，**呈现**未核）

## 跑的命令（全部只读）
```
grep -nE "_sessionBlock|会话|解绑|已绑定" lib/settings/mods/persona_panel.dart | sed -n '1,10p'
sed -n '16,30p' lib/settings/mods/persona_panel.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **禁令、误解防护、依赖注入边界各一**
> 「- 「导入并生效」默认绑定 **`ctx.activeSessionId`**（当前会话）：只影响这个会话；
>   换会话不串卡、回原会话仍在。**没有活动会话时它禁用并说清降级语义，
>   绝不偷偷改成全局导入（那正是这一波要根除的串人设）**。」                       // :18-21
> 「- 「导入为全局人设（所有会话）」是**旧的全局行为**……**文案里写明它是全局的，
>   不让人误以为绑定了会话**。」                                                   // :21-23
> 「- 运行态里的 sessions / active_session 由 **Mod 的 `state_json`** 给出：面板显示
>   「**已绑定 N 个会话**」，并标出**服务端记录的**活动会话。」                        // :24-25
> 「# **粘贴是主路径，文件选择要宿主接线**
>   粘贴框是**始终可用**的那条路（卡本来就是 JSON）；「选择 PNG 角色卡文件」按钮
>   **只在宿主注入了 `PersonaCardFilePicker` 时出现** —— 面板**不**直接 import
>   `app/browser_io.dart`，理由见那个 typedef 的注释……」                            // :27-31
五个可核点：
1. ⭐⭐⭐ **「偷偷降级到另一个作用域」被写成禁令**，而**理由点名了它要根除的那个缺陷**（「串人设」）
   ⇒ ⇒ **同 B0244 的 P28 家族**（不假装可用）**并加一层**：
   **不假装可用时，也不要改用另一个作用域**
2. ⭐⭐ **按钮文案要防的是「误解」而不是「不知道」**：「**不让人误以为绑定了会话**」
3. ⭐⭐ **会话计数来自 Mod 的 `state_json`，面板不自数**
   ⇒ ⇒ **B0220 核过 Mod 侧确有 `sessions` / `active_session`** ⇒⇒ **两侧一致**
4. ⭐⭐⭐ **依赖注入边界**：「只在宿主注入了 `PersonaCardFilePicker` 时出现」+
   「面板**不**直接 import `app/browser_io.dart`」
   ⇒ ⇒⇒ **「不依赖 `package:web` ⇒ 可在 VM 里测」的**第三次**
   （B0226 `chat_panel` · B0328 `message_bubble` · **本批 `persona_panel`**）
   ⇒ ⇒ **而它连理由都指向了**（那个 `typedef` 的注释）⇒ ⇒ **又一次「指路」**
5. ⭐⭐⇒ **这个约束**塑���了设计** ⇒ **「粘贴是主路径」不是偏好，是边界的结果**，
   **而偏好与边界写在同一段** ⇒ ⇒ **读代码的人能分清哪些是「要的」哪些是「只能这样」**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的第 4 点最值钱**：**一条测试性约束决定了主路径**，
⇒ ⇒ 且**这一点被写在设计决策旁边**，所以后来者不会把主路径「优化」成文件选择

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `AdminEmpty` 本体 ·
   `PersonaCardFilePicker` 的 typedef 注释（`:31` 指过去的那处）
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1810 / `live2d_stage` ~580 / `memory_panel` ~515 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425)
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
