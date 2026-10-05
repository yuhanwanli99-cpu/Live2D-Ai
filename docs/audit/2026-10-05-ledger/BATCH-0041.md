# BATCH-0041 · core_tests 断言体（难形态）+ 模式 B/H 第 4 次复查

Phase 1 · 域覆盖 → `crates/live2d-ai-core`

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-core/src/core_tests/mod.rs` — 300（定点 196-275：本批唯一深读对象）
2. `crates/live2d-ai-core/src/events.rs` / `reducer.rs` / `lib.rs` — 前批已读，本批复用

## 跑过的命令（全部只读）
```
grep -n "fn reused_ids_across_epochs_are_isolated_by_epoch_gate" -A 40 core/src/core_tests/mod.rs
sed -n '237,275p' core/src/core_tests/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 维度 J：**模式 B / H 第 4 次复查零命中**（含 5 类改法都会让该测试变红的证明）
- 维度 C：不变量 1（epoch 闸门）在「ID 复用」这一最难场景下被真断言覆盖

## 未核实项
1. `core_tests/{latches,actions}.rs`(386) 与 `tests/root_flow.rs`(336) 的断言体未读
2. `action/{mod,rules}.rs`(582) —— **休眠代码本体**，未读
3. `performance/{mod,player}.rs`(421) —— 休眠代码本体，未读
4. `state.rs`(83) / `ids.rs`(71) 未读

## 本批新增
**0 条**（净产出：一条最硬测试的判别力证明 + 模式 B/H 的第 4 次复查与校准结论固化）
