# BATCH-0157 · `parse_v1` 的校验顺序与硬不变量 —— 0 条新发现，外加**一条对「析取恒真」判据的精化**

Phase 1 · 域覆盖 · `live2d-ai-runtime/performance/plan.rs`（分段校验 · `PlanError` 全族 · cue 解析）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/performance/plan.rs` — 857（定点 579-632：`parse_v1` 全段）

## 跑的命令（全部只读）
```
sed -n '575,632p' performance/plan.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**校验顺序是有意的**，且不变量是**两个都承重的子句**
### ① 校验顺序：便宜的先做，且**错误都带定位信息**
| 顺序 | 检查 | 行 | 错误携带 |
|---|---|---|---|
| 1 | `speak` 共现 ⇒ **warn 而非失败** | :586-589 | `PlanWarning::SpeakIgnored`（O1） |
| 2 | `segments` 缺失 / 非数组 | :590-591 | 枚举 |
| 3 | **段数上限（O(1) 先拒）** | :592-597 | 带**实际段数** |
| 4 | 逐段 `as_str` | :601 | **段序号** |
| 5 | **总字符预算（循环后判）** | :602/:605-609 | 带**实际字符数** |
| 6 | **核心不变量** | :610-613 | `SegmentsNotPartition` |
| 7 | `cues` 上限 / 逐条 | :616-:632 | **序号 + 越界字段名** |
⇒ 「段数上限」放在**任何逐段工作之前** ⇒ 畸形输入 **O(1) 拒绝**
⇒ 与 B0122 核的 `apply_patch`「**合并后**再校验最终值」**同一形状**：先收窄，再对**最终状态**判定。

### ② ⭐ 硬不变量的**两个子句都承重**（⇒ 精化「析取恒真」判据）
```rust
// V1 核心不变量：**逐码点拼接恒等**；**空原文必须是空切分方案**。      // :610
if segments.concat() != source || (source.is_empty() && !segments.is_empty()) {   // :611
    return Err(PlanError::SegmentsNotPartition);
}
```
⇒ 看似第二个子句「被第一个蕴含」（`source` 空 ⇒ 拼接应为空）—— **但不然**：
`segments = [""]` 时 `[""].concat() == ""` **通过**第一个子句，
而这份 plan **声称有一段空文本** ⇒ **第二个子句堵的是这个真洞**，**不是冗余**。
⇒ ⭐ **由此精化本审计的「析取恒真」判据**（F-0001-02 `model_root.rs:111` 那条 P1 的形态）：
> **不要按语法判「或」是否恒真，要按「**去掉另一半会怎样**」判。**
> - `model_root.rs:111`：`A || !B` —— 去掉 `!B` 不改变任何行为 ⇒ **废命题**；
> - `plan.rs:611`：`A || B'` —— 去掉 `B'` 会放过 `[""]` 这类 plan ⇒ **承重**。
⇒ **同一种语法，一个恒真、一个承重** ⇒ **判据必须是「反事实测试」，不是「看起来像不像」**。
（这与 B0128 对 F-0127-01 的「范围收窄」同属**用更强的判据替换较弱的**。）

### ③ 逐条 cue 的错误**都带序号 + 越界值**
`:628-632` `Err(PlanError::UnknownField { index, field: other.to_string() })`
⇒ **既说「第几个」也说「错成了什么」** ⇒ 排障不需要猜。
另 `:602` `segment.chars().count()` ⇒ **按码点计**（与 B0127 核的 `chars().take()` **同一口径**）
⇒ CJK plan 不会被按字节虚高计数。

## 未核实项
1. `plan.rs` 余 ~590 行未读（`PlanError` 全族的 `code()/message()` 逐一、`json_schema_strict` 余段、
   `action_cue_payload`(851)、该文件自测）
2. `client.rs` 余 ~540 行未读（`request` 主体、structured 降级、wire 回归测试体）
3. `llm.rs` 余 ~200 行 · `sse.rs` 余 ~200 行 · `mod.rs` 余 ~350 行 · `config.rs:51-140` 未读
4. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
5. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
