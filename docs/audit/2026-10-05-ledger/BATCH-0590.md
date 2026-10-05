# BATCH-0590 落盘（极简）· W9 正文 = 明确写了「禁止裁决」，与 Q1 不冲突

## 命令（只读）
```
grep -n "W9" docs/plans/IMPL-PROMPTS-actions-performance-round.md
  -> :1 标题 W1–W9 / :13 Wave 3 / :14 Wave 4 / :427 分隔 / :442 任务标题
sed -n '442,470p' …
```

## W9 块逐字（要点）
```
:442 【任务 W9：单一驱动者——退役「拉 latest.preset_id」那条通道，统一到 a…
:443 （**注**：这条通道退役后，W10/W11 才能把动作协议换成字段化形状；W9 …
:444 背景（RESEARCH 报告 §3.5 / D5）：前端现在有**两条**未协调的驱动通道：
  A) 按句 action_cue：main.dart:421-425（存计划）+ 443-456（该句音频开始时应…
  B) 一次性拉 GET /api/v1/mods/director/state 的 latest.preset_id：main.dart:417-419…
  两条**取到的集合不同**（B 走 presets.resolve() 只给一条；A 走 resolve_slots…
必做：
1. **退役通道 B**：删掉 _applyDirectorPreset()、_directorTurnHandled、_lastDirectorSe…
   舞台的动作预设**只**由 action_cue 驱动。删完必须确认：**director 启用…**
   （它的 cue 走 SentenceReady → ModServices.cues → WS action_cue；这条链路不许…）
2. latest / recent_decisions 仍作为**面板只读展示**（director_panel.dart 照旧读 s…
3. 断言（可测，缺一条即未完成）：
   - Dart 侧：新增一条纯逻辑/组件测试证明「收到 action_cue 帧 → 按 sente…
     并证明**没有任何代码路径**会 state('director') + applyPreset（**grep 级断言**）
   - 保留并跑通既有 shell/flutter/test/action_cue_test.dart
   - 文字断言：grep -rn 'mods/director/state' shell/flutter/lib/ 只剩面板读取处
4. 修掉与实情不符的注释与文档句（app_event.rs:233 / …HANDOFF-2026-09-22 §6）
禁止：改 action_cue 帧结构；删面板；让两条通道都活跃；**裁决「导演是…**
验收：回报里给出两种配置下**谁**驱动表情/手势的实测 —— 「director …」
      证据 = 起服务后的渲染面 HUD preset: 行 + WS 帧序列（**不要只用单测替…**）
【文件归属】main.dart、app_event.rs、architecture/{performance-layer-v0.md,
           director-mod-v0.md}、HANDOFF…、action_cue_test.dart（或新建
           driver_policy_test.dart）
```

## 结论：B0589 第 2 点的冲突**不成立**
1. **W9 处理的是「两条通道重复」，不是「AI 人是一个模型还是两个」。**
   它的判据写在 W9 自己手里：**`presets.resolve()` 给一条 vs `resolve_slots` 给多条，
   两条取到的集合不同** ⇒ 这是**同一件事被投递两次**的问题，
   与 Q1 无关。
2. **W9 的「禁止」清单里明写「**裁决「导演是…**」** ⇒⇒⇒⇒**
   也就是说派工者**主动把 Q1 排除在 W9 之外**。
   ⇒⇒ **⇒⇒⇒⇒** B0589 我提的「Q1 与 W9 方向相反」不成立：
   **W9 不依赖 Q1，因为它明说不裁决。**
3. **⇒⇒⇒⇒ 一处需要更正的话**：B0589 我写「§3.7 那道闸**在下游文件里没有被继承**」。
   **⇒⇒⇒⇒ 这话不准确。** `:248` 说的「不要派」指的是 **W8/W9 整体**，
   而本文件 `:8` 的 Wave 0「可立即派」明确**不含 W8**（列的是 W1/W2/W3/…），
   `:10` 只说 W8 的**文件重叠关系**、`:13` 的 W9 排在 Wave 3。
   ⇒⇒ **⇒⇒⇒⇒** 真正的疑点只剩一个：**W9 排在 Wave 3、且已被写出可复制提示词，
   算不算「已经派了」**。而它明写「不裁决 Q1」⇒⇒ **⇒⇒⇒⇒ 我撤回「闸未被继承」这句**，
   改成：「闸**部分**被继承 —— W8 没进 Wave 0，W9 排在 Wave 3 且自带不裁决条款」。

## 一条正面模式（可提炼）
W9 的验收段要求「**起服务后的渲染面 HUD `preset:` 行 + WS 帧序列**」，
并写「**不要只用单测替代**」⇒⇒ **⇒⇒⇒⇒** 判据：**能实测的验收项写「实测 + 证据形态」，
并显式禁止用便宜手段代替**（与 AGENTS「肉眼验收待做」「B 站抓取不在主仓」同族）。

## 未核
§3.5 / D5 的原文 · W8 块正文（它在 Wave 0 还是不在）·
`performance-layer-v0.md` 是否存在 · `app_event.rs:233` 的注释现状
