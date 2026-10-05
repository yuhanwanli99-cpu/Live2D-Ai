# BATCH-0306 · ✅ **判据的另一半**：两个调用点**各自排除了另一条路**（0 条新发现）

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `_releaseTurn` 的两个调用点（B0305 留）

## 跑的命令（全部只读）
```
grep -n "_releaseTurn" lib/chat/chat_controller.dart
sed -n '28,44p' lib/chat/chat_controller.dart ; sed -n '276,290p' lib/chat/chat_controller.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐ **B0305 那条判据在调用点也成立**
### ① 调用点一：WS 通道掉了（`:28-39`）
> 「**通道掉了就收口**（2026-09-11）：见 `turn_liveness.dart` 的 **`mustReleaseTurnOnWsLoss`** ——
> **收口帧在断连期间不会重放，丢了就再也回不来**，**输入框会永久卡在「停止本轮」**而 `send()` 静默 return」
⇒ ⇒ **判据是从机制推出来的**（「不会重放 ⇒ 丢了回不来 ⇒ 必须就地收口」），不是选出来的
⇒ ⇒ 谓词是**另一个文件里的具名函数**，且注释**指过去**（又一次「指路」）

### ② ⭐⭐ 调用点二：被**别的客户端**抢占（`:276-285`）
> 「**别的客户端**发起的（**本机开了两个页面**时很常见），本客户端那一轮就永远等不到收口帧，
> **输入框锁死在「停止本轮」**。
> **用户自己按的停止已经在上面的 `stop()` 里先收口了，走到这里说明这一轮是**外部**中断的**。」
> `if (_streaming) _releaseTurn(message: kTurnPreemptedMessage, code: kTurnPreemptedCode);`
⇒ ⇒ ⭐⭐ **它写明了「为什么这里不是 `stop()`」** ⇒ ⇒ **同一个动作有两条路，而各自的前置条件被排除**
⇒ ⇒ **这正是 B0305 的判据在调用点的落地**
⇒ ⇒ 而且**两个 reason 各有自己的 code**（`kWsDroppedMidTurnCode` / `kTurnPreemptedCode`）
⇒ ⇒ **两码分立 ⇒ 可搜**（同 B0159 的码纪律）

### ③ ⇒ 与 B0305 合起来：**判据在两端都成立**
| 位置 | 判据 |
|---|---|
| **定义处**（B0305） | 两个函数**按谁造成的中断**分 |
| **调用处**（本批） | 每一处**把「另一条路已处理」写明** |
⇒ ⇒ ⇒ **0 findings**

### ④ ⭐ 而两个场景都写出「**不收口会怎样**」—— 同一句后果，两次重现
「**输入框会永久卡在「停止本轮」**」在 `:31` 与 `:279` **各出现一次** ⇒ ⇒ **两次都指向同一个用户可见后果**
⇒ ⇒ **这不是重复论证，是同一后果在两条路上的重现** ⇒ ⇒ 与 B0284「同一约束多点出现」的家族相反：
**这里重复的是「后果」，而「规则」被就地排除了**

## 未核实项
1. `mustReleaseTurnOnWsLoss` 的**本体未读**（它在什么条件下返回 true）—— 本批只核了它的**被引用**
2. `kWsDroppedMidTurnMessage` / `kTurnPreemptedMessage` 的**文案本体**未读（B0305 核了「要准」这条规则，**未核这两句准不准**）
3. `stop()` 的本体未读（`:278` 说「用户自己按的停止已经在上面的 `stop()` 里先收口了」）
4. `_finishTurn` 的 `wordless` 分支本体未读
5. `ApiClient.setActiveSession` 的说明本体未读（`main.dart:539` 指过去的那处）
6. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
7. `live2d_bridge.dart` 余面未读 · `main.dart:1240+` 余面未读
8. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
9. `main.rs` 余约 930 行未读
10. 真实动作计划的 token 长度（F-0020-01 永久敞口）
11. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
12. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
13. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
14. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
15. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
16. **两侧措辞同步无机制**（B0260 敞口）
17. Mod crates 42 未读；`mod-system` 余 8 文件未读
18. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
19. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
20. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
