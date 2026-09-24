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
  /// 舞台当前该用的动作幅度（**草稿优先** = 即时预览的真源，2026-09-16 修）。
  ///
  /// 三滑条的即时预览就是这条路：草稿里正在拖的值优先于磁盘值；保存后草稿清空，
  /// 两者合一（`SettingsController` 用服务端回填的 view 覆盖 remote）。
  ActionSettingsView? get _effectiveActionScales {
    final SettingsDraft draft = _settings.draft;
    return effectiveActionScales(
      remote: _settings.remote?.action,
      draftHead: draft.actionHeadScale,
      draftBody: draft.actionBodyScale,
      draftExpression: draft.actionExpressionScale,
    );
  }

  /// 服务端动作幅度 → 渲染面 `sync.actionScales`（**即时预览**）。
  ///
  /// 未加载（`remote == null` 且草稿为空）→ `null`：渲染面用自己的出厂默认
  /// （0.75 / 0.80 / 1.0），**不谎报成 1.0**。
  Map<String, double>? get _actionScalesPayload {
    final ActionSettingsView? action = _effectiveActionScales;
    return action == null ? null : toActionScalesPayload(action);
  }

  /// **给舞台的唯一取值口**：临时覆盖 > 草稿 > 磁盘值（W7，2026-09-23）。
  ///
  /// `main.dart` 把它交给 `Live2DStage(actionScales: …)`——iframe 重建 / 重挂后
  /// 舞台自己用这份快照补发，不再退回渲染面出厂默认（RESEARCH §2.3 的死参数接上）。
  Map<String, double>? get _stageActionScales =>
      _actionScalesSyncer.active(_actionScalesPayload);

  /// 动作幅度即时预览的防抖下发（实现见 [ActionScalesSyncer]）。
  void _scheduleActionScalesSync() {
    _actionScalesSyncer.schedule(_actionScalesPayload);
  }

  /// 立刻下发当前幅度（onReady / iframe 重建后补发）。
  ///
  /// **临时覆盖生效时不发产品值**——由 [ActionScalesSyncer] 保证（它发的永远是
  /// `active()`）。所以这里的 `force: true` 只表达「新桥要收到当前值」。
  void _syncActionScalesNow({bool force = false}) {
    _actionScalesSyncer.syncNow(_actionScalesPayload, force: force);
  }

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
    // 首帧 / 重建 iframe 后补发幅度（`actionScales` 是 `sync` 的一个字段，
    // 但它的真源是**设置控制器**而不是 `DisplayPrefs`，所以不能在 `sync` 里带）。
    //
    // `force: true` **不会**冲掉临时覆盖：syncer 发的是它的 `active()`
    // （临时覆盖 > 产品值）。改主题 / 调音量等任意偏好变更走到这里，
    // 临时值照样在（W7 的确定性缺陷 1，见 action_scales_sync.dart）。
    _syncActionScalesNow(force: true);
  }

  /// 偏好变更：更新内存 + 落盘 + 立即下发。
  ///
  /// 这是**纯本机偏好变更的落点**：主题 / 缩放 / 口型 / 音量 / 静音 / 背景图
  /// 与舞台背景轮播列表都经过这里，改完立刻 `saveDisplayPrefs` 并下发渲染面。
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

  /// 「加入轮播」：把**当前**舞台图追加进 `stagePlaylist`（Wave 2）。
  ///
  /// 三条预算都在 `appendToStagePlaylist`（纯函数）里把关；这里只把**原因**
  /// 翻译成一句可读文案——超限时列表**原样不动**，不会悄悄膨胀到把整份偏好
  /// 写坏（rc.5 的教训，见 `kStagePlaylistMaxChars`）。
  void _addStageImageToPlaylist() {
    final DisplayPrefs prefs = widget.prefs;
    final StagePlaylistAppendResult result = appendToStagePlaylist(
      prefs.stagePlaylist,
      prefs.stageImage,
    );
    if (!result.added) {
      _setImageMessage(
        forShell: false,
        text: switch (result.reason) {
          'empty' => '还没有舞台背景图——先「选择背景图」，再把它加入轮播',
          'item_too_large' => '这张图太大了，不能进轮播列表；换一张小一点的图',
          'limit_reached' => '轮播列表已到上限 $kStagePlaylistMaxItems 张——'
              '先「清空轮播」再重新加入',
          'budget_exceeded' => '轮播列表的总长度已达上限（本机存储放不下更多），'
              '先「清空轮播」再重新加入',
          _ => '这张图没能加入轮播列表',
        },
        failed: true,
      );
      return;
    }
    _updatePrefs(prefs.copyWith(stagePlaylist: result.playlist));
    _setImageMessage(
      forShell: false,
      text: '已加入轮播（共 ${result.playlist.length} 张）',
      failed: false,
    );
  }

  /// 「清空轮播」：清空列表（`stageImage` 不动——当前这张仍留在舞台上）。
  void _clearStagePlaylist() {
    final DisplayPrefs prefs = widget.prefs;
    if (prefs.stagePlaylist.isEmpty) return;
    _updatePrefs(prefs.copyWith(stagePlaylist: const <String>[]));
    _setImageMessage(forShell: false, text: '已清空轮播列表', failed: false);
  }

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
