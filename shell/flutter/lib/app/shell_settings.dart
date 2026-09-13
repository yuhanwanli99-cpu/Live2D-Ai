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
    ]);
  }

  /// 「保存」：走 `SettingsController`，文案由 `apply_status` 决定。
  Future<void> _saveSettings() async {
    final SaveOutcome outcome = await _settings.save();
    if (!mounted) return;
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
    // 保存的是 dev_mode 时，重新取一次状态（诊断面板要跟着变）。
    if (outcome.isSuccess) unawaited(_loadLogs());
  }

  /// 未保存改动的确认框。**三处拦截共用**（换分区 / 关浮层 / 刷新页面）。
  Future<bool> _confirmDiscard() async {
    if (!_settings.dirty) return true;
    final bool? leave = await showDialog<bool>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('有未保存的改动'),
        content: const Text('离开会丢掉这些改动。要先保存吗？'),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('留下'),
          ),
          TextButton(
            onPressed: () async {
              await _saveSettings();
              if (dialogContext.mounted) {
                Navigator.of(dialogContext).pop(true);
              }
            },
            child: const Text('保存并离开'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('放弃改动'),
          ),
        ],
      ),
    );
    return leave ?? false;
  }

  /// 角色卡导入：**文件读取在组合根**（`dart:html`/`package:web` 只能在这里）。
  Future<void> _pickPersonaCard() async {
    final Uint8List? bytes = await pickLocalFile();
    if (bytes == null || !mounted) return;
    final ({String message, bool failed}) r = applyPersonaImport(
      controller: _settings,
      bytes: bytes,
    );
    _importMessage = r.message;
    _importFailed = r.failed;
    _refresh();
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
        return PersonaSection(
          controller: _settings,
          view: view,
          devMode: _devMode,
          importMessage: _importMessage,
          importFailed: _importFailed,
          onImport: (_) => unawaited(_pickPersonaCard()),
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
          stageImageMessage: _stageImageMessage,
          stageImageFailed: _stageImageFailed,
        );
      case SettingsSection.mods:
        return ModsSection(
          mods: _mods,
          loading: _adminLoading,
          error: _adminError,
          busyId: _busyId,
          onToggle: _toggleMod,
          onReload: _loadAdmin,
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
        return DeveloperSection(
          devMode: _devMode,
          // dev_mode 走设置草稿（顶层三态），保存后才写盘。
          onDevModeChanged: (bool v) => _settings.edit(
            (SettingsDraft d) => d.devMode = v,
          ),
          // 启动参数强制开启时**不谎报可关**。
          forcedByLaunchFlag: false,
        );
    }
  }
}
