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

  /// 「保存」：走 `SettingsController`，文案由 `apply_status` 决定。
  ///
  /// 返回 [SaveOutcome]：确认框的「保存并离开」据此决定**是否真的离开**
  /// （失败留在弹窗里，见 `showConfirmDiscardDialog`）。
  Future<SaveOutcome> _saveSettings() async {
    final SaveOutcome outcome = await _settings.save();
    if (!mounted) return outcome;
    _refresh();
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    messenger.showSnackBar(
      SnackBar(
        // 失败时把服务端给的 `code：message` 一起上屏（`messageWith`）——
        // 只显示「保存失败」等同于「没有错误代码」（用户 2026-09-11 报）。
        content: Text(outcome.messageWith(error: _settings.error)),
        backgroundColor: outcome.needsAttention
            ? appPaletteOf(context).warning
            : null,
      ),
    );
    if (outcome.isSuccess) {
      // **保存成功就刷新生效态**：dev_mode 的开关（草稿 vs 已保存 vs --dev-mode）、
      // 诊断面板里的 dev_mode 与日志都读服务端状态，不刷新就会停在上一次。
      unawaited(_loadAppStatus());
      unawaited(_loadLogs());
    }
    return outcome;
  }

  /// 未保存改动的确认框。**三处拦截共用**（换分区 / 关设置 / 关浮层）。
  ///
  /// 统一走 `SettingsController.confirmLeave`：它负责「放弃 → 立刻清草稿」，
  /// 于是三处拦截与弹窗三按钮只有一套语义（过去「放弃改动」只关窗不清草稿，
  /// 用户会看到「未保存」一直挂着）。
  Future<bool> _confirmDiscard() async {
    if (!_settings.dirty) return true;
    // 防重入：Esc 连按 / 同时从两个入口进来时，不叠第二个弹窗。
    if (_confirmingLeave) return false;
    _confirmingLeave = true;
    try {
      return await _settings.confirmLeave(
        () => showConfirmDiscardDialog(
          context,
          onSave: _saveSettings,
          errorOf: () => _settings.error,
        ),
      );
    } finally {
      _confirmingLeave = false;
    }
  }

  /// 分区内容构建器：8 个分区各自的 pane。
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
        // M5.1：主链只剩系统提示词；角色卡导入已迁到标准 Mod，这里没有接线。
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
      case SettingsSection.llm:
        return LlmSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          envKey: _envStatus.forSection('llm'),
          envFile: _envStatus.envFile,
          onSaveKey: _saveEnvKey,
          onTest: _testLlm,
          testResult: _llmTest,
          testing: _llmTesting,
        );
      case SettingsSection.tts:
        return TtsSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          serverMuted: _serverMuted,
          envKey: _envStatus.forSection('tts'),
          envFile: _envStatus.envFile,
          onSaveKey: _saveEnvKey,
          onTest: _testTts,
          testResult: _ttsTest,
          testing: _ttsTesting,
        );
      case SettingsSection.appearance:
        return AppearanceSection(
          prefs: widget.prefs,
          onPrefsChanged: _updatePrefs,
          devMode: _devMode,
          onPickStageImage: () => unawaited(_pickStageImage()),
          onClearStageImage: _clearStageImage,
          // 轮播列表的最小操作（列表由用户手动维护、只存本机）。
          onAddToPlaylist: _addStageImageToPlaylist,
          onClearPlaylist: _clearStagePlaylist,
          stageImageMessage: _stageImageMessage,
          stageImageFailed: _stageImageFailed,
          // 2026-09-14（rc.5）：壳全局背景（同步开时与舞台共用同一张图）。
          onPickShellImage: () => unawaited(_pickShellImage()),
          onClearShellImage: _clearShellImage,
          shellImageMessage: _shellImageMessage,
          shellImageFailed: _shellImageFailed,
          // 动作幅度（2026-09-16）：服务端产品设置，改的是设置草稿，
          // 点「保存」才写盘；渲染面在草稿保存 / 设置加载后由 main.dart 下发。
          action: view.action,
          onHeadScaleChanged: (double v) => _settings.edit(
            (SettingsDraft d) => d.actionHeadScale = v,
          ),
          onBodyScaleChanged: (double v) => _settings.edit(
            (SettingsDraft d) => d.actionBodyScale = v,
          ),
          onExpressionScaleChanged: (double v) => _settings.edit(
            (SettingsDraft d) => d.actionExpressionScale = v,
          ),
        );
      case SettingsSection.mods:
        return ModsSection(
          mods: _mods,
          loading: _adminLoading,
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
        );
      case SettingsSection.diagnostics:
        return DiagnosticsSection(
          status: _status,
          capabilities: _capabilities,
          wsStatusLabel: _ui.wsStatus.description,
          logs: _logs,
          logsError: _logsError,
          devMode: _devMode,
          copied: _copied,
          onReload: _loadAdmin,
          onCopy: _copyDiagnostics,
        );
      case SettingsSection.developer:
        // 开关值 = **三态显示**：启动参数强制 > 草稿 > 已保存。
        //
        // 过去直接显示 `_devMode`（服务端生效值），于是「关掉开关」在界面上
        // **没有任何可见变化**——开关弹回 on、只多一个「未保存」徽标，
        // 用户以为点了没反应。现在草稿优先，关掉立刻看得见；保存成功后
        // `_loadAppStatus` 把生效态刷新，草稿清空，两者合一。
        final bool forcedByLaunch = _devMode && !view.devMode;
        final bool shownDevMode = forcedByLaunch
            ? true
            : (_settings.draft.devMode ?? view.devMode);
        return DeveloperSection(
          devMode: shownDevMode,
          // dev_mode 走设置草稿（顶层三态），保存后才写盘。
          onDevModeChanged: (bool v) => _settings.edit(
            (SettingsDraft d) => d.devMode = v,
          ),
          // 启动参数强制开启时**不谎报可关**（rc.3 N3，2026-09-13 接线）。
          //
          // 判据：**有效** dev_mode（`GET /api/v1/app/status` 的 `dev_mode`，
          // 已含 `--dev-mode` 的覆盖）是 on，而**落盘设置**是 off——那就只可能是
          // 启动参数压着，界面里关不掉。
          //
          // 旧实现写死 `false`：开关看起来能关，关完服务端还是 on
          // （「看起来关了、其实没关」）。两者都由既有字段推出，**没有新增协议字段**。
          forcedByLaunchFlag: forcedByLaunch,
          // 展示名读共享表（不在 Dart 手写中文）。
          presetLabels: _presetLabels,
          // P0-3：动作调试——前端直发 preset 帧到渲染面（不经后端 / LLM）。
          // L1（2026-09-16）：把调试面板滑条的强度一起透传（渲染面钳 [0,3]）。
          onApplyPreset: (String id, double intensity) => unawaited(
            _stageKey.currentState?.applyPreset(
                  id,
                  source: 'debug',
                  intensity: intensity,
                ) ??
                Future<void>.value(),
          ),
          presetStatus: _stageKey.currentState?.presetStatus,
          // 2026-09-16：调试面板可显示 + **临时**覆盖三项幅度倍率
          // （产品设置是真源；这里不落盘，只发渲染面）。
          //
          // W7（2026-09-23）：临时覆盖是 **syncer 的显式状态**，不再直发
          // `stage.sync(actionScales: …)`——那一版不更新 `_sent`，与产品值
          // 互相冲掉（RESEARCH §2.4）。下发只有一个出口：`ActionScalesSyncer`。
          productScales: view.action,
          // D2：把 syncer 里的**显式 pin** 只读传进面板（没有就是 null）——
          // 面板滑条按 `pin ?? 产品值` 播种，离开 Developer 分区再回来时
          // 显示的仍是渲染面正在用的那组临时值，而不是产品值（T4 实测）。
          pinnedScales: _actionScalesSyncer.pinned,
          onApplyScales: (double head, double body, double expression) {
            _actionScalesSyncer.pin(<String, double>{
              'head': head,
              'body': body,
              'expression': expression,
            });
            // 让 `Live2DStage.actionScales`（重挂后的自愈值）跟着变成临时值。
            _refresh();
          },
          // 「恢复产品设置」：清掉临时覆盖并**强制**写回产品值
          // （临时值可能恰好等于产品值，去重会挡住普通下发——见 syncer.clearPin）。
          onClearScales: () {
            _actionScalesSyncer.clearPin(_actionScalesPayload);
            _refresh();
          },
        );
    }
  }
}
