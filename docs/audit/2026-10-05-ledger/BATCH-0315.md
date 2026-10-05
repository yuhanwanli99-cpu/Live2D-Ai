# BATCH-0315 · ✅ **两条结清**；而**分支顺序本身就是「优先级」**，且**逻辑与接线各被钉住**

Phase 1 · **未核实项清账（第 3 批）**

## 跑的命令（全部只读）
```
sed -n '73,92p' shell/flutter/lib/chat/turn_liveness.dart
grep -rn "settleTurn|mustReleaseTurnOnWsLoss" shell/flutter/test/
```
未跑任何 cargo / flutter / pnpm 命令。

## 本批产出：**0 条新发现**；✅ **两条结清**
| 未核实项 | 结果 |
|---|---|
| `settleTurn` 分支顺序 | ✅ `:73-86`，**顺序即优先级** |
| `settleTurn` / `mustReleaseTurnOnWsLoss` 有没有测试 | ✅ **有，而且分两层**（见 ②） |

### ① 分支顺序 = 优先级，且**与枚举文档逐条对应**
```dart
if (text.trim().isNotEmpty) return TurnSettlement.keep;          // ① 有文字
if (failed)              return TurnSettlement.failed;         // ② 失败
if (stopped)             return TurnSettlement.stopped;        // ③ 停止
return hasReasoning ? keepReasoningOnly : wordless;              // ④ 只有思考 / 什么都没有
```
⇒ ⇒ ⭐⭐ **① 排在最前，意味着「有文字」压过 `failed`**，而枚举文档逐字写着
「有文字：原样保留（**失败时那段文字就是错误说明**）」⇒ ⇒ **顺序与文档一致**
⇒ ⇒ 且 `turn_liveness_test.dart:40` 就有 **`settleTurn(failed: true, text: '说话…')`** 这一条
⇒ ⇒ **优先级被测试单独钉住**（不是顺带覆盖）
⇒ ⇒ `:45` / `:190` 另有两条**纯空白**（`' '` / `'   \n  '`）⇒ ⇒ **`trim` 的判据被单独测了**
⇒ ⇒ **五个分支全覆盖**

### ② ⭐⭐ 而**逻辑与接线分两层被钉住**（B0187 那一族）
| 层 | 文件 | 钉住什么 |
|---|---|---|
| **逻辑** | `turn_liveness_test.dart:16-190` | `settleTurn` 五分支 + `mustReleaseTurnOnWsLoss` 的 WS 状态矩阵（`:71-107` 四条） |
| **接线** | `chat_notice_test.dart:150-167` | 「`_finishTurn` **走** `settleTurn`（而**不**是…）」`expect(controller.contains('settleTurn('), …)`；「断连收口**走** `mustReleaseTurnOnWsLoss`」 |
⇒ ⇒ ⭐ **结构测试（源码 `contains`）钉住「不许绕过判据自己判」，行为测试钉住「判据本身对不对」**
⇒ ⇒ **两层各管一件事、互不替代** ⇒ ⇒ 同 B0187 记录的「结构 + 行为」两层结构
⇒ ⇒ ⇒ 而这解释了为什么 `settleTurn` **必须是纯函数**：
**纯函数才能被行为测试穷举分支；接线才需要靠源码扫描钉住。**

## 未核实项（本批后仍开着 11 条）
1. `ModServices.apply_settings` 本体 · `ActionCue` 定义 · `api_client.dart:217-226` 的 `_testEndpoint`
2. `settings_models.dart` 的 `fromJson` 体 · `ApiClient.setActiveSession` 的说明
3. `live2d_bridge.dart` 余面（队列 · 两个 Timer · `_modelLoads` 注入点）· `main.dart:1240+`
4. `apply_bridge_effects` 调用顺序（B0279）· `stage-bg.dataUrl` 发送方（B0277）
5. `main.rs` 余约 930 行
6. `requestAnimationFrame` 的**暂停语义**
7. 面板余面（`dev_tools_section` ~1845 / `live2d_stage` ~600 / `memory_panel` ~545 /
   `persona_panel` ~520 / `message_bubble` ~460 / `chat_panel` ~465）
8. **`reqwest` `.timeout()` 是否覆盖 body 读取** —— **永久不可核**（禁止运行）
9. **动作计划的 token 长度** —— **仓库里拿不到**
10. `_finishTurn` 是否幂等 · `secrets.rs` 断言体 · B0120「chmod toml」是否有别处
11. GitHub 侧 secret scanning / push protection（**仓库设置，不在代码里**）
