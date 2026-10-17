/// 设置草稿与分区 pane 接线（**`part of` 组合根**）。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**的行为边界之一：
/// 懒加载、保存/放弃（8 个分区的 pane 构建器在 `shell_settings_panes.dart`——
/// 同库 part，按行数预算把「存档逻辑」与「每页怎么画」分开）。
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

  /// 「保存并重载」：**Mod 表单草稿 + 服务端草稿 + 本机偏好草稿**一起保存，
  /// 然后**只刷新一次**。
  ///
  /// 顺序：
  /// 0. 有 Mod 表单草稿 → 逐张提交（`POST /api/v1/mods/{id}/config`，含导演的
  ///    `motion_bias`；T2，2026-10-13）；
  /// 1. 有服务端草稿 → 现有保存（写盘 + 热重载；需要时只重启 18080）；
  /// 2. 有本机偏好草稿 → 写入本机存储（`widget.onPrefsChanged`）；
  /// 3. 都走完 → 刷新一次 `/app/`（**只这一处**，两次保存不各刷一次）。
  ///    **只改了本机偏好时不重启 18080**。
  ///
  /// 任一步失败：**不刷新**，草稿保留，返回 [SaveOutcome.failed] 让确认框留下。
  /// 返回 [SaveOutcome]：确认框的「保存并离开」据此决定是否真的离开。
  Future<SaveOutcome> _saveAll() async {
    if (_savingAll) return SaveOutcome.failed;
    _savingAll = true;
    _prefsSaveError = null;
    _modDraftSaveError = null;
    _refresh();
    SaveOutcome outcome = SaveOutcome.savedApplied;
    try {
      // 0. Mod 表单里尚未提交的配置（T2，2026-10-13 执行单）：先写 mods.json，
      //    紧接着第 1 步的写盘会触发一次热重载——两样一起生效（含导演的
      //    motion_bias：按句清洗器只在重载时重读它）。卡片自己的失败文案会
      //    留在那张卡上（进路由桶，活过这次刷新）。
      final List<String> modFailed = await _modDrafts.submitPending();
      if (modFailed.isNotEmpty) {
        _modDraftSaveError = '这些 Mod 的配置没保存成功：${modFailed.join('、')}';
      }
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
          outcome.messageWith(
            error: _prefsSaveError ?? _modDraftSaveError ?? _settings.error,
          ),
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
  ///   而不是一个停在旧值的界面）；
  /// - 「热重载失败」类（S-001，2026-10-16）：**不刷新**——写盘成功但引擎没
  ///   装上，刷新会把这条提示和草稿现场一起抹掉；用户要能看见它并就地重试。
  Future<void> _reloadPageAfterSave(SaveOutcome outcome) async {
    if (outcome == SaveOutcome.savedReloadFailed) {
      return;
    }
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
}
