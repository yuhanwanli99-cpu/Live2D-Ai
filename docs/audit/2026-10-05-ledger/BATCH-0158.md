# BATCH-0158 · 错误码族：19 个**闭合**码全局唯一，但**扫描有覆盖缺口**（如实标注）

Phase 1 · 域覆盖 · 全 workspace 的错误码字符串（红线「错误码两侧同源」的下游）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/plan.rs` — 857（定点 307-311 / 434-449：`PlanWarning` 与
   `PlanError` 的**全部** `code()` 分支）
2. 全 workspace：`git ls-files crates` 下**全部 `.rs`**，正则扫 `=> "snake_case"`.

## 跑的命令（全部只读）
```
grep -n "Self::.* => \"performance" performance/plan.rs
python3 - <<'PY'   # 扫全部 .rs 的 => "…" 字符串，统计重复
PY
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；一个**成立但覆盖面受限**的结论
### ① 表演层 19 个码：形状统一、**逐条不同**
```
performance_plan_speak_ignored · performance_expression_unknown_id（PlanWarning，2）
performance_plan_not_json / not_object / missing_segments / segments_type /
  segment_not_string / segments_too_long / segments_not_partition / speak_type /
  missing_cues / cues_type / too_many_cues / cue_not_object / cue_field /
  unknown_field / bad_anchor / unknown_preset        （PlanError，17）
```
⇒ 全部 `<stage>_<layer>_<suffix>`，与 AGENTS.md 的 `code()` 契约（`<stage>_<suffix>`）**一致**，
`stage` 恒为 `performance` ⇒ 前端能按 stage 分流。

### ② 机械扫全 workspace：**24 个码字符串，零重复**
```
疑似码字符串（=> "snake_case"）：24 出现 / 24 去重 ⇒ 零重复
前缀分布：performance 21 · always/content/base 各 1（后者多为他类字符串，非错误码）
```
⇒ **对本次扫描命中的集合**，「拿界面上的码去日志里搜」**不会歧义** —— 一个码只对应一处。

### ⚠ ③ 但**必须标注覆盖缺口**（否则这个结论会被误用）
主链 `ErrorKind::code()` 的码（如 `llm_upstream_401`，B0015 已核其两端同源）
**没有出现在这次扫描里** ⇒ 说明它们**不是字符串字面量**，而是由 `format!` 之类**拼出来的**
⇒ **「全局唯一」这个结论对本扫描集合成立，不是对全系统成立。**
⇒ ⭐ 而这暴露了一个**值得记录的结构差异**（**记为观察，不记发现**）：
| 族 | 机制 | 基数 | 用户可搜性 |
|---|---|---|---|
| 表演层 `PlanError` | **闭合枚举**（19 条，各带字面量） | 固定 | 强（可枚举、可文档化） |
| 主链 `ErrorKind` | **开函数**（含**上游状态码**） | 随上游变化 | 仍具体（每次响应一个码） |
⇒ 两族**用不同机制实现同一条契约**（B0015 已核主链那一侧两端同源）。
这**不是缺陷**（`llm_upstream_401` 本来就应该是开集合），
**但它意味着「错误码清单」这份文档只能覆盖闭合族** ⇒ 若要写全码表，
得**分别**列举，不能靠一次全局 grep。

## 未核实项
1. `plan.rs` 余 ~560 行未读（`message()` 逐一、`json_schema_strict` 余段、`action_cue_payload`(851)、自测）
2. `client.rs` 余 ~540 行未读（`request` 主体、structured 降级、wire 回归测试体）
3. **主链 `ErrorKind::code()` 的实现未读**（本批只从 B0015 的结论间接知道其形态）⇒ **下批第一件事**
4. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `mod.rs` 余 ~350 行 · `config.rs:51-140` 未读
5. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
6. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
