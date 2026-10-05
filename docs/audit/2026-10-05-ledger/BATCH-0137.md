# BATCH-0137 · `TurnReport` 只有 2 个字段 —— **「薄」是写明的设计，且失败信号被显式化了**

Phase 1 · 域覆盖 · `conversation/mod.rs`（报告面）

## 读过的文件（来自 `git ls-files` + `wc -l`）
1. `crates/live2d-ai-runtime/src/conversation/mod.rs` — 548（定点 382-399：`TurnStatus` 三变体 +
   `TurnReport` 两个字段及其文档）

## 跑的命令（全部只读）
```
awk '/pub struct TurnReport/,/^}$/' conversation/mod.rs | grep -E "pub "
grep -n "pub struct TurnReport" -B 4 -A 12 conversation/mod.rs
grep -n "pub enum TurnStatus" -A 10 conversation/mod.rs
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；**「薄」有据，且失败信号从「缺失」改成了「显式」**
### ① 薄是**写明**的设计
```rust
/// `run_turn` 的返回值：轻量汇总；**全部细节都经事件通道传递**。          // :392
pub struct TurnReport {
    pub status: TurnStatus,
    /// 本轮 LLM 生成的完整正文（**含未成句残余；取消时为截断处的前缀**）。  // :397-398
    pub assistant_text: String,
}
```
⇒ 与我在早前批次核过的 `ErrorKind::code()` → `AppEvent::Error` → WS `error` 帧
（B0015，码两处一致）是**同一套两通道设计**：**结果在返回值、细节在事件**。

### ② `status` 是 **3 变体枚举**（不是 bool），且每个变体都写明**细节去哪**
| 变体 | 文档（逐字要点） |
|---|---|
| `Completed`（:383） | 「LLM 正常结束且 TTS 排空（途中可能有**已通过事件上报的可恢复错误**）」 |
| `Failed`（:385-386） | 「因致命错误或 LLM 阶段失败结束（**细节经先行 `Error` 事件上报**；终态即 `Terminal { Failed }`——**不再用 Done 隐示成败**）」 |
| `Cancelled`（:388） | 「收到取消（终态 `Terminal { Cancelled }`）。**历史不提交**」 |

### ③ ⭐ 顺带记一条**设计演进**（`Failed` 的文档里写着）
> 「**不再用 Done 隐示成败**」

⇒ 过去「失败」是用「**不发 Done**」来隐示的（靠**缺失**表达），现在有**显式变体**。
⇒ 这与本仓我反复核到的一条纪律**完全一致**：**用显式信号，不用「本该出现却没出现」来推断**
（句界不用 epoch 推断 / `dirty` 不用「碰过控件」推断 / 结局不用「有没有 Done」推断）。
⇒ 归入正面模式：**把隐式信号改成显式枚举**。

## 未核实项
1. `conversation/mod.rs` 其余 ~490 行未读（配置类型 / 事件族细节 / 该文件的自测）
2. `engine.rs` 余 390 行（Stage A/B/C 主体）· `turn.rs` 余 530 行未读
3. 各 `EngineEvent` 处理臂的**内部**未逐臂读
4. `worker.rs` 余 200 行 / `llm.rs` 余 360 行 / `client.rs` 余段 / `plan.rs` 余 700 行未读
5. `secrets.rs` 10 条测试断言体未读；B0120「是否别处 chmod 过 toml」未核
