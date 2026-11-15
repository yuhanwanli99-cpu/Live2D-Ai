part of 'package:live2d_ai_shell/main.dart';

/// 草稿防抖探针的接线（2026-11-16 热槽记忆 M3 的前端半边）。
///
/// 从 `main.dart` 拆出：那个文件贴着 Dart `lib` 的 **800 行硬棘轮**（上限 0），
/// 而这段接线自带一段「为什么挂 listener 而不是 onChanged」的说明。
///
/// # 链路
///
/// 输入框内容变化 → 400 ms 停顿 → `POST /api/v1/chat/draft` → 服务端**整块替换**
/// 该会话的热卡 → **下一次发送**的 system 文本里带上它（`hotmem` 四块之一）。
///
/// 发送成功后 `reset()`：那一轮的热卡已经用掉了，草稿不该继续挂着。
extension _ShellDraftWiring on _ShellRootState {
  /// 建探针并把 listener 挂到输入框上。`initState` 调一次。
  void _initDraftProbe() {
    // 为什么挂 `TextEditingController.addListener` 而不是 `TextField.onChanged`：
    // 后者只覆盖「用户敲键盘」，而这里要跟的是**输入框内容**——粘贴、语音写入、
    // 程序化清空都算。listener 是同一个事实的更完整来源。
    _draftProbe = DraftProbe(api: _api, sessionId: _chat.sessions.activeId);
    _input.addListener(_onDraftChanged);
  }

  /// 拆 listener 并取消计时器。`dispose` 调一次。
  void _disposeDraftProbe() {
    _input.removeListener(_onDraftChanged);
    _draftProbe?.dispose();
    _draftProbe = null;
  }

  /// 输入框内容变了 → 重置防抖计时器（**不发请求**，只重置）。
  ///
  /// 会话跟着当前活动会话走：切了会话，旧草稿的检索结果不该落到新会话上。
  void _onDraftChanged() {
    final DraftProbe? probe = _draftProbe;
    if (probe == null) return;
    probe.sessionId = _chat.sessions.activeId;
    probe.onChanged(_input.text);
  }
}
