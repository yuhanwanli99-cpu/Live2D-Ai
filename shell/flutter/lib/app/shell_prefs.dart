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
      _stageImageMessage = '偏好没能写入本机存储（无痕模式 / 存储被禁 / 配额满）——本次会话有效，刷新会丢。';
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
  /// # 两条通道，两套上限（2026-09-27 修）
  ///
  /// | 通道 | 字节住哪 | 上限从哪来 |
  /// | --- | --- | --- |
  /// | **背景库** | IndexedDB | [kBackgroundImageMaxBytes]（24 MB，手抖保护）|
  /// | **舞台那张** | localStorage + WS `stage-bg` | [kStageImageMaxChars]（**协议**限制）|
  ///
  /// 舞台那张的上限**不是**存储限制而是**协议限制**：它要塞进一帧
  /// WebSocket 消息交给 wasm 解码，所以不能因为「浏览器还有地方存」就放开。
  /// 这就是为什么超大图请走背景库——那条路不经过渲染面。
  ///
  /// # 两条结果，都要如实说
  ///
  /// - 存进去了 → 「已加进背景库（第 N 项，x MB，会记住）」；
  /// - 存不进去 → 说清楚是**哪一关**拦的、已有的一张有没有动。
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
    // 偏好**在 await 之前重新读**：选图对话框可能开了几秒，期间主题 / 音量
    // 可能已变，用进对话框之前那份会把那次改动吞掉。
    final DisplayPrefs prefs = widget.prefs;
    // 「来源」是判据：来源=舞台那张 时，换图改的是 [stageImage]；
    // 来源=背景库 时，图片进 [backgrounds]。
    final bool shared =
        !forShell ||
        prefs.backgroundSource == DisplayPrefs.backgroundSourceStageImage;
    if (shared) {
      if (dataUrl.length > kStageImageMaxChars) {
        if (forShell) {
          _setImageMessage(
            forShell: true,
            text:
                '这张图 ${_mb(dataUrlBytes(dataUrl))}，走**舞台背景**这条通道只能到 '
                '${_mb(dataUrlBytes('x' * kStageImageMaxChars))}'
                '（要整帧塞进渲染面）。'
                '把「背景来源」切成**背景库**就能放进去了——那条路不走渲染面。',
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
              '这张图 ${_mb(dataUrlBytes(dataUrl))} 超过舞台背景上限 '
              '${_mb(dataUrlBytes('x' * kStageImageMaxChars))}——'
              '**本次有效，重新打开页面会丢失**。'
              '想永久用它，请把「背景来源」切成**背景库**（上限 24 MB）。',
          failed: true,
        );
        return;
      }
      _updatePrefs(prefs.copyWith(stageImage: dataUrl));
      _setImageMessage(
        forShell: forShell,
        text: '已应用（与舞台共用同一张图，${_mb(dataUrlBytes(dataUrl))}，会记住）',
        failed: false,
      );
      return;
    }

    // ── 背景库：字节进 IndexedDB，偏好里只留一个 id ──
    final int bytes = dataUrlBytes(dataUrl);
    if (bytes > kBackgroundImageMaxBytes) {
      _setImageMessage(
        forShell: true,
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
        forShell: true,
        text: '背景库里已经有这张图了（${prefs.backgrounds.length} 项）',
        failed: false,
      );
      return;
    }
    final BackgroundItem item = BackgroundImage(id: id, dataUrl: dataUrl);
    if (!DisplayPrefs.canAddBackground(prefs.backgrounds, item)) {
      _setImageMessage(
        forShell: true,
        text: '加不进背景库（${_rejectReason(prefs.backgrounds, item)}）——'
            '已有的一张都没动。',
        failed: true,
      );
      return;
    }
    // **先存字节，成功了才改偏好**。反过来会出现「界面上有了、刷新后消失」，
    // 而提示写着「已加进背景库」——P4 的头号形态。
    final bool stored = await storeBackground(widget.store, id, dataUrl);
    if (!mounted) return;
    if (!stored) {
      _setImageMessage(
        forShell: true,
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
      forShell: true,
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

  Future<void> _pickStageImage() => _pickImage(forShell: false);

  Future<void> _pickShellImage() => _pickImage(forShell: true);

  /// 清掉背景图。同步开着时清的是**共用那张**（舞台与壳一起回到各自底色）。
  void _clearImage({required bool forShell}) {
    final DisplayPrefs prefs = widget.prefs;
    // 「来源」是判据：来源=舞台那张 时，换图改的是 [stageImage]；
    // 来源=背景库 时，图片进 [backgrounds]。
    final bool shared =
        !forShell ||
        prefs.backgroundSource == DisplayPrefs.backgroundSourceStageImage;
    if (shared) {
      _updatePrefs(prefs.copyWith(clearStageImage: true));
    } else {
      // **字节也要删**：删清单不删字节，磁盘上的图会永远留着
      // （IndexedDB 不会自动回收没人引用的记录）。
      for (final BackgroundItem item in prefs.backgrounds) {
        if (item is BackgroundImage) {
          unawaited(forgetBackground(widget.store, item.id));
        }
      }
      _updatePrefs(prefs.copyWith(backgrounds: const <BackgroundItem>[]));
    }
    _setImageMessage(
      forShell: forShell,
      text: shared ? '已清除背景图，舞台与壳回到各自底色' : '已清空背景库，壳回到主题底色',
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
    _setImageMessage(
      forShell: true,
      text: '已移除，剩 ${next.length} 项',
      failed: false,
    );
  }

  /// 删一项的**字节**（清单与字节必须同进同退，否则磁盘上留孤儿）。
  void _forget(BackgroundItem item) {
    if (item is BackgroundImage) {
      unawaited(forgetBackground(widget.store, item.id));
    }
  }

  /// 拖动排序背景库（旧下标 → 新下标）。
  ///
  /// 语义细节：拖动是**按身份**（那一项）而不是按「下标」。
  /// 所以这里先取出那一项、删掉、再插到新位置——
  /// 直接对列表做 `removeAt/insert` 在 `old < new` 时会差一位
  /// （`ReorderableListView` 传的是「移除之后」的下标）。
  void _reorderBackground(int oldIndex, int newIndex) {
    final DisplayPrefs prefs = widget.prefs;
    final List<BackgroundItem> items = prefs.backgrounds;
    if (oldIndex < 0 || oldIndex >= items.length) return;
    int target = newIndex;
    if (target > oldIndex) target -= 1;
    target = target.clamp(0, items.length - 1);
    if (target == oldIndex) return;
    final List<BackgroundItem> next = List<BackgroundItem>.of(items);
    final BackgroundItem moved = next.removeAt(oldIndex);
    next.insert(target, moved);
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
      forShell: true,
      text: '已删除 ${drop.length} 项，剩 ${next.length} 项',
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

  /// 点缩略图 = 切到这一张看效果（**不落盘**：索引是运行时状态）。
  void _previewBackground(int index) {
    _jumpBackground(index);
  }

  /// 往背景库加一个内置图案。
  ///
  /// **图案不占配额**（它没有 dataURL），所以这里唯一要拦的是「项数上限」。
  void _addPattern(int id) {
    final DisplayPrefs prefs = widget.prefs;
    final BackgroundPattern item = BackgroundPattern(id);
    if (!DisplayPrefs.canAddBackground(prefs.backgrounds, item)) {
      _setImageMessage(
        forShell: true,
        text: '背景库最多 $kBackgroundMaxCount 项，先删一张',
        failed: true,
      );
      return;
    }    if (prefs.backgrounds.any((BackgroundItem b) => b.sameAs(item))) {
      _setImageMessage(forShell: true, text: '已经有这个图案了', failed: false);
      return;
    }
    _updatePrefs(
      prefs.copyWith(backgrounds: <BackgroundItem>[...prefs.backgrounds, item]),
    );
    _setImageMessage(forShell: true, text: '已加进背景库（内置图案，不占存储）', failed: false);
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
