# BATCH-0174 · ⭐ wire 回归：**用真实请求计数钉住「不重试」**，且**「思考不进下游」在客户端被钉住**

Phase 1 · 域覆盖 · `performance/client.rs`（wire 回归断言体 —— **runtime 最后一块**）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（枚举 `wire_tests` 全部用例名 + 读
   `server_errors_and_blank_content_fall_back_to_none` 断言体）

## 跑的命令（全部只读）
```
grep -n "    async fn |    fn " performance/client.rs
awk '/^mod wire_tests/,0' performance/client.rs | grep -oE "async fn [a-z0-9_]+"
awk '/async fn server_errors_and_blank_content_fall_back_to_none/,/^    }$/' performance/client.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；两处**强形态**核验
### ① 「5xx 不得重试」是**数真实请求**钉住的
```rust
let (base, handle) = scripted_server(vec![(500, r#"{"error":"boom"}"#)]).await;
assert_eq!(client.request("s","u",2_000).await, None);
assert_eq!(handle.await.expect("mock").len(), 1, "**5xx 不得重试**");
```
⇒ 断言**不是**「返回了 `None`」，而是「**恰好发了 1 个请求**」
⇒ **一个加了重试的实现会发 2 个** ⇒ **测试变红** ⇒ 「不重试」这条规则**在网络层被钉住**。
⇒ 这是 **正面模式 P7（每条断言配对照）** 的**最强形态**：
**对照不是另一个测试用例，而是「线上真实发生的次数」**。

### ② ⭐ 「**思考不进下游**」在**客户端**被钉住（此前只在其它层核过）
```rust
(200, r#"{"choices":[{"message":{"content":"","reasoning_content":"想一想"}}]}"#)
…
assert_eq!(client.request("s","u",2_000).await, None);
let raws = handle.await.expect("mock");     // ← 取回**记录下来的请求原文**
```
⇒ 上游**确实返回了 `reasoning_content`**，而客户端**拒绝**把它当正文（返回 `None`）
⇒ 且随后**取回请求原文**继续断言 ⇒ 请求体侧也不带思考
⇒ **这是我见到的该红线唯一一处「客户端层面」的回归** ——
此前核到的三处分别是：解析层单列 `LlmEvent::ReasoningDelta`（B0009）·
WS 用**独立帧** `reasoning_delta`（B0015）· 请求体**只有 system+user**（B0151 读头注）
⇒ **现在补上了第四处：客户端拿到带思考的 200 响应时的行为。**
⇒ 与 B0009 早前核到的 `reasoning_never_reaches_the_sentence_assembler` **成对**：
那条在**装配器**入口拦，这条在**客户端**入口拦 ⇒ **两道闸，各自独立**。

### ③ `wire_tests` 三个用例名也各自对应一个规则
`openai_client_posts_json_schema_and_parses_content`（正常路）·
`auto_mode_degrades_to_prompt_on_4xx`（B0163 的降级）· `server_errors_and_blank_content_fall_back_to_none`（本批）
⇒ **正常 / 降级 / 回落**三态**各有独立回归** ⇒ 与 B0117 的「正常 / 显式不清空 / 缺省」三向**同形**。

## 未核实项（`live2d-ai-runtime` 收尾的最后清单）
1. `mod.rs` 余 ~180 行（`Resolution` 字段全文、余方法、自测）
2. `client.rs` 另两条 wire 用例的**断言体**（`openai_client_posts_json_schema_and_parses_content` ·
   `auto_mode_degrades_to_prompt_on_4xx`）未读 —— **注**：降级那条的**请求计数**（是否也断言
   「恰好 2 个请求」）**未核**
3. `plan.rs` cue 解析后半（`BadAnchor` / `UnknownPreset`）与两文件自测未读
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. **`mod_count_is_five` 的断言体未读**（只核了函数名）
8. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
