# BATCH-0172 · ⭐ `to_json` 的脱敏是**结构性**的：六个原子量，**密钥无处可放**

Phase 1 · 域覆盖 · `performance/mod.rs`（`to_json` 脱敏面 —— **runtime 收尾**）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/mod.rs` — 478（定点 124-130 `PerformanceStats` 字段 · 168-184 `to_json`）

## 跑的命令（全部只读）
```
sed -n '168,189p' performance/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；⭐ **红线 R 第四处兑现，且是其中最强的一档**
```rust
/// 状态面 JSON（**脱敏：没有正文、没有密钥**）。          // :168
pub fn to_json(&self) -> Value {                          // :169
    let structured = match self.last_structured.load(…) { 1 => Some("json_schema"), 2 => Some("prompt"), _ => None };
    json!({ "plans", "fallbacks", "noops", "speak_turns", "cue_turns",
            "last_fallback", "last_structured" })          // :175-183
}
```
⇒ 七个字段**逐一**核过：**五个是 `u64` 计数**，**两个是 `&'static str`**
（`last_fallback` 来自 `FallbackReason::code()` · `last_structured` 是两个字面量之一）
⇒ ① **无正文**：**没有字段能装文本** ⇒ 「没有正文」是**结构性**的，不是「靠不写」
⇒ ② **无密钥**：`PerformanceStats` 本身（:124-130）**只有六个原子量**，
**连一个 key 字段都没有** ⇒ **密钥在结构上无处可放** ⇒ 无论谁将来读它都安全
⇒ ③ 字符串字段是 **`&'static str`** ⇒ **不受用户内容影响**（与 B0160 `as_u16()`、
B0167 `AtomicU8` 同族的「用类型化的东西而不是文本」）

### ⭐ 四处兑现的**强弱排序**（这张表现在完整了）
| # | 层 | 机制 | 靠什么保证 | 核验 |
|---|---|---|---|---|
| 1 | 端点层 | `GET /api/v1/env` 永不回值 | **选择不输出** | B0009 |
| 2 | Mod 状态面 | 只回 `api_key_set` **布尔** | **选择输出一个安全的替身** | B0105 |
| 3 | 客户端错误面 | `ApiSecret` 类型 + `Debug` 只输出布尔 | **类型 + 选择** | B0153/B0155 |
| 4 | **表演层状态面** | `PerformanceStats` **只有六个原子量** | ⭐ **结构上无处可放** | **本批** |
⇒ **第 4 档强于前 3 档**：前三档是「**我们选择不输出**」，第四档是「**没有地方可输出**」
⇒ 与正面模式 **P10**（「不许 derive 出来」比「记得别打印」强）**同一形状**：
**让违规在结构上不可能，而不是靠记得。**

⇒ 另注：B0167 已核这个状态面**目前无生产读者** ⇒ 脱敏是**当前冗余**，
但**是正确的冗余**：类型**无论将来被谁读**都安全 ⇒ 冗余在**类型**上，不在纪律上。

## 未核实项（本批之后 `live2d-ai-runtime` 的剩余面）
1. `mod.rs` 余 ~180 行未读（`Resolution` 字段全文、`PerformanceRuntime` 余方法、自测）
2. `plan.rs` 的 `message()` 逐一 + 该文件自测未读
3. `client.rs` 余 ~430 行（wire 回归**断言体**、`build_body`）
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `config.rs:51-140` 未读
5. **`reqwest` 的 `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **未核，禁止运行**
6. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
7. **`mod_count_is_five` 的断言体未读**（只核了函数名）
8. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
