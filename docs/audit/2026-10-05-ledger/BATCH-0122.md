# BATCH-0122 · `apply_patch`：**先合并后校验**，三态逐字段穷举 —— 0 条新发现

Phase 1 · 域覆盖 · `live2d-ai-runtime`（老代码区，查内部）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/settings/patch.rs` — 780（定点 405-450 三态拆解头段 · 690-737 收尾校验 +
   结局归类 + `validate_base_url_strict` + `plan_atomic_write` 完整契约）

## 跑过的命令（全部只读）
```
grep -n "fn apply_patch" -A 40 runtime/src/settings/patch.rs
sed -n '690,737p' patch.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；两处正面 + **对 F-0121-01 的一次自洽校验**
### ① 校验在**合并之后**、对**最终状态**做（正面，且是容易写反的顺序）
```rust
// 合并后再做一次 base_url 合法性总校验（非空就是非法的硬约束；空串
// 视为「未配置」，会由 resolve 阶段在启动时报，这里允许通过以便
// 用户在 UI 上分两步：先清空再填新值）。                      // :693-696
if !next.llm.base_url.is_empty() { validate_base_url_strict("llm", &next.llm.base_url)?; }   // :697-699
```
⇒ 校验**合并后的 `next`**，不是**进来的 patch** ⇒ 即使当前值本身就非法、而 patch 没碰它，
**也会被拦下**。这是「增量补丁」最容易写反的地方，此处是对的。
`validate_base_url_strict`（:710-717）要求**绝对 http/https URL**，错误带**段名 + 原值**。

### ② 三态是**逐字段穷举**的，且 `changed` 只在**值真的变**时置位
`:414-438` 的 `llm` 段：`None => 整段显式清空`（**5 个字段逐个判、逐个可能置 `changed`**，
`max_tokens` / `show_reasoning` 清成 `None` 回落默认）/ `Some(fields) => 逐字段 Keep/Clear/Set`。
⇒ 故 `:702-708` 的 `changed → Updated / 否则 NoChange` **是准确的**（不是「收到请求就算变更」）。

### ③ ⭐ 对 F-0121-01 的**自洽校验**：那份契约其实**很完整**，缺的**只有**权限
`:719-737` 的 `plan_atomic_write` 文档是**五步完整契约** + 崩溃语义 + PID 并发说明 +
「失败保证不留半写文件」+ **「与旧实现的差异」清单**（tmp 路径形状 / 自动清理 / fdatasync / 拒绝空内容）。
⇒ **F-0121-01 的指控因此是精确的**：不是「文档缺失」，而是
**在一份高质量契约里恰好缺了「文件权限」这一项**，而那一项**恰好被一个调用方需要**。
⇒ 这也再次印证 CONSOLIDATION-12 §③：**老代码的缺陷多是「局部的具体疏漏」，
而不是「整体缺失」** —— 与 Mod 根那种「接缝问题」是两种形态。

## 未核实项
1. `patch.rs` 中段（tts / persona / performance / action 等其余段的三态）未逐段核
2. `secrets.rs` 其余方法（`parse` / `lookup` / `refresh_from_disk` / `known_keys` / `is_valid_key`）未审
3. `performance/plan.rs`(857) / `llm.rs`(530) / `conversation/{engine,mod,worker}.rs` 未审
4. `sse.rs`(361) / `audio/{resample,rms}.rs` / `error_code.rs` 未审
5. `SettingsView` ↔ `AppSettings` 的**字段对等性**（两侧各一份清单，类似 B0075 那次四方对账）未做
