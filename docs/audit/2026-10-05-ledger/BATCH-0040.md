# BATCH-0040 · core 不变量 3/5 + 休眠台账锚点失效

Phase 1 · 域覆盖 → `crates/live2d-ai-core`

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-core/src/reducer.rs` — 272（**读 115-272，本批补齐**）
2. `crates/live2d-ai-desktop/src/main.rs` — 519（读 60-104 / 450-479）
3. `crates/live2d-ai-core/src/core_tests/*.rs` — （**枚举全部 20 个测试名**）
4. `crates/live2d-ai-core/src/events.rs` — （B0039 已读 88-118）

## 跑过的命令（全部只读）
```
sed -n '115,272p' core/src/reducer.rs
grep -rn "    fn |^fn " core/src/core_tests/*.rs | grep "fn "
grep -rn "action_request_is_dormant|mod_count_is_three|mod_count_is_four|factory_count" crates/ --include=*.rs
grep -rn "AVAILABLE_MOD_FACTORIES" desktop/src/*.rs
grep -rn "FACTORIES.len()|fn .*factory|工厂" desktop/src/main.rs
sed -n '60,104p' desktop/src/main.rs ; sed -n '450,479p' desktop/src/main.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 不变量 3（双闩锁）/ 4（单 active 动作）/ 5（stop 原子性）：**逐行核实通过**
- 「恰一次」的实现范式：靠**移除状态**而非布尔标志 ⇒ 单一真源（值得记的范式）
- 红线 R（休眠台账可核性）：**F-0040-01**

## 未核实项
1. `core_tests/` 的**断言体**未逐行读（只枚举了 20 个测试名）——按模式 H 的执行建议，
   下一个该问的是「**难形态**有测试吗」：`reused_ids_across_epochs_are_isolated_by_epoch_gate`
   与 `stale_events_of_every_kind_are_dropped_after_stop` 是否真能失败
2. `action/{mod,rules}.rs`(582) 与 `performance/{mod,player}.rs`(421) 未读
3. `state.rs`(83) / `ids.rs`(71) 未读
4. `tests/root_flow.rs`(336) 未读
5. F-0040-01 的反证 (c) 未完成：`docs/architecture/` 下的 Mod 文档是否已有澄清未读

## 本批新增
P0 0 · P1 0 · P2 0 · **P3 1**
