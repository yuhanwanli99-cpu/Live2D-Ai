# BATCH-0307 · ✅ **用 B0305 那条规则去审那三个实例：全过** —— 而规则**就写在实例上面**

Phase 1 · 域覆盖 · 前端 `.dart`（机械值 37/218）—— `turn_liveness.dart` 的三个码与文案（B0306 留的「用规则审实例」）

## 跑的命令（全部只读）
```
grep -rn "kWsDroppedMidTurn|kTurnPreempted|kTurnFailedWithoutDetail" lib/chat/turn_liveness.dart
sed -n '124,136p;155,162p;188,196p' lib/chat/turn_liveness.dart
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；⭐⭐⭐ **规则 → 实例的自我应用，三句全过**
| 常量 | 文案 | 过不过 B0305 的规则（「说清原因，**不写另一个原因**」） |
|---|---|---|
| `kWsDroppedMidTurnMessage` | 「本轮回复未收到（**实时通道断开**）」 | ✅ **说的是通道断了，不是「模型没说话」** |
| `kTurnPreemptedMessage` | 「本轮已被中断（**新的代次开始**）」 | ✅ **点机制，不拿模型当借口** |
| `kTurnFailedWithoutDetailMessage` | 「本轮失败，但**服务端未给出错误详情**（可打开『设置 → 开发模式 → 诊断日志』…）」 | ✅⭐ **最纯的一处** —— **它承认「服务端没告诉我为什么」** ⇒ **不编造原因** |

### ① ⭐⭐ 而第三处的注释**把规则写在了实例的正上方**（`:189-191`）
> 「`error` 帧到达时，是否要**立刻**收口这一轮（而不是等 `turn_state`）。**禁止「无字无错」**：
> 界面上既没有正文、也没有任何原因时，**必须给一个能拿去搜索的码**，并指向诊断日志
> （**而不是假装知道原因**）。」
⇒ ⇒⇒ **「而不是假装知道原因」** —— **规则逐字写在遵守它的那个常量上面**
⇒ ⇒ **B0305 的规则在此被**同一个文件**自己复述了一遍**

### ② ⭐⭐ 而第一处的注释**给出的是「同一原则的另一半」**（`:129-133`）
> 「**同一句话同时用于两处**（错误横幅 + 会话里的系统行）：**横幅是暂时的，而历史是长久的** ——
> 只写横幅的话，**用户刷新之后只会看到「一条没有回复的消息」，又回到了「与坏了无法区分」那个坑**。」
⇒ ⇒ ⭐⭐ **这是 B0305 那条原则**上移一层**：
**只出现在临时位置的说明不够** ⇒ 因为**刷新之后「没有回复」与「坏了」不可区分**
⇒ ⇒ ⇒ **这是 B0247「『未收尾』必须落盘」那条要求**，
**套在「错误行」上而不是「正文」上** ⇒ ⇒ **同一条要求，剩下的一半**
⇒ ⇒ 且三个码（`ws_dropped_mid_turn` / `turn_preempted` / `turn_failed_no_detail`）
**分立且都「能拿去搜索」** ⇒ ⇒ **可搜**（B0159 家族）

### ③ ⇒ 「规则 → 实例」的自我应用，**这是本链第四次递进**
B0305（定义处按原因分）→ B0306（调用处排除另一条路）→ **B0307（用规则审实例，全过）**
⇒ ⇒ 而 B0307 额外**没预期到**的收获：**规则被同一个文件逐字复述了一遍**，而第一处还给出了**原则的另一半**

## 未核实项
1. `mustReleaseTurnOnWsLoss` 的**本体未读**（什么条件下返回 true）· `stop()` 的**本体未读**
2. `kTurnFailedWithoutDetailMessage` 的**完整后半句**被 `cut` 截断（只看到「可打开『设置 → 开发模式 → 诊断日志』…」）
3. `live2d_bridge.dart` 余面未读 · `main.dart:1240+` 余面未读 · `ApiClient.setActiveSession` 说明未读
4. `message_bubble.dart` 余约 460 · `live2d_stage` 余约 600 · `shell_admin` 余面 · `env_key_field` 余约 110 未读
5. `apply_bridge_effects` 调用顺序未核（B0279 留）· `stage-bg.dataUrl` 发送方未核（B0277 留）
6. `main.rs` 余约 930 行未读
7. 真实动作计划的 token 长度（F-0020-01 永久敞口）
8. 那四个「未知 id」在文档里有没有被点名（B0270 敞口）· `refresh_from_disk` 敞口（B0266）
9. 能否点名会在持锁期间 panic 的 Mod（B0268 敞口）
10. 面板余面：`dev_tools_section` ~1845 · `memory_panel` ~545 · `persona_panel` ~520 · `chat_panel` ~465
11. 其余 6 处 `mod_count_is_*` 逐条对账未核 · `ErrorResponse` 定义未读
12. `onReady` 是否间接清过 `presetStatus`（F-0263-01 反证 (a)，**结论不依赖它**）
13. **两侧措辞同步无机制**（B0260 敞口）
14. Mod crates 42 未读；`mod-system` 余 8 文件未读
15. `settings_controller.dart:120-222` / `:296-409` 未读；`logs` 采集点/轮转未核
16. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— 未核，禁止运行
17. `join_endpoint` 回归断言体 · `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」未核
