# BATCH-0135 · ⭐ `EngineEvent` 的 9 个变体 ↔ supervisor 的 9 个处理臂 —— **完全对等，且无兜底臂**

Phase 1 · 域覆盖 · `conversation/mod.rs`（类型面）+ `supervisor/handlers.rs`（接缝）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/mod.rs` — 548（机械枚举 `EngineEvent` **全部变体**）
2. `crates/live2d-ai-desktop/src/supervisor/handlers.rs` — （机械枚举 `handle_engine_event` **全部处理臂**）

## 跑的命令（全部只读）
```
awk '/^pub enum EngineEvent/,/^}$/' conversation/mod.rs | grep -E "^    [A-Z]" | awk '{print $1}'
awk '/fn handle_engine_event/,/^}$/' supervisor/handlers.rs | grep -E "^\s+EngineEvent::" | awk '{print $1}' | sort | uniq -c
awk '/fn handle_engine_event/,/^}$/' supervisor/handlers.rs | grep -nE "^\s+(_|other|unknown|unsupported)"
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**把前几批的零散观察接成了一张完整契约表**
### ① 9 变体 ↔ 9 处理臂，**一一对应、零缺口、零兜底**
```
EngineEvent::{ TextDelta, ReasoningDelta, AudioChunk, SentenceReady,
                SentenceVoiced, TextFallback, ActionCue, Error, Terminal }   # 共 9
handle_engine_event 的臂：同名 9 个，各恰好 1 次                              # 无 `_ =>` 兜底臂
```
⇒ **两个推论，第二个比第一个重要**：
1. **没有任何变体被静默丢弃** —— 每一类事件都到达处理函数；
2. **将来新增变体会编译失败**（match 穷尽、无兜底）⇒
   **「加了一个事件类型、supervisor 忘了处理」这个失败模式在结构上不可能发生** ——
   它不靠测试、也不靠纪律，**靠类型系统**。
⇒ **这比 `DisabledStaging`（B0106）与 `FieldRuntime::frame`（B0097）更强一层**：
那两处保护的是**当前状态**，这一处保护的是**未来的改动**。
⇒ 可归入正面模式 **P3「建模优于加判断」** 的最强样本。

### ② 9 个变体各自对应的红线（前几批的核验点）现在**连成了一张表**
| 变体 | 对应的不变量 | 核验于 |
|---|---|---|
| `TextDelta` / `ReasoningDelta` | 思考**不进**装配器/TTS | B0009 |
| `AudioChunk` | 空末块**仍发边界帧**（`first/final_chunk` 都置位） | B0006 / B0132 |
| `SentenceReady` / `SentenceVoiced` | **一句一单元**（先完整合成再上屏） | B0006 / B0116 |
| `TextFallback` | rc.3 N0：生成返回后的残余事件**不许丢** | B0133 |
| `ActionCue` | 表演 cue 走 `action_cue` 帧 | B0015 |
| `Error` / `Terminal` | 致命分类（`is_fatal`）与终态顺序 | B0017 |
⇒ **整条 turn 管线的事件面现在全部有据可查**，且**没有一环是「靠测试盯着」**——
`TextFallback` 那条甚至还多一道**残余排空**（B0133）。

## 未核实项
1. `conversation/mod.rs` 其余 ~500 行未读（`TurnReport` 字段 / `ConversationUiEvent` / 配置类型 / 各项测试）
2. `handle_engine_event` **各臂内部**未逐臂读（只核了「臂与变体的对应关系」）
3. `engine.rs` 余 390 行（Stage A/B/C 主体）未读；`supervisor/turn.rs` 余 530 行未读
4. `worker.rs` 余 200 行 / `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
5. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
