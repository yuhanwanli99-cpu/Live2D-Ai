# BATCH-0139 · `engine.rs` 阶段 1.5/4：**6 路门控 + `debug_assert!` 把「Completed 的前提」钉住**

Phase 1 · 域覆盖 · `conversation/engine.rs`（阶段 1.5 表演层 / 阶段 4 兜底 / 结局归类）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（定点 393-416 阶段 1.5 门控 · 539-548 结局归类）
2. `shell/flutter/lib/chat/turn_liveness.dart` — 200（定点 70-97：`settleTurn` 全文 + `mustReleaseTurnOnWsLoss` 头注）

## 跑的命令（全部只读）
```
grep -n "TextFallback" -B 12 -A 10 conversation/engine.rs
grep -n "settleTurn" shell/flutter/lib/chat/*.dart
sed -n '70,97p' shell/flutter/lib/chat/turn_liveness.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；两处核验通过 + **一条如实标为「未核实」的跨层疑问**
### ① 表演层入口是 **6 路门控**，且注释**点名它与兜底路径互补**
```rust
if let Some(perf) = performance.as_ref()
    && !cancelled && !tx_closed && !llm_failed && !enqueue_failed && !queue_dead   // :406-412
```
注释（:404-405）：「只有「**表演层开着 + 正常流完 + 未取消 + 无致命**」才走这里；
**失败轮的正文兜底仍由阶段 4 的 `TextFallback` 负责**（那一轮不发 TTS）」

⇒ **两条路径显式互补**，不存在「失败轮两头都不管」的洞；且 `enqueue_failed` / `queue_dead`
这两个失败标志**都在门控里** ⇒ 「入队失败导致正文丢失」也被显式排除。

### ② 结局归类的第三臂**对自己的前提下断言**（正面）
```rust
let status = if cancelled { TurnStatus::Cancelled }
  else if llm_failed || enqueue_failed || queue_dead || worker_exit == WorkerExit::Failed { TurnStatus::Failed }
  else { debug_assert!(llm_finished, "非取消/非致命路径必须来自正常流耗尽"); TurnStatus::Completed };
```
⇒ 「**Completed ⇒ 流确实耗尽**」这条不变量在 **debug 构建里被断言**、在 release 里被写在消息里
⇒ 三臂里唯一「可能悄悄错」的那一臂，**不再是沉默的**。

### ⚠ ③ 一条**跨层优先级**的疑问：**记为未核实，不记发现**
- Rust（:540-546）：**`cancelled` 先判** ⇒ 同时「失败 + 被停止」的轮次报 **`Cancelled`**
- Dart（`settleTurn` :79-81）：**`failed` 先于 `stopped`** ⇒ 同一情形返回 **`failed`**
⇒ **表面上优先级相反**。**但我没有证据说它真的分歧**，因为：
  1. `settleTurn` 的**第一条**是「`text.trim().isNotEmpty` ⇒ 一律 `keep`」（:71-72「哪怕这一轮失败了、
     哪怕是被停止的：那是**真实收到的内容**，丢了就是篡改记录」）⇒ **只要收到过正文，两侧都不会走到那两个分支**；
  2. 真正的分歧只在「**无正文 + 失败 + 被停止**」这一窄情形，而那需要知道 Dart 侧 `failed`
     这个 bool **从哪来**（若它来自 WS `turn_state.status == "failed"`，则后端说 `cancelled` 时它本就是
     `false` ⇒ 两侧**一致**）——**我没能在本批定位到 `failed` 的赋值处**（两次 grep 均无命中）。
⇒ **按纪律：未能演示的分歧不记发现**，已写入 STATE「未核实」。
建议（若日后确认是真分歧）：**两侧的优先级应显式对齐并各写一条用例**（无正文 + 失败 + 被停止）——
这类「同一状态在两层各判一次」的地方，是**接缝缺陷的高发处**（与模式 C 同族）。
