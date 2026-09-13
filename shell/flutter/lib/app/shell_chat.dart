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
    final ChatMessage? last = _chat.messages.isEmpty ? null : _chat.messages.last;
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
    _syncLiveRegion();
  }

  Future<void> _stop() async {
    await _chat.stop();
    _syncLiveRegion();
    // 停止后立刻收口并显示「已打断」（不等服务端的 turn_state，那是它的节奏）。
    _ui.markStopped();
  }
}
