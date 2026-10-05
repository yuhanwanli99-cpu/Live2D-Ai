# BATCH-0162 · 上游响应体片段**会到浏览器** —— **记入 STATE 候选池，不记发现**（无法证明触发）

Phase 1 · 域覆盖 · `error.rs` + `error_code.rs` 余段 · WS 投影侧交叉核

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/error.rs` — 216（定点 6/:27/:32/:110-119 片段截断 + :127-138 回归）
2. `crates/live2d-ai-desktop/src/app_event.rs` — （定点 62/:80/:95/:99/:279/:299：`message` 的**产生处**）
3. `crates/live2d-ai-desktop/src/web_api/ws/events.rs` — （定点 192-195：`error` 帧四字段）

## 跑的命令（全部只读）
```
grep -n "Display|fn message|body" runtime/src/error.rs
grep -rn "message:" desktop/src/web_api/ws/events.rs
grep -rn "AppEvent::Error|EngineEvent::Error" -A 8 ws/events.rs
grep -rn "message" desktop/src/app_event.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；但核到一条**可陈述的链路**，按纪律**进候选池**
### ① 链路（**四步全部核到行**）
```
Error::Status { status, body }                       error.rs:32
#[error("上游非成功状态 {status}: {body}")]          ⇒ **Display 含 body 片段**
  → app_event.rs:80   message: kind.to_string()      ⇒ **取 Display**
  → ws/events.rs:194  "message": err.message         ⇒ 进 WS `error` 帧
```
⇒ **上游响应体片段（截断后）会到浏览器。**

### ② 而这**不是意外**（有回归把它钉住）
`error.rs:127-138` `status_body_is_truncated_by_chars`：断言截断到
`MAX_SNIPPET_CHARS + 1`（含省略号）、`body.ends_with('…')`，且注释写
「**Display 可观测：状态码与片段都在**」⇒ **片段进 Display 是刻意的诊断选择**。

### ③ 为什么**不记发现**（三条，逐条）
1. **触发无法从仓库证明**：需要某个上游在错误响应体里**回显凭据**。
   本仓对此**没有任何证据**（既无反例也无正例）⇒ 按本审计纪律
   （**没有摘录/证据的不记发现**）⇒ **进 STATE 候选池**。
2. **片段有界**：B0160 已核（`chars().take(MAX_SNIPPET_CHARS)` + `…`）⇒ **不是流量/DoS 问题**。
3. **请求侧已被证明干净**：`Http` 变体的 `Display` 明确「只含 URL 与传输层描述——**不含请求头
   （密钥安全）**」（:6/:27）⇒ 我们**自己**不会把请求头（key）带出去。

### ④ 但**它值不值得改**（这是我能从仓库陈述的部分）
| 消费者 | **需不需要**响应体片段 | 理由 |
|---|---|---|
| **日志**（`tracing::error!`，B0015 核过） | **需要** | 「用户拿界面上的码去日志里搜」⇒ 日志是**排障现场** |
| **WS 帧 / 浏览器** | **不需要** | 用户要的是**码 + hint**（B0161 核过 hint 已是可执行处置）⇒ **片段是多余传输** |
⇒ ⇒ **不对称已经存在** ⇒ 修法是**投影层**的一行（`events.rs:194` 的 `message` 不带 body，
或改用不含 body 的摘要），**不必动 `Display`**（日志继续拿得到片段）。
⇒ 这也正是正面模式 **P2**（私有汇合）的形状：**「日志要的」与「前端要的」在投影层分家**。

## 未核实项（本批新增）
1. ⭐ **候选池（KL-2）**：上游响应体片段经 `Display` → WS `error` 帧到浏览器
   （`error.rs:32` + `app_event.rs:80` + `events.rs:194`）⇒ **触发需上游回显凭据，本仓无证据**
   ⇒ 建议的最小修法：**投影层**分家（日志留片段、WS 不带），**不动 `Display`**
2. `error.rs` 余 ~75 行（`Error` 枚举其余变体的 `Display`）未读
3. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
4. `client.rs` 余 ~540 行未读（`request` 主体、structured 降级、wire 回归测试体）
5. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `mod.rs` 余 ~350 行 · `config.rs:51-140` 未读
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
