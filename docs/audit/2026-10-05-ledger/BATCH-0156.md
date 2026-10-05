# BATCH-0156 · `[DONE]` 的「**恰好一次**」：一个 flag + 两种补发姿势，**0 条新发现**

Phase 1 · 域覆盖 · `live2d-ai-runtime/llm.rs`（SSE 消费侧 / `[DONE]` 判定）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/llm.rs` — 530（定点 6 头注契约 · 187-189 解析 · **248-283 消费侧** · 510 测试注释）

## 跑的命令（全部只读）
```
grep -n "DONE|\[DONE\]|llm_finished|Done" llm.rs
sed -n '248,283p' llm.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；「恰一次」纪律的**最小实例**核验通过
```rust
if self.done { return Ok(None); }                    // :252-255 【DONE】之后**不再消费底层连接**
…
Ok(None) => {                                        // 对端关流
    if let Some(event) = self.decoder.finish() {     // :270 先**兜底 flush** 解码器残余
        map_sse_data(&event.data, &mut self.done, &mut mapped);
    }
    self.queue.extend(mapped);
    if !self.done && !self.queue.is_empty() {        // :274 情况 A：残余已排进队列
        self.done = true; self.queue.push_back(Ok(LlmEvent::Done));      // ⇒ Done 排在残余**之后**
    } else if !self.done {                           // :277 情况 B：什么都没有
        self.done = true; return Ok(Some((LlmEvent::Done, self)));      // ⇒ 直接返回
    }
}
```
四个可核性质：
1. **`self.done` 是唯一闸** ⇒ 见过 `[DONE]` 后**不再读底层** ⇒ 不可能在 `[DONE]` 之后又产事件；
2. **补发前先 flush 残余**（:270）⇒ 关流瞬间缓冲里**可能还有未成事件的数据**（末句没跟空行）⇒ 先取出，**再**补；
3. **残余与 `Done` 的顺序被区分**：残余非空 ⇒ `Done` **入队尾**（数据先）；残余为空 ⇒ **直接 return**；
4. 两分支都以 `self.done = true` 收口，而 `map_sse_data` 收到真 `[DONE]` 时**也**置 `self.done`（:263 传 `&mut self.done`）
   ⇒ 真 `[DONE]` 已到时两个 `if` **都不进** ⇒ **恰好一次**。

### 与头注契约、与测试三方对齐
- 头注 :6「`LlmEvent::Done`：收到 `[DONE]`；**对端未发 `[DONE]` 就关流时也兜底补发一次**」
  ⇒ `Done` 是**保证一定来的**（恰一次），不是「有就发」；
- 测试注释 :510「收尾必定返回 `Done`（**恰一次**），所以**不能断言「空」**——要断言的是……」
  ⇒ 断言的是**恰一次**而非「至少一次」⇒ 模式 H 的硬形态。

⇒ ⭐ 归入同族：**core 双闩锁**（B0040「终态恰一次」）· **Stage C 的 `GenerationFinished` 落闩恰一次**（B0143）·
本批的 **`Done` 恰一次** ⇒ 三处是**同一纪律在三个层次上的实例**（状态机 / 编排 / 传输），
且**每一处都有自己的唯一闸**（闩锁 / `saw_fatal_kind` 落闩 / `self.done` flag）。
⇒ 另 :259 又是 **P5**（部分投递不得静默截断）：「关键：任意切割的字节先进缓冲，再按『完整事件』粒度取出」。

## 未核实项
1. `llm.rs` 余 ~200 行未读（`ChatChunk` 解析、`Error` 映射、其余测试体）
2. `sse.rs` 余 ~200 行未读（消费侧封装）
3. `plan.rs` 余 ~700 行未读（分段算法 / `PlanError` 全族 / `action_cue_payload`）
4. `config.rs:51-140` 未读；`performance/client.rs` 余 ~540 行未读；`mod.rs` 余 ~350 行未读
5. `runtime/src/lib.rs` 的 `join_endpoint` 回归（:184）断言体未读
6. `_finishTurn` 是否幂等 · `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
