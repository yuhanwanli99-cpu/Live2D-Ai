# BATCH-0138 · `engine.rs` 阶段 1（LLM 流式循环）：**D8 语义被逐字写明，取消感知贯穿发送路径**

Phase 1 · 域覆盖 · `conversation/engine.rs`（turn 管线主体）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（定点 228-282：阶段 1 的 `select!` 循环）

## 跑的命令（全部只读）
```
sed -n '228,282p' conversation/engine.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；三处语义核验通过
### ① 流中途出错的处置**逐字对应 D8 裁决**
```rust
Err(e) => {
    // 流中途出错（D8）：**已完整切句并入队的内容允许播完**；
    // **未封口残余不再 TTS**。**不取消 worker**。        // :256-257
    let _ = send_event(&event_tx, &cancel,
        EngineEvent::Error { epoch, ts_ms: now_ms(), kind: ErrorKind::Llm(e) }).await;
    llm_failed = true; break 'llm;
}
```
⇒ **发 `Error` 事件**（⇒ WS `error` 帧带码，B0015 核过两侧码同源）·
**不取消 worker**（已入队的句子播完）· 未封口残余不进 TTS
⇒ 与我在早前批次记下的 D8 裁决**逐字一致**。

### ② 「流耗尽」与「流出错」被**明确区分**（易错的两种收尾）
```rust
next = stream.next() => match next {
    Some(item) => item,
    None => { /* try_unfold 在 Done 之后恰好返回 None：流正常耗尽。*/  llm_finished = true; break 'llm; }
}
```
⇒ `llm_finished` 与 `llm_failed` 是**两个独立标志** ⇒
「正常收尾」与「中途失败」不会互相污染 ⇒ 结局判定（`TurnStatus`）因此可靠。
（注释 :246 还记了**为什么 `None` 意味着耗尽** —— 这是 `try_unfold` 的一个易误解点。）

### ③ 所有发送都走 **`send_event(&event_tx, &cancel, …)`**（取消感知）
⇒ 事件通道**满 + 无接收者**时不会把 turn 卡死 ⇒ 与 B0134 核过的
`tts_queue_capacity.max(1)` 属**同一族防御**（不让「容量/阻塞」变成隐性死锁）。
⇒ `assistant_text.push_str(text)` 累积全量正文，与 B0137 核过的
`TurnReport.assistant_text`（「**含未成句残余**」）契约对齐。

## 未核实项
1. `engine.rs` 其余 ~300 行未读（阶段 2/3：TTS 排空等待、`TextFallback` 触发、
   `TurnReport` 汇总、性能层 snapshot 注入点）
2. `supervisor/turn.rs` 余 530 行未读（Stage A/B/C 全文）
3. `conversation/mod.rs` 余 ~490 行未读（配置类型 / 自测）
4. 各 `EngineEvent` 处理臂**内部**未逐臂读
5. `worker.rs` 余 200 行 / `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
6. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
