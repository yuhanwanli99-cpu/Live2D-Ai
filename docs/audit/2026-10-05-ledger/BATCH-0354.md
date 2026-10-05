# BATCH-0354 · ⭐⭐ **第四次「状态归属被声明」** —— 而这次给出了**放错位置的两个具体后果**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `dev_tools_section` 的 **Mod 配置卡片**

## 跑的命令（全部只读）
```
sed -n '565,586p' lib/settings/sections/dev_tools_section.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **一个已成惯例的做法，而这次理由最具体**
> 「# **为什么表单状态住在这里而不是宿主**                                        // :569
> 宿主只负责「**把保存请求发出去**」；每个 Mod 的**字段草稿、保存中的禁用、成功/失败文案**
> 都是**这一张卡片的局部状态**。**放进宿主会让「保存了哪个 Mod 的哪一项」与全局的
> `_adminMessage` 缠在一起，也给不出逐卡片的结果**。」                            // :571-575
四个可核点：
1. ⭐⭐⭐ **归属被声明，且给出放错位置的**两个具体后果**
   ⇒ ① 「**保存了哪个 Mod 的哪一项**」与全局 `_adminMessage` 缠在一起 ② **给不出逐卡片的结果**
   ⇒ ⇒ **不是「局部状态更好维护」，而是两个具体的坏结果** ⇒ ⇒ **P1 家族**
2. ⭐⭐ **宿主职责被收窄成一句可核对的话**：「**只负责把保存请求发出去**」
   ⇒ ⇒ **一行职责** ⇒ ⇒ 同 B0323「宿主只需 `GlobalKey` 调 `retry()`」**同族**（窄接口 + 宽内部）
3. ⭐⭐ **而「逐卡片的结果」正是本仓反复做的事**（B0236 编辑对话框 · B0330「id 不变」·
   B0346「下一条命中它的用户话，本轮就会」）⇒ ⇒ **它被当成理由提出来了**
4. ⭐ **四个具名回调**（`onToggle` / `onSaveConfig` / `onLoadState` / `onModChanged`）都挂在**这张卡片**上
   ⇒ ⇒ 与 B0351 的 `stagePhase`/`stageProgress`/`stageError`/`onRetryStage` **同形**
   ⇒ ⇒ **一组具名回调 = 一份显式契约**

⇒ ⇒ **0 findings**；⇒ ⭐ **而这是「状态归属被声明」的**第四次**
（B0245 的 env 字段 · B0230 的 memory 面板 · B0286 的 env key 字段 · **本批**）
⇒ ⇒⇒ **它已是本仓的惯例，而每一处都给出了违反它的具体代价**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体 · `_ModConfigTile` 的 save 失败分支
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1835 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~430）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
