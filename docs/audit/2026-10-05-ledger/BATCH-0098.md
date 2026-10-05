# BATCH-0098 · `field_map.rs` 白名单 + `table.rs` 的 `ALLOWED_PARAMS`

Phase 1 · 域覆盖 · 渲染面 `preset/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/preset/field_map.rs` — 459（**读 1-40** 头注 + `FACIAL_PARAMS` + import 面）
2. `crates/l2d-wasm-demo/src/preset/table.rs` — 321（定点 `ALLOWED_PARAMS` 定义，机械计数）
3. `crates/l2d-wasm-demo/src/preset/mod.rs` — 908（定点 58 / 692 / 696）

## 跑过的命令（全部只读）
```
sed -n '1,40p' field_map.rs
grep -n "ALLOWED_PARAMS" -A 18 mod.rs
python3 -c "…数 table.rs 里 ALLOWED_PARAMS 的 Param 条目…"     # 第一次对 mod.rs 跑失败（定义在 table.rs）
grep -rn "静默降级" preset/*.rs ; grep -rn "override_param" preset/field_runtime.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**F-0098-01（P3）**（文档不一致，**安全方向**）
`preset/mod.rs:58` 与 `:696` 说缺参数是「**静默降级**／不报错」，
而 `field_runtime.rs:28-29` 说「**缺参数不得静默** → 发 `preset-dropped`（或 `applied + degraded=true`）」。
**实现是「不静默」的那一方**（B0057/B0080 已核）⇒ **文档低估了保证**。
⇒ 三种后果：读者可能加**冗余兜底探测**、**不信任** `preset-dropped` 信号、同 crate 内两处矛盾。
⇒ **定 P3**（安全方向、不影响运行时行为），并**修正我自己的模式表**：
**模式 D 只跟踪了「注释高估」；实际有高估（危险）与低估（安全）两个方向，两者都会让读者误判行为。**

## 核验通过项（正面）
- `ALLOWED_PARAMS` **13 条**（合测试名），**不含 `ParamMouthOpenY`**（机械确认）
- 白名单是**单一真源**（定义在 `table.rs`，`mod.rs:692` 再导出，`field_map.rs:22` 消费）⇒ **无第二份清单**
- `FACIAL_PARAMS` 7 条：不含口型、不含 `ParamAngle*` / `ParamBodyAngle*`（V3）
⇒ **红线 O 的第三层保险**（协议侧结构禁止动作写口型），与 core 侧 `ParameterMask`（B0043）**同构**

## 未核实项
1. `field_map.rs` 余 ~420 行（`FieldMap::with_override_json` 的合并语义 / `expression_targets` /
   轴符号与量程）未读 —— **外置覆盖表能否突破白名单**是未核的关键点
2. `table.rs` 余段（`PresetTable::from_json` 的校验 / `amplitude_limit` / `pack_limit`）未读
3. `field_runtime.rs:646+`（`frame` 后半段：写参数 / `expired` 的后续用途）未读
4. `preset/tests/{packs,mod,assets}.rs` 断言体未读

## 本批新增
P0 0 · P1 0 · P2 0 · **P3 1**
