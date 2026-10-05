# BATCH-0167 · ⭐ `PerformanceStats` **确有人读**（含降级值被回归钉住）—— P9 两端闭合

Phase 1 · 域覆盖 · `performance/mod.rs` + `engine.rs` + 两处测试面

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/mod.rs` — 478（定点 124-130 `PerformanceStats` 字段）
2. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（定点 85-86 `performance_stats()`）
3. `crates/live2d-ai-runtime/src/performance/tests.rs` — （定点 647-649 / 674 / 690 断言面）
4. `crates/live2d-ai-runtime/tests/conversation_engine_performance.rs` — （定点 265）

## 跑的命令（全部只读）
```
grep -rn "PerformanceStats|stats()|\.fallbacks|last_reason" crates/ --include=*.rs
grep -n "pub struct PerformanceStats" -A 6 performance/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**P9 的「信号有出口」在两端都核实了**
### ① 生产端：六个计数 + 最近原因（`mod.rs:124-130`）
```rust
pub struct PerformanceStats {
    plans: AtomicU64, fallbacks: AtomicU64, noops: AtomicU64,
    cue_turns: AtomicU64, speak_turns: AtomicU64, last_reason: AtomicU8,
}
```
⇒ `last_reason` 用 **`AtomicU8`** 存**枚举下标**而不是字符串 ⇒ **有界、免分配、可原子更新**（与 `code_suffix()` 用 `as_u16()` 同族的「用类型化的东西而不是文本」）。

### ② 暴露端：引擎**公开**这个 stats
`engine.rs:85-86` `pub fn performance_stats(&self) -> Option<Arc<PerformanceStats>>`
⇒ 不是私有字段、也不是只在模块内可见 ⇒ **上一层真的能拿到**。

### ③ ⭐ 消费端：**有读者，而且回归把「降级值」钉住了**
```
performance/tests.rs:647-649   assert_eq!(rt.stats().plans(), 1);
                              assert_eq!(rt.stats().fallbacks(), 0);
                              assert_eq!(rt.stats().last_fallback(), "performance_ok");
performance/tests.rs:690      assert_eq!(rt.stats().last_fallback(), "**performance_plan_invalid**");
tests/conversation_engine_performance.rs:265  assert!(eng.performance_stats().is_some());
```
⇒ :690 断言的正是**降级**那一侧的取值 ⇒ **「降级发生了」这件事被一条断言钉住** ⇒
**它不会悄悄腐烂成「没人看的计数」**。
⇒ ⇒ **P9 两端闭合**：① 有类型化原因 + 计数（B0151）② **确有人读，且有断言钉住降级值**（本批）。

### ④ 但**必须诚实标注「可见」的准确含义**（不夸大）
| 层 | 有无消费者 | 结论 |
|---|---|---|
| 模块内 → 引擎 | **有**（`performance_stats()` 公开） | P9 的「上一级接住」**已满足** |
| 引擎 → **HTTP / WS / UI** | **无**（`web_api` 里零命中） | **用户看不到**「表演层回退了 N 次」 |
⇒ ⇒ **不记发现**：`stats` 的作用是**可观测性基础设施**（给测试与未来 UI 用），
**把一个内部计数器接进 UI 是产品决策，不是纪律缺口**。
⇒ 但**记一句**：`P9` 的门槛是「**信号有没有被上一级接住**」—— 此处**接住了**；
**若要判「用户可感知」，则还差一跳**，而那**不是** P9 的要求。
（这一条是**对 P9 自身适用边界的精化**，避免 P9 被当成「一切降级都必须弹给用户」。）

## 未核实项
1. `mod.rs` 余 ~340 行未读（`PerformanceRuntime` 主体、`Budget` 注入点、自测）
2. `plan.rs` 余 ~560 行未读（`message()` 逐一、`action_cue_payload`(851)、自测）
3. `client.rs` 余 ~430 行未读（wire 回归**断言体**、`build_body`）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
