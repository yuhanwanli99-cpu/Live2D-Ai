# BATCH-0042 · 休眠代码本体：action/

Phase 1 · 域覆盖 → `crates/live2d-ai-core`

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-core/src/action/mod.rs` — 328（读 1-120 + 定点 108-130 / 295-320）
2. 跨文件对照：`desktop/src/mod_registry.rs:288`（唯一生产引用）

## 跑过的命令（全部只读）
```
grep -rn "LlmTool" crates/ --include=*.rs
grep -n "priority|fn priority" -A 10 core/src/action/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 A（真源）：`ActionSource` 三档的优先级仲裁与模块头注逐字相符 ✔
- 维度 E（契约一致性）：**F-0042-01** —— 一个「活着但名字过期」的优先级档
- 与 F-0040-01 **同根因**（台账与代码演进不同步）的第三处症状

## 未核实项
1. `action/rules.rs`(254) 未读（`rule_fallback` 确定性文本规则）
2. `performance/{mod,player}.rs`(421) 未读
3. `state.rs`(83) / `ids.rs`(71) 未读
4. `core_tests/{latches,actions}.rs`(386) 断言体未读
5. `mod_registry.rs:288` 附近（HostChannels 构造）的完整上下文未读

## 本批新增
P0 0 · P1 0 · P2 0 · **P3 1**
