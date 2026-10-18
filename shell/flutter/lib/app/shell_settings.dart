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
  /// T3（2026-10-18）：顺序与「成功才生效」的判据搬进 [SaveFlow]（纯逻辑、
  /// VM 可测）——宿主只注入回调，不自己决定顺序。成功 = 恰好一次 PATCH、
  /// 一次偏好写入、一次动作幅度下发、一次 `/app/` 刷新；任一步失败 =
  /// **0 下发 / 0 刷新**，草稿保留，返回 [SaveOutcome.failed] 让确认框留下。
  Future<SaveOutcome> _saveAll() async {
    if (_savingAll) return SaveOutcome.failed;
    _savingAll = true;
    _prefsSaveError = null;
    _modDraftSaveError = null;
    _refresh();
    try {
      final SaveFlowResult result = await SaveFlow(
        // Mod 表单里尚未提交的配置（T2，2026-10-13）：先写 mods.json，
        // 紧接着第 1 步的写盘会触发一次热重载——两样一起生效。卡片自己的
        // 失败文案会留在那张卡上（进路由桶，活过这次刷新）。
        modDirty: _modDrafts.hasPending,
        submitModDrafts: _modDrafts.submitPending,
        serverDirty: () => _settings.dirty,
        saveServer: _settings.save,
        // 运行期再判一次（PATCH 可能耗时几秒）：只写「按下保存时确实脏」的那份。
        prefsDirty: () => _prefsDirty,
        savePrefs: () {
          final DisplayPrefs? draft = _prefsDraft;
          if (draft == null) return false;
          return widget.onPrefsChanged(draft);
        },
        onPrefsSaved: _commitPrefsSave,
        // 保存成功后**唯一一处**动作幅度下发：载荷只含服务端回填的已保存值
        // （`_actionScalesPayload` 不读草稿），同步器自己去重。
        syncStage: _syncActionScalesNow,
        reload: (SaveOutcome outcome) =>
            unawaited(_reloadPageAfterSave(outcome)),
      ).run();
      if (result.failedMods.isNotEmpty) {
        _modDraftSaveError = '这些 Mod 的配置没保存成功：${result.failedMods.join('、')}';
      }
      if (result.prefsWriteFailed) {
        _prefsSaveError =
            '偏好没能写入本机存储（无痕模式 / 存储被禁 / 配额满）——草稿保留，未落盘。';
      }
      if (!mounted) return result.outcome;
      _refresh();
      _showSaveSnack(result.outcome);
      if (result.outcome.isSuccess ||
          result.outcome == SaveOutcome.noChange) {
        // 保存成功才刷新生效态（dev_mode / 日志读服务端状态）。
        unawaited(_loadAppStatus());
        unawaited(_loadLogs());
      }
      return result.outcome;
    } finally {
      if (mounted) {
        _rebuild(() => _savingAll = false);
      } else {
        _savingAll = false;
      }
    }
  }

  /// 本机偏好**写成功**后的提交（由 [SaveFlow] 在写盘之后调用）。
  ///
  /// 顺序：删被移除项的字节 → 把音量 / 静音落到正在播的音频 → 清草稿并通知
  /// 浮层宿主。**失败路径不会走到这里**（草稿保留）。
  void _commitPrefsSave() {
    final DisplayPrefs? draft = _prefsDraft;
    if (draft == null) return;
    _commitPrefsDraft();
    _audio.muted = draft.muted;
    _audio.volume = draft.volume;
    _rebuild(() => _prefsDraft = null);
    _bumpPrefsRevision();
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
    // **不补发**（T3，2026-10-18）：舞台 / 音频本来就吃已保存那份，从未被草稿
    // 改过；iframe 重建后的自愈归舞台自己的 `_attach` / `didUpdateWidget`。
    // 这里下发一次就等于「放弃改动」也产生舞台帧——规则要求 0 次。
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

  // ── 本模型动作幅度覆盖：**走设置草稿**（0.3.6 T3，2026-10-18） ──
  //
  // T3 之前它经 `ModelOverrideCoalescer` 直接 PATCH（拖动即写盘 + 热重载，
  // 且 `_settings.load()` 会静默吞掉用户其它字段的草稿），是设置面唯一绕开
  // 草稿的路径。现在三条滑条 / 开关 / 「恢复跟随全局」都只改
  // `SettingsDraft.actionModels`（见 `settings_controller.dart`），由 [_saveAll]
  // 的同一个 PATCH 一次提交；切分区 / 关设置 / 取消零写盘零下发。

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
