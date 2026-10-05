# BATCH-0151 · 表演层「静默回退」：**降级在本层静默，在上一层类型化 + 计数**

Phase 1 · 域覆盖 · `live2d-ai-runtime/performance/`（`client.rs` 652 + `mod.rs` 478）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/client.rs` — 652（**读 1-38 头注全段** + 结构枚举）
2. `crates/live2d-ai-runtime/src/performance/mod.rs` — 478（定点 23-36 / 65-130：回退原因与统计面）

## 跑的命令（全部只读）
```
grep -n "pub fn |fn |retry|timeout|重试" performance/client.rs
sed -n '1,38p' performance/client.rs
grep -n "None =>|fallback|回退|clean_for_tts|degraded|note" performance/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**假设被证伪，且得到一个跨 4 处的一致形状**
### ① 头注自陈「失败 = `None`（静默回退）」—— 但**降级是可见的**
`client.rs:30-34`：
> 「连接失败 / 超时 / 非 2xx / 响应不是 JSON / `content` 缺失或空白 → 一律 `None`，
> 由 `PerformanceRuntime` 回退到 `speak=clean_for_tts(原文)` + 规则 cue。」

我以为这意味着「表演层失败**对用户不可见**」。**读 `mod.rs` 后证伪**：
```rust
//! 回退原因码见 [FallbackReason::code]，**计数与最近原因见 [PerformanceStats]**。   // :28
pub enum FallbackReason { … None => "performance_ok" … }                            // :68-88
pub fn is_fallback(&self) -> bool { … }                                             // :96-97
… fallbacks: AtomicU64,                                                              // :126
```
⇒ **本层返回裸 `None`，上一层记**类型化原因码 + 计数器 + 最近原因** ⇒ 降级**在状态面可见**。
⇒ 且 `FallbackReason::code` 的文档写明「稳定回退原因码（进日志与状态面；**不含正文**）」
⇒ **可观测的原因里排除了正文** ⇒ 与红线 R（「不进日志/状态面」）**同一条纪律**，
只是这次管的是**用户内容**而非密钥。

### ② ⭐ 由此提炼一个**跨 4 处一致**的形状（正面模式 P9）
> **降级在本层静默，在上一层类型化 + 计数。**
本审计已见 4 例：
1. `drain_residual_events`（B0134）：本层丢弃、上一层排空并警告
2. director 的 `staging`（B0106）：`DisabledStaging` 无能力、上一层 `degraded_note` 可见
3. `client.rs → PerformanceRuntime`（本批）：本层 `None`、上一层 `FallbackReason` + `PerformanceStats`
4. `verify_core_chain.py`（B0053）：探针**不可用**、但 job 状态是 **skipped 而非 passed**

⇒ 这就是本仓对「不要伪造绿灯」的一致答案：
**信号会 traveling（向上类型化 + 计数 + 稳定码），即使**局部返回值**是裸 `None`/裸 `false`。**
⇒ 与 F-0048/F-0049/F-0050 那三条**方向相反**：那三条是**信号没有出口**（CI 不调扫描器、
不构建产物、探针读错文件）；这里是**信号有出口**。**分界线很清楚：信号有没有被上一级接住。**

### ③ 顺带核到 `client.rs` 的两条降级策略（都写明边界）
- `:20-22` `Auto` 降级：**只有上游回 4xx**（典型「不认识 `response_format`」）才原地降级重发一次；
  **传输失败 / 5xx / 超时不重试**（那些不是「模型不支持 structured」）
- `:11-14` `stream=false` + `temperature=0`（要一份 JSON 不是流）；**body 只有 system+user 两条，
  没有 `reasoning_content` 字段，也不回灌上游思考** ⇒ 与红线「思考不进下游」一致

## 未核实项
1. `client.rs` 余 ~600 行未读（`request` 实现、structured 降级、wire 回归的测试体）
2. `mod.rs` 余 ~350 行未读（`PerformanceStats` 的对外暴露面、Budget 的注入点）
3. `llm.rs` 余 ~360 行未读；`config.rs` 未读；`sse.rs` 余段未读
4. `plan.rs` 余 700 行未读
5. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
