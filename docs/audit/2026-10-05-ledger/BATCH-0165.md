# BATCH-0165 · `SYSTEM_JSON_ONLY`：**提示侧与强制侧是同一句话的两种语言**（0 条新发现）

Phase 1 · 域覆盖 · `performance/prompt.rs`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/prompt.rs` — （定点 33-41 `SYSTEM_JSON_ONLY` · 46-49 `build_user_prompt`）

## 跑的命令（全部只读）
```
grep -rn "SYSTEM_JSON_ONLY" -A 16 performance/prompt.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 核到「同一条不变量的**提示侧与强制侧**」
### ① 提示把**三种常见失败模式**逐一点名
:34 「只输出 JSON，**不要解释**、不要 **Markdown 围栏**、不要**多余字段**」
⇒ 解释性散文 / ```json 围栏 / 额外字段 —— 正是 LLM 输出 JSON 的三种典型走偏。

### ② ⭐ 而切分纪律在提示侧与强制侧是**同一条**
| 位置 | 表述 |
|---|---|
| **提示侧**（`SYSTEM_JSON_ONLY`:38） | 「segments 只能切分原文，**逐字不变**，全部按顺序拼接**必须与原文完全相同**；原文为空给空数组」 |
| **提示侧**（`build_user_prompt`:49） | 「【切分纪律】segments 只能切分上面的主模型原文：逐字不变，拼接后必须与原文完全相同」 |
| **强制侧**（`plan.rs:611`，B0157） | `if segments.concat() != source \|\| (source.is_empty() && !segments.is_empty()) { Err(SegmentsNotPartition) }` |
⇒ **不是「提示说一套、代码查另一套」** —— 解析器拒绝一份 plan 时，**模型的指令就是「为什么」的出处**。
⇒ ⭐ 记一条：**正面模式 P13 —— 同一条约束在「提示侧」与「强制侧」用同一句话表述**
⇒ 于是它与 B0116（`DISCIPLINE_TEMPLATE` ↔ `sentence.rs`「不许硬切」）、B0132（AGENTS「空末块发边界帧」↔ `worker.rs` 双边界标记）
**同族**：跨语言对齐的**三处**样本，且**每一处都是「同一句规则、两种语言」**。

### ③ 枚举值也**在提示里写死**，与 schema 一致
`field: body|head|expression` · `at: now|seg:N|after_prev` · `hold: true|false` · `intensity 取 1/2/3` ·
`expression 必须给 id`（`none` = 撤销）⇒ 与 `json_schema_strict`(:816-832) 的 `enum`/`minimum`/`maximum` **逐项对应**
⇒ 而解析侧对词表外的值**报错**（B0157 核的 `UnknownField` / `ExpressionUnknownId`）⇒ **三层同形**：提示给全集、schema 给机器可校验、解析给错误。

### ④ `build_user_prompt` 头注：思考与原文**都不动**
:45 「**不含思考**；**不修改任何文本**（原文只作输入，切分方案由表演层给出）」
⇒ 与「思考不进下游」红线在**提示构造侧**也成立（B0151 已核 body 只有 system+user、无 `reasoning_content`）。

## 未核实项
1. `prompt.rs` 其余部分未读（**文件总长未核**）
2. `client.rs` 余 ~470 行未读（`post_once` 主体、wire 回归**断言体**）
3. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
4. `mod.rs` 余 ~350 行未读（`PerformanceStats` 对外暴露面、Budget 注入点）
5. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
