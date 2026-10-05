# BATCH-0144 · `turn.rs` 阶段 B：**部分投递必须放回，不得静默截断**（同纪律第 4 例）

Phase 1 · 域覆盖 · `supervisor/turn.rs`（阶段 B：pending PCM 收尾泵）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/supervisor/turn.rs` — 682（定点 361-402：阶段 B 主体）

## 跑的命令（全部只读）
```
sed -n '361,402p' supervisor/turn.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；核到同一条纪律的**第 4 个实例**
```rust
const MAX_STALL: u32 = 250;   // 连续 5s @20ms 零进展 ⇒ AudioStalled 致命          // :364
while let Some(mut prep) = pending_pcm.take() {
    let r = audio.as_mut().map(|a| a.try_enqueue_prepared(&mut prep));
    // B3-P0-1：`WouldBlock{accepted}` 的剩余样本**必须放回**下一轮继续泵送，
    // 否则**尾段被静默截断**；仅 `Enqueued`/`None`（dry-run）才结束本 prepared。 // :377-378
    match r {
        Some(TryEnqueue::WouldBlock { .. }) => {
            if accepted_now > 0 { stall_ticks = 0; }   // 有真实进展 ⇒ 重置零进展计时
            pending_pcm = Some(prep);                   // :384 放回，下一轮继续泵
        }
        _ => break,
    }
```
### ① 部分入环 ⇒ **放回继续泵**，而不是当作完成
`:377-378` 的注释把**它防的缺陷**写明：「否则**尾段被静默截断**」
⇒ 这是同一条纪律的**第 4 个实例**：
| # | 位置 | 「部分/残缺」的东西 | 处置 |
|---|---|---|---|
| 1 | `worker.rs:113-117`（B0132） | 空句的音频块 | 发**带双边界标记**的空块，边界不丢 |
| 2 | `turn.rs:327-344`（B0133） | 生成返回后的**残余事件** | **补处理**（不丢） |
| 3 | `broadcaster.rs`（B0136） | 句界 | **由引擎给**，不用 epoch 推断 |
| 4 | **`turn.rs:377-384`（本批）** | **部分入环的 PCM 尾段** | **放回下一轮**继续泵 |

### ② 停滞判定的粒度是「**是否有真实进展**」，而非「是否完全成功」
`:381-383` `if accepted_now > 0 { stall_ticks = 0; }`
⇒ `WouldBlock` 但**接受了若干样本**算**有进展**；接受 0 才累加零进展计数
⇒ 5 秒（:364 `250 × 20ms`）真零进展 ⇒ `AudioStalled` 致命
⇒ **粒度恰当**：部分成功不算卡死，真卡死才会升级。

### ③ `select!` 带 `biased` 且控制通道在先 ⇒ **停止优先于一切**
`:389-391` `tokio::select! { biased; maybe_ctrl = control_rx.recv(), … }`
⇒ `Stop` / `Quit` / `Reload` 三种控制命令都在**第一顺位**
⇒ 与我在 B0138 核过的「`select!` 里 `cancel` 先判」**同一纪律**（取消必须先于业务）。
另 `:400-401` 「在飞 turn（Stage B 泵期）收到 `Reload`：**丢弃，留在 idle 重建**」
⇒ **配置重载不拽在飞的 turn**（有理由，不是漏处理）。

## 未核实项
1. `turn.rs` **阶段 A**（:200-360，生成 + **即时** PCM 泵）未读 ⇒ turn 管线**仅剩这一段**
2. `:402` 之后的 `select!` 其余分支（健康哨兵 / 停滞哨兵 / finish）未读
3. `engine.rs` 仅剩「表演层 snapshot 注入点」（:406-416）未读
4. `conversation/mod.rs` 余 ~490 行、各处理臂内部、`worker.rs` 余 200 行、
   `llm.rs` 余 360 行、`client.rs` 余段、`plan.rs` 余 700 行未读
5. `_finishTurn` 是否幂等（B0140 留）· `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
