# BATCH-0141 · `engine.rs` 阶段 2：**关队列 → 等排空 → 退出原因参与终态**（三个要点都对）

Phase 1 · 域覆盖 · `conversation/engine.rs`（阶段 2 收尾）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/engine.rs` — 586（定点 523-534 阶段 2；:141 顺带核到「播放排空」的另一层）

## 跑的命令（全部只读）
```
grep -n "job_tx|drop(job_tx)|worker.await|WorkerExit|排空|收口" conversation/engine.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；三个要点逐条核验通过
```rust
// ---- 阶段 2：关队列 → 等 worker 排空（**无条件 join，杜绝泄漏**）。          // :523
// 句子必须由 worker 正常排空播放；**未封口残余也不再封口送 TTS**。            // :525
drop(job_tx);              // :530 关闭队列：worker 排空剩余句子后自然退出
let worker_exit = worker.await.unwrap_or(WorkerExit::Failed);                   // :534
```
### ① 关队列 → 等排空，是「**已入队的句子照样播完**」的标准且正确的顺序
worker 的 `for job in rx` 在**所有 sender 丢弃且缓冲排空后**才结束 ⇒
`drop(job_tx)` 不会截断已入队的句子 ⇒ 与 B0138 核过的 D8（「已完整切句并入队的内容**允许播完**」）
**同一条决策**，且在**两处都写明**。

### ② 「**无条件 join，杜绝泄漏**」⇒ 没有游离的 tokio 任务
`worker.await`（:534）**一定执行** ⇒ turn 结束后**不存在**还在跑的 TTS 任务
⇒ 不产生「轮次已收口、后台还在出声」的越界现象。

### ③ worker 的**退出原因参与终态判定** ⇒ 排空途中的失败不被吞掉
:531 的注释 + :542 `|| worker_exit == WorkerExit::Failed`
⇒ 若 worker **在排空途中**因 TTS/解码错误退出，该轮仍判 `Failed`
⇒ 与「排空等待」这个容易变成「等完就当成功」的坑**恰好相反**。

### ④ 顺带澄清「**排空」有两个不同层**（不要混为一谈）
- **本处**：引擎的 **TTS 队列排空**（生成侧：PCM 块都交给 supervisor 了）
- `engine.rs:141` 的注释提到另一层：「由 supervisor 在「**真实播放排空** + 未取消 + 无设备故障」
  后显式调用」⇒ 那是**播放侧排空**，正是 B0040 核过的 core 侧**双闩锁**不变量
  3（「**生成结束 ≠ turn 完成**」）
⇒ **两处都要有**这一层：本处保证「PCM 都交出去了」，core 的闩锁保证「PCM 都播完了」。

## 未核实项
1. `engine.rs` 余 ~270 行未读（`TurnReport` 汇总体 / 表演层 snapshot 注入点 / 该文件自测）
2. `supervisor/turn.rs` 余 530 行未读（Stage A/B/C 全文）
3. `conversation/mod.rs` 余 ~490 行未读（配置类型 / 自测）
4. 各 `EngineEvent` 处理臂**内部**未逐臂读
5. `worker.rs` 余 200 行 / `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
6. `_finishTurn` 自身是否幂等（B0140 留）· `secrets.rs` 10 条测试断言体未读
7. B0120「是否别处 chmod 过 toml」未核
