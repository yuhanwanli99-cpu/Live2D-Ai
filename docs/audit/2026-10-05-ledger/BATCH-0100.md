# BATCH-0100 · 覆盖表可达性 + 缺文件降级

Phase 1 · 域覆盖 · 渲染面 `preset/` 收尾

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/main.rs` — 962（定点 222-249：`load_field_map`）

## 跑过的命令（全部只读）
```
git ls-files | grep -i "actions/"
grep -rn "with_override_json" crates/ --include=*.rs
sed -n '222,249p' l2d-wasm-demo/src/main.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；并**第四次补强 F-0098-01**
- **可达性**：`assets/actions/` 只有 `preset_labels.json` + `presets.json`
  ⇒ **`field_map.json` 不在树里**，而 `with_override_json` 在**生产**被调（:236）
  ⇒ 覆盖路径是**给用户的扩展点**，**出厂缺它是常态**（头注已声明）
- **降级三层、每层带原因**（:230 缺文件 / :234 非 UTF-8 / :244-247 告警**条数与内容**上屏）
- **边界澄清**：此处 `fetch_bytes` 用**编译期常量** `FIELD_MAP_URL` ⇒ **B0091/F-0046-01 的顾虑不适用**
- ⇒ F-0098-01：**实际降级没有一条是静默的**（四处反例），`mod.rs:58/:696` 是**唯一**声称静默处
  ⇒ 该措辞不是「过时」而是**与实现相反** ⇒ 原判成立并加强

## 未核实项
1. `table.rs` 余段（`PresetTable::from_json` 校验 / `amplitude_limit` / `pack_limit`）未读
2. `merge_expressions`（:271 调用点）本体未读
3. `field_runtime.rs:646+`（`frame` 后半段）未读
4. `preset/tests/{packs,mod,assets,fields_map}.rs` 断言体未读（`fields_map_tests.rs:96-111`
   已见有覆盖测试，但断言体未读）
5. `FIELD_MAP_URL` 的具体取值未核（它指向 `/actions/field_map.json`？需确认不会与
   `asset_root` 之外的网络源相交 —— 与红线 K 有关）

## 本批新增
**0 条**（+ F-0098-01 第四次补强）
