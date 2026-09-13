/// 显示偏好 / 背景图 / 舞台缩放接线（**`part of` 组合根**）。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**的行为边界之一。
/// 偏好是「一份真相、一次持久化、主题与组件同源」：状态住在本 State 上，
/// 变更经 `widget.onPrefsChanged` 上报给 `Live2DShellApp`（`MaterialApp` 的父级）。
///
/// 用 `part` + 扩展方法而不是独立协作者的理由见 `main.dart` 头注。
///
part of 'package:live2d_ai_shell/main.dart';

String _kb(int chars) => '约 ${(chars / 1024).round()} KB';

extension _ShellPrefsWiring on _ShellRootState {
  /// 舞台角标三键 → 渲染面（渲染面自己算缩放，真值从 `stage-ack` 取）。
  Future<void> _zoom(String dir) async {
    await _stageKey.currentState?.sendStageZoom(dir);
    if (!mounted) return;
    _stageScaleFromAck = _stageKey.currentState?.lastAck?.scale;
    _refresh();
  }

  /// 把 [ShellRoot.prefs] 下发给渲染面（协议 v1 `sync`）。
  ///
  /// [override] 是**即将生效**的偏好。为什么需要它：改偏好的流程是
  /// 「先通知宿主 → 宿主 `setState` → 本 State 的 `widget.prefs` 才更新」，
  /// 而这里是在 `setState` **之前**同步调用的——读 `widget.prefs` 会拿到
  /// **上一次的值**。切主题时这个时序错误的表现是「界面变白了、舞台还是黑的」：
  /// 发下去的是旧主题的 `stageColor`，而之后再没人重发。
  void _applyPrefs([DisplayPrefs? override]) {
    final DisplayPrefs prefs = override ?? widget.prefs;
    final AppPalette palette = AppPalette.of(prefs.theme);
    // **`stageColor` 不在这里发**（2026-09-11，P1-3）。
    //
    // 它过去由这里下发：切主题时一按就发终色 ⇒ 界面还在 200 ms 里插值，
    // 舞台已经跳到终色（「舞台先闪一下」）。现在舞台底由
    // [Live2DStage] 自己跑一条**同样 200 ms** 的动画逐帧下发，
    // 两边的观感才是一起变的。
    //
    // 仍然保留的是「首帧 / 重建 iframe 后的补发」：那条路在
    // `Live2DStage._attach()` 里（它用的是 `widget.stageColor`），
    // 所以这里不需要也不该再发一次。
    _stageKey.currentState?.sync(
      scale: prefs.scale,
      dark: palette.dark,
      lipSync: prefs.lipSync,
      idleEnabled: prefs.idleEnabled,
      mouthSensitivity: prefs.mouthSensitivity,
      // 「允许拖动与缩放」下发到渲染面的 `sync.clickEnabled`。
      clickEnabled: prefs.allowDragZoom,
      tier: prefs.tier,
    );
    // 背景图与纯色底是**叠放**关系（有图盖住底色），所以单独走 `stage-bg`。
    unawaited(
      _stageKey.currentState?.sendStageBg(prefs.stageImage) ??
          Future<void>.value(),
    );
  }

  /// 偏好变更：更新内存 + 落盘 + 立即下发。
  void _updatePrefs(DisplayPrefs next) {
    if (next == widget.prefs) return;
    // 落盘结果要接住：写失败（无痕 / 配额满 / 存储被禁）过去是**静默**的，
    // 用户看到「已应用」却在刷新后丢失（rc.3 §9.1 候选原因 1）。
    final bool saved = widget.onPrefsChanged(next);
    if (!saved) {
      _stageImageMessage =
          '偏好没能写入本机存储（无痕模式 / 存储被禁 / 配额满）——本次会话有效，刷新会丢。';
      _stageImageFailed = true;
    }
    // 静音与音量是纯本机输出设置，不经渲染面——直接作用于 AudioPlayer。
    _audio.muted = next.muted;
    _audio.volume = next.volume;
    // 传 `next` 而不是让它去读 `widget.prefs`（那还是旧值）。
    _applyPrefs(next);
  }

  /// 选一张舞台背景图。
  ///
  /// # 两条结果，都要如实说
  ///
  /// - 在 [kStageImageMaxChars] 以内 → 写进偏好（重开页面还在）。
  /// - 超限 → **只在本次会话生效**，不写盘。UI 必须说清「重开就没了」，
  ///   否则用户会以为它坏了。
  Future<void> _pickStageImage() async {
    final ({String? dataUrl, String? error}) picked = await pickImageDataUrl();
    if (!mounted) return;
    final String? dataUrl = picked.dataUrl;
    if (dataUrl == null) {
      // 取消 → 静默（用户自己关的对话框）；读失败 → 说实话。
      if (picked.error != null) {
        _stageImageMessage = picked.error;
        _stageImageFailed = true;
        _refresh();
      }
      return;
    }
    if (dataUrl.length <= kStageImageMaxChars) {
      _updatePrefs(widget.prefs.copyWith(stageImage: dataUrl));
      _stageImageMessage = '已应用背景图（${_kb(dataUrl.length)}，会记住）';
      _stageImageFailed = false;
      _refresh();
      return;
    }
    // 超限：不经偏好，直接下发到渲染面（本次会话有效）。
    unawaited(_stageKey.currentState?.sendStageBg(dataUrl) ?? Future<void>.value());
    _stageImageMessage =
        '图太大了（${_kb(dataUrl.length)}，上限 ${_kb(kStageImageMaxChars)}）——'
        '本次有效，**重新打开页面会丢失**。换一张小一点的图就能记住。';
    _stageImageFailed = true;
    _refresh();
  }

  /// 清掉背景图（回到纯色舞台）。
  void _clearStageImage() {
    _updatePrefs(widget.prefs.copyWith(clearStageImage: true));
    _stageImageMessage = '已清除背景图，回到纯色舞台';
    _stageImageFailed = false;
    _refresh();
  }
}
