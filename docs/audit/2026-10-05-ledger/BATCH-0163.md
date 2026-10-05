# BATCH-0163 · `Auto` 降级路径：**三重门 + 语义化重发 + 诚实标注**，0 条新发现

Phase 1 · 域覆盖 · `performance/client.rs`（`request` 主体）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（定点 312-338 `request` 全段 + :319/:325/:327 逐行）

## 跑的命令（全部只读）
```
grep -n "Auto|degrade|降级|StructuredMode::Prompt" performance/client.rs
sed -n '312,343p' performance/client.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；降级门控的**四个可核性质**
```rust
let structured = matches!(self.mode, StructuredMode::JsonSchema | StructuredMode::Auto);      // :319
let (ok, body) = self.post_once(system, user, timeout_ms, structured).await?;                  // :320
if ok { return parse_completion(&body).map(|raw| PerformanceReply { raw, structured }); }      // :321-322
if self.mode == StructuredMode::Auto && structured && client_error_4xx(&body) {                 // :325
    let (ok2, body2) = self.post_once(prompt::SYSTEM_JSON_ONLY, user, timeout_ms, false).await?;  // :326-328
    if ok2 { return parse_completion(&body2).map(|raw| PerformanceReply { raw, structured: false }); }
}
None                                                                                           // :336
```
### ① 降级条件是**三重合取**（:325）
`mode == Auto` **且** 首次确实发了 structured **且** 响应是 **4xx**
⇒ `Prompt` 模式**根本不发** structured ⇒ 不会触发无谓重试；
`JsonSchema`（**用户显式指定**）模式**不降级** ⇒ 失败**如实上浮**，不被悄悄改写。

### ② 重发是**语义性的**，不是「换个 flag 再试」
:327 第二次调用传的是 **`prompt::SYSTEM_JSON_ONLY`**（**提示式 JSON 指令**），
而不是调用方原来的 `system` ⇒ 既然降级的前提是「**这个端点不认识 `response_format`**」，
那重发就必须**带上一份能替代它的东西** ⇒ 否则重发毫无意义。

### ③ `structured` 标记**如实回传**
:322 回 `structured`（真发的是什么）· :332 回 `structured: false`（降级后）
⇒ 调用方能分辨「这份 JSON 是**哪种模式**产出的」⇒ 这正是 B0151 核的
`FallbackReason` / `PerformanceStats` 能给出**类型化**降级原因的**数据来源**。

### ④ **只重发一次**，随后 `None`（:336）⇒ 服务端持续 4xx 也不会循环。
⇒ ⭐ 而 ①②合起来是同一纪律：**显式的用户意图不被静默改写**
（显式 `JsonSchema` 不降级）—— 与 B0109 的「显式 off 压过推断」、rc.5 的
「壳与舞台**共用一张图**是一份真相」**同族**。
⇒ 而「降级发生后**如实标注是降级**」= 正面模式 **P9（降级必须有出口）** 的数据侧。

## 未核实项
1. `client.rs` 余 ~510 行未读（`post_once` 主体、`client_error_4xx`、wire 回归的测试体、
   `prompt::SYSTEM_JSON_ONLY` 全文、自测）
2. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
3. `mod.rs` 余 ~350 行未读（`PerformanceStats` 对外暴露面、Budget 注入点）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
6. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
