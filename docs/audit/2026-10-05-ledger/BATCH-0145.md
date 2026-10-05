# BATCH-0145 · `turn.rs` 阶段 A：**「绝不 abandon」** —— ⭐ **turn 管线全部读完**

Phase 1 · 域覆盖 · `supervisor/turn.rs`（阶段 A：生成 + 即时 PCM 泵）—— **本管线最后一段**

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-desktop/src/supervisor/turn.rs` — 682（定点 200-249：阶段 A 的 `select!` 骨架）

## 跑的命令（全部只读）
```
sed -n '200,249p' supervisor/turn.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## ⭐ 本批产出：**0 条新发现**；**turn 管线 100% 读过**
### ① 「**绝不 abandon（D2 契约）**」—— 资源生命周期的第三处一致纪律
```rust
// B4-P0：通道关闭（句柄被 drop）与 Quit 同义；**摘臂后继续轮询 gen_fut 至自然返回，
// **绝不 abandon（D2 契约）**。                                        // :222-223
Some(ControlCommand::Reload) => {}                                   // :232 在飞 turn 收到重载 → 丢弃
```
⇒ 控制通道断开时**不丢弃生成 future**，而是**摘掉该臂后继续轮询到自然结束**
⇒ 引擎自己的收尾（`drop(job_tx)` + **`worker.await` 无条件 join**，B0141）**才真的会执行**。
⇒ **三处同一纪律**：① B0141 `worker.await` 杜绝泄漏 · ② 本处 `gen_fut` 绝不 abandon ·
③ 残余事件**排空**而非丢弃（B0133）
⇒ 统一性质：**每个 spawn 出去的 future / 每条打开的通道，都被驱动到一个确定的终态，
绝不留成悬空的**。

### ② 两处 `#[allow(unused_assignments)]` **都写了「静态分析看不见什么」**
- `:208-210` 「gen 臂是唯一 break 出口且必先赋值；**跨 select 静态不可见**」
- `:211-213` 「写（None 分支）/读（守卫条件）**跨 select 迭代，静态分析不可见**」
⇒ **压制 lint 时说明「为什么分析器看不到」**，而不是随手 `#[allow]`。

### ③ `biased` 的**饿死风险被显式处理**
`:240-244`：`finish_closed = true, // **摘臂防 biased 饿死**`
⇒ `biased` 让控制/事实通道优先于 20ms tick，但若事实通道持续有值会**饿死 tick** ⇒
**通道关闭即摘臂** ⇒ 三个臂（control / finish / tick）**都能饿死其余两个**，而这一处处理了。
（B0144 的阶段 B `select!` 同样 `biased`，控制通道第一。）

## ⭐ 里程碑：**turn 管线已 100% 读过**（B0132–B0145，14 批，0 条缺陷）
```
engine::run_turn   阶段 1 LLM 流式(B0138) · 阶段 1.5 表演层门控(B0139) ·
                   阶段 2 关队列/等排空/退出原因(B0141) · 阶段 4 正文兜底(B0142) · 返回值(B0142)
conversation::worker  空句不发给定请求 + 双边界标记(B0132) · 格式门禁 · 取消优先(B0132/B0138)
supervisor::turn    阶段 A 即时泵(B0145) · 阶段 B 收尾泵(B0144) · 阶段 C 权威落闩 + 竞态同步关闭(B0143)
handle_engine_event 9 变体 9 臂无兜底(B0135) · drain_residual_events 自带「别用在这儿」警告(B0134)
事件面             ConversationUiEvent 7 → WS 7 投影 · AudioChunk 独立通道(B0136)
```
⇒ 覆盖的不变量：思考隔离 / 一句一单元（提示·协议·引擎·线上帧**四处对齐**）/ 句界由引擎给 /
    空末块发边界帧 / 残余不丢 / 退出原因参与终态 / 权威是报告 / 竞态在落闩前关闭 / 排空两层。
⇒ **14 批 0 条缺陷** ⇒ 这条管线的质量**高于本审计见过的任何一区**（与 `l2d`、`mod-system`
同档）；而它的**四次事故**（rc.3 N0、B3-P0-1、B3-P0-2、B4-P0）**全部有编号、有修复、有复审**。

## 未核实项（turn 管线之外）
1. `turn.rs:249-360`（阶段 A 的 tick 分支内部：`on_any_pcm_progress!` 宏体与 saw_fatal 的断链）与
   `:402-437`（阶段 B 的 select 其余分支）未读
2. `engine.rs` 仅剩「表演层 snapshot 注入点」（:406-416）未读
3. `conversation/mod.rs` 余 ~490 行、各处理臂内部、`worker.rs` 余 200 行、
   `llm.rs` 余 360 行、`client.rs` 余段、`plan.rs` 余 700 行未读
4. `_finishTurn` 是否幂等（B0140 留）· `secrets.rs` 10 条测试断言体未读 · B0120「chmod toml」未核
