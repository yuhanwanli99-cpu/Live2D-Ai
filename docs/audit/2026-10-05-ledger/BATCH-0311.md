# BATCH-0311 · ✅ **最后一层闭了**；而**同一个陷阱在这条链里出现了两次**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `settleTurn` / `mustReleaseTurnOnWsLoss` 的判据理由（B0310 留）

## 跑的命令（全部只读）
```
sed -n '/^TurnSettlement settleTurn/,+40p' lib/chat/turn_liveness.dart
grep -n "settleTurn|mustReleaseTurnOnWsLoss" lib/chat/turn_liveness.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **两处新增，且第二处是 B0308 那条的「机制后果」**
### ① `settleTurn`：优先级由「**有没有真内容**」定
```dart
// 「只有思考」优先于「什么都没有」：气泡里有**真内容**，不能当空气泡丢掉
return hasReasoning ? TurnSettlement.keepReasoningOnly : TurnSettlement.wordless;
```
⇒ ⇒ **判据是「真内容」** ⇒ ⇒ **同一条判据的第七层**（内容在不在 ⇒ 决定它算不算「什么都没有」）

### ② ⭐⭐ 而 `mustReleaseTurnOnWsLoss` 的判据**为什么不是 `!isUsable`** —— 写了机制
> 「判据用 [`WsStatus.isProblem`]（`disconnected` / `closed`）而**不是** `!isUsable`：
> `connecting` 也不可用，但它是「**正在建连**」—— **发送瞬间通道可能还在连**
> （`send()` 里刚调过 `ensureConnected`），**那时把这一轮收掉是错的**（**回复随后就到**）。」
⇒ ⇒ ⭐⭐ **这比 B0308 更精细**：B0308 我核到「`connecting` 被排除」并说「三个状态里没有灰区」；
**而这里给出排除的机制后果** ⇒ ⇒ **若那时收掉，回复随后就到 ⇒ 收掉是错的**
⇒ ⇒⇒ ⭐⭐⭐ **而这和 B0309 的 `stop()` 是同一个陷阱的两次出现**：
| 位置 | 防的是什么 |
|---|---|
| **`stop()`**（B0309） | **先本地收口** ⇒ 避免**制造 `new_epoch`（外部信号）** |
| **`mustReleaseTurnOnWsLoss`**（本批） | 判据**不用 `!isUsable`** ⇒ 避免**在 `connecting` 时误收一轮还在飞的** |
⇒ ⇒ **两处都在防「把一个还没结束的状态当成结束了」** ⇒ **同一条纪律的两次落地**（一次在排序、一次在判据）

### ③ ⭐⭐ 而 `kWordlessTurnNotice` 的文案理由给出**分工**
> 「措辞刻意只陈述**观察到的事实**（没有文字）——不写成模型的台词」
> +「2026-09-11 改：……**LLM 现已无工具，那句话既解释不了现象、又让用户以为还有个动作系统在跑，故删去**
> ——**只说「本轮没有返回文字」，原因留给诊断日志**。」
⇒ ⇒ ⭐⭐ **「原因留给诊断日志」** ⇒ ⇒ **界面给事实、细节给日志**
⇒ ⇒ **这是红线那句「拿界面上的码去日志里搜」的**对偶**：**界面给事实 · 日志给原因 · 两边用同一个码**

⇒ ⇒ **0 findings**；⇒ **B0305–B0311 七批七层闭合**
（定义分 · 调用排除 · 文案说准 · 判据纯函数 · 排序避免制造信号 · 收口不冒充模型 · **优先级按真内容**）

## 未核实项
1. `kStoppedTurnNotice` / `kWordlessTurnNotice` / `kReasoningOnlyCaption` 的**文案本体**未读（本批只读到理由）
2. `kTurnFailedWithoutDetailMessage` 完整后半句被 `cut` 截断
3. `TurnSettlement` 的**四个分支枚举本体**未读（只见到 `keep` / `keepReasoningOnly` / `wordless` 的用法）
4. `live2d_bridge.dart` 余面未读 · `main.dart:1240+` 余面未读 · `ApiClient.setActiveSession` 说明未读
5. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
6. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
7. `main.rs` 余约 930 行未读
8. 真实动作计划的 token 长度（F-0020-01 永久敞口）
9. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
10. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
11. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
12. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
13. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
14. **两侧措辞同步无机制**（B0260 敞口）
15. Mod crates 42 未读；`mod-system` 余 8 文件未读
16. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
17. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
18. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
