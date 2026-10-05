# BATCH-0134 · `drain_residual_events` 本体 + `engine::run_turn` 头段 —— **0 条新发现**

Phase 1 · 域覆盖 · `conversation` 引擎面 + supervisor 收尾面

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/supervisor/handlers.rs` — （定点 351-361：`drain_residual_events` **本体**）
2. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（定点 162-200：`run_turn` 头段；结构枚举）

## 跑的命令（全部只读）
```
sed -n '351,362p' supervisor/handlers.rs
grep -n "^pub enum |^pub struct |^    pub async fn |^    async fn |^pub fn " conversation/engine.rs
sed -n '162,200p' conversation/engine.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；两处核验通过 + 一处**危险用法被函数自己挡住的**设计
### ① `drain_residual_events` 把「**这个函数不能用在正常收尾**」写在**函数自己身上**
```rust
pub(crate) fn drain_residual_events(rx: &mut mpsc::Receiver<EngineEvent>) {
    // 上限防御：正常 close 后至多几个事件；异常时也不无限自旋。
    //
    // **只用于 stop/cancel 路径**（那时丢弃是对的：终止轮次的事实不再有意义）。
    // 正常收尾路径上不能丢——生成返回后仍可能有一批事件在通道里，必须走
    // [`handle_engine_event`]（见 `turn.rs` Stage A 末尾的排空循环）。
    for _ in 0..256 { if rx.try_recv().is_err() { break; } }
}
```
⇒ 这与 B0132 核到的**两个调用点**正好对上：`turn.rs:327-344`（正常收尾 ⇒ **不能**用这个函数，
要用排空循环 + `handle_engine_event`）与 `turn.rs:353-354`（停止 ⇒ **正是**用这个函数丢弃）。
⇒ **命名（`drain_` = 丢弃）+ 函数体自带「别用在这儿」的警告** ⇒
一个「在 A 上下文正确、在 B 上下文危险」的函数，**危险用法被函数自己挡住了**，
而不是靠调用点自觉。**这是本审计见到的最干净的一处 API 约束写法。**

### ② `run_turn` 的两处防御形状都很对
- **取消令牌按作用域派生**（:170-171）：
  「致命错误时可以只中止自己的 TTS worker，**而不触碰调用方的令牌语义**
  （父取消会自动传播到 child）」⇒ 引擎能收掉自己的 worker，**但没有能力**取消调用方的其它工作
  ⇒ **作用域最小化**，且理由写明。
- **容量在使用点夹紧**（:178-179）：`tts_queue_capacity.max(1)` 与
  `audio_chunk_samples.max(1)` ⇒ 配置写成 0 也不会造出**零容量通道**
  ⇒ 防御放在**消费点**（与 B0122 的「合并后再校验」、`plan.rs` 的钳位同一形状：
  **不指望上游一定给合法值**）。

## 未核实项
1. `engine.rs` 其余 ~390 行未读（`run_turn` 的 Stage A/B/C 主体、`build_messages`、
   终态顺序、`TurnReport` 汇总）
2. `conversation/mod.rs`(548) 未读（类型面 / `EngineEvent` 全族 / `TurnReport` 字段）
3. `supervisor/turn.rs` 其余 ~530 行未读
4. `worker.rs` 余 200 行未读；`llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
5. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
