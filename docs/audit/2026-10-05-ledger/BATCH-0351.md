# BATCH-0351 · ⭐⭐⭐ **「用构建产物做断言」抓到的第二个「写好了却没人用」**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— 舞台覆盖层的**接线面**（B0328 核过归属，**内容**未核）

## 跑的命令（全部只读）
```
grep -rln "StageHost" lib/ ; grep -rn "模型加载|加载失败|进度" lib/ --include=*.dart
sed -n '344,360p' lib/app/app_shell.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一种我没在本仓见过两次的验证手法**
> 「// ── 渲染面状态（转给 `StageHost` 的覆盖层）──
> 这些**必须真的传进 `StageHost`**：**第一版里 `StageHost` 写好了却没人用**，
> 于是「模型加载中 / 加载失败 + 重试」的覆盖层与**舞台语义标签**都不在
> 是靠「**构建产物里搜不到『Live2D 舞台』这句话**」才发现的（见 v0.4.13）。」   // :344-349
四个可核点：
1. ⭐⭐⭐ **这是「写好了却没人用」的第二个实例**
   （B0000 记了第一个：设置分区「**永远打不开**」⇒ `dev_mode` 的开关被自己所在的分区藏起来）
   ⇒ ⇒ ⭐⭐⭐ **而这一次的发现方式是「**构建产物里搜不到某句话**」**
   ⇒ ⇒⇒ ⭐ **「用产物做断言」是一种可核的验证手法** ——
   它不问「代码对不对」，而问「**做出来的东西里有没有那句话**」
2. ⭐⭐ **要传的四样都以具名字段存在**（`:350-360`）：`stagePhase` · `stageProgress`（**0..1**）·
   `stageError` · `onRetryStage` ⇒⇒ **「传进去」被做成了四个具名字段**
   ⇒ ⇒ 同 B0297 核的 `onAck` 族（`onRenderEvent`/`onAck`/`onError`/`onReady`）**同一形状**
3. ⭐ **进度是 `double?`（0..1）而非整数百分比** ⇒ ⇒ **`null` = 「还没有进度」与「进度是 0」被区分**
   ⇒ ⇒ 同 **B0349 的可空性用法**（`null` 有含义）
4. ⇒ ⇒ 而 B0328 核过「双覆盖层」的归属（「加载态与错误态**不在这里画**」）
   ⇒ ⇒ **这两个缺陷是同一个区域的两次教训** ⇒⇒ **一个区域连出两次「以为接上了」**

⇒ ⇒ **0 findings**；⇒ ⭐ **而本批的收获是「一种验证手法的第二次出现」**：
**「在产物里找一句话」抓到的东西，与「读代码找一句话」抓到的东西，是不同的**
⇒ ⇒ **前者抓「没接上」，后者抓「写错」**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体
2. `live2d_bridge.dart` 余面 · `LiveRegionThrottle.feed` 本体
3. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1840 / `live2d_stage` ~585 / `memory_panel` ~515 /
   `persona_panel` ~505 / `message_bubble` ~440 / `chat_panel` ~435）
4. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 / GitHub secret scanning）
