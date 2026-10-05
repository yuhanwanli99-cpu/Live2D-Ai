# BATCH-0168 · `action_cue_payload`：**纯投影，不做决策** ⇒ 红线 O 的结构性主张站得住

Phase 1 · 域覆盖 · `performance/plan.rs`（收尾：`json_schema_strict` 余段 + `action_cue_payload`）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/plan.rs` — 857（定点 **850-857 `action_cue_payload`**；
   837-848 `json_schema_strict` 尾段）—— **本文件主体至此读完**

## 跑的命令（全部只读）
```
sed -n '840,857p' performance/plan.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐ 核到「投影层**刻意不做决策点**」这一结构
```rust
/// 把 plan 的 cue 列表封成 WS action_cue 的 payload 形态。
pub fn action_cue_payload(epoch: u64, covers_upto_seq: u64, cues: &[PerformanceCue]) -> Value {   // :851
    json!({ "epoch", "covers_upto_seq", "cues": cues.iter().map(PerformanceCue::to_json).collect::<Vec<_>>() })
}
```
⇒ **三个字段 + 逐 cue `to_json`，无任何加工** ⇒ 它**不是决策点，是纯投影**。
⇒ 而验证**全部发生在两侧**：
| 侧 | 做了什么 | 核验 |
|---|---|---|
| **生产侧** `parse_plan` | 词表外 ⇒ `UnknownField` / `ExpressionUnknownId`；锚点/强度/ttl 越界 ⇒ 报错 | B0157 / B0128 |
| **消费侧** `FieldRuntime::frame` | **13 项 `ALLOWED_PARAMS` 白名单**（红线 O 的协议层） | B0015 |
| **中间** `action_cue_payload` | **什么都不做** | **本批** |
⇒ ⇒ **红线 O 的「结构性」主张因此站得住**：白名单**不会被中间层绕过**，
因为**中间层里根本没有能改写字段的东西**。

### ⭐ 与 B0166 的一条结构观察**互为镜像**（合起来更完整）
| 位置 | 类型造成的局面 |
|---|---|
| B0166 `post_once -> (bool, String)` | **状态码被丢掉** ⇒ 上层被迫写**启发式**去猜 |
| 本批 `action_cue_payload -> json!{…map(to_json)}` | **决策被留在两侧** ⇒ 中间层**无需**写任何判断 |
⇒ **统一成一句**：**类型决定决策落在哪里。**
⇒ 好的类型让决策**只出现在真正该出现的地方**（生产侧校验 + 消费侧白名单），
差的类型会把决策**挤到不该出现的地方**（靠嗅探响应体猜状态码）。

## 未核实项
1. `plan.rs` 剩余未读：**`message()` 逐一**（`PlanError` 的人类可读文案，与 B0161 的 `hint` 同族）+
   `PlanError` 全族定义 + 该文件自测 ⇒ **如实标注：本文件未 100% 读完**
2. `mod.rs` 余 ~340 行未读（`PerformanceRuntime` 主体、`Budget` 注入点、自测）—— **runtime 收尾最后一块**
3. `client.rs` 余 ~430 行未读（wire 回归**断言体**、`build_body`）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
