part of 'app_shell.dart';

class AppShellState extends State<AppShell> {
  /// 稳定的舞台 key（reparent 时保留状态）。
  final GlobalKey stageKey = GlobalKey(debugLabel: 'app-stage');

  /// 内联侧板是否展开（expanded 专用；浮层宿主由路由自己管生命周期）。
  bool settingsOpen = false;

  /// **内部**分区状态。
  ///
  /// 为什么需要它，而不是直接读 `widget.section`：
  /// `showModalBottomSheet` 的 `builder` **只在路由入栈时构建一次**，
  /// 之后外壳 `setState` 不会重建浮层内容。于是「在浮层里点另一个分区」
  /// 或「刚点 rail 就打开浮层」都会显示**旧的**分区——用户看到点了没反应。
  /// 用一个 `ValueNotifier` 作为导航的单点真相，宿主与浮层都订阅它。
  ///
  /// 对外的受控接口（`widget.section` + `onSectionChanged`）保持不变：
  /// 外部改了 `widget.section` 时由 `didUpdateWidget` 同步进来。
  late final ValueNotifier<SettingsSection> sectionNotifier =
      ValueNotifier<SettingsSection>(widget.section);

  /// 设置**数据**的代际：`settingsChanges` 每通知一次 +1（草稿 / 远端视图 /
  /// 保存结果都在里面，而它们都会被 [AppShell.sectionBuilder] 读走）。
  ///
  /// 与 [AppShell.settingsRevision] 分工：那个管宿主状态，这个管设置数据——外壳
  /// 看不见后者（浮层那条路连外壳的 `setState` 都进不去）。用**独立的
  /// `ValueNotifier`** 而不是「监听器里顺手改个字段」：它自己订阅、自己驱动
  /// 重建，于是与监听器注册顺序无关（靠「谁先跑」决定代际会静默冻住面板）。
  final ValueNotifier<int> _settingsTick = ValueNotifier<int>(0);

  Listenable? _settingsChangesSeen;

  void _subscribeSettingsChanges() {
    if (identical(_settingsChangesSeen, widget.settingsChanges)) return;
    _settingsChangesSeen?.removeListener(_onSettingsChanges);
    _settingsChangesSeen = widget.settingsChanges;
    widget.settingsChanges.addListener(_onSettingsChanges);
  }

  void _onSettingsChanges() => _settingsTick.value++;

  @override
  void initState() {
    super.initState();
    // 键盘解锁：全局处理器**不吞键**（返回 false），所以不会干扰 TextField
    // 或任何快捷键。用全局处理器而不是 `Focus.onKeyEvent` 是因为后者依赖焦点
    // 冒泡——纯键盘用户在焦点还没落到我们节点上时按键不会到达。
    HardwareKeyboard.instance.addHandler(_onKey);
    _subscribeSettingsChanges();
  }

  @override
  void didUpdateWidget(AppShell oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 外部（宿主）改了分区 → 同步进来。内部已一致时不触发通知（避免空重建）。
    if (widget.section != sectionNotifier.value) {
      sectionNotifier.value = widget.section;
    }
    // 宿主换了另一个 `settingsChanges`（测试里常见）→ 换订阅对象。
    _subscribeSettingsChanges();
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_onKey);
    _settingsChangesSeen?.removeListener(_onSettingsChanges);
    _settingsTick.dispose();
    sectionNotifier.dispose();
    super.dispose();
  }

  bool _onKey(KeyEvent event) {
    if (event is KeyDownEvent) widget.onUserGesture?.call();
    return false; // 不吞：事件继续正常派发。
  }

  /// 关掉内联 / 整页设置。
  ///
  /// **脏草稿要走同一套 `confirmDiscard`**（P0，2026-09-20）：过去这里直接
  /// `setState`，关设置完全不问——「换分区问、关设置不问」两套语义。
  /// `confirmDiscard` 为空（测试 / 未注入宿主）时保持原来的同步行为。
  Future<void> closeSettings() async {
    if (!settingsOpen) return;
    if (widget.settingsDirty) {
      final Future<bool> Function()? ask = widget.confirmDiscard;
      if (ask != null) {
        final bool leave = await ask();
        if (!leave || !mounted) return;
      }
    }
    if (!mounted) return;
    setState(() => settingsOpen = false);
  }

  void _select(SettingsSection section) {
    sectionNotifier.value = section;
    widget.onSectionChanged?.call(section);
  }

  /// 打开会话列表（底部浮层）。
  ///
  /// 与设置浮层同一套宿主约定：`showModalBottomSheet` + 指针垫层
  ///（浮层压在舞台上，不垫就点不着）。**重命名与删除都在浮层内部行内完成**，
  /// 不新开 overlay——理由见 `ui/session_sheet.dart` 的头注。
  Future<void> openSessions() => showSessionSheet(
    context: context,
    // 宿主没接 listenable / 读回调时（旧测试、孤立泵）回落打开那一刻的快照。
    listenable: widget.sessionsListenable ?? const NeverNotifies(),
    sessionsOf: widget.readSessions ?? () => widget.sessions,
    activeIdOf: widget.readActiveSessionId ?? () => widget.activeSessionId,
    onNew: () => widget.onNewSession?.call(),
    onSelect: (String id) => widget.onSelectSession?.call(id),
    onRename: (String id, String title) =>
        widget.onRenameSession?.call(id, title),
    onDelete: (String id) => widget.onDeleteSession?.call(id),
  );

  /// 「设置」按钮 / compact 聊天面板头的入口 → 打开设置。
  ///
  /// 三种断点的分流就在这里（也是唯一的入口）：
  /// - expanded：内联侧板（纯 `setState`，不开路由）；
  /// - compact：**页内整页过渡**（[PageCrossFade]）。2026-09-11（P2-1）改的：
  ///   过去是「抽屉（第 1 层）→ 浮层（第 2 层）」**两步跳**，观感上像弹了两次，
  ///   而且「先选分区」这件事**恰好**掩盖了一个加载 bug（见下）；
  /// - medium：底部浮层。
  ///
  /// # 打开之前必须先让宿主**加载当前分区的内容**（2026-09-11 修）
  ///
  /// 分区内容是由宿主按需加载的（`SettingsController.load()`），而这里过去
  /// 只切 `settingsOpen` —— 于是 expanded / medium 上直接点「设置」，看到的是
  /// 一个写着**「读不到服务端设置 / 未知原因」**的空壳，**必须再点一个分区**
  /// 才会加载。compact 之所以看起来正常，纯属巧合：它先弹抽屉、必然选一个
  /// 分区，恰好走了加载那条路。
  ///
  /// 所以加载**不能**挂在「分区变了」上（那是结果，不是原因）——
  /// 「打开设置」本身就是那个原因。改成整页过渡之后那条巧合没有了，
  /// 这条调用也成了 compact 唯一的加载触发点。
  Future<void> openSettings({SizeClass? sizeClass}) async {
    final SizeClass sc =
        sizeClass ?? Breakpoints.sizeClassOf(MediaQuery.sizeOf(context).width);

    // inline 与 page 都是**页内**宿主：切一个布尔 + 触发加载，不开路由。
    if (settingsHostOf(sc) != SettingsHost.sheet) {
      setState(() => settingsOpen = true);
      widget.onEnsureSectionLoaded?.call();
      return;
    }

    widget.onEnsureSectionLoaded?.call();
    if (!mounted) return;

    await showSettingsSheet(
      context: context,
      // 用**浮层自己的** context 关掉自己：用外壳的 context 去 `maybePop`
      // 依赖「浮层恰好是栈顶」这个巧合，不稳健。
      //
      // 浮层压在舞台上（medium 下几乎整块）⇒ 必须垫指针垫层，
      // 否则整块设置面板「看得见、点不着、也滑不动」。
      builder: (BuildContext sheetContext) => ListenableBuilder(
        // `PopScope.canPop` 必须**跟着草稿变**：浮层 builder 只跑一次，
        // 不订阅的话 canPop 冻结在打开那一刻（打开时通常还不脏）。
        listenable: widget.settingsChanges,
        builder: (BuildContext context, Widget? _) => PopScope(
          // 脏草稿时不放行 Esc / 点遮罩，先过确认框——与换分区、✕ 同一个入口。
          canPop: !widget.settingsDirty,
          onPopInvokedWithResult: (bool didPop, Object? _) {
            if (didPop) return;
            unawaited(_closeSheetGuarded(sheetContext));
          },
          child: StagePointerInterceptor(
            child: _settingsSurface(
              onClose: () => unawaited(_closeSheetGuarded(sheetContext)),
            ),
          ),
        ),
      ),
    );
  }

  /// 关设置浮层前先过草稿拦截（Esc / 遮罩 / ✕ 三条路都到这里）。
  Future<void> _closeSheetGuarded(BuildContext sheetContext) async {
    if (widget.settingsDirty) {
      final Future<bool> Function()? ask = widget.confirmDiscard;
      if (ask != null && !await ask()) return;
    }
    if (!sheetContext.mounted) return;
    Navigator.of(sheetContext).pop();
  }

  /// 设置内容（三种宿主共用**同一个** widget）。
  ///
  /// 订阅**两件事**：
  /// - `sectionNotifier`：分区导航（浮层的 builder 只跑一次，不订阅就换不了区）；
  /// - `settingsChanges`：设置数据（同上——不订阅就永远停在加载中的转圈）。
  ///
  /// 两者都必须在**浮层内部**订阅。靠外层重建是没有用的：
  /// 浮层的 builder 不会因为外壳 `setState` 而重跑。
  Widget _settingsSurface({
    required VoidCallback onClose,
    bool narrow = false,
  }) => ListenableBuilder(
    listenable: widget.settingsChanges,
    builder: (BuildContext context, Widget? _) =>
        ValueListenableBuilder<SettingsSection>(
          valueListenable: sectionNotifier,
          builder: (BuildContext context, SettingsSection section, Widget? _) =>
              SettingsScaffold(
                sections: widget.sections,
                selected: section,
                // 窄屏（compact 整页）把分区导航压成单行横向滚动。
                narrow: narrow,
                // 换分区也要过拦截（「离开分区」是规格 §4.3 的三处拦截之一）。
                onSelect: _selectGuarded,
                onClose: onClose,
                dirty: widget.settingsDirty,
                saving: widget.settingsSaving,
                onSave: widget.onSaveSettings,
                onDiscard: widget.onDiscardSettings,
                statusMessage: widget.settingsStatus,
                statusIsError: widget.settingsStatusIsError,
                // 背景域的**运行时**上下文（DEC-6 / 9b）：设置面板里的
                // 「外观与互动」要显示「此刻画的是哪一项」、并按它决定铺法 /
                // 位置区的显隐。判据（_currentBackground）只在本文件一处，
                // 这里只负责注入，不另算一份。
                child: BackgroundRuntimeScope(
                  index: widget.backgroundIndex,
                  current: _currentBackground,
                  hydrating: widget.backgroundHydrating,
                  // 设置数据的代际：自己订阅、自己驱动重建（浮层那条路
                  // 根本没有外壳重建）。
                  child: ValueListenableBuilder<int>(
                    valueListenable: _settingsTick,
                    builder: (BuildContext _, int tick, Widget? _) =>
                        _pane(context, section, tick),
                  ),
                ),
              ),
        ),
  );

  /// 当前要画的那一项：背景库里轮播索引指向的项。
  BackgroundItem? get _currentBackground {
    final List<BackgroundItem> items = widget.prefs.backgrounds;
    if (items.isEmpty) return null;
    return items[widget.backgroundIndex.clamp(0, items.length - 1)];
  }

  // ── 分区内容的缓存（F-0005-2）──
  // 任一条代际变了才重算；没变就复用同一个 widget 实例（父级重建被短路）。

  Widget? _paneCache;

  /// 缓存对应的那条代际（记录的结构相等性就是判据）。
  (int revision, int tick, SettingsSection section)? _paneKey;

  /// 当前分区的**内容**（[AppShell.sectionBuilder] 的产物），按代际缓存。
  ///
  /// 为什么不每帧重算（F-0005-2）：外壳会被聊天增量（毫秒级）高频重建，而设置
  /// 面板常驻树里（折叠**不卸载**，见 `CollapsiblePanel` /
  /// `settings_panel_keepalive_test.dart`）。失效条件只有三条，都真的与内容
  /// 有关：宿主代际、设置数据代际、换分区。三种宿主（内联 / 浮层 / 整页）
  /// 共用同一实例是安全的——同一时刻只有一个在树上。
  ///
  /// ⚠️ `context` 必须是 `BackgroundRuntimeScope` **之上**那一层
  /// （`SettingsScaffold.child` 的实参位置）：外观区靠一个 `Builder` 去读 scope，
  /// 约定与回归见 `test/app_shell_background_test.dart`；别改成 tick 那一层的。
  Widget _pane(BuildContext context, SettingsSection section, int tick) {
    final (int, int, SettingsSection) key = (
      widget.settingsRevision,
      tick,
      section,
    );
    if (_paneCache == null || _paneKey != key) {
      _paneKey = key;
      _paneCache = widget.sectionBuilder(context, section);
    }
    return _paneCache!;
  }

  /// 有没有东西要画（决定脚手架底要不要让出来）。
  ///
  /// 判据**只有** [DisplayPrefs.hasBackgroundAt] 一处。2026-09-27 之前这里
  /// 另写了一份，而且多了一条 `opacity < 1`：滑杆推到 100% 的那一刻，
  /// 设置页的说明还写着「聊天面板会跟着透」，实际面板已经退回不透明——
  /// **界面在骗人**。那份条件连同它造成的断层一起删掉了。
  bool get _hasBackground => widget.prefs.hasBackgroundAt(_currentBackground);

  /// 换分区前先过草稿拦截。
  void _selectGuarded(SettingsSection next) {
    final Future<bool> Function()? ask = widget.confirmDiscard;
    if (ask == null || next == sectionNotifier.value) {
      _select(next);
      return;
    }
    unawaited(() async {
      if (await ask()) _select(next);
    }());
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      // 指针手势解锁（既有行为，保留）。
      behavior: HitTestBehavior.translucent,
      onPointerDown: (_) => widget.onUserGesture?.call(),
      child: LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) {
          final SizeClass sizeClass = Breakpoints.sizeClassOf(
            constraints.maxWidth,
          );
          final bool compact = sizeClass.isCompact;
          final SettingsHost host = settingsHostOf(sizeClass);
          final bool inlineSettings = host == SettingsHost.inline;
          final bool pageSettings = host == SettingsHost.page;

          // **必须走 `StageHost`**：加载/错误覆盖层与舞台语义都在它里面。
          // 直接铺 `RepaintBoundary(child: widget.stage)` 会让那些东西全部失效。
          final Widget stage = StageHost(
            key: stageKey,
            stage: widget.stage,
            phase: widget.stagePhase,
            progress: widget.stageProgress,
            errorMessage: widget.stageError,
            onRetry: widget.onRetryStage,
            modelName: widget.modelName,
          );

          // ── 关键：三种断点共用同一个 Flex，只换 direction 与子项 flex。
          //    子树形状不变 ⇒ 舞台 Element 不被反激活 ⇒ iframe 不重建。 ──
          final Widget body = Flex(
            direction: compact ? Axis.vertical : Axis.horizontal,
            children: <Widget>[
              Flexible(
                flex: compact ? 3 : 1,
                child: Stack(
                  children: <Widget>[
                    Positioned.fill(child: stage),
                    // 断线横幅常驻在树里，靠**纵向折叠**滑入/滑出
                    // （2026-09-11，P1-2）。条件插入会让它「啪」地出现，
                    // 而断线本身已经是一次惊吓，不该再补一个跳变。
                    //
                    // 垫层跟着 `enabled` 一起收：横幅折到 0 高之后，
                    // 舞台顶端那一条必须还给 iframe（否则拖不动模型）。
                    Positioned(
                      top: 0,
                      left: 0,
                      right: 0,
                      child: StagePointerInterceptor(
                        enabled: widget.wsStatus != WsStatus.connected,
                        child: CollapsiblePanel(
                          expanded: widget.wsStatus != WsStatus.connected,
                          axis: Axis.vertical,
                          child: OfflineBanner(
                            status: widget.wsStatus,
                            onRetry: widget.onRetryConnection,
                            devMode: widget.devMode,
                          ),
                        ),
                      ),
                    ),
                    // 内联侧板**常驻在树里**（`expanded` 只折宽度，不卸载）：
                    // 关掉再打开时滚动位置、分区草稿、内联测试结果都还在。
                    // 见 `InlineSettingsDock` 与 `CollapsiblePanel` 的头注。
                    if (inlineSettings)
                      Positioned.fill(
                        child: InlineSettingsDock(
                          expanded: settingsOpen,
                          onDismiss: () => unawaited(closeSettings()),
                          child: _settingsSurface(
                            onClose: () => unawaited(closeSettings()),
                          ),
                        ),
                      ),
                    // 舞台浮标：**压在舞台之上**，不占布局、不挤舞台宽度。
                    // 压着 iframe 就必须垫指针垫层，否则四键全点不着。
                    if (widget.stageCorner != null)
                      Positioned(
                        right: 0,
                        bottom: 0,
                        child: StagePointerInterceptor(
                          child: widget.stageCorner!,
                        ),
                      ),
                  ],
                ),
              ),
              if (compact)
                Divider(height: 1, color: appColorsOf(context).hairline)
              else
                VerticalDivider(width: 1, color: appColorsOf(context).hairline),
              // `flex: 0` = 不参与弹性分配，用自身尺寸（`SizedBox(width: …)`）。
              // compact 下反过来：聊天按 3:2 分高度。
              Flexible(
                flex: compact ? 2 : 0,
                child: SizedBox(
                  width: compact
                      ? null
                      : (sizeClass.isExpanded
                            ? NavMetrics.chatWidthExpanded
                            : NavMetrics.chatWidthMedium),
                  child: _chat(compact: compact),
                ),
              ),
            ],
          );

          final Widget scaffold = Scaffold(
            // **有背景 → 脚手架底必须完全透明**（`Colors.transparent`），
            // 让 [ShellBackdrop] 的图透上来。
            //
            // ⚠️ `null` 不是「不变」而是「退回
            // `ThemeData.scaffoldBackgroundColor`」——那是 `palette.stage`，
            // **不透明**，会整块盖住背景。这两个分支在 2026-09-27 被写反过一次
            // （`_hasBackground ? null : Colors.transparent`），症状是
            // 「导入图也看不见」；A/B 截图（两态只差一个亮度档）把它抓了出来。
            // `app_shell_background_test.dart` 现在逐分支钉住方向。
            backgroundColor: _hasBackground ? Colors.transparent : null,
            appBar: AppBar(
              // 2026-09-27：顶栏**只在有背景时**补上一层与面板同色的面。
              //
              // 为什么要补：顶栏是**完全透明**的，而下面的聊天面板是
              // `surface @ panelAlpha`。同一张背景图在两处的叠法不同 →
              // 顶栏是「原图」、面板是「图透过 0.55 的面」，中间那道
              // **不连续的接缝**正是用户说的「上侧标题和下面不统一」。
              //
              // 只在 `backdropVisible` 时加：没有背景时补了等于凭空加一层
              // 半透明色，整条顶栏会发灰。
              backgroundColor: _hasBackground
                  ? appPaletteOf(context).surface
                        .withValues(alpha: appColorsOf(context).panelAlpha)
                  : Colors.transparent,
              scrolledUnderElevation: 0,
              title: const Text('Live2D Ai'),
              actions: <Widget>[
                StatePill(
                  phase: widget.phase,
                  compact: true,
                  // 胶囊的「出路」= 出错时重发上一条用户消息。组合根在
                  // **没有上一条可重发**时给的就是 null ⇒ 这里传 null ⇒
                  // 胶囊**不可点**（Semantics 也不带 button 角色），而且它的
                  // 标签只说状态（「出错」），从不承诺点按会重发——
                  // **不许把静默 no-op 换个地方**（2026-10-06 裁决 R4-T5）。
                  onTap: widget.error == null ? null : widget.onRetryLast,
                ),
                const SizedBox(width: Space.s2),
                ConnectionBadge(
                  status: widget.wsStatus,
                  onRetry: widget.onRetryConnection,
                ),
                // **文字按钮，不是齿轮图标**（用户裁决「尽量少用图片用文字
                // 做按钮」）。「设置」两个字不需要翻译，齿轮要先认图标。
                //
                // compact 不在这里放：它把整个工作台换成了设置页
                // （`PageCrossFade`），入口在聊天面板头部（那一屏本来就窄，
                // AppBar 上再挤一个按钮会跟两个徽标打架）。
                if (!compact) ...<Widget>[
                  TextButton(
                    onPressed: () => unawaited(openSessions()),
                    child: const Text('会话'),
                  ),
                  TextButton(
                    onPressed: () => openSettings(sizeClass: sizeClass),
                    child: const Text('设置'),
                  ),
                ],
                const SizedBox(width: Space.s2),
              ],
            ),
            // compact 的「整页设置」包在最外层：它要盖住**整个 body**
            // （舞台 + 聊天），而不是只盖住其中一列。
            body: pageSettings
                ? PageCrossFade(
                    showSecond: settingsOpen,
                    first: body,
                    second: StagePointerInterceptor(
                      // 收起时把舞台那一块还给 iframe（否则 compact 下
                      // **永远拖不动模型**）。与 `CollapsiblePanel` 那处
                      // 同一类问题、同一个开关。
                      enabled: settingsOpen,
                      child: _compactSettingsPage(),
                    ),
                  )
                : body,
          );

          // 快捷键挂在**根**（`CallbackShortcuts` 最省样板，且不吞未匹配的键）。
          // 外面再包一个 `FocusTraversalGroup`：整页的 Tab 顺序是确定的，
          // 不会因为某次重构而静默改变。
          //
          // 最外层是**启动揭示**（`AppDurations.reveal`，仅此一处长动画）：
          // 舞台 iframe 的第一帧是空白，这一次淡入把那段白挡在背后。
          // 它的控制器住在 State 里，所以外壳被 WS 事件频繁重建也**只播一次**。
          // 壳根：**全局背景**在最外一层，脚手架（含 AppBar）在它上面。
          // 没有背景图时 [ShellBackdrop] 只画一层主题底色，与原来的
          // `scaffoldBackgroundColor` 同色，观感不变。
          return ShellBackdrop(
            baseColor: appPaletteOf(context).stage,
            item: _currentBackground,
            // DEC-4 全局开关（2026-09-28 · R6-a 加的两行传参之一）：
            // 关掉 = 不画图、只留底色。与下面那行各自独立，
            // 都不依赖 R6-b 之后对本文件的改动（DEC-6 传运行时索引等）。
            enabled: widget.prefs.backgroundEnabled,
            opacity: widget.prefs.backgroundOpacity,
            // 铺满、居中、不模糊、遮罩自动。这些不再是用户旋钮。
            tileSize: DisplayPrefs.defaultTileSize,
            fit: DisplayPrefs.fitCover,
            align: DisplayPrefs.defaultImageAlign,
            blur: DisplayPrefs.defaultBackgroundBlur,
            scrim: DisplayPrefs.defaultBackgroundScrim,
            uiTransparency: widget.prefs.uiTransparency,
            patternColors: _currentBackground is BackgroundPattern
                ? patternColorsFor(
                    (_currentBackground! as BackgroundPattern).id,
                    appPaletteOf(context),
                  )
                : null,
            child: StartupReveal(
              child: CallbackShortcuts(
                bindings: widget.shortcuts.bindings(),
                child: FocusTraversalGroup(
                  policy: OrderedTraversalPolicy(),
                  child: scaffold,
                ),
              ),
            ),
          );
        },
      ),
    );
  }

  /// compact 的整页设置（[PageCrossFade] 的第二页）。
  ///
  /// 外形是一次性的：一层实色底 + 内边距 + 内容，**没有卡片、没有圆角、
  /// 没有阴影**——它就是「另一个页面」，不是浮在工作台上的面板。
  ///
  /// 底必须是**不透明**的：第一页（工作台）在它后面淡出，透出来就会花。
  /// 用 `colorScheme.surface`（与设置栈其余部分同一套面），不新造颜色。
  Widget _compactSettingsPage() => ColoredBox(
    color: Theme.of(context).colorScheme.surface,
    child: SafeArea(
      child: Padding(
        padding: const EdgeInsets.all(Space.s2),
        child: _settingsSurface(
          onClose: () => unawaited(closeSettings()),
          narrow: true,
        ),
      ),
    ),
  );

  Widget _chat({required bool compact}) => _withModRestartNotice(
    ChatPanel(
    messages: widget.messages,
    phase: widget.phase,
    input: widget.input,
    onSend: widget.onSend,
    onStop: widget.onStop,
    onDismissError: widget.onDismissError ?? () {},
    error: widget.error,
    errorActions: widget.errorActions,
    volume: widget.volume,
    muted: widget.muted,
    onVolumeChanged: widget.onVolumeChanged,
    onMutedChanged: widget.onMutedChanged,
    serverMuted: widget.serverMuted,
    audioUnlocked: widget.audioUnlocked,
    onEnableSound: widget.onEnableSound,
    onOpenSettings: compact ? () => openSettings() : null,
    onOpenSessions: compact ? () => unawaited(openSessions()) : null,
    onRetryLast: widget.onRetryLast,
    announcement: widget.announcement,
    // 朗读高亮（纯转发；判据与状态都在 chat/ 那一层）。
    playingSentenceSeq: widget.playingSentenceSeq,
    // 「有背景时才让面板留一点透」——判据与壳根是同一个派生，避免两处不一致。
    backdropVisible: _hasBackground,
    listenSupported: widget.listenSupported,
    listening: widget.listening,
    listenStatus: widget.listenStatus,
    listenError: widget.listenError,
    onToggleListen: widget.onToggleListen,
    listenNote: widget.listenNote,
  ),
  );

  /// 把 [AppShell.modRestartNotice] 叠在聊天区**顶部**（没有提示时原样返回，
  /// 布局与改动前逐字一致）。
  ///
  /// 为什么要 `Expanded` 包一层：ChatPanel 自己是个撑满高度的列，直接塞进一个新
  /// Column 会拿到无界高度而报错。多出来的这一层只在有提示时存在。
  Widget _withModRestartNotice(Widget chat) {
    final String? notice = widget.modRestartNotice;
    if (notice == null) return chat;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        InlineNotice(
          message: notice,
          severity: NoticeSeverity.warning,
          onDismiss: widget.onDismissModRestart,
        ),
        Expanded(child: chat),
      ],
    );
  }
}

/// 一个**永不通知**的 [Listenable]。
///
/// 只在宿主没接 `settingsChanges` 时当占位（测试里常见）。
/// 刻意不写成可空 + 条件包一层：那会让「忘了接线」和「不需要」长得一模一样，
/// 而这两件事的后果差很远——前者是又一个永远转圈的设置面板。
class NeverNotifies implements Listenable {
  const NeverNotifies();

  @override
  void addListener(VoidCallback listener) {}

  @override
  void removeListener(VoidCallback listener) {}
}
