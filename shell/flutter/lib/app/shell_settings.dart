/// 设置草稿与分区 pane 接线（**`part of` 组合根**）。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**的行为边界之一：
/// 懒加载、保存/放弃、以及 8 个分区的 pane 构建器。
/// 草稿状态在 `settings/settings_controller.dart`（可单测），这里只做接线。
///
/// 用 `part` + 扩展方法而不是独立协作者的理由见 `main.dart` 头注。
///
part of 'package:live2d_ai_shell/main.dart';

extension _ShellSettingsWiring on _ShellRootState {
  /// 进设置面时懒加载一次。**不放在 initState**：这些数据多数时候用不到，
  /// 启动就打三个请求是白花钱（也拖慢首屏）。
  Future<void> _ensureSettingsLoaded() async {
    if (_settingsLoadedOnce) return;
    _settingsLoadedOnce = true;
    await Future.wait<void>(<Future<void>>[
      _settings.load(),
      _loadAdmin(),
      // 展示名真源（失败静默 → 面板显示稳定 id）。
      _loadPresetLabels(),
    ]);
  }

  /// 「保存并重载」：**服务端草稿 + 本机偏好草稿**一起保存，然后**只刷新一次**。
  ///
  /// 顺序：
  /// 1. 有服务端草稿 → 现有保存（写盘 + 热重载；需要时只重启 18080）；
  /// 2. 有本机偏好草稿 → 写入本机存储（`widget.onPrefsChanged`）；
  /// 3. 两样都成功 → 刷新一次 `/app/`。**只改了本机偏好时不重启 18080**。
  ///
  /// 任一步失败：**不刷新**，草稿保留，返回 [SaveOutcome.failed] 让确认框留下。
  /// 返回 [SaveOutcome]：确认框的「保存并离开」据此决定是否真的离开。
  Future<SaveOutcome> _saveAll() async {
    if (_savingAll) return SaveOutcome.failed;
    _savingAll = true;
    _prefsSaveError = null;
    _refresh();
    SaveOutcome outcome = SaveOutcome.savedApplied;
    try {
      // 1. 服务端草稿（现有链路一字未改）。
      if (_settings.dirty) {
        outcome = await _settings.save();
        if (outcome == SaveOutcome.failed) {
          if (!mounted) return outcome;
          _refresh();
          _showSaveSnack(outcome);
          return outcome;
        }
      }
      // 2. 本机偏好草稿 → 落盘。
      if (_prefsDirty) {
        final DisplayPrefs draft = _prefsDraft!;
        final bool saved = widget.onPrefsChanged(draft);
        if (!saved) {
          _prefsSaveError =
              '偏好没能写入本机存储（无痕模式 / 存储被禁 / 配额满）——草稿保留，未落盘。';
          if (!mounted) return SaveOutcome.failed;
          _refresh();
          _showSaveSnack(SaveOutcome.failed);
          return SaveOutcome.failed;
        }
        // 落盘成功：提交背景字节改动（删被移除项的字节）、清草稿、同步音频。
        _commitPrefsDraft();
        _audio.muted = draft.muted;
        _audio.volume = draft.volume;
        if (!mounted) return outcome;
        _rebuild(() => _prefsDraft = null);
        _bumpPrefsRevision();
        if (!outcome.isSuccess) outcome = SaveOutcome.savedApplied;
      }
      if (!mounted) return outcome;
      _refresh();
      _showSaveSnack(outcome);
      if (outcome.isSuccess || outcome == SaveOutcome.noChange) {
        // 保存成功就刷新生效态（dev_mode / 日志读服务端状态）。
        unawaited(_loadAppStatus());
        unawaited(_loadLogs());
        // **只在这一处**刷新页面——服务端草稿 + 本机偏好草稿两样都已落定。
        unawaited(_reloadPageAfterSave(outcome));
      }
      return outcome;
    } finally {
      if (mounted) {
        _rebuild(() => _savingAll = false);
      } else {
        _savingAll = false;
      }
    }
  }

  /// 保存结果 SnackBar（失败时把 `code：message` 一起上屏）。
  void _showSaveSnack(SaveOutcome outcome) {
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    messenger.showSnackBar(
      SnackBar(
        content: Text(
          outcome.messageWith(error: _prefsSaveError ?? _settings.error),
        ),
        backgroundColor: outcome.needsAttention
            ? appPaletteOf(context).warning
            : null,
      ),
    );
  }

  /// 放弃**两份**草稿：服务端草稿 + 本机偏好草稿（含删掉本次新加的背景图字节）。
  void _discardAllDrafts() {
    _settings.discard();
    _discardPrefsDraft();
  }

  /// 丢掉本机偏好草稿，回到已保存那份。
  ///
  /// 顺带删掉「本次草稿新加、还没写进已保存偏好」的背景图字节；被移除项的
  /// 字节**不删**（它们仍在已保存偏好里，删了就是静默丢数据）。
  void _discardPrefsDraft() {
    if (_prefsDraft == null) return;
    for (final String id in _draftAddedBgIds) {
      unawaited(forgetBackground(widget.store, id));
    }
    _draftAddedBgIds.clear();
    _draftRemovedBgIds.clear();
    _prefsSaveError = null;
    _rebuild(() => _prefsDraft = null);
    _bumpPrefsRevision();
    // 舞台 / 音频本来就吃已保存那份，从未被草稿改过；补一次下发只为让桥在
    // 草稿期间可能发生的外部变动（如重挂）后自愈。
    _applyPrefs();
  }

  /// 提交本机偏好草稿的背景字节改动（保存成功后调用）：删掉被移除项的字节。
  void _commitPrefsDraft() {
    for (final String id in _draftRemovedBgIds) {
      unawaited(forgetBackground(widget.store, id));
    }
    _draftAddedBgIds.clear();
    _draftRemovedBgIds.clear();
  }

  /// 保存成功后的**最后一步：重新加载 `/app/`**。
  ///
  /// - 热重载类：等一档动效时长（让 SnackBar 先露个脸）就刷新；
  /// - 「需重启」类：服务端正在重启 18080 上这个进程，**等它回来**再刷新
  ///   （最多约 15 秒；超时也刷新——刷出来的会是「连不上」的如实状态，
  ///   而不是一个停在旧值的界面）。
  Future<void> _reloadPageAfterSave(SaveOutcome outcome) async {
    if (outcome == SaveOutcome.savedRestartRequired) {
      await _api.waitUntilReachable();
    } else {
      await Future<void>.delayed(AppDurations.reveal);
    }
    if (!mounted) return;
    reloadAppPage();
  }

  // ── 本模型动作幅度覆盖：直接 PATCH（阶段5 D40，2026-09-26） ──
  //
  // **为什么不经设置草稿**：`[action.models.<id>]` 是逐模型三态表，
  // `SettingsDraft` 里没有对应字段（`settings_controller.dart` 不在本波
  // 授权面内）。这里走 PATCH → `_settings.load()` → `_refresh()`：
  // 与「保存」同一个服务端口径（写盘 + 热重载），并把服务端钳位后的
  // 权威值回填——绝不在本地猜结果。

  /// 覆盖 PATCH 的**串行**入口。
  ///
  /// # 为什么必须串行（2026-09-27 修）
  ///
  /// 防抖保证「一次拖动只发一帧」，但保证不了**两帧之间**的到达顺序：
  /// 先拖 head（PATCH A）再点「恢复跟随全局」（PATCH B），若 B 先到、A 后到，
  /// 服务端最终留下 head 覆盖——用户看到「删了又回来」。这里把请求串成一条
  /// 链，服务端看到的顺序 = 用户操作顺序。
  ///
  /// 失败**不打断**链条（[now] 内部已把原因上屏）。
  Future<void> _patchActionModels(
    Map<String, ActionModelOverridePatch?> models, {
    required String okMessage,
    required int epoch,
  }) {
    final Future<void> next = _modelOverrideChain.then(
      (void _) =>
          _patchActionModelsNow(models, okMessage: okMessage, epoch: epoch),
    );
    _modelOverrideChain = next.then<void>((void _) {}, onError: (Object _) {});
    return next;
  }

  /// [models] 真正落盘：PATCH → 回读权威设置 → 刷新。
  ///
  /// 不走设置草稿：`SettingsDraft` 里没有 `[action.models.<id>]` 字段
  /// （`settings_controller.dart` 不在本波授权面内）。PATCH 与「保存」
  /// 同一个服务端口径（写盘 + 热重载），回读把服务端钳位后的权威值回填，
  /// 绝不在本地猜结果。
  Future<void> _patchActionModelsNow(
    Map<String, ActionModelOverridePatch?> models, {
    required String okMessage,
    required int epoch,
  }) async {
    try {
      await _api.patchSettings(
        SettingsPatch(action: ActionSettingsPatch(models: models)),
      );
    } on ApiException catch (e) {
      if (!mounted) return;
      _modelOverrideCoalescer.acknowledge(epoch);
      _modelOverrideIntent = null;
      _modelOverrideMessage = '本模型覆盖保存失败：${e.message}';
      _modelOverrideFailed = true;
      _refresh();
      return;
    }
    if (!mounted) return;
    await _settings.load();
    if (!mounted) return;
    // 回读之后再放开临时值：滑条改绑磁盘上的新数，不会先弹回旧数。
    _modelOverrideCoalescer.acknowledge(epoch);
    _modelOverrideIntent = null;
    _modelOverrideMessage = okMessage;
    _modelOverrideFailed = false;
    _refresh();
  }

  /// 合并防抖器的到点回调：把「模型 id + 只含被改键的补丁」交给串行 PATCH。
  void _flushModelOverride(
    String modelId,
    ActionModelOverridePatch patch,
    int epoch,
  ) {
    unawaited(
      _patchActionModels(
        <String, ActionModelOverridePatch?>{modelId: patch},
        okMessage: '已保存本模型覆盖（$modelId）',
        epoch: epoch,
      ),
    );
  }

  /// 单键覆盖：**只记这一键**，由 [ModelOverrideCoalescer] 收成一帧再发。
  ///
  /// 三条一起发会把 body / expression 钉死成当时的全局值，之后改全局不再跟随，
  /// 那正是「三键各自可选」要避免的事。
  void _patchModelOverride(
    String modelId, {
    double? headScale,
    double? bodyScale,
    double? expressionScale,
  }) {
    _modelOverrideCoalescer.record(
      modelId,
      headScale: headScale,
      bodyScale: bodyScale,
      expressionScale: expressionScale,
    );
    // 滑条读的是临时值。不重建的话，手指在动、滑块停在磁盘旧值上。
    _refresh();
  }

  /// 开启「本模型覆盖」：**先取消防抖**，再用当前**有效**三键建初始覆盖。
  ///
  /// 取消是必需的：待发的单键只会写一个键，先到会把刚建的初始覆盖改掉一半。
  Future<void> _seedModelOverride(
    String modelId,
    ActionSettingsView effective,
  ) {
    if (modelId.isEmpty) return Future<void>.value();
    _modelOverrideCoalescer.cancel(modelId);
    return _patchActionModels(
      <String, ActionModelOverridePatch?>{
        modelId: ActionModelOverridePatch(
          headScale: TriSet<double>(effective.headScale),
          bodyScale: TriSet<double>(effective.bodyScale),
          expressionScale: TriSet<double>(effective.expressionScale),
        ),
      },
      okMessage: '已开启本模型覆盖（$modelId）',
      epoch: _modelOverrideCoalescer.epoch,
    );
  }

  /// 关闭覆盖 / 「恢复跟随全局」：`models.<id> = null`（服务端删掉整份覆盖）。
  ///
  /// **必须先取消防抖**：否则一个迟到的单键 PATCH 会在删除之后又把覆盖写回来。
  /// **必须是显式 null**：省略该 id = 「不动」，用户点了删除却删不掉。
  Future<void> _clearModelOverride(String modelId) {
    if (modelId.isEmpty) return Future<void>.value();
    _modelOverrideCoalescer.cancel(modelId);
    return _patchActionModels(
      <String, ActionModelOverridePatch?>{modelId: null},
      okMessage: '已恢复跟随全局（$modelId 的覆盖已删除）',
      epoch: _modelOverrideCoalescer.epoch,
    );
  }

  /// 未保存改动的确认框。**三处拦截共用**（换分区 / 关设置 / 关浮层）。
  ///
  /// 2026-10-10：判据从「服务端草稿脏」扩成「**任一**草稿脏」（服务端 + 本机
  /// 偏好）。「保存并离开」走 [_saveAll]（两份草稿一起保存并刷新）；
  /// 「放弃改动」丢掉两份草稿（本机偏好回到已保存那份、删掉本次新加的背景图）。
  Future<bool> _confirmDiscard() async {
    if (!_anyDraftDirty) return true;
    // 防重入：Esc 连按 / 同时从两个入口进来时，不叠第二个弹窗。
    if (_confirmingLeave) return false;
    _confirmingLeave = true;
    try {
      final bool leave = await showConfirmDiscardDialog(
        context,
        onSave: _saveAll,
        errorOf: () => _prefsSaveError ?? _settings.error,
      );
      if (leave) {
        _settings.discard();
        _discardPrefsDraft();
      }
      return leave;
    } finally {
      _confirmingLeave = false;
    }
  }

  /// 分区内容构建器：**7 个**分区各自的 pane（2026-10-09 一级 8 → 7）。
  ///
  /// | 分区 | pane |
  /// | --- | --- |
  /// | persona（模型对话） | PersonaSection |
  /// | models（模型库） | ModelsSection |
  /// | service（模型服务） | ServiceSection（语言模型 / 语音合成两张卡） |
  /// | theme（主题） | ThemeSection（配色 + 背景） |
  /// | motion（Live2D 设置） | MotionSection（舞台与口型 + 渲染档位 + 互动） |
  /// | mods（扩展） | ModsSection（+ 动作幅度旋钮接给 director 卡片） |
  /// | developer（开发模式） | DeveloperSection（+ 诊断，仅 devMode 渲染） |
  Widget _buildSection(BuildContext context, SettingsSection section) {
    // 未加载出来时先显示骨架，而不是一个空面板让人以为坏了。
    if (!_settings.loaded && _settings.loading) {
      return const Padding(
        padding: EdgeInsets.symmetric(vertical: Space.s8),
        child: Center(child: CircularProgressIndicator()),
      );
    }
    final SettingsView? view = _settings.remote;
    if (view == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: Space.s7),
        child: Column(
          children: <Widget>[
            Text('读不到服务端设置', style: Theme.of(context).textTheme.titleSmall),
            const SizedBox(height: 8),
            Text(
              _settings.error ?? '未知原因',
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 12),
            FilledButton.tonal(
              onPressed: () => unawaited(_settings.load()),
              child: const Text('重试'),
            ),
          ],
        ),
      );
    }

    switch (section) {
      case SettingsSection.persona:
        // 显示名是「模型对话」；主链只剩系统提示词 + 记住几轮。
        // 角色卡导入属**扩展**，入口在「扩展」分区的 persona 卡片。
        return PersonaSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
        );
      case SettingsSection.models:
        return ModelsSection(
          models: _models,
          loading: _adminLoading,
          error: _adminError,
          devMode: _devMode,
          busyId: _busyId,
          activateMessage: _adminMessage,
          onActivate: _activateModel,
          onImport: _importModel,
          onReload: _loadAdmin,
        );
      case SettingsSection.service:
        // 原一级「对话」+ 原一级「语音合成」合成这一页：两段字段的逻辑
        // 仍住在 llm_section.dart / tts_section.dart（**没有第二套**）。
        // focusGroup 是错误横幅「去语音合成设置」带来的页内定位值。
        return ServiceSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          serverMuted: _serverMuted,
          llmEnvKey: _envStatus.forSection('llm'),
          ttsEnvKey: _envStatus.forSection('tts'),
          envFile: _envStatus.envFile,
          onSaveKey: _saveEnvKey,
          onTestLlm: _testLlm,
          onTestTts: _testTts,
          llmTestResult: _llmTest,
          ttsTestResult: _ttsTest,
          llmTesting: _llmTesting,
          ttsTesting: _ttsTesting,
          focusGroup: _settingsGroupFocus,
        );
      case SettingsSection.theme:
        // 只拿原「外观」那一组：配色 + 背景（纯本地 DisplayPrefs 草稿）。
        return ThemeSection(
          prefs: _shownPrefs,
          onPrefsChanged: _updatePrefs,
          devMode: _devMode,
          onPickShellImage: () => unawaited(_pickShellImage()),
          onClearShellImage: _clearShellImage,
          onRemoveBackground: _removeBackground,
          onRemoveBackgrounds: _removeBackgrounds,
          onReorderBackground: _reorderBackground,
          onPreviewBackground: _previewBackground,
          shellImageMessage: _shellImageMessage,
          shellImageFailed: _shellImageFailed,
        );
      case SettingsSection.motion:
        // 舞台与口型 + 允许拖动与缩放 + 渲染档位；动作幅度**不在这里**（见 mods 分支）。
        return MotionSection(
          prefs: _shownPrefs,
          onPrefsChanged: _updatePrefs,
        );
      case SettingsSection.mods:
        // 动作幅度（2026-10-09 从「外观与互动」搬到导演卡片）：模型 id 取服务端
        // 权威字段（顶层 active_model_id → action.activeModelId）；覆盖表来自
        // action.models。「有没有覆盖」= 这个模型是否在表里。
        //
        // 滑条显示草稿和还没回读完的本模型覆盖。磁盘值只作兜底。
        final ActionSettingsView actionView = view.action;
        final String activeModelId = actionView.activeModelId;
        final bool hasModelOverride =
            activeModelId.isNotEmpty &&
            actionView.models[activeModelId] != null;
        final bool overrideOn = _modelOverrideIntent ?? hasModelOverride;
        final ActionSettingsView shownAction = displayActionScales(
          remote: actionView,
          draftHead: _settings.draft.actionHeadScale,
          draftBody: _settings.draft.actionBodyScale,
          draftExpression: _settings.draft.actionExpressionScale,
          pendingModelId: _modelOverrideCoalescer.latchedModelId,
          pendingKeys: _modelOverrideCoalescer.latchedKeys,
        );
        return ModsSection(
          mods: _mods,
          loading: _adminLoading,
          // 只决定卡片副标题（ID · 版本 · 协议版本）画不画（2026-10-08）。
          devMode: _devMode,
          error: _adminError,
          busyId: _busyId,
          onToggle: _toggleMod,
          // M2：有 settings_spec 的 Mod 展开后按 spec 渲表单，保存走这里。
          onSaveConfig: _saveModConfig,
          onReload: _loadAdmin,
          // L1 基座：统一重启提示 + 会话绑定上下文 + 面板变更回调。
          restartNotice: _modRestartNotice,
          onDismissRestart: _dismissModRestart,
          activeSessionId: _chat.sessions.activeId,
          onModChanged: _notifyModChanged,
          // 角色卡文件选择器：与 persona 面板同一份组合根注入。
          pickCardFile: pickPersonaCardFile,
          // 动作幅度：数据仍是 [action] / [action.models.<id>]，仍走现有 PATCH
          // 与 ActionScalesSyncer；**不写进 director 的 mods.json**。
          actionScales: ActionScalesWiring(
            action: shownAction,
            modelOverrideEnabled: overrideOn,
            onHeadScaleChanged: (double v) =>
                _settings.edit((SettingsDraft d) => d.actionHeadScale = v),
            onBodyScaleChanged: (double v) =>
                _settings.edit((SettingsDraft d) => d.actionBodyScale = v),
            onExpressionScaleChanged: (double v) =>
                _settings.edit((SettingsDraft d) => d.actionExpressionScale = v),
            // 开关值 = 是否有覆盖；开 = 用当前有效值建初始覆盖，
            // 关 = 删除 = 「恢复跟随全局」。三条滑条只在覆盖开启时接管。
            onModelOverrideEnabledChanged: (bool on) {
              _modelOverrideIntent = on;
              _refresh();
              unawaited(
                on
                    ? _seedModelOverride(
                        activeModelId,
                        shownAction.effectiveForActiveModel,
                      )
                    : _clearModelOverride(activeModelId),
              );
            },
            onModelHeadScaleChanged: (double v) =>
                _patchModelOverride(activeModelId, headScale: v),
            onModelBodyScaleChanged: (double v) =>
                _patchModelOverride(activeModelId, bodyScale: v),
            onModelExpressionScaleChanged: (double v) =>
                _patchModelOverride(activeModelId, expressionScale: v),
            onResetModelOverride: overrideOn
                ? () {
                    _modelOverrideIntent = false;
                    _refresh();
                    unawaited(_clearModelOverride(activeModelId));
                  }
                : null,
            modelOverrideMessage: _modelOverrideMessage,
            modelOverrideFailed: _modelOverrideFailed,
          ),
          // 表情调试 / 动作调试 / 临时幅度（2026-10-09 从核心「开发模式」页
          // 整块搬到导演卡片）：数据与回调一字未改，仍只在开发者模式里渲染。
          directorDebug: DirectorDebugWiring(
            // 展示名读共享表（不在 Dart 手写中文）。
            labels: _presetLabels,
            // P0-3：动作调试——前端直发 preset 帧到渲染面（不经后端 / LLM）。
            onApplyPreset: (String id, double intensity) => unawaited(
              _stageKey.currentState?.applyPreset(
                    id,
                    source: 'debug',
                    intensity: intensity,
                  ) ??
                  Future<void>.value(),
            ),
            status: _stageKey.currentState?.presetStatus,
            productScales: view.action,
            // D2：把 syncer 里的**显式 pin** 只读传进面板（没有就是 null）。
            pinnedScales: _actionScalesSyncer.pinned,
            onApplyScales: (double head, double body, double expression) {
              _actionScalesSyncer.pin(<String, double>{
                'head': head,
                'body': body,
                'expression': expression,
              });
              // 让舞台重挂后的自愈值跟着变成临时值。
              _refresh();
            },
            // 「恢复产品设置」：清掉临时覆盖并**强制**写回产品值。
            onClearScales: () {
              _actionScalesSyncer.clearPin(_actionScalesPayload);
              _refresh();
            },
          ),
        );
      case SettingsSection.developer:
        // 开关值 = **三态显示**：启动参数强制 > 草稿 > 已保存。
        //
        // 过去直接显示服务端生效值，于是「关掉开关」在界面上没有任何可见变化
        // ——开关弹回 on、只多一个「未保存」徽标，用户以为点了没反应。
        // 现在草稿优先，关掉立刻看得见；保存成功后重新取生效态，草稿清空。
        final bool forcedByLaunch = _devMode && !view.devMode;
        final bool shownDevMode = forcedByLaunch
            ? true
            : (_settings.draft.devMode ?? view.devMode);
        return DeveloperSection(
          devMode: shownDevMode,
          // dev_mode 走设置草稿（顶层三态），保存后才写盘。
          onDevModeChanged: (bool v) =>
              _settings.edit((SettingsDraft d) => d.devMode = v),
          // 启动参数强制开启时**不谎报可关**（rc.3 N3，2026-09-13 接线）。
          //
          // 判据：**有效** dev_mode（GET /api/v1/app/status 的 dev_mode，
          // 已含 --dev-mode 的覆盖）是 on，而**落盘设置**是 off——那就只可能是
          // 启动参数压着，界面里关不掉。
          forcedByLaunchFlag: forcedByLaunch,
          // 诊断（2026-10-09）：一级「诊断」取消后收进这一页，
          // 只有 devMode 为真时 DeveloperSection 才画它。
          // 表情/动作调试与临时幅度**已搬到**「扩展 → 导演」卡片（见下面的
          // directorDebug）——这一页只剩开关与诊断。
          diagnostics: DiagnosticsSection(
            status: _status,
            capabilities: _capabilities,
            wsStatusLabel: _ui.wsStatus.description,
            logs: _logs,
            logsError: _logsError,
            devMode: _devMode,
            copied: _copied,
            onReload: _loadAdmin,
            onCopy: _copyDiagnostics,
          ),
        );
    }
  }
}
