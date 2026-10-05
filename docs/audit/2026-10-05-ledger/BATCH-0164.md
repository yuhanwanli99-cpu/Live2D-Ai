# BATCH-0164 · ⭐ `client_error_4xx`：**保守启发式 + 显式声明误判代价 + 代价被结构性封顶**（正面模式 P12）

Phase 1 · 域覆盖 · `performance/client.rs`（`client_error_4xx` + wire 回归设施）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（定点 341-355 `client_error_4xx` ·
   357-374 `wire_tests` 头部：脚本化 loopback 服务器）

## 跑的命令（全部只读）
```
sed -n '341,374p' performance/client.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ **立正面模式 P12**（这是本审计第一次核到「**A 处的安全性依赖 B 处的性质**」）
### ① 判据本体：要求**两个独立信号**同时成立（偏保守）
```rust
let has_error = lower.contains("\"error\"");                        // :348
let looks_like_param_problem = lower.contains("\"type\"") || lower.contains("\"param\"")
    || lower.contains("invalid") || lower.contains("unsupported") || lower.contains("unknown");  // :349-353
has_error && looks_like_param_problem                               // :354
```
### ② 而**头注把「这是启发式」和「误判代价」都写明了**（:341-345）
> 「我们只从 `post_once` 拿得到『成功与否 + 正文』，**拿不到状态码**，
> 所以这里用一个**保守**的启发式：只在 `Auto` 的降级判定里用它；
> **误判的代价只是多一次请求**（然后仍按失败回退）。」

⇒ 三个要素齐备：
1. **承认是启发式**（不假装精确）—— 精确地说出**为什么**不精确（拿不到状态码）；
2. **写明误判代价**；
3. ⭐ **代价被结构性地封顶** —— 上一批（B0163）核到的 `:336`「**只重发一次**后 `None`」
   ⇒ 「多一次请求」这句话**有结构约束托着**，否则它只是希望。
⇒ ⇒ **正面模式 P12**：
> **拿不到精确信号时，用「保守启发式 + 显式声明误判代价 + 代价被结构性上界约束」。**
> **第一要素：承认是启发式**（不假装精确）· **第二：写明误判代价** · **第三：代价被上界约束**。
> ⭐ **第三要素是第一、二要素能成立的前提** —— 没有它，前两者只是免责声明。

⇒ ⭐ **本审计第一次核到「A 处的正确性依赖 B 处的性质」**：
**B0163 的「只重发一次」** ⇒ **B0164 的「误判代价只是多一次请求」才成立**
⇒ 两处**同在 `client.rs` 一个文件内**，相距约 10 行 ⇒ **这不是两条独立事实，是一条**。

### ③ 顺带：`wire_tests` 用**真实 loopback TCP 服务器**，不是 mock
:367-374 `scripted_server(responses: Vec<(u16, String)>)` ⇒ 按顺序回应 N 个请求，
并把**每个请求的原始文本**收回（`JoinHandle<Vec<String>>`）
⇒ 测试可以**断言线上内容**（请求体/头）⇒ 模式 H 的**强形态**应用于 HTTP。

## 未核实项
1. `client.rs` 余 ~470 行未读（`post_once` 主体、`prompt::SYSTEM_JSON_ONLY` 全文、wire 回归的**断言体**）
2. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
3. `mod.rs` 余 ~350 行未读（`PerformanceStats` 对外暴露面、Budget 注入点）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
6. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
