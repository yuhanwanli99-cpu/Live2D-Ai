# BATCH-0175 · ⭐ `auto_mode_degrades_to_prompt_on_4xx`：**本审计读过的最强的一条测试**（0 条新发现）

Phase 1 · 域覆盖 · `performance/client.rs`（wire 回归断言体 —— **runtime 收尾**）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（`auto_mode_degrades_to_prompt_on_4xx`
   断言体全段）

## 跑的命令（全部只读）
```
awk '/async fn auto_mode_degrades_to_prompt_on_4xx/,/^    }$/' performance/client.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；⭐ **一条测试同时钉住五件事**（且全是**外部可观察量**）
```rust
let (base, handle) = scripted_server(vec![(400, err), (200, ok)]).await;   // 4xx → 200
let reply = client.request("SYS","USER",3_000).await.expect("降级后成功"); // ⇒ 降级**必须成功**
assert!(!reply.structured,  "第二次必须不带 response_format");            // ①
assert_eq!(raws.len(), 2,  "auto 必须先 json_schema 再 prompt");          // ②
assert!( raws[0].contains("\"response_format\""));                        // ③
assert!(!raws[1].contains("\"response_format\""));                        // ④
assert!( raws[1].contains("只输出 JSON"), "降级请求要带 JSON-only system");// ⑤
```
| 断言 | 钉住什么 | 什么样的错实现会红 |
|---|---|---|
| ① `!reply.structured` | 降级这件事**能被调用方看见** | 返回值不报状态 ⇒ 观测面失明 |
| ② `raws.len() == 2` | **恰好**两个请求 | 多一次重试 / 不重试 |
| ③ `raws[0]` **带** `response_format` | 第一次**确实是 structured** | 一开始就降级 |
| ④ `raws[1]` **不带** | 降级的**全部意义** | 只改标志不改请求 |
| ⑤ `raws[1]` 含「**只输出 JSON**」 | 重发**带上了替代指令** | 只翻 flag、没换 system |

### ① ⭐ 它**闭合了 B0165 留的那个口**
B0165 我记下 `:327` 的降级是「**语义性**的」（第二次传 `SYSTEM_JSON_ONLY` 而非原 `system`），
并写明「**除非有测试，否则那只是一条设计主张**」⇒ ⇒ **有**：
**⑤ 断言的正是第二个请求的线上正文里含有那句指令。**
⇒ **B0165 的主张从「设计意图」升级为「被测试钉住的行为」。**

### ② ⭐ 它同时是 **B0164 那个保守谓词的端到端正例**
测试构造的 4xx 体是：
`{"error":{"message":"response_format is not supported","type":"invalid_request_error"}}`
⇒ 含 `"error"` ✓ **且** 含 `"type"` ✓ ⇒ **正好命中 `client_error_4xx` 的两个必要条件**
⇒ 而 B0166 已核「状态码**没被传出**，所以只能嗅探」⇒ **这条测试证明那个嗅探谓词在真实形状上有效**
⇒ 即：**B0164 的「保守 + 声明误判代价」不是孤立的，它有一条端到端的正例钉着。**

### ③ 而它与 B0174 是**同一手法的两次应用**
B0174：「5xx 不得重试」用 `len() == 1` 钉住；本批：「先 schema 再 prompt」用 `len() == 2` +
**两次请求各自的正文**钉住 ⇒ **两者都是「数真实请求 + 读真实请求体」**
⇒ 正面模式 **P7 最强形态**（对照 = 线上真实发生的次数/内容）**已有 2 个样本**。

### ④ 顺带：三个 wire 用例覆盖**正常 / 降级 / 回落**三态，各有独立回归
`openai_client_posts_json_schema_and_parses_content`（正常）· 本批（降级）·
`server_errors_and_blank_content_fall_back_to_none`（回落 + 不重试 + 思考不进下游）
⇒ **三态齐全**，与 B0117「正常 / 显式不清空 / 缺省」三向**同形**。

## 未核实项（`live2d-ai-runtime` 收尾最后清单）
1. `mod.rs` 余 ~180 行（`Resolution` 字段全文、余方法、自测）—— **本区最后一个整段空白**
2. `client.rs` 第三条（正常路）断言体未读；`build_body` 未读；`prompt.rs` 余段未读
3. `plan.rs` cue 解析后半（`BadAnchor` / `UnknownPreset`）与两文件自测未读
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. **`mod_count_is_five` 的断言体未读**（只核了函数名）
8. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
