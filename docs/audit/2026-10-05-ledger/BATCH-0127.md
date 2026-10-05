# BATCH-0127 · ⭐ **F-0127-01（P2）**：`plan.rs` 的 `speak` 静默截断（同函数内唯一例外）

Phase 1 · 域覆盖 · `live2d-ai-runtime`（LLM 请求构造面，F-0020-01 的另一端）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/plan.rs` — 857（API 枚举 + 定点 61-79 全部上限常量 ·
   489-530 `parse_plan` 头段 · 592-594 段数上限）

## 跑过的命令（全部只读）
```
grep -n "pub fn |const MAX|fn budget|fn trim" plan.rs
grep -n "MAX_SPEAK_CHARS|MAX_SEGMENTS" plan.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**F-0127-01（P2）**
**同一个解析函数里，三种上限、两种处置**：
| 上限 | 处置 |
|---|---|
| `speak` 文本 > 4,000 字符（`:514`） | **`chars().take()` 静默截断**（**从中间切**） |
| `cues` > 16（`:521-522`） | `Err(PlanError::TooManyCues)` |
| `segments` > 64（`:592-594`） | 报错 + 消息 |
⇒ **该文件自己的约定是「超限 ⇒ 告诉调用方」，`speak` 是唯一例外**；
且 `parse_plan` 的返回类型**没有告警通道**（对比 memory Mod 的 `warnings`，B0102）
⇒ 这种截断**结构上不可能**被上报。
**与 F-0020-01（P1）的关系**：那是 `MAX_TOKENS` 从请求侧截断**整个 plan**；
这里是**字段级**的第二次截断，**独立存在**，两者叠加症状相同（半句 / 一个字都不上屏）。
**定 P2**：默认配置下不易触发（需很大 `max_tokens` 或手写 plan），但一旦触发就是**静默的半句话**。
**建议**：改成与邻居一致的 `Err`；或至少让调用方拿得到「被截断」（加 `speak_truncated` 标记 / 告警通道），
且**不要从中间切**（或补省略标记，让「念到一半断掉」可归因）。

## 未核实项
1. `plan.rs` 其余 ~700 行未读（`PerformanceCue` / `PlanError` 全族 / `json_schema_strict` /
   `action_cue_payload` / 分段算法 :530-592）
2. **`json_schema_strict`(787) 未读** —— 它是给 LLM 的 **JSON Schema**，
   若 schema 里写了 `maxLength` 就可能让上游自己截断（**那是更好的修法**）⇒ **下批第一件事**
3. `llm.rs`(530) 未审；`client.rs`(652) 只在 B0013 定点过 `MAX_TOKENS`
4. `conversation/{engine,mod,worker}.rs` 未审；`sse.rs`(361) / `audio/*` / `error_code.rs` 未审
5. `secrets.rs` 10 条测试的断言体未读；CRLF 与不配对引号两形态无测试（B0126）
