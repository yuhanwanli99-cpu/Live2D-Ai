# BATCH-0176 · ⭐ `Resolution` / `is_noop()` / `Debug` —— **runtime 最后一个空白读完**

Phase 1 · 域覆盖 · `performance/mod.rs`（`Resolution` + `is_noop` + `Debug`）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/mod.rs` — 478（定点 **187-232**：`Resolution` 五字段 + `is_noop`
   + `PerformanceRuntime` 字段 + **`Debug` 实现**）

## 跑的命令（全部只读）
```
sed -n '187,232p' performance/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；三处核验通过，**runtime 收尾**
### ① `Resolution` 五个字段，每个都写明**语义差别**（不只是类型）
```rust
pub speak:    Option<String>,        // v0/回退；**`None` = 不说**
pub segments: Option<Vec<String>>,   // v1；**`Some` = D22 一对一送 TTS，不再二次切句**
pub cues:     Vec<PerformanceCue>,   // 空 = **不动**
pub reason:   FallbackReason,        // （`None` **变体** = 表演层成功，B0151 核过）
pub structured: Option<bool>,         // 回退时 **`None`**
```
⇒ ⭐ `structured: Option<bool>` 里 **`None` ≠ `Some(false)`** ⇒
**「没试 structured」与「试了、走 prompt 路」是两个不同状态** ⇒ 正面模式 **P11（显式胜过推断）**的又一例。

### ② ⭐ `is_noop()` **按路径各取自己的字段** —— 而这**依赖** B0171 那个 `segments: None`
```rust
let no_text = match &self.segments {
    Some(segments) => segments.is_empty(),   // v1 看 segments
    None => self.speak.is_none(),            // v0 看 speak
};
no_text && self.cues.is_empty()
```
⇒ B0171 已核回退时给的是 **`segments: None`**（**不是 `[]`**）⇒
  **正是这个 `None` 让这里能分路径判定** ⇒ 类型选择与判定逻辑**互为支撑**。
⇒ ⭐ **B0168「类型决定决策落在哪里」的更小同形**：
  若 `segments` 是 `Vec` 而非 `Option<Vec>`，这里就得**再加一个「走的哪条路」的标志**才能分路径。

### ③ ⭐ 而 `Debug for PerformanceRuntime` 是红线 R 的**第五处**，形态**与前四处都不同**
```rust
f.debug_struct("PerformanceRuntime")
    .field("client",   &self.client.kind())          // **类型名**（不是实例）
    .field("enabled",  &self.client.enabled())        // **布尔**
    .field("allow_len", &self.allow.len())            // ⭐ **长度，不是内容**
    .field("timeout_ms", &self.timeout_ms)
    .field("mode",     &self.mode)
    .field("has_rule_fallback", &self.rule.is_some()) // **布尔**
    .field("stats",    &self.stats.to_json())         // B0172 核过脱敏
```
⇒ **七项，零项携带配置内容**：
  - `allow` 装的是**能力集**（**可能是用户起的表情名**）⇒ 只给**长度**；
  - `rule` 是个**闭包** ⇒ 只给**有没有**；
⇒ ⇒ 与前四处对比：① 端点层「不输出」② Mod「输出安全替身」③ 客户端「类型 + 常量」④ 状态面「结构上无处可放」⑤ **整段配置的 `Debug` ⇒ 逐项给「长度/布尔/类型名」**
⇒ **五种形态，同一条纪律** ⇒ 这是红线 R 最完整的一张图。

## 未核实项
1. `mod.rs` 余 ~90 行未读（该文件自测 + `new()` 构造体）—— **runtime 主体已读完**，
   但**未 100%**（自测体未读）
2. `client.rs` 第三条 wire（正常路）断言体 · `build_body` · `prompt.rs` 余段未读
3. `plan.rs` cue 解析后半（`BadAnchor` / `UnknownPreset`）与两文件自测未读
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. **`mod_count_is_five` 的断言体未读**（只核了函数名）
8. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
