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
  /// 建探针、挂输入框 listener、并订阅「活动会话变了」（`initState` 调一次）。
  ///
  /// 为什么还要订阅会话：`_onDraftChanged` 只在**输入框变化**时才刷新会话 id。
  /// 「A 会话打了字 → 立刻切到 B → 400 ms 到点」这条路上，输入框不再变化，
  /// 探针会拿 B 的 id 把 A 的草稿发出去——正是本文件头注里说的那类串台。
  /// 所以切会话时**先 cancel**（不补发），下一段输入自然重新计时。
  ///
  /// 2026-11-18（#20）：探针自己也在**排计时器那一刻**固定会话 id，触发时发现
  /// 目标会话变了就不发（见 `draft_probe.dart`）。这里的 cancel 是第一道防线，
  /// 不再承担全部正确性——监听顺序出岔子也不会把 A 的检索落到 B 的热卡上。
  void _initDraftProbe() {
    // 为什么挂 `TextEditingController.addListener` 而不是 `TextField.onChanged`：
    // 后者只覆盖「用户敲键盘」，而这里要跟的是**输入框内容**——粘贴、语音写入、
    // 程序化清空都算。listener 是同一个事实的更完整来源。
    _draftProbe = DraftProbe(api: _api, sessionId: _chat.sessions.activeId);
    _input.addListener(_onDraftChanged);
    // `ChatController` 是 ChangeNotifier：切会话 / 新建 / 删除都会 notify。
    _chat.addListener(_onChatSessionMaybeChanged);
  }

  /// 会话可能变了：把探针的目标会话同步过去，并**取消**待发的那一次。
  ///
  /// 只在 id 真的变了才动——`ChatController` 每个 `text_delta` 都会 notify，
  /// 每次都 cancel 会把正常的防抖窗口切碎（那就等于没有防抖）。
  void _onChatSessionMaybeChanged() {
    final DraftProbe? probe = _draftProbe;
    if (probe == null) return;
    final String? active = _chat.sessions.activeId;
    if (probe.sessionId == active) return;
    probe.cancel();
    probe.sessionId = active;
  }

  /// 拆 listener 并取消计时器。`dispose` 调一次。
  void _disposeDraftProbe() {
    _input.removeListener(_onDraftChanged);
    _chat.removeListener(_onChatSessionMaybeChanged);
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
