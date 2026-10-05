# BATCH-0313 · ✅ **五条未核实项一次结清**；而 `TurnSettlement` 是第七层的**枚举级**落地

Phase 1 · **未核实项清账**（按停止规则换轴）—— 结清 B0307/B0310/B0311 留下的文案与枚举

## 跑的命令（全部只读）
```
grep -n "kStoppedTurnNotice\s*=|kWordlessTurnNotice\s*=|kReasoningOnlyCaption\s*=" lib/chat/*.dart
sed -n '/enum TurnSettlement/,/^}/p' lib/chat/turn_liveness.dart
```
未跑任何 cargo / flutter / pnpm 命令。

⚠ **本批有一次工具层事故（如实记）**：第一次写这个文件时用 shell heredoc，
而正文里出现了与 shell 分隔符同形的行 ⇒ **shell 把后续的 python 与 `echo` 一起吞进了文件（103 行）**
⇒ **已用 `write` 工具重写**（这也是我第一次因「文件未读」被工具拒绝——**先读再写**）
⇒ ⇒ ⭐ **两条工具层教训**：
① **账本正文里若出现 shell 的分隔符，就会被 shell 吃掉** ⇒ **凡写进 `AUDIT-REPO/` 的长文本，一律用 `write` 工具**
② **`write` 要求先读** ⇒ ⇒ **重写一份被工具层污染的文件时，「先读」不是形式，是工具的硬要求**

## 本批产出：**0 条新发现**；✅ **五条未核实项结清**
| 未核实项 | 结果 |
|---|---|
| `kWordlessTurnNotice` 文案 | `'本轮模型没有返回文字'` |
| `kStoppedTurnNotice` 文案 | `'已停止本轮'` |
| `kReasoningOnlyCaption` 文案 | 定义在 `:121`（**值在下一行，本批未取**）⇒ **仍开着** |
| `TurnSettlement` 枚举本体 | ✅ **五个变体，逐个有据** |
| 「判据在哪个文件」 | ✅ **同文件、类型级**（此前只核到函数级与排序级） |

### ① ⭐⭐ `stopped` 变体 = B0305 那条判据的**枚举级**落地
> 「用户（或别的客户端）**主动停止**了这一轮……**与 [`wordless`] 必须分开：两者都是「没有文字的收口」，
> 但原因完全不同** —— 一个是**模型的产出**，一个是**用户的意志**。**用同一句话会把用户自己按的……说成「模型没返回文字**」。」

⇒ ⇒ B0305 核的是**函数**级 · B0309 核的是**排序**级 · **本批核到「类型」级** ⇒ ⇒ **三级齐了**

### ② ⭐⭐⭐ 而 `keepReasoningOnly` 的立类理由有**三个**，其中两个是「用户会失去什么」
> 「为什么单列一类：**实测**推理模型的思考会占用**同一份输出预算**，长思考会把正文挤到为空或只剩半句 ——
> 而**半句切不出完整句**……这时界面上其实**有**东西可看（思考就在气泡上），只是没有正文。
> - [`wordless`] → 换成一条**系统行**（气泡里确实什么都没有）；
> - [`keepReasoningOnly`] → **保留气泡**（思考留给用户看）……
> **若这里也换成系统行，就等于把用户最想看的那段思考扔掉。**」

⇒ ⇒ 三个理由：**① 机制** · **② 红线的后果**（引 B0132「半句切不出完整句」）· **③ 用户会失去什么**
⇒ ⇒ 而它与 `wordless` 的差别**在 UI 上可见**（换系统行 vs 保留气泡）

### ③ 而 `wordless` 变体把 B0310 那条禁令**写进了枚举**
> 「没失败、也没有文字：写成一条**系统提示**。**绝不「什么都不显示」** —— 见文件头注。」

### ④ ⭐ 而文案层面**又一次「按原因选用」**
`kWordlessTurnNotice = '本轮模型没有返回文字'` ——
**B0305 说过「不能用『模型没有返回文字』那条」**，而**那是在「外部中断」的场合**
⇒ ⇒ **同一个文案，在「模型真的没返回文字」时该用、在「被断连打断」时不该用**
⇒ ⇒ **又一次「按原因/场合选用」**
而 `kStoppedTurnNotice = '已停止本轮'` ⇒ **四个字、不解释** ⇒ 与前一句**措辞明显不同**
⇒ ⇒ **B0305「用户的意志不当成模型产出」在文案上兑现**

## 未核实项（本批后仍开着 16 条）
1. `kReasoningOnlyCaption` 的**值**（定义在 `:121`，值在下一行）—— **本批明确未取**
2. `ErrorResponse` 定义 · `ModServices.apply_settings` 本体 · `ActionCue` 定义
3. `chat_session.dart` 的 `maxSessions = 50` **裁剪体**（`_trimSessions`）
4. `api_client.dart:217-226` 的 `_testEndpoint` 全貌
5. `settings_models.dart` 的 `fromJson` 体 · `ApiClient.setActiveSession` 的说明
6. `settleTurn` 的**完整分支顺序**（本批只见到尾部 `hasReasoning ? … : …`）
7. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
8. `apply_bridge_effects` 调用顺序（B0279 留）· `stage-bg.dataUrl` 发送方（B0277 留）
9. `mustReleaseTurnOnWsLoss` / `settleTurn` **有没有测试**
10. `main.rs` 余约 930 行
11. `requestAnimationFrame` 的**暂停语义**（WASM 侧 B0193 已核 Flutter 侧是 `scheduleFrame`）
12. `dev_tools_section.dart` 余 ~1845 · `live2d_stage` ~600 · `memory_panel` ~545 ·
    `persona_panel` ~520 · `message_bubble` ~460 · `chat_panel` ~465
13. **`reqwest` `.timeout()` 是否覆盖 body 读取**（KL-3 前提）—— **永久不可核**（禁止运行）
14. **动作计划的 token 长度**（F-0020-01 撤回的永久敞口）—— **仓库里拿不到**
15. `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」是否有别处
16. GitHub 侧 secret scanning / push protection（**仓库设置，不在代码里**）
