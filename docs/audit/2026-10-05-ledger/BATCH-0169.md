# BATCH-0169 · ⭐ `resolve`：`:301` 那一行**让 P13 在代码里真正成立**（0 条新发现）

Phase 1 · 域覆盖 · `performance/mod.rs`（`PerformanceRuntime::resolve` 主体）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/mod.rs` — 478（定点 265-306+：`resolve` 前半段）

## 跑的命令（全部只读）
```
grep -n "Budget|budget|pub fn |fn |enabled" performance/mod.rs
sed -n '265,306p' performance/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **`:301` 是让 B0165 那条结论真正成立的那一行**
### ① 三路前置门：两个原因**不发请求**就定了
```rust
let assistant = assistant_text.trim();                       // :283
if !self.client.enabled() { return self.fallback(assistant, FallbackReason::Disabled); }      // :284-285
if assistant.is_empty()      { return self.fallback(assistant, FallbackReason::EmptyAssistant); } // :287-288
```
⇒ `Disabled` / `EmptyAssistant` **两个原因码各自独立**、且**零 I/O** ⇒ 「关掉」与「没话说」
在观测面上**可区分**（B0167 的 `last_reason` 能给出不同值）⇒ 正面模式 **P9** 的粒度够用。

### ② 提示按 mode 在**一处**选
:290-293 `Prompt ⇒ SYSTEM_JSON_ONLY`，否则 `SYSTEM_STRUCTURED`（B0165 核过前者）

### ③ ⭐ 而 `:301` 让「提示侧与强制侧是同一句话」**在代码里真的成立**
```rust
// 剥围栏是**宽容解析**，契约不变：**仍然必须过同一个校验器**。              // :299
let raw = prompt::strip_code_fence(&reply.raw);
// **V1 拼接基准 = 送进提示词的同一份 trimmed 原文**。                      // :301
let user = prompt::build_user_prompt(user_text, assistant, &self.allow);  // :294 ← 同一个 `assistant`
match plan::parse_plan(&raw, &self.allow, assistant) {                    // :302 ← 也是同一个 `assistant`
```
⇒ **trim 后的 `assistant` 同时**是「送进提示词的正文」**与**「拼接校验的基准」
⇒ 所以 `segments.concat() != source`（B0157）拒掉一份 plan 时，
**「为什么」是确定的**：模型**没有照着这句指令切**它**实际收到的那段文本**。
⇒ ⇒ **若两侧用不同的 `source`，B0165 的 P13 就只是「两句话长得像」**；
**`:301` 这一行才是把「同一句话」变成「同一个字符串」的地方**。
⇒ ⭐ **可提炼为正面模式 P14**：**「提示与校验同源」不能只靠两处写同样的文字，
必须让两侧消费**同一个值**（否则是两份可能漂移的副本）。**

### ④ 「宽容」被放在**进门前**，而**不放宽校验器**
:299 逐字：「剥围栏是**宽容解析**，契约不变：**仍然必须过同一个校验器**」
⇒ 容忍 ```json 围栏的位置在**解析入口**（无害），
而 `parse_plan`（B0157 核过的不变量）**一步不让** ⇒ **宽容没有渗进契约**。

### ⑤ 成功路径也记账
:304-305 `plans.fetch_add(1)` + `last_reason.store(0)` ⇒ 计数同时反映**成功**与**降级**
⇒ 与 B0167 的 `:690`（断言降级侧取值）**成对**：两边都有断言。

## 未核实项
1. `mod.rs` 余 ~300 行未读（`resolve` 后半段 `fallback()` 实现、`Budget` 相关、自测）
   ⇒ **注**：本批 `grep` 未见 `Budget` 字样 ⇒ **该词在本文件不存在**（不是漏搜）
   ⇒ `injection_budget`（B0104/B0113）是**记忆 Mod** 的字段，**表演层没有同类预算** ——
   **是否应当有**（例如给 `resolve` 加超时外的调用频次上限）**未核，属产品判断**
2. `plan.rs` 的 `message()` 逐一与自测未读
3. `client.rs` 余 ~430 行未读（wire 回归**断言体**、`build_body`）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
