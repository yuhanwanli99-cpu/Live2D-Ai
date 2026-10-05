# BATCH-0589 落盘（极简）· Q1/Q2 未在任何地方被回答；但 W8/W9 仍被派了工

## 命令（只读）
```
grep -rln "Q1\|speak" docs/plans/IMPL-PROMPTS-*.md docs/plans/BACKLOG-0.2.0-closeout-*.md
  -> IMPL-PROMPTS-0.2.0-seal-2026-09-27.md / IMPL-PROMPTS-actions-performance-round.md
grep -n "speak" IMPL-PROMPTS-0.2.0-seal-2026-09-27.md     -> 只有 :14 一处，且是 WS 帧名列表
grep -n "W8\|W9" IMPL-PROMPTS-actions-performance-round.md
sed -n '8,14p' 同上
```

## Q1/Q2 的下落
- **两份 IMPL-PROMPTS + BACKLOG 里都没有对 Q1/Q2 的正式回答**。
- `0.2.0-seal-2026-09-27.md` 里的 `speak` 只有 `:14` 一处，且出现在
  「不许改既有帧字段名（V11 只增不改）：action_cue / …」这个句子里
  ⇒ **是帧名字面量，不是 Q1 的答案。**

## 但 W1–W10 的波次里，W8 / W9 都被安排了
```
:8  - **Wave 0（可立即派，6 路并行）**：W1 文档对齐 / W2 幅值重标定 / W3 调试…
:10   W8 与 W1 文件不重叠，但**与 W4 共用 docs/architecture/performance-layer-v0.md**
:11 - **Wave 1（W2 / W8 收口后）**：W4 撤销语义（独占 main.dart / live2d_stage.dart）
:12 - **Wave 2（W4 收口后）**：W7 幅度下发通道整修
:13 - **Wave 3（W7 收口后）**：W9 单一驱动者（退役 latest.preset_id 那条通道，统…
:14 - **Wave 4（W9 收口后）**：W10 动作协议字段化（body/head/expression）+ 叠加 + h…
```

## 三个可核点
1. **Q1/Q2 至今无人回答**（B0588 的判据兑现：去别的文件找过，两份 IMPL + BACKLOG
   都没有）。⇒⇒ **⇒⇒⇒⇒** 「未定」在 09-21 写下，到 09-28 的 IMPL 里仍没有答案。
2. **⚠ 而 RESEARCH §3.7 `:248` 明写「在这两问有答案之前：**W8 与 W9 都不要派**」，
   而本文件 `:8/:10` 把 W8 放进 Wave 0「**可立即派**」、`:13` 把 W9 放进 Wave 3。**
   ⇒⇒ **⇒⇒⇒⇒** 判据：**Q1 问的是「AI 人是一个模型还是两个」；W9 的内容是
   「单一驱动者（退役 `latest.preset_id` 那条通道）」** ——
   **⇒⇒⇒⇒ 二者若答案相反，改动方向就相反。**
   ⇒⇒ **⇒⇒⇒⇒** 但**本审计不下这个结论**：W9 也可能是在「两个提供者都要」的前提下
   做「通道收口」，那就不冲突。**⇒⇒⇒⇒ 判据（第二层）：先确定 W9 的改动是否依赖 Q1 的答案。**
   **⇒ 未核**：`IMPL-PROMPTS-actions-performance-round.md` 的 W9 块正文（本批只看波次表）。
3. **⇒⇒⇒⇒ 一处正面观察**：`:10` 写「W8 与 W1 文件不重叠，但**与 W4 共用
   `docs/architecture/performance-layer-v0.md`**」⇒⇒ 判据：**波次表里写清「谁与谁文件重叠」，
   就等于给并行派工上了互斥锁**；而 RESEARCH §3.7 那道「两问有答案前不要派」的闸
   **在下游文件里没有被继承** ⇒⇒ **⇒⇒⇒⇒ 闸写在 A 文档里，不等于 A 的约束传到 B 文档。**

## 未核
`IMPL-PROMPTS-actions-performance-round.md` 的 W9 块正文（决定第 2 点）·
§3.8 正文 · `performance-layer-v0.md` 是否存在 · Q1 的可能落点 `[performance]` 端点是否已在代码里
