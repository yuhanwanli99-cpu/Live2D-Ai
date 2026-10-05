# BATCH-0039 · live2d-ai-core：休眠台账 + epoch 闸门

Phase 1 · 域覆盖 → **Rust 未审区**（`live2d-ai-core` 15 文件 / 3216 行，此前 0 审）

## 读过的文件（全部来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-core/src/lib.rs` — 83（**全读**）
2. `crates/live2d-ai-core/src/reducer.rs` — 272（读 1-115）
3. `crates/live2d-ai-core/src/events.rs` — （定点 88-118）

## 跑过的命令（全部只读）
```
git ls-files 'crates/live2d-ai-core' | xargs wc -l | sort -rn
sed -n '1,83p' core/src/lib.rs
sed -n '1,115p' core/src/reducer.rs
grep -n "fn epoch" -A 26 core/src/events.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 红线 R（休眠台账）：「谁休眠/为什么/谁能唤醒」**逐项可答**；含一条防误删的显式警告
- 维度 C（竞态）：**不变量 1「唯一权威闸门」在休眠路径上无旁路**（`Event::Action` 带 epoch 并在闸内）
- 架构：休眠由**三层**独立挡住，且第三层不依赖休眠本身

## 未核实项
1. `reducer.rs:115-272`（双闩锁完成判定 / stop 原子性 / action_playback_finished）未读——
   **不变量 3「turn 终态双闩锁」与不变量 5「stop 原子性」是本 crate 最核心的两条，都还没核**
2. `action/{mod,rules}.rs`(582) 未读——休眠代码本身
3. `performance/{mod,player}.rs`(421) 未读
4. `state.rs`(83) / `ids.rs`(71) 未读
5. `core_tests/`(686) 与 `tests/root_flow.rs`(336) 未读——**休眠台账的回归钉子在这里**
6. AGENTS.md 记的第四层（`main.rs::mod_count_is_three` 工厂数断言）未核

## 本批新增
**0 条**（净产出：红线 R 休眠台账 + 不变量 1 的端到端验证；三层防护的清点）
