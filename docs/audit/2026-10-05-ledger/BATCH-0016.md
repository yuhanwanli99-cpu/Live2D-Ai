# BATCH-0016 · LLM 产生侧：max_tokens 默认值 / SSE 解码 / 流关闭时序

Phase 1 · 域覆盖 → `crates/live2d-ai-runtime/**`

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/llm.rs` — 530（读 1-325 + **枚举全部 15 条测试名**）
2. `crates/live2d-ai-runtime/src/sse.rs` — 361（读 1-200 + 枚举全部 14 条测试名）
3. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（读 130-279）
4. `crates/live2d-ai-runtime/src/config.rs` — （片段 15-40）
5. `crates/live2d-ai-runtime/src/settings.rs` — （片段 180-200 / 636-655）

## 跑过的命令（全部只读）
```
grep -rn "DEFAULT_MAX_TOKENS" crates/ --include=*.rs ; grep -rn "4096" crates/ --include=*.rs
grep -rn "effective_max_tokens" crates/ --include=*.rs
grep -rn "OpenAiClient::new|LlmConfig::new" crates/ --include=*.rs | grep -v "tests|#\[test\]"
grep -n "    fn |#\[test\]" llm.rs sse.rs
sed -n '185,200p;636,655p' settings.rs ; sed -n '15,40p' config.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批结论：**无新发现**（诚实记零），但**关闭了两条红线**

### 红线 3「思考与正文共用 `max_tokens`，默认 4096」—— **端到端验证通过**
完整取值链（逐跳都有原文）：
1. `DEFAULT_MAX_TOKENS: u32 = 4096`（settings.rs:187）——**全仓唯一定义**
   （`grep -rn "DEFAULT_MAX_TOKENS"` 的定义命中只有这一处，其余都是 re-export / 使用）；
2. `LlmSettings::effective_max_tokens()` = `self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS)`（settings.rs:192-194）；
3. `ResolvedSettings.llm.max_tokens = self.llm.effective_max_tokens()`（settings.rs:644）；
4. `LlmConfig.max_tokens: u32`，字段注写明「已由 `effective_max_tokens` 解析过默认值，
   所以这里拿到的是**最终生效值**」（config.rs:21-23）；
5. 上线： `chat_request_body(..., self.llm.max_tokens)`（llm.rs:302），
   `#[serde(skip_serializing_if = "is_zero_u32")]`（llm.rs:131-132）——`0` 才省略字段。
6. 守门测试两处：`settings_tests.rs:438` `assert!(DEFAULT_MAX_TOKENS >= 2048, ...)`（改回 512 会红）；
   `llm.rs:431` `wire_request_omits_max_tokens_only_when_unlimited`（直钉「什么时候才省略」）。
⇒ **toml 省略 → 4096 上线**；显式 0 → 省略字段（= 不限制，走上游默认）。历史缺陷
「512 时正文被挤成半句 → 一个字都不上屏」的成因链已完全闭合且有守卫。

### 红线「事件不得丢失（尤其收尾事件）」—— **SSE + 流关闭路径验证通过**
- `SseDecoder::finish()`（sse.rs:70-85）只取「残余缓冲当一行 + 一个 pending 事件」，
  但这**不丢数据**：`pop_line`（:88-115）保证 `buf` 里至多剩**一条无终止符的半行**，
  `pending_data`/`pending_event` 是单槽 ⇒ `Option` 返回值在结构上够用；
- 截断的 JSON 不会被静默吞：`finish` 派发半条 → `map_sse_data` 的 `serde_json::from_str`
  失败 → `Err(Error::Sse{ message 含 96 字 data 片段 })`（llm.rs:195-204）——**变成可观测错误**；
- `advance()` 的 `Ok(None)`（关流）三分支逐个验过（llm.rs:267-281）：
  已见 `[DONE]` → 不补发；残余事件 + 未见 `[DONE]` → **先把残余入队、Done 压队尾**（顺序正确）；
  无残余 + 未见 `[DONE]` → 立即返回 Done。三种情况都恰好一次 `Done`。
- 守卫测试名全部实义：`sse.rs:322 arbitrary_chunk_cuts_produce_identical_results`、
  `:237 crlf_split_across_pushes_is_not_two_lines`、`:285 utf8_multibyte_split_across_chunks_survives`、
  `:306/:314 finish_flushes_*`。**零 Pattern B。**

### 引擎取消纪律（engine.rs 130-279）—— 验证通过
- `worker_cancel = cancel.child_token()`（:171）：致命错误只中止自己的 worker，不碰父令牌 ✔；
- 阶段 0（:211-233）与阶段 1（:236-251）都用 `tokio::select!` 同时看 `cancel.cancelled()` ✔；
- **`debug_assert!(llm_finished, ...)`（:545）是可靠的**：`'llm` 循环的 8 个 `break` 点
  （:241 / :248 / :269 / :305 / :351 / :367 / :372 / :386）**每一个**都设置了
  `cancelled` / `llm_finished` / `llm_failed` / `tx_closed` / `queue_dead` / `enqueue_failed` 之一，
  而 :539 又重算 `cancelled = cancelled || tx_closed || cancel.is_cancelled()`
  ⇒ 走到 Completed 分支时 `llm_finished` 必为 true，不会在 release 下被绕过。

## 本批主动推翻的假设（4 次，全部有据，防下批重走）
1. 「`advance()` 的 `Ok(None)` 分支里 `!self.queue.is_empty()` 是死代码」——**证伪**：
   到达 `chunk()` 时队列为空，但紧接着 `self.queue.extend(mapped)` 把 `finish()` 产出的
   残余事件**放了进去**，所以该分支**可达且必要**（它正是保证「残余先于 Done」的那条）。
   （我先写下了「死代码」的判断，重读 `extend` 之后自己推翻。）
2. 「`finish()` 返回 `Option` 会丢掉关流时的多条残余事件」——**证伪**（见上）。
3. 「`LlmConfig::new` 把 `max_tokens` 设 0，生产路径可能绕过 4096 默认」——**证伪**：
   `grep` 显示全部 `LlmConfig::new` 调用点都在 `#[cfg(test)]` / doctest / 文档注释里；
   唯一的生产构造是 `supervisor.rs:406` `OpenAiClient::new(resolved.llm.clone(), ...)`。
4. 「`debug_assert!(llm_finished)` 可能被 release 静默绕过」——**证伪**（见上，8 个 break 全带标志）。

## 未核实项
1. `llm.rs:326-530` 的测试体未逐行读（只枚举了 15 条测试名）；测试**能否失败**只按命名与断言形态推断。
2. `sse.rs:200-361` 的测试体未逐行读。
3. `engine.rs:1-130`（模块文档 + `build_messages` + `commit_completed_turn`）未读。
4. `conversation/mod.rs`（548）与 `conversation/error_code.rs`（249）未读——后者是
   「后端 tracing 的 code == WS 帧 code」这条红线的实现地，**顺延 BATCH-0017**。

## 本批新增
**0 条发现**（P0/P1/P2/P3 全零）。按 §反空转规则，下批**换轴**：
从「LLM 产生侧 / 红线验证」切到「**错误码契约面**（`conversation/error_code.rs` + `error.rs`）」——
那是本仓第 6 条红线（后端 tracing 与 WS 帧必须是同一个 code 字符串）的实现地。
