# BATCH-0043 · 收尾 `live2d-ai-core`：performance/ + state 侧的契约核验

Phase 1 · 域覆盖 → `crates/live2d-ai-core`（**本目录收尾**）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-core/src/performance/mod.rs` — 349（读 1-105 + 定点 89-91 / 267-324）
2. `crates/live2d-ai-core/src/action/mod.rs` — （B0042 已读，复用）

## 跑过的命令（全部只读）
```
grep -n "envelope(" core/src/performance/mod.rs
grep -n "attack|release_from" core/src/performance/mod.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 维度覆盖（本批）
- 红线 O（口型归属）：**本层永不驱动嘴开合**，「`MouthOpenY` 归 TTS RMS 通道」——
  休眠层与高频通道的**所有权边界被写死**，即使将来唤醒也不会打架
- 维度 A（真源）：`ParameterMask` 的语义是「释放所有权」而非「写零压住 idle/物理层」（:95-101），
  这是比「写中性值」更正确的所有权模型
- 维度 C：`envelope` 的除零风险 —— **证伪**

## 未核实项
1. `performance/player.rs`(72) 未读（`play`/`interrupt` 时间轴）
2. `action/rules.rs`(254) 未读（`rule_fallback`）
3. `state.rs`(83) / `ids.rs`(71) 未读
4. `core_tests/{latches,actions}.rs`(386) 断言体未读
5. `tests/root_flow.rs`(336) 未读

## 本批新增
**0 条**（净产出：口型所有权边界核验 + 除零假设证伪 + 模式 J「休眠层的契约仍然有效」的首个样本）
