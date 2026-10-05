# BATCH-0102 · `PresetTable::from_json`：**每个字段都有闸 + 每个回退都有 warning**

Phase 1 · 域覆盖 · 渲染面 `preset/` 收尾

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/preset/table.rs` — 321（定点 20-55：`ALLOWED_PARAMS` / `amplitude_limit` /
   `pack_limit`；120-200 `from_json`；248-275 `parse_param_list`；277+ `warn_expression_shape`）

## 跑过的命令（全部只读）
```
grep -n "fn from_json" -A 38 table.rs
sed -n '158,200p' table.rs
grep -n "ALLOWED_PARAMS|amplitude_limit|fn spec_channels" table.rs
sed -n '30,55p' table.rs ; sed -n '248,290p' table.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**；红线 O 协议侧保险在**三条外部数据路径**上全部成立
`parse_param_list`（:248-275）对 `presets.json`（**在树里**，用户可编辑）的每条参数：
| 闸 | 行为 | warning |
|---|---|---|
| `value` 非数字 | 丢弃 | ✔「value 不是数字，已丢弃」 |
| **`!ALLOWED_PARAMS.contains(&pid)`** | **丢弃** | ✔「**通道外参数** {pid} 已丢弃」 |
| `!v.is_finite()` | 丢弃 | ✔「value 非有限，已丢弃」 |
| `v.abs() > pack_limit(kind,pid)` | **钳位** | ✔「幅值超上限 {limit}，**已钳位**」 |

`from_json` 本身同样逐项防御：空 id 跳过(:131) · **重复 id 保留先出现的**(:135) ·
非法 `kind` 跳过(:139) · `duration_ms` 校验 `is_finite() && >0` 并 **`.min(MAX_TTL_MS)`**(:147-158) ·
未知 `wave` 宽容回落 `Single`(:159-172) · morph 极不全 ⇒ **morph 失效回落普通包**(:196-200)。
⇒ **红线 O 协议侧保险的三条路径齐了**：① 内建表（编译期 `ALLOWED_PARAMS`）
② `field_map.json` 覆盖（B0099 两道闸）③ **`presets.json`（本批）**。

### 一处**结构差异**（记此备忘，**不记发现**）
字段路径有 `field_allows(field, param)`（B0099）做**字段↔通道**兼容性闸；
预设路径**没有**对应闸 —— `pack_limit` 只管**幅值**、不管**通道归属**。
⇒ 后果被限制在：用户自改的 `presets.json` 里，一个 **Motion（手势）包**可以声明一个**五官通道**
（`ParamEyeLOpen` 等，在 13 条白名单内），此时 `amplitude_limit` 给它的是 `HEAD_LIMIT`(30)
（:38-43 的两档策略：身 ≤10 / 其余 ≤30，**文档与代码一致**）。
**为何不记发现**：无安全影响、无数据丢失、需用户**自己改本地文件**、后果是**自己模型上的观感问题**、
且**白名单与幅值钳位两道闸都在**；而「字段↔通道」这一层的缺失更像**粗粒度取舍**而非缺陷。
**记此备忘**是为了：日后若有人问「为什么表情包不能被手势包借用通道」，答案在这里。

## 未核实项
1. `merge_expressions`（`field_map.rs:271` 调用点）本体未读
2. `field_runtime.rs:646+`（`frame` 后半段：写参数 / `expired` 后续用途）未读
3. `warn_expression_shape`（:277+）只读了开头与 `high_params` 一行，其余未读
4. `preset/tests/{packs,mod,assets,fields_map}.rs` 断言体未读
5. `l2d-wasm-demo` 其余未读面：`stage_bg.rs`(295) · `web/surface/{render,idle}.rs` 余段 ·
   `web/gpu.rs` · `release-build.yml`(225)

## 本批新增
**0 条**
