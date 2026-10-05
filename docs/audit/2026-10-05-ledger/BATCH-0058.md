# BATCH-0058 · 帧循环实体 + 换模型资源侧

Phase 1 · 域覆盖 → 渲染面

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/l2d-wasm-demo/src/web/surface/render.rs` — 551（定点 245-275：`tick`）
2. `crates/l2d/src/model.rs` — 318（定点 69-100：`ModelHandle`）
3. `crates/l2d/src/renderer/model_core.rs` — （定点 84-104：`load_model` / `update`）

## 跑过的命令（全部只读）
```
grep -n "pub fn tick|fn tick" -A 30 web/surface/render.rs
grep -n "struct ModelHandle" -A 30 l2d/src/model.rs ; grep -n "impl Drop" -A 12 l2d/src/model.rs
grep -rln "fn load_model" crates/l2d/src/ ; grep -rn "fn load_model" -A 20 renderer/model_core.rs
```
未跑任何 cargo / flutter / trunk / pnpm 命令。

## 本批产出：**0 条新发现**（三项核验通过）
1. `tick` 的 dt 钳 [1/240, 1/15]s 防「切后台回来」巨步；`core.update` 失败 ⇒
   **`drop(st)` 后** `status()` 再 `return`，不空转（顺序正确，错了会 RefCell panic）
2. 帧循环**记录了被推翻的方案**（R4 60Hz accumulator：修好 240Hz 四倍速、引入「姿态每 4 帧跳一次」）
   ⇒ **第三处「记录推翻过的方案 + 实测理由」样本**
3. 换模型：`self.stack = PoseStack::from_loaded_model(loaded)` **赋值替换** ⇒ 旧 PoseStack
   （含**有状态**的 `physics_work` / `PhysicsEngine`）被 drop；`ModelHandle` 是纯 `Arc`、
   **无自定义 Drop**（正确的设计，不是遗漏）

**审计边界如实标注**：纹理 / wgpu 资源在**上游 crate `ayagami`**内，**不在本仓范围**，
本条对其**不作断言**；红线 N 的**时序侧**（B0055）与**本仓资源侧**（本批）均已核过。

## 未核实项
1. `preset/{mod,field_map,table}.rs`(1688) 未读
2. `web/surface/idle.rs`(270) 未读 —— **`idle.rs` 与 `pose_stack` 的 idle 层是否真是
   AGENTS.md 说的「两套机制」**（「删动作时绝不要连带删它」那条边界）**尚未核**
3. `l2d/src/asset/mod.rs`(571) / `renderer/{offscreen,renderer}.rs` 未读
4. `l2d-wasm-demo/src/preset/tests/{packs,mod,assets}.rs`(1436) 的断言体未读
5. 「同 seq 重复帧」在字段状态机的处理仍未核（B0056 起三批未结）

## 本批新增
**0 条**
