# BATCH-0007 — Phase 1 · lib/chat/ + lib/state/（对话回合与相位真源）

> 账本：`AUDIT-B/`。行号基于 HEAD `5ef879f4`。

## 本批读过
| 文件 | 行 | 一句话 |
|---|---|---|
| lib/state/ui_state_tracker.dart | 250 | 全读：相位信号真源；定时器可注入；dispose 后不通知（`_disposed` 守卫，注释写明「停止对话后偶发崩溃」的来由）；`deriveUiPhase` 顺序即契约 ✓ |
| lib/state/ui_phase.dart | 140 | 全读：纯函数；判定顺序四条理由都在注释里；`kInterruptedHold` 真源在 tokens ✓ |
| lib/state/live_region.dart | 89 | 全读：节流两档（整句即刻 / 1.5 s 限速）；只在真更新时 `notifyListeners`（无反馈环）✓ |
| lib/chat/chat_controller.dart | 623 | 全读：`_assistant` 引用语义 + epoch 门禁 + `_turnInFlight` 双重判据；`_finishTurn` 五路收口；**本地失败路径不清 UI 相位（F-0007-1）** |
| lib/chat/chat_session.dart | 422 | 读 150-349 + 头注：版本门禁整体丢弃、坏条目丢弃、`activeId` 悬空回落 null、`replace` 保位置、两处 500 上限同源 ✓ |
| lib/chat/chat_markdown.dart | 199 | 读 60-189：逐字符扫描无正则回溯；空行内码/空强调**不认**（宁少勿吞）；`hasChatMarkdown` 用「解析一遍比对」而非正则猜 ✓ |
| lib/chat/chat_message.dart · turn_liveness.dart | 328 | B5 已读，本批复核 `toJson/fromJson` 与 `settleTurn` 五分支的耦合 ✓ |
| lib/ui/error_actions.dart | 51 | 补读：**「打断并重发」只调 `onStop`**（F-0007-2） |
| 契约核对（只读） | — | `crates/live2d-ai-desktop/src/web_api/chat_routes.rs:154/:487-497`：`no_supervisor`=503、`busy`=**429**、两者都**不广播任何 WS 帧** ⇒ F-0007-1 的「没有帧来收口」是后端行为确认，不是猜测 |
| test 抽查 | — | `test/ui_state_tracker_test.dart`（30+ 条真断言，覆盖连接/轮次/打断/error 五组）· `test/chat_session_test.dart`（32 条）· `test/turn_liveness_test.dart` — 三份都**不含**源码扫描式断言，判别力良好 |

## 发现
- **F-0007-1（P1）** `POST /api/v1/chat` 本地失败（429 busy / 网络错误）不清理 `UiStateTracker._turnActive` ⇒ 状态胶囊永久「思考中」、发送键永久变「停止本轮」。
- **F-0007-2（P2）** 错误动作「打断并重发」只调 `onStop`，**不重发**——文案承诺与实现不符，且它是 busy 场景唯一的恢复出口。

## 本批核对过、不成发现的（正面记录）
- **`_syncLiveRegion()` 在 `ShellRoot.build` 里调 `notifyListeners` 会不会崩？** 逐帧推演后确认**不会**：`ListenableBuilder` 的元素是当前 build target（`_ShellRootState`）的后代，落在 `Element.markNeedsBuild` 的 `_debugIsInScope` 早退分支（SDK `framework.dart`，`if (_debugIsInScope(owner._debugCurrentBuildTarget!)) return;`），既不抛也不产生额外重建。**但这条正确性依赖「被通知者是当前 build 的后代」**，若有人把 `feed()` 挪到别的子树就会变成 `setState() called during build` 断言——记此备忘，不成发现。
- 双订阅（`UiStateTracker.consume` + `ChatController._onWsEvent` 同一条 WS 流）**不算状态源分裂**：前者只管相位布尔，后者只管消息与音频，两者没有重叠字段；`markTurnAccepted` 是唯一的「轮次开始」入口，`errorActionsFor` 按**码**分流不按文案（`error_actions.dart:32-42`）✓。
- `_failLocalTurn` 只在 `identical(_assistant, bubble)` 时改写气泡 —— 迟到的 HTTP 结果不会覆盖服务端已收口的失败（防「复活空转气泡」）✓。
- `_abandonTurn`（切会话）同样不碰 `_ui`，但**会自愈**：那一轮仍在服务端跑，`turn_state` 到达时 `consume` 清 `_turnActive`。F-0007-1 之所以不自愈，正是因为本地失败**没有任何帧**（后端行为已核对）。
- 字体子集：`⚠`(U+26A0) 与 `•`(U+2022) **都在**子集内（按 `.ranges.txt` 实算 90 个区间），所以 `_failLocalTurn` 里的 `'⚠ $message'` 与 markdown 的 bullet 正则**不会**触发 fonts.gstatic.com 回退（红线 L 在本域安全）。
- `settleTurn` 的五个分支与气泡渲染一一对应；`replace()` 保位置（注释解释了「位置本身就是语义」）✓。

## 本批未核实
- F-0007-1 的**触发频率**（429 busy 在实际使用中多常见）需要真机/多端观察；可达性已由后端源码确认。
- `ChatController` 因 `package:web` 在 VM 测试里加载不了，所以这条接缝**结构上无法用现有测试覆盖**——修法之一就是把「本地失败 ⇒ 相位回落」抽成 `turn_liveness.dart` 的纯函数（那里已有 8 个纯判据 + 30 条真断言的现成模式）。
