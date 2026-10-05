/// 对话回合与读屏播报接线（**`part of` 组合根**）。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**的行为边界之一：
/// 发送 / 打断 / 把流式正文喂给节流播报器。
/// 真正的会话与流式状态在 `chat/chat_controller.dart`（可单测）。
///
/// 用 `part` + 扩展方法而不是独立协作者的理由见 `main.dart` 头注。
///
part of 'package:live2d_ai_shell/main.dart';

extension _ShellChatWiring on _ShellRootState {
  /// 把流式正文喂给节流播报器。
  ///
  /// **按「整句到达」优先**：末尾出现句末标点就立刻播报；否则按 1.5 s 限速。
  /// 两级放行的判据在 `LiveRegionThrottle.feed` 里，可单测。
  void _syncLiveRegion() {
    final ChatMessage? last = _chat.messages.isEmpty
        ? null
        : _chat.messages.last;
    if (last == null) return;
    if (last.role != ChatRole.assistant) return;
    if (!last.streaming) {
      // 收口：把最后一小段补播一次（末尾往往没有句末标点，比如「好呀」）。
      _live.finish(last.text);
      return;
    }
    _live.feed(last.text);
  }

  // ── 动作子系统已于 2026-09-11 整条移除（用户裁决：LLM 无工具、只做对话）──
  //
  // 这里曾经是 `_onActionState`：把 WS 的 `action_state` 帧投影成 `ActionEvent`，
  // 再转成渲染面的 `action-state`（含 `cease` 帧要靠 `_performingAction` 游标
  // 找回动作名才能停）。随着 `live2d_perform_action` 工具、`action_state` 帧与
  // `/api/v1/commands` 端点一起删除——服务端不再发这一帧，前端也就没有任何
  // 动作通道可接了。触发与日志的归档见分支 `archive/action-trigger-p5`。

  Future<void> _send() async {
    final String text = _input.text;
    if (text.trim().isEmpty) return;
    if (_chat.streaming) return;
    _input.clear();
    _ui.markTurnAccepted();
    await _chat.send(text);
    // **本地失败必须回落相位**（2026-10-01，审计 F-0007-1）。网络异常 / 非 2xx
    // （`no_supervisor`=503）/ 200 但未受理（`busy`=429）这三条本地失败路径后端
    // **不广播任何 WS 帧**，所以不会有任何收口帧来清「思考中」：不补偿的话状态
    // 胶囊**永久**停在「思考中」、发送键永久变「停止本轮」（幽灵态）。
    //
    // 判据用 `_chat.sendFailedLocally` 而**不是** `_chat.error != null`：
    // 服务端推来一条非致命 `error` 帧（本轮仍在飞、语音还要播完）时后者**也为真**，
    // 那样会把一条正常进行的轮次提前说成结束。
    if (_chat.sendFailedLocally) _ui.markTurnFailed();
    _syncLiveRegion();
  }

  /// busy 的出路：**打断并重发**（F-0007-2，审计 2026-09-28）。
  ///
  /// 缺陷现场：按钮写着「打断并重发」，实现只接 `_stopWithCancellation()`
  /// ——用户按下去只看到「已打断」，那条消息**并没有被重新发出去**，而这是
  /// busy 场景**唯一**的恢复出口（此时相位已由 F-0007-1 的修复回落，界面
  /// 不会再自己重试）。
  ///
  /// 顺序与「两步都要做」都是契约，收在 [interruptAndResend] 里（可在 VM
  /// 上测）：`main.dart` 是 `package:web` 组合根，这里只接线。
  Future<void> _interruptAndResend() => interruptAndResend(
    stop: _stopWithCancellation,
    resend: _resendLastUserMessage,
  );

  /// 把**上一条用户消息**原样重发一次（不新增第二条用户气泡）。
  ///
  /// 与 [_send] 的分工：那条路读输入框（用户刚打的字）；这条路读**会话记录**
  /// ——busy 时 `_send` 早已 `clear()` 了输入框，走输入框只会空转。
  ///
  /// 相位的处理与 [_send] 同形（`markTurnAccepted` 在 await **之前**，
  /// 失败按 `sendFailedLocally` 回落），否则会退回 F-0007-1 的幽灵态。
  Future<void> _resendLastUserMessage() async {
    // 已有一轮在飞：不抢（重发的前提是 stop 已经腾出 turn）。
    if (_chat.streaming) return;
    final String? text = _chat.lastUserMessageText;
    // 没得重发：如实什么都不做（不伪造一轮）。
    if (text == null || text.trim().isEmpty) return;
    _cancelStageForTurnBoundary('new-message');
    _ui.markTurnAccepted();
    await _chat.resendLastUserMessage();
    if (_chat.sendFailedLocally) _ui.markTurnFailed();
    _syncLiveRegion();
  }

  Future<void> _stop() async {
    await _chat.stop();
    _syncLiveRegion();
    // 停止后立刻收口并显示「已打断」（不等服务端的 turn_state，那是它的节奏）。
    _ui.markStopped();
  }
}
