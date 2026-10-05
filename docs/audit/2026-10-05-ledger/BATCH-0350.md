# BATCH-0350 · ⭐⭐⭐ **读屏播报与切句共用「什么算一句话」的判据** —— 那条红线在这个轴上也有落地

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `shell_chat.dart` 的**节流播报接线**（B0179 核过 throttle 存在）

## 跑的命令（全部只读）
```
grep -rn "LiveRegionThrottle|announcement" lib/ --include=.dart
sed -n '1,30p' lib/app/shell_chat.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一条红线在第四个轴上落地**
> 「把流式正文喂给节流播报器。**按「整句到达」优先**：末尾出现**句末标点就立刻播报**；
> 否则按 **1.5 s** 兜底。**两级放行的判据在 `LiveRegionThrottle.feed` 里，可单测**。」   // :14-17
> `if (last.role != ChatRole.assistant) return;`                                        // :23
> `if (!last.streaming) { _live.finish(last.text); return; }`                             // :24-27
> 「收口：把最后一小段补播一次（**末尾往往没有句末标点**，比如「好呀…」）」               // :26
四个可核点：
1. ⭐⭐⭐ **第一级放行的判据是「末尾出现句末标点」= 「整句到达」**
   ⇒ ⇒ 而 **B0132 核过 `sentence.rs` 用的就是同一组句末标点**（`。！？…`）
   ⇒ ⇒⇒ **读屏播报与切句共用「什么算一句话」的判据** ⇒ **不是两套定义**
   ⇒ ⇒⇒⇒ **而「一句一单元」这条红线此前我只核到落在切句（B0132）与 TTS（B0310 音频尾）上，
   现在它在**第四个轴**（读屏）上也有落地**；第二级兜底 = **1.5 s**
2. ⭐⭐ **收口路径单独存在，且给出理由**：「**末尾往往没有句末标点**，比如「好呀…」」
   ⇒ ⇒ `_live.finish(...)` 补播 ⇒ ⇒ 与 B0313 的 `wordless`/`stopped` **同族**：
   **让「最后没句号的那句」也有出口**
3. ⭐ **`last.role != ChatRole.assistant ⇒ return`** ⇒ ⇒ **只播报助手侧**
   ⇒ ⇒ **用户自己说的话不会被读屏再念一遍** ⇒ ⇒ **一个具体的用户动作被点名**
4. ⭐ **判据被指到别处且标明可测**（「判据在 `LiveRegionThrottle.feed` 里，**可单测**」）
   ⇒ ⇒ 又是「**指路**」+「**判据集中**」

⇒ ⇒ **0 findings**；⇒ ⭐ **而 ① 是本批真正的收获**：
**一条红线能在四个轴上落地，前提是每次落地都复用同一个判据而不是重新发明**
⇒ ⇒ 而**复用判据的代价**是：**这几个轴必须一起改**（B0132 的句末标点一改，读屏播报跟着变）

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面（队列 · `_modelLoads` 注入点 · 事件流）· `LiveRegionThrottle.feed` 本体
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~590 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~435）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
