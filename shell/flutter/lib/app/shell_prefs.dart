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

  /// 选一张背景图。`forShell` = 从「壳背景」那一行进来的。
  ///
  /// # 为什么两条路共用一个实现（2026-09-14，rc.5）
  ///
  /// 「与舞台同步」开着时，壳背景**就是**舞台那张图（`effectiveShellImage`），
  /// 所以壳那行的选图落到的仍是 `stageImage`——两份 UI、一份真相。
  /// 只有同步关掉时，图片才写进 `shellImage`（壳自己那张）。
  ///
  /// # 两条结果，都要如实说
  ///
  /// - 在 [kStageImageMaxChars] 以内 → 写进偏好（重开页面还在）。
  /// - 超限 → 舞台可以**只在本次会话生效**（直接下发渲染面，不写盘）；
  ///   壳没有这条通道（背景是 Flutter 自己画的），只能如实说「没应用」。
  Future<void> _pickImage({required bool forShell}) async {
    final ({String? dataUrl, String? error}) picked = await pickImageDataUrl();
    if (!mounted) return;
    final String? dataUrl = picked.dataUrl;
    if (dataUrl == null) {
      // 取消 → 静默（用户自己关的对话框）；读失败 → 说实话。
      if (picked.error != null) {
        _setImageMessage(forShell: forShell, text: picked.error!, failed: true);
      }
      return;
    }
    if (dataUrl.length > kStageImageMaxChars) {
      if (forShell) {
        _setImageMessage(
          forShell: true,
          text:
              '图太大了（${_kb(dataUrl.length)}，上限 ${_kb(kStageImageMaxChars)}）——'
              '壳背景没有应用。换一张小一点的图。',
          failed: true,
        );
        return;
      }
      // 超限：不经偏好，直接下发到渲染面（本次会话有效）。
      unawaited(
        _stageKey.currentState?.sendStageBg(dataUrl) ?? Future<void>.value(),
      );
      _setImageMessage(
        forShell: false,
        text:
            '图太大了（${_kb(dataUrl.length)}，上限 ${_kb(kStageImageMaxChars)}）——'
            '本次有效，**重新打开页面会丢失**。换一张小一点的图就能记住。',
        failed: true,
      );
      return;
    }
    // 同步开着时壳与舞台共用 stageImage（一份真相）。
    // 偏好**在 await 之后重新读**：选图对话框可能开了几秒，期间主题 / 音量可能已变，
    // 用进对话框之前那份会把那次改动吞掉。
    final DisplayPrefs prefs = widget.prefs;
    final bool shared = !forShell || prefs.syncShellStageBg;
    _updatePrefs(
      shared
          ? prefs.copyWith(stageImage: dataUrl)
          : prefs.copyWith(shellImage: dataUrl),
    );
    _setImageMessage(
      forShell: forShell,
      text: shared
          ? '已应用（与舞台共用同一张图，${_kb(dataUrl.length)}，会记住）'
          : '已应用壳背景（${_kb(dataUrl.length)}，会记住）',
      failed: false,
    );
  }

  Future<void> _pickStageImage() => _pickImage(forShell: false);

  Future<void> _pickShellImage() => _pickImage(forShell: true);

  /// 清掉背景图。同步开着时清的是**共用那张**（舞台与壳一起回到各自底色）。
  void _clearImage({required bool forShell}) {
    final DisplayPrefs prefs = widget.prefs;
    final bool shared = !forShell || prefs.syncShellStageBg;
    _updatePrefs(
      shared
          ? prefs.copyWith(clearStageImage: true)
          : prefs.copyWith(clearShellImage: true),
    );
    _setImageMessage(
      forShell: forShell,
      text: shared ? '已清除背景图，舞台与壳回到各自底色' : '已清除壳背景，壳回到主题底色',
      failed: false,
    );
  }

  void _clearStageImage() => _clearImage(forShell: false);

  void _clearShellImage() => _clearImage(forShell: true);

  /// 落一条选图 / 清图结果——舞台与壳**各有各的那条**，不互相冒充。
  void _setImageMessage({
    required bool forShell,
    required String text,
    required bool failed,
  }) {
    if (forShell) {
      _shellImageMessage = text;
      _shellImageFailed = failed;
    } else {
      _stageImageMessage = text;
      _stageImageFailed = failed;
    }
    _refresh();
  }
}
