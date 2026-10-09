/// 显示偏好 / 背景图 / 舞台缩放接线（**`part of` 组合根**）。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**的行为边界之一。
/// 偏好是「一份真相、一次持久化、主题与组件同源」：状态住在本 State 上，
/// 变更经 `widget.onPrefsChanged` 上报给 `Live2DShellApp`（`MaterialApp` 的父级）。
///
/// 用 `part` + 扩展方法而不是独立协作者的理由见 `main.dart` 头注。
///
part of 'package:live2d_ai_shell/main.dart';

/// 字节 → 人话（背景现在是**按字节**说的；过去那套「字符数」口径已作废）。
String _mb(int bytes) {
  if (bytes < 1024 * 1024) return '${(bytes / 1024).round()} KB';
  return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
}

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
    //
    // # 同值不重发（F-0002-1，2026-09-28）
    // 这条漏斗**每次偏好变更都要过**（音量滑杆过去每帧一次），原来无条件重发一整个
    // `sync` + 一整串 `stage-bg`（后者是 ≤1.5 M 字符的整张图）。`sync` 按**桥身份 +
    // 字段指纹**去重（`lipSync`/`idleEnabled`/`mouthSensitivity`/`clickEnabled`
    // 只有这里发，iframe 重建会换桥）；`stage-bg` **只按值**（重建补发归 `_attach`）。
    final Live2DBridge? bridge = _stageKey.currentState?.bridge;
    final List<Object?> syncKey = <Object?>[
      prefs.scale,
      palette.dark,
      prefs.lipSync,
      prefs.idleEnabled,
      prefs.mouthSensitivity,
      // 「允许拖动与缩放」下发到渲染面的 `sync.clickEnabled`。
      prefs.allowDragZoom,
      prefs.tier,
    ];
    if (!identical(bridge, _lastSyncBridge) ||
        !listEquals(syncKey, _lastSyncKey)) {
      _lastSyncBridge = bridge;
      _lastSyncKey = syncKey;
      _stageKey.currentState?.sync(
        scale: prefs.scale,
        dark: palette.dark,
        lipSync: prefs.lipSync,
        idleEnabled: prefs.idleEnabled,
        mouthSensitivity: prefs.mouthSensitivity,
        clickEnabled: prefs.allowDragZoom,
        tier: prefs.tier,
      );
    }
    // 背景图与纯色底是**叠放**关系（有图盖住底色），所以单独走 `stage-bg`。
    final Live2DStageState? stage = _stageKey.currentState;
    final String? projected = DisplayPrefs.stageProjectionUrl(
      prefs,
      _backgroundIndex,
    );
    if (stage != null && _lastSentStageBg != projected) {
      _lastSentStageBg = projected;
      unawaited(stage.sendStageBg(projected));
    }
    // 首帧 / 重建 iframe 后补发幅度（`actionScales` 是 `sync` 的一个字段，
    // 但它的真源是**设置控制器**而不是 `DisplayPrefs`，所以不能在 `sync` 里带）。
    //
    // `force: true` **不会**冲掉临时覆盖：syncer 发的是它的 `active()`
    // （临时覆盖 > 产品值）。改主题 / 调音量等任意偏好变更走到这里，
    // 临时值照样在（W7 的确定性缺陷 1，见 action_scales_sync.dart）。
    _syncActionScalesNow(force: true);
    // 偏好「已生效」的最后一道：背景轮播在这里重新对表。
    //
    // 为什么不放在 `_updatePrefs` 里：那里是**提交**的那一刻，
    // 而轮播要读的是生效后的值；`_applyPrefs` 正是这条「已生效」的唯一漏斗
    // （首次靠 `onReady`、之后每次变更都过它）。
    syncSlideshow(prefs);
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
      _shellImageMessage =
          '偏好没能写入本机存储（无痕模式 / 存储被禁 / 配额满）——本次会话有效，刷新会丢。';
      _shellImageFailed = true;
    }
    // 静音与音量是纯本机输出设置，不经渲染面——直接作用于 AudioPlayer。
    _audio.muted = next.muted;
    _audio.volume = next.volume;
    // 传 `next` 而不是让它去读 `widget.prefs`（那还是旧值）。
    _applyPrefs(next);
  }

  /// 选一张图进背景库。
  ///
  /// 字节进 IndexedDB（上限 [kBackgroundImageMaxBytes]）。当前这一项由
  /// [DisplayPrefs.stageProjectionUrl] 投影到舞台；超过舞台帧上限的图壳照样画，
  /// 舞台回到纯色。
  Future<void> _pickImage() async {
    final ({String? dataUrl, String? error}) picked = await pickImageDataUrl();
    if (!mounted) return;
    final String? dataUrl = picked.dataUrl;
    if (dataUrl == null) {
      if (picked.error != null) {
        _setImageMessage(text: picked.error!, failed: true);
      }
      return;
    }
    // 偏好在 await 之前重新读：选图对话框可能开了几秒，期间别的偏好可能已变。
    final DisplayPrefs prefs = widget.prefs;
    final int bytes = dataUrlBytes(dataUrl);
    if (bytes > kBackgroundImageMaxBytes) {
      _setImageMessage(
        text: '单张 ${_mb(bytes)} 超过上限 ${_mb(kBackgroundImageMaxBytes)}'
            '——已有的一张都没动。',
        failed: true,
      );
      return;
    }
    final String id = backgroundIdOf(dataUrl);
    if (prefs.backgrounds.any(
      (BackgroundItem b) => b is BackgroundImage && b.id == id,
    )) {
      _setImageMessage(
        text: '背景库里已经有这张图了（${prefs.backgrounds.length} 项）',
        failed: false,
      );
      return;
    }
    final BackgroundItem item = BackgroundImage(id: id, dataUrl: dataUrl);
    if (!DisplayPrefs.canAddBackground(prefs.backgrounds, item)) {
      _setImageMessage(
        text: '加不进背景库（${_rejectReason(prefs.backgrounds, item)}）——'
            '已有的一张都没动。',
        failed: true,
      );
      return;
    }
    // 先存字节，成功了才改偏好。反过来会出现「界面上有了、刷新后消失」。
    final bool stored = await storeBackground(widget.store, id, dataUrl);
    if (!mounted) return;
    if (!stored) {
      _setImageMessage(
        text: '浏览器存不下这张图（磁盘配额满 / 无痕模式 / 站点数据被禁）——'
            '已有的一张都没动。换张小一点的图，或先删掉几张。',
        failed: true,
      );
      return;
    }
    final DisplayPrefs after = widget.prefs;
    _updatePrefs(
      after.copyWith(
        backgrounds: <BackgroundItem>[...after.backgrounds, item],
      ),
    );
    _setImageMessage(
      text: '已加进背景库（${_mb(bytes)}，共 ${after.backgrounds.length + 1} 项，'
          '会记住）',
      failed: false,
    );
  }

  /// 为什么不加进去 —— **一句人话**，不说「false」。
  String _rejectReason(List<BackgroundItem> current, BackgroundItem item) {
    if (current.length >= kBackgroundMaxCount) {
      return '最多 $kBackgroundMaxCount 项，先删一张';
    }
    if (item is BackgroundImage &&
        backgroundBytesOf(item) > kBackgroundImageMaxBytes) {
      return '单张超过 ${_mb(kBackgroundImageMaxBytes)}，换张小一点的';
    }
    return '这一项当前画不出来（字节没读回来）';
  }

  Future<void> _pickShellImage() => _pickImage();

  /// 清空背景库，并忘掉 IndexedDB 里的字节。
  void _clearShellImage() {
    final DisplayPrefs prefs = widget.prefs;
    for (final BackgroundItem item in prefs.backgrounds) {
      if (item is BackgroundImage) {
        unawaited(forgetBackground(widget.store, item.id));
      }
    }
    _updatePrefs(prefs.copyWith(backgrounds: const <BackgroundItem>[]));
    _setImageMessage(text: '已清空背景库，壳与舞台回到主题底色', failed: false);
  }

  /// 从背景库移除第 N 项。
  ///
  /// 越界**静默忽略**而不是抛：那是从 UI 事件来的下标，
  /// 而库长度可能在一次重建里变过。
  void _removeBackground(int index) {
    final DisplayPrefs prefs = widget.prefs;
    final List<BackgroundItem> items = prefs.backgrounds;
    if (index < 0 || index >= items.length) return;
    final BackgroundItem dropped = items[index];
    final List<BackgroundItem> next = List<BackgroundItem>.of(items)
      ..removeAt(index);
    _forget(dropped);
    _updatePrefs(prefs.copyWith(backgrounds: next));
    _setImageMessage(text: '已移除，剩 ${next.length} 项', failed: false);
  }

  /// 删一项的**字节**（清单与字节必须同进同退，否则磁盘上留孤儿）。
  void _forget(BackgroundItem item) {
    if (item is BackgroundImage) {
      unawaited(forgetBackground(widget.store, item.id));
    }
  }

  /// 拖动排序背景库（旧下标 → 新下标）。
  ///
  /// 算术在 [reorderBackgroundItems]（纯函数，见 `data/background_reorder.dart`）。
  /// 语义细节：`ReorderableListView.onReorderItem` 交来的 `newIndex`
  /// **已经**是「把被拖那一项移走之后」的目标下标（SDK 明文），
  /// 所以这里**绝不能再减 1**——老实现多减一次的表现是
  /// 「向下拖早一格、**向下拖一格完全没反应**」（P0-3）。
  void _reorderBackground(int oldIndex, int newIndex) {
    final DisplayPrefs prefs = widget.prefs;
    final List<BackgroundItem> next = reorderBackgroundItems(
      prefs.backgrounds,
      oldIndex,
      newIndex,
    );
    // 纯函数在「什么都不用做」时返回同一个实例，据此跳过写盘。
    if (identical(next, prefs.backgrounds)) return;
    _updatePrefs(prefs.copyWith(backgrounds: next));
  }

  /// 批量移除（**升序**的下标集合）。
  ///
  /// 先收集项再统一删：边删边按下标取会跳过元素
  /// （删掉 index 2 之后，原来的 index 3 变成了 2）。
  void _removeBackgrounds(List<int> indices) {
    final DisplayPrefs prefs = widget.prefs;
    final List<BackgroundItem> items = prefs.backgrounds;
    final Set<int> drop = <int>{
      for (final int i in indices)
        if (i >= 0 && i < items.length) i,
    };
    if (drop.isEmpty) return;
    final List<BackgroundItem> next = <BackgroundItem>[
      for (int i = 0; i < items.length; i++)
        if (!drop.contains(i)) items[i],
    ];
    for (final int i in drop) {
      _forget(items[i]);
    }
    _updatePrefs(prefs.copyWith(backgrounds: next));
    _setImageMessage(
      text: '已删除 ${drop.length} 项，剩 ${next.length} 项',
      failed: false,
    );
  }

  /// 点缩略图 = 切到这一张看效果（不落盘：索引是运行时状态）。
  void _previewBackground(int index) {
    _jumpBackground(index);
  }

  /// 落一条选图 / 清图结果。
  void _setImageMessage({required String text, required bool failed}) {
    _shellImageMessage = text;
    _shellImageFailed = failed;
    _refresh();
  }
}
