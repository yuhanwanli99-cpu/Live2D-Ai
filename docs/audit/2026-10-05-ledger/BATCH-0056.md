# BATCH-0056 · preset/*：红线 P 渲染侧落点

Phase 1 · 域覆盖 → 渲染面最大未审块

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/preset/field_runtime.rs` — 953（读 1-40 头注全段 + 定点 526-575 / 788-808）

## 跑过的命令（全部只读）
```
grep -n "fn accept_cue" -A 45 preset/field_runtime.rs
grep -n "fn sync" -A 22 preset/field_runtime.rs
grep -n "ttl|fn tick|fn advance|fn evaluate|pub fn " preset/field_runtime.rs
sed -n '1,40p' preset/field_runtime.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（四项核验通过）
`accept_cue` 三个关键性质（跨 epoch 清全部字段 / `none` 撤销哨兵保 `prev` 可解析 /
未知 id 不静默而发 `dropped`）+ 时钟域切换 `dt` 归零。**红线 P 渲染侧落点核验通过。**
⭐ 另提炼一条**记账卫生判据**（见 FINDINGS 核验三）：写时核对过的行数声明至今都真；
跨多轮改动没人回头核的（`chat_routes.rs:52`、AGENTS.md `mod_count_is_three`）都已漂。

## 未核实项
1. `preset/mod.rs`(908) / `field_map.rs`(459) / `table.rs`(321) 未读
2. `preset/tests/` 3 个测试文件（packs 707 / fields 476 / mod 455）未读 ——
   **「先加后钳」「跨批 replace」「hold/ttl」三条语义是否有能失败的测试**未核（模式 H）
3. `ttl_ms` 到期移除的**执行点**未定位（`grep expire` 无命中，可能由帧循环的
   `advance/evaluate` 驱动，但本批未找到该方法名）
4. 「同 seq 重复帧」在该状态机里的处理未核（`PresetCueStore.replace` 用 map 天然吃掉同 seq 重复，
   字段版未见对应机制）

## 本批新增
**0 条**（净产出：红线 P 渲染侧核验 + 一条可复用的记账卫生判据）
