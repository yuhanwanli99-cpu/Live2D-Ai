# BATCH-0369 · ⭐⭐ **我以为的矛盾是我自己压平的**：换桥重发（必要）· 同值重发（浪费）

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `test/stage_bg_resend_dedupe_test.dart`

## 跑的命令（全部只读）
```
grep -nE "test\(|expect|dedupe|重发|去重" shell/flutter/test/stage_bg_resend_dedupe_test.dart | head -8
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **一个我自己制造的矛盾**
**我上一批说**（B0290）：「`:123`「现在 `_attach` **每次挂桥都重发一次**」」⇒ **「重发」**
**而这个测试叫** `stage_bg_resend_dedupe` ⇒ **「去重」**
⇒ ⇒ 我准备记一条「代码与测试互相矛盾」的发现 ⇒ ⇒ **核完发现是我错了**

### ① 两者在**不同层**，而测试自己的头注就写着
> 「**F-0002-1（P1）· 同值不重发**：`stage-bg` 是**整份状态帧**（dataUrl 可达 **1.5 M 字符**）；
> 重发一帧的代价是渲染面**重新 base64 → Blob → createImageBitmap → write_texture**」（`:1-2`）
> 「**去重只许去同值**……就整串重发一次 ⇒ 设了壁纸的用户**一拖滑杆，主线程同步 MB 级序列化**」（`:4`/`:10`）
> `:76` `reason: '同值重发 = 渲染面重解一次整图；**去重只去同值，真变化不许去**'`
⇒ ⇒ **「每次挂桥都重发」= 换桥（重建 iframe）⇒ 那是**必要的重建**，不是浪费
⇒ ⇒ **「同值不重发」= 同一座桥上、同一张图被反复要求 ⇒ 那是**浪费的重发**
⇒ ⇒⇒ **两句话都对，因为它们不在同一层** ⇒ **矛盾是我压平的**

### ② ⭐⭐ 而「去重的边界」被写在断言的 **`reason`** 里
「**去重只去同值，真变化不许去**」⇒ ⇒ ⇒ **B0339 那条「克制/边界写进 reason」的正面对照**
（`:76` 的 reason 文本本身**就是判据**）⇒ ⇒ **而它不只解释「为什么断言」，它解释了「为什么边界在这」**

### ③ ⭐⭐ 而「一拖滑杆就 MB 级序列化」这条**正是 B0329 那个拖动优化的动机**
| 批 | 同一事故的修法 |
|---|---|
| **B0329** | `onChanged` → `onChangeEnd`（**把草稿留在控件里**） |
| **本批** | `stage-bg` **同值去重**（**不重发整帧**） |
⇒ ⇒⇒ **一次事故 · 两处修法 · 两处记录** ⇒ ⇒ **B0329 核的是控件侧、本批核的是传输侧**
⇒ ⇒⇒ ⭐ **两处都被记住了** —— 而我**直到这一批才把它们并排**

### ④ ⚠ 而我据此提炼**第六种失效形态**
> **凭片段** —— **读了两侧，但从没并排**
⇒ ⇒ 与前五种并列：**凭记忆** · **凭零命中** · **凭一次 grep** · **凭印象补全** · **凭印象怀疑** · **凭片段**
⇒ ⇒ **这一种最隐蔽**：因为它的两个来源**都是真的** ⇒ ⇒ **只有并排读才会暴露**

## 未核实项（本批后仍开着 3 条 + 4 条永久）
1. `UiSignals` 定义本体 · `ModServices.apply_settings` 本体 · `settings_models.dart` 的 `fromJson` 体 ·
   `LiveRegionThrottle.feed` 本体 · `ModConfigResult` 定义 · `PersonaCardFilePicker` typedef 注释
2. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）· `main.rs` 余约 930 行 ·
   `requestAnimationFrame` 的**暂停语义** ·
   面板余面（`dev_tools_section` ~1790 / `live2d_stage` ~580 / `memory_panel` ~510 /
   `persona_panel` ~500 / `message_bubble` ~435 / `chat_panel` ~425）
3. **永久不可核 4 条**（`reqwest` timeout / 动作计划 token / `_finishTurn` 幂等 · GitHub secret scanning）
