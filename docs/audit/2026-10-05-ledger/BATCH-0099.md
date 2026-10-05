# BATCH-0099 · `field_map.rs` 外置覆盖表：白名单无法被突破

Phase 1 · 域覆盖 · 渲染面 `preset/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/preset/field_map.rs` — 459（定点 144-168 `field_allows` / `model_id_from_url`；
   244-352 `with_override_json` / `merge_field` / `parse_targets`）

## 跑过的命令（全部只读）
```
grep -n "fn with_override_json" -A 40 field_map.rs
grep -n "fn parse_targets" -A 32 field_map.rs
grep -n "fn field_allows" -A 24 field_map.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；红线 O 第三层保险在**内建 + 外置两条路径**上都成立
1. **两道独立闸**（`parse_targets` :340 / :345）：① `ALLOWED_PARAMS.contains` ② `field_allows`
   ⇒ 外部 `field_map.json` **无法**引入 13 条外的通道，**也无法**让 `body`/`head` 写五官通道
2. ⭐ **第 4 个「显而易见的实现会踩红线、他们选了更严的并写下为什么」样本**：
   `Field::Expression` 用**显式小词表**而非 `scale_class == Expression`，
   注释点名松写法会放进 **`ParamMouthOpenY`（口型）/ `ParamBreath`（呼吸）**
   ⇒ **正面模式 P3 待立**：「**更松的写法在哪里**」要写进注释（见 FINDINGS）
3. **补强 F-0098-01**：本批证明拒绝时**明确 push warning** ⇒ `mod.rs:58/:696` 的「静默降级」
   是**第三处**不成立；且正确描述需区分两条路径的形式（`field_map` 丢弃+warning vs
   `field_runtime` 发 `preset-dropped`/`applied+degraded`）

## 未核实项
1. `merge_expressions`（:271 调用）本体未读 —— 它是否**也**经过 `parse_targets`（推断：是，但未核）
2. `field_map.json` 是否真的存在于仓库（`assets/actions/`）未查 —— 若不存在，覆盖路径**在本产品里不可达**
3. `table.rs` 余段（`PresetTable::from_json` 校验 / `amplitude_limit` / `pack_limit`）未读
4. `field_runtime.rs:646+`（`frame` 后半段：写参数 / `expired` 后续）未读
5. `preset/tests/{packs,mod,assets}.rs` 断言体未读

## 本批新增
**0 条**
