# BATCH-0097 · `preset/*`：**结掉挂了 7 批的 `ttl` 执行点**，并证伪「两套 runtime」假设

Phase 1 · 域覆盖 · 渲染面 `preset/`

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/preset/field_runtime.rs` — 953（定点 592-646：`frame` 每帧方法）
2. `crates/l2d-wasm-demo/src/preset/mod.rs` — 908（定点 368/428/447/658/731-753：两套 runtime 的关系）

## 跑过的命令（全部只读）
```
grep -rn "ttl" preset/*.rs | grep -i "expire|remove|retain|<=|>=|now"
grep -n "PresetRuntime|FieldRuntime" preset/mod.rs
sed -n '592,646p' preset/field_runtime.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## ★ 本批产出：**0 条新发现**；**一个挂了 7 批的未核实项关闭**
### ① `ttl` 到期的执行点 = **`FieldRuntime::frame(wall_ms, sink)`（:596）**
```rust
/// 每帧：推进 ttl、兑现段锚点、写参数、发 `preset-applied` / `preset-expired`。
pub fn frame(&mut self, wall_ms: f64, sink: &mut impl PresetSink) -> FrameOutcome {   // :596
    …
    // 2. 非 hold 到点 → 移除自己的贡献 + `preset-expired`。
    let due = { let c = &self.states[idx].contributions[i];
                !c.hold && c.elapsed_ms >= c.ttl_ms };                              // :628
    if due { let c = self.states[idx].contributions.remove(i);
             out.events.push(AckEvent::from_contribution(AckCtx { kind: AckKind::Expired, … }));
             expired.push(c.seq); } else { … }
```
**逻辑正确，两处细节都对**：
- `!c.hold &&` ⇒ **`hold` 的贡献永不到点、也不发 `preset-expired`**
  （正是头注 :22-23 冻结的语义「`hold=true` 保持到下次指令、**不**到点回基准、**不**产生 `preset-expired`」）
- `remove(i)` 后 **`i` 不自增**（只在 `else` 分支 `i += 1`）⇒ 连续多个到点时**不会跳过元素**
  （这是「按下标删除」循环最经典的错，这里是对的）

### ② 我前 7 批为什么找不到它：**按名字猜，而不是穷举**
B0056/B0057 我依次试过 `expire` / `tick` / `advance` / `evaluate` 四个**猜的**方法名 ——
真名是 **`frame`**，不在我的猜测列表里。
⇒ **规则 3 第 6 个变体（正面查找方向）**：
> **按名字找东西时，猜名字等价于否定式断言。**
> 必须先**枚举该类型的全部方法**（`grep -n "    pub fn \|    fn "`），
> 再逐个比对；**「我猜的名字没找到」不能推出「它不存在」**。
> 与规则 3 变体 1–5 同根：**默认假设「我看到的就是全部」。**

### ③ 「两套 runtime 是同一职责的两个实现」—— **证伪，是有意并存**
`preset/mod.rs:658`：
> 「与旧的 `preset_id` 双槽（`PresetRuntime`）**并存**：旧路径**一字不删**（V11）。」
而 B0045 已核旧路径**仍然可达**：`l2d-wasm-demo/src/main.rs:507-533` 的 `preset` 分支
按 `payload.get("field")` 是否存在分流 —— **有** `field` 走 `FieldRuntime.accept_cue`（新），
**无**则走 `preset.resolve`（旧，兼容 V11 协议）。
⇒ **不是重复实现，是协议 v1 与旧协议的两条路径**，各有测试（`tests/fields.rs` / `tests/packs.rs`）。

## 未核实项
1. `preset/mod.rs` 余段（`PresetRuntime::resolve` 本体 :753+ / 外置表加载 / 撤销语义）未读
2. `preset/field_map.rs`(459) —— **通道白名单真源**未读（B0057 只从测试名知道它有「十三个」）
3. `preset/table.rs`(321) 未读；`preset/tests/{packs,mod,assets}.rs`(1436) 的断言体未读
4. `FieldRuntime::frame` 的后半段（写参数 / `expired` 的后续用途）未读

## 本批新增
**0 条**（+ 关闭 1 项挂了 7 批的未核实项 + 证伪 1 个假设 + **规则 3 第 6 变体**）
