# BATCH-0142 · `engine.rs` 阶段 4 + 报告返回：**「权威是报告，不是事件」**

Phase 1 · 域覆盖 · `conversation/engine.rs`（阶段 4 收尾 + 返回值）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（定点 548-585：阶段 4 正文兜底 +
   终态投递 + `TurnReport` 返回；**至此 `run_turn` 主体读完**）

## 跑的命令（全部只读）
```
sed -n '548,585p' conversation/engine.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；两处核验通过，且后者是**韧性设计**
### ① `TextFallback` 的触发条件精确，`Cancelled` 的排除**写了理由**
```rust
// 只在 `Failed` 发：`Cancelled`（用户按了停止）**不该再补一段文字**。            // :553
let fallback_text = crate::dialogue::clean_for_tts(&assistant_text);           // :556
if status == TurnStatus::Failed && !fallback_text.is_empty() { … }            // :557
```
⇒ 两个条件：状态是 `Failed` **且**清洗后正文非空。
⇒ 兜底正文**走与健康轮相同的清洗器**（:554-555「否则失败轮会把 (动作) / *舞台指示* / **
原样显示出来，**与健康轮不一致**」）⇒ 上屏口径跨成败路径**统一**（与 B0005/B0006 核过的
`clean_for_tts` 同源）。

### ② ⭐ 终态事件的投递**有上限**，而**权威是返回值**
```rust
// 终态兜底投递：消费端卡死时最多等 [`super::TERMINAL_SEND_TIMEOUT`]，之后放弃——
// supervisor 的**权威终态是 [`TurnReport`]**，**不依赖本事件送达**。          // :569-570
send_terminal(…).await;
TurnReport { status, assistant_text }                                         // :581-584
```
⇒ **消费端卡死不会丢掉本轮结局** —— 事件投递超时就放弃，但调用方**一定**拿到 `status`
⇒ 这是 B0137 核过的两通道设计（「结果在返回值、细节在事件」）的**韧性侧**：
连**结果**都不依赖通道送达。
⇒ 与 B0131 的 `TimeoutError` 处理（`engine.rs:229-230` 那条路径同样用 `let _ = send_event`）
**同一纪律**：**事件发不出去是常态，不是灾难；权威在别处。**

## 未核实项
1. `engine.rs` 仅剩「表演层 snapshot 注入点」（:406-416 的 `resolution` 消费与 segments 逐段送 TTS）未读；
   `run_turn` **其余流程已读完**
2. `supervisor/turn.rs` 余 530 行未读（Stage A/B/C 全文）—— **turn 管线最后一块大未读面**
3. `conversation/mod.rs` 余 ~490 行未读（配置类型 / 自测）
4. 各 `EngineEvent` 处理臂**内部**未逐臂读
5. `worker.rs` 余 200 行 / `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
6. `_finishTurn` 是否幂等（B0140 留）· `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
