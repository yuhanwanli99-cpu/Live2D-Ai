# BATCH-0143 · ⭐ `turn.rs` 阶段 C：**一个带对抗性评审编号的竞态，在权威落闩前被同步关掉**

Phase 1 · 域覆盖 · `supervisor/turn.rs`（turn 管线最后一块大未读面；本批读阶段 C）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/supervisor/turn.rs` — 682（**阶段标注与入口枚举** + 定点 438-479 阶段 C）

## 跑的命令（全部只读）
```
grep -n "阶段 [ABC]|Stage [ABC]|^pub(crate) async fn|^fn |^struct " supervisor/turn.rs
sed -n '438,479p' supervisor/turn.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ★ 本批产出：**0 条新发现**；核到**本审计读过最强的单处工程**
### ① 权威规则被**拆成四条互斥的规则**（无歧义）
```rust
// B3-P0-2：**事实权威必须与 root 一致**——声卡 fault/stall 是**内部致命**，
// 无论引擎自身返回什么都归一化为 Failed；仅用户取消才允许 Cancelled，
// 仅 LLM 中途失败才沿用引擎 Failed，**真实成功才 Completed**。      // :439-441
```
⇒ **四种结局各有判定来源**（音频 / 取消 / LLM / 真成功）⇒ 没有「引擎说什么就是什么」的默认路径。

### ② ⭐ 竞态被**带场景编号**地命名，并在**权威落闩之前**同步关掉
```rust
// 补强（**复审 P0-2 场景 B**）：生成期声卡 fault 可能**恰好卡在 gen_fut 完成的瞬间**，
// 20ms tick 观察**尚未置位** saw_fatal_kind 就到 Stage C——若此时直接沿用引擎 status
// 会**错误定格为 Completed**，**Stage D 补发 Cleared 也无法纠正 root 的权威 outcome**。
// 因此 Stage C 判定前**同步健康检查**一次（**原子读，零成本**）。          // :443-447
if audio.as_deref().is_some_and(|a| !a.healthy()) { saw_fatal_kind = true; audio_stalled = true; }  // :448
let outcome = if saw_fatal_kind || audio_stalled { Failed } else { gen_outcome(turn_phase.report.status) };
```
三个要素齐备：
- **窗口被精确描述**（"恰好卡在 `gen_fut` 完成的瞬间" · "20ms tick 观察**尚未置位**"）
  ⇒ 异步观察者（20ms 轮询）与同步判定之间的真实竞态；
- **为什么不能事后补**（"Stage D 补发 Cleared **也无法纠正** root 的权威 outcome"）
  ⇒ 因为 Stage C 正是**落闩点**（:461-465 把 `GenerationFinished` 喂进 `root`），
  **闩过就改不了** —— 这与我在 B0040 核过的 core 侧不变量 3
  （「turn 终态双闩锁…生成闩…**恰一次**」）**直接对上**；
- **修法带成本论证**（"原子读，零成本"）⇒ 不是「顺手加一次检查」。

⇒ **归入正面模式（新增样本）**：**竞态要用对抗性评审的编号记录，并在权威落闩前关闭**。
与 B0133 的「残余排空」、B0136 的「不再用 epoch 推断」同族 —— 都是**把「差一点就不对」的地方写成可复核的决策**。

### ③ 顺带：`turn.rs:8` 记着一次**被放弃的重构**
> 「本文件当前 533 行（≤1000 区间）。**曾尝试**把 Stage B `pump_pending` 与 …」
⇒ 与 B0039–B0042 核过的多处「记录推翻过的方案」同形（`+crt-static` 移除、`+crt-static` 两个叠加问题、
wasm 背景那次的「已证伪并回滚」）⇒ **本仓把「试过又撤回」写进代码，而不是只留在记忆里。**

## 未核实项
1. `turn.rs` 阶段 A（:200-360，生成 + 即时 PCM 泵）与阶段 B（:361-437，pending PCM 收尾泵）
   **未读** ⇒ 本批只覆盖阶段 C
2. `:400` 「在飞 turn（Stage B 泵期）收到 `Reload`：**丢弃，留在 idle 重建**」未读上下文
3. `engine.rs` 仅剩「表演层 snapshot 注入点」（:406-416）未读
4. `conversation/mod.rs` 余 ~490 行、各处理臂内部、`worker.rs` 余 200 行、
   `llm.rs` 余 360 行、`client.rs` 余段、`plan.rs` 余 700 行未读
5. `_finishTurn` 是否幂等（B0140 留）· `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
