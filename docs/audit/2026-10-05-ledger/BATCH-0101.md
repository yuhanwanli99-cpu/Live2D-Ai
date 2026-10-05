# BATCH-0101 · **红线 K 的第三层核验：渲染面的 URL 常量全部同源**

Phase 1 · 域覆盖 · 渲染面（红线 K 收口）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/main.rs` — 962（定点 102-111：三个 URL 常量）

## 跑过的命令（全部只读）
```
grep -rn "_URL\s*:|const .*http|https://" crates/l2d-wasm-demo/src/ --include=*.rs | grep -v "^.*//"
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；红线 K 在**第三个独立层面**上成立
渲染面（`l2d-wasm-demo`，整个 `/render`）的 URL 常量**恰好三个**，**全部是同源绝对路径**：
```rust
pub(crate) const DEFAULT_MODEL_URL: &str = "/models/bai/runtime/bai.model3.json";  // :102
pub(crate) const PRESETS_URL:        &str = "/actions/presets.json";               // :107
pub(crate) const FIELD_MAP_URL:      &str = "/actions/field_map.json";             // :111
```
`grep "https://"` 在该 crate 的**非注释代码**里 **零命中**；也无 `http://`、无协议相对 `//`。

⇒ **红线 K 现在有三层互相独立的证据**：
| 层 | 证据 | 批次 |
|---|---|---|
| 源码（宿主 + 前端） | 12 个 URL 全注释/localhost；无外链 | B0010 / B0025 / B0026 |
| **源码（渲染面）** | **3 个常量全同源**（本批） | B0101 |
| 构建产物 | 由 `ignite.sh --check` 探测，但**探针文件选错**（F-0050-01） | B0049/0050 |

⇒ 同时**闭合了 B0100 的边界澄清**：`FIELD_MAP_URL` 是同源路径 ⇒ 不可能是外部源，
且 B0091 已证 `resolve_relative` **无法换 host**（`dir` 前缀恒在）
⇒ **连动态构造的 URL 也在同源内** ⇒ 渲染面**没有**任何出网路径。

## 未核实项
1. `preset/table.rs` 余段（`PresetTable::from_json` 校验）未读 —— 本批被红线 K 占满
2. `merge_expressions` 本体、`field_runtime.rs:646+` 未读
3. `l2d-wasm-demo` 其余未读面：`web/surface/{render,idle}.rs` 余段 · `stage_bg.rs`(295) ·
   `web/gpu.rs` · `tests/` —— 另 `release-build.yml`(225)（门禁面唯一未读 workflow）
4. **`crates/l2d-wasm-demo/src/preset/*` 覆盖率**：`field_runtime.rs`（4 段）· `field_map.rs`（2 段）
   · `mod.rs`（若干定点）· `table.rs`（仅计数）⇒ 按文件 4/8，按行约 **30%**

## 本批新增
**0 条**
