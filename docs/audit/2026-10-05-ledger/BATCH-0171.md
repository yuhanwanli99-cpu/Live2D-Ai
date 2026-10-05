# BATCH-0171 · ⭐ `fallback()`：**计数的口径被写死**，含最容易被误优化的那个 case（0 条新发现）

Phase 1 · 域覆盖 · `performance/mod.rs`（`fallback()` 实现）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/mod.rs` — 478（定点 **367-390 `fallback()`** · 393-398 `clean_speak`）

## 跑的命令（全部只读）
```
grep -n "fn fallback" -A 30 performance/mod.rs
sed -n '367,398p' performance/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；⭐ 立**正面模式 P16**
```rust
fn fallback(&self, assistant: &str, reason: FallbackReason) -> Resolution {          // :367
    // 口径写死：**每一次没用上表演层 JSON 的轮次都计入 fallbacks**（**含关闸**）。
    // 想区分「关闸」与「开了但失败」看 [PerformanceStats::last_fallback] 的原因码。   // :368-369
    self.stats.fallbacks.fetch_add(1, Ordering::Relaxed);                             // :370
    self.stats.last_reason.store(reason.as_u8(), Ordering::Relaxed);                  // :371-373
    if reason.is_fallback() {
        tracing::warn!(target: "performance", code = reason.code(),                   // :375-379 ← **结构化 `code=`**
            "表演层回退：segments=[clean_for_tts(原文)] + 规则 cue（v0 回退口径）");
    }
    let speak = clean_speak(Some(assistant));                                         // :381
    let cues = self.rule.as_ref().map(|f| f(assistant)).unwrap_or_default();           // :382
    Resolution { speak, segments: None, cues, reason, structured: None }              // :383-389
}
```
### ① ⭐ **计数口径被一句话写死，且**点名了最容易被「优化」掉的 case**
「**每一次没用上表演层 JSON 的轮次都计入 fallbacks**（**含关闸**）」
⇒ 「关着」也计为回退 —— 这**恰恰是**一个后来者会「顺手修正」的判断
（「关闸是**预期**行为，凭什么计失败？」），而他们**写死并说明了**。
⇒ 并且**立刻给出想要更细信息的人的出路**：「想区分『关闸』与『开了但失败』**看 `last_fallback` 的原因码**」
⇒ **一个计数 + 一个原因码 = 两条信息**，不必为此加第二个计数器。
⇒ ⭐ **正面模式 P16（新）**：**给计数器写死「什么算一次」，并把最容易被误优化的那个 case 显式点名。**

### ② 降级**进了日志，且带结构化 `code=`**
:375-379 `tracing::warn!(target: "performance", code = reason.code(), …)`
⇒ 符合 AGENTS.md 的错误契约「`tracing`，**带 `code=` 结构化字段**」（本审计 B0001-01 记过
「pre-dispatch 端点零日志」那条 P1 的**对照面**）⇒ 且 `target` 可单独订阅。
⇒ 与 B0160（码**有界**，`as_u16()`）· B0161（码**带可执行 hint**）**接成同一条链**：
**降级有码 → 码可搜 → 码带处置。**

### ③ 三处形状细节都对
- **「是不是回退」由 `reason.is_fallback()` 判定**（:374）⇒ 判定**不在这层**（正面模式 P2）
- **host 未注入规则函数 ⇒ `cues` 为空**（:382 `.map(...).unwrap_or_default()`）⇒ **不崩、不强依赖**
- ⭐ `segments: None` 是**显式的**（:385）⇒ 回退形态**不是「空数组」**
  ⇒ 因为「空数组」在 v1 口径里另有一义（「本轮不做动作」，`plan.rs` cue 语义）⇒
  **用 `None` 而非 `[]`，避免「回退」与「不动」混淆** ⇒ 与 B0128 核的「`segments` 缺席 ⇒ 走旧
  `SentenceAssembler` 路径」**接得上**。

## 未核实项
1. `mod.rs` 余 ~200 行未读（`to_json` 脱敏面、`PerformanceStats` 余下方法、自测）
2. `plan.rs` 的 `message()` 逐一与自测未读
3. `client.rs` 余 ~430 行（wire 回归**断言体**、`build_body`）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. **`mod_count_is_five` 的断言体未读**（只核了函数名）
8. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
