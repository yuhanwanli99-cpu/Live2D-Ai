# BATCH-0591 落盘（极简）· W8 = 专门治理「文档漂移」，且显式禁止改 AGENTS.md

## 命令（只读）
```
grep -n "W8" docs/plans/IMPL-PROMPTS-actions-performance-round.md
  -> :8 / :10 / :11 / :67 / :78 / :382 / :397
sed -n '397,420p' …
```

## W8 块逐字（要点）
```
:382 ===== 复制给 Worker W8 =====
:397 【任务 W8：导演口径与**文档漂移治理**（**只改文档与资产注释，不改行…**）
:398 背景（维护者 2026-09-21，**已确认**；规格见 RESEARCH 报告 §3.7 / §3.8 / §8 …
:399   产品形态 = 「**酒馆（类酒馆角色扮演内核）+ Live2D 皮套壳子**」
:401   **各功能由现有 mod 矩阵承担**（记忆 / 外部输入 / 语音 / 人设 / 导演…）
:402   **导演是一个 AI**（不是给用户操控皮套的辅助）。
:403   ⇒ **架构不改造**。要治理的是**文档漂移**：多处仍把导演写成「遗…」
:404      既与代码事实不符（它会产出 `latest.preset_id` 与 `action_cue`），也与产品…
必做（只改文档与资产注释）：
1. director-rfc.md：文件头加一节「**现行状态与已过时红…**」；把「主链不新增
   第二 LLM」「端点权威只有 [tts]/[llm]」「骨架不投递任何…」标成
   「这些是 2026-09-14 骨架期结论；产品已演进为『酒馆 + 皮套壳子』」
   另：把 §2.1 TextDelta「回复侧**补充**证据」标注为**不采用**
2. director-mod-v0.md：
   - 清掉「遗留并行实现」「不再是主路由」「零投递」「不驱动动作」
   - 顶部写清**实际职责**：按情绪/意图选动作包 → 产出 `latest.preset_id`
   - §0 的「host 下行通道仍然零调用」改成事实描述：action_tx / apply_setti…
     但**下行确实存在** —— 走 `ModServices.cues` → WS `action_cue`
3. performance-layer-v0.md：把「director staging 是遗留并行实现」改成
   「主链 [performance] 与 Mod staging_* 是**两个可选提供者，都默认关**…
     谁的 speak 能力该保留**未定**（RESEARCH §3.7 Q1）」
4. assets/actions/preset_labels.json：_doc 与 sources.neko 各加一句…
5. assets/actions/presets.json **不要动**（归 W2）；**AGENTS.md 不要动（归 W1）**
禁止：改任何 .rs / .dart；**裁决**「导演是否产出 speak」（Q1 未定，只写…
```

## 三个可核点
1. **W8 就是 B0585 核的那一节的生产者。** `director-rfc.md:24`
   「## 现行状态与已过时红线（2026-09-21）」与 W8 必做第 1 条**逐字对应**
   （同一标题、同样三条被推翻的措辞、同样「TextDelta 不采用」）。
   ⇒⇒ **⇒⇒⇒⇒** 判据：看到成品里有一段「治理」，去找**派它的那份提示词** ——
   那是这段治理的**验收标准原文**。
2. **⇒⇒⇒⇒ 与 F-0637-01 的关系，现在闭环了**：
   - AGENTS 那一行写「director **已删除**」（AGENTS 归 W1，**W8 明令不许碰**）
   - W8 那一波治理的是 **director-rfc / director-mod-v0 / performance-layer-v0**
     三份文档里「把它写成遗留/零投递」的措辞
   ⇒⇒ **⇒⇒⇒⇒** 也就是说：**两批人同时在动「director 的状态描述」，
   而 AGENTS 那份被单独划给了 W1。**
   ⇒⇒ **⇒⇒⇒⇒** 判据：**一个对象的「状态描述」被拆到多份文件、交给不同波次时，
   就一定会出现彼此不同步的那一份**（本例：AGENTS）。
3. **W8 的「禁止」清单第二行就是「裁决 Q1」** ⇒⇒ 与 W9 同（`:442` 那条），
   **⇒⇒⇒⇒ 两块都显式把 Q1 排除 ⇒⇒⇒⇒ RESEARCH §3.7 的闸确实被继承了**，
   我在 B0590 的修正**成立且更完整**。

## 一条正面模式
W8 必做第 2 条把 `director-mod-v0.md` 的 §0 改成
「`action_tx` / `apply_settings` … 但**下行确实存在** —— 走 `ModServices.cues` → WS `action_cue`」
⇒⇒ **⇒⇒⇒⇒** 判据：**修「零投递」这类措辞时，要同时写出「哪条链路是活的」**，
否则只是把一个错误换成另一个错误。

## 未核
W1 块正文（AGENTS 那一份由它负责，本批未读）· §3.5 / D5 原文 ·
`performance-layer-v0.md` 与 `assets/actions/preset_labels.json` 是否存在
