# BATCH-0221 · ⭐ 「**开了总闸但没接上**」在本仓被建模成**独立的可观测状态**（四处，形状各异）

Phase 1 · 域覆盖 · Mod crates 逐文件（15/61）—— `mod-memory/src/summary.rs`（摘要旁车 = 该 Mod 里唯一调 LLM 的路）

## 跑的命令（全部只读）
```
wc -l mod-memory/src/*.rs | tail -3
grep -n "degraded|Err(|warn|回退|不摘要" mod-memory/src/summary.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**一条跨 Mod 的语义观察**（正面）
### ① 「开了但没接上」是**带原因的独立状态**，不是一个 `false`
```rust
/// 装配结果：客户端 + 「**开了总闸但没接上**」的原因（**可观察 degraded**）。    // :126
pub degraded_note: Option<String>,                                            // :131
degraded_note: None,                                                          // :139  正常装配
pub fn degraded(reason: impl Into<String>) -> Self { degraded_note: Some(reason.into()) }  // :144-147
// 2. 开了但缺 base_url / model 或 URL 非法 -> Disabled + degraded_note；        // :155
```
四个可核点：
1. ⭐ **「用户开了但它用不了」是一个有名字的状态**（`degraded`），**不是** `enabled=false`
2. ⭐ **它有一个必须给理由的构造器** `degraded(reason)` ⇒ **写不出「降级了但不知道为什么」**
3. **`Disabled` 与 `degraded` 是两个不同结果**（:155 明确「开了但缺 base_url / model 或 URL 非法
   → Disabled **+** `degraded_note`」）⇒ **「关着」与「开着但坏了」在观测面上可区分**
4. `degraded_note` 会被读出去（B0102 已在 director 侧核过同名字段到达 `state_json`）⇒ **P9 闭环**

### ② ⭐ 而这是一个**跨 Mod 的语义**，**四处各有形状**
| Mod | 「没接上」怎么表达 |
|---|---|
| `external-input` | **`403 mod_disabled`**（B0005 核过停用门禁） |
| `director` | **`DisabledStaging` 空对象 + `degraded_note`**（B0106） |
| `persona` | `apply_settings` **返回 `false`**（B0110 核过「一等化 + 走私被删」） |
| `memory`（本批） | **`degraded_note` + 专用构造器**（:126-155） |

⇒ ⇒ **形状不统一，语义统一**
⇒ ⭐ 而**这正是「跨 Mod 边界的正确状态」**：Mod 是**独立 crate**（P25/B0217 已核「架构边界导致的有理由的重复」），
**它们不应该共享实现**；**该统一的是语义，不是类型**
⇒ ⇒ 与 B0152 的 `join_endpoint` 判据**同源**：**架构边界 ⇒ 允许形状不同，但要求语义可对照**。

## 未核实项
1. `summary.rs` 其余部分未读（请求构造、超时、冷却与保留轮数的**执行**侧；
   B0113/B0114 只读了 `config.rs` 的字段与 `strategy.rs` 的扣减口径）
2. `mod-memory` 其余：`store.rs` 已定点 · `summary_store.rs` 未读 · `tests.rs`(766) 未读
3. Mod crates 逐文件覆盖率（15/61）
4. `dev_tools_section.dart` 余面(1874) · `tokens.dart` 余 ~600 行 · `design_tokens_test.dart` 余 ~32 断言
5. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
6. **前端 `.dart` 183 未读（最大面）**
7. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
8. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
