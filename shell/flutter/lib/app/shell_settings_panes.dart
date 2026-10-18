/// 设置分区的 pane 构建器（**`part of` 组合根**）。
///
/// 2026-10-17（rc4）从 `shell_settings.dart` 拆出：那份文件只剩「草稿 / 保存 /
/// 放弃」的接线，本文件只回答「每个一级分区画什么」。两个文件同属 `main.dart`
/// 这个库，所以 `_ShellRootState` 的私有成员与扩展方法照旧可用。
///
/// 拆的理由：导演升为一级分区后，pane 构建器本身越过了 500 行预算，而「存档逻辑」
/// 与「每页怎么画」本来就是两件事。
part of 'package:live2d_ai_shell/main.dart';

extension _ShellSettingsPanes on _ShellRootState {
  /// 分区内容构建器：**8 个**分区各自的 pane（2026-10-17 rc4：导演回到一级）。
  ///
  /// | 分区 | pane |
  /// | --- | --- |
  /// | persona（模型对话） | PersonaSection |
  /// | models（模型库） | ModelsSection |
  /// | service（模型服务） | ServiceSection（语言模型 / 语音合成两张卡） |
  /// | theme（主题） | ThemeSection（配色 + 背景） |
  /// | motion（Live2D 设置） | MotionSection（舞台与口型 + 渲染档位 + 互动） |
  /// | director（导演） | DirectorSection（常驻状态 + 动作幅度；devMode 才画观测/调试） |
  /// | mods（扩展） | ModsSection（各 Mod 的产品卡片；**不再有导演卡**） |
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
          activateFailed: _modelSwapFailed,
          onActivate: _activateModel,
          onRetryModel: _retryModelSwap,
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
        // 舞台与口型 + 允许拖动与缩放 + 渲染档位；动作幅度**不在这里**（见 director 分支）。
        return MotionSection(
          prefs: _shownPrefs,
          onPrefsChanged: _updatePrefs,
        );
      case SettingsSection.director:
        // 2026-10-17（rc4）：导演升为一级卡、并回核心常驻——**没有启用开关**。
        // 动作幅度（2026-10-09 从「外观与互动」搬来）仍住这里：模型 id 取服务端
        // 权威字段（顶层 active_model_id → action.activeModelId）；覆盖表来自
        // action.models。「有没有覆盖」= 这个模型是否在表里。
        //
        // 2026-10-18（0.3.6 T3）：**本模型覆盖也是草稿**——三条滑条 / 开关 /
        // 「恢复跟随全局」只改 `SettingsDraft.actionModels`，与全局三键一起由
        // 底部「保存并重载」的同一个 PATCH 提交。切分区 / 关设置 / 取消零写盘；
        // 不再有直接 PATCH、回读与结果位（那些是 T3 之前的行为）。
        // 数据仍是 [action] / [action.models.<id>]；**不写进 mods.json、
        // 不新增键、不接 action_tx**。
        final ActionSettingsView actionView = view.action;
        final String activeModelId = actionView.activeModelId;
        final SettingsDraft draft = _settings.draft;
        final bool hasModelOverride =
            activeModelId.isNotEmpty &&
            actionView.models[activeModelId] != null;
        final bool pendingDelete =
            activeModelId.isNotEmpty &&
            draft.isModelOverridePendingDelete(activeModelId);
        final Map<String, double> pendingKeys = activeModelId.isEmpty
            ? const <String, double>{}
            : draft.pendingModelKeys(activeModelId);
        // 开关值 = 服务端已有覆盖 或 草稿里正在编辑；「待删除」优先显示关。
        final bool overrideOn =
            !pendingDelete && (hasModelOverride || pendingKeys.isNotEmpty);
        final ActionSettingsView shownAction = displayActionScales(
          remote: actionView,
          draftHead: draft.actionHeadScale,
          draftBody: draft.actionBodyScale,
          draftExpression: draft.actionExpressionScale,
          pendingModelId: pendingDelete ? null : activeModelId,
          pendingKeys: pendingKeys,
        );
        return DirectorSection(
          devMode: _devMode,
          actionScales: ActionScalesWiring(
            action: shownAction,
            modelOverrideEnabled: overrideOn,
            onHeadScaleChanged: (double v) =>
                _settings.edit((SettingsDraft d) => d.actionHeadScale = v),
            onBodyScaleChanged: (double v) =>
                _settings.edit((SettingsDraft d) => d.actionBodyScale = v),
            onExpressionScaleChanged: (double v) =>
                _settings.edit((SettingsDraft d) => d.actionExpressionScale = v),
            // 开关值 = 是否有覆盖；开 = 用当前有效值给草稿建初始覆盖，
            // 关 = 草稿记「删除」= 「恢复跟随全局」。都只进草稿，保存才提交。
            onModelOverrideEnabledChanged: (bool on) {
              _settings.edit((SettingsDraft d) {
                if (on) {
                  d.seedActionModelOverride(
                    activeModelId,
                    shownAction.effectiveForActiveModel,
                  );
                } else {
                  d.clearActionModelOverride(activeModelId);
                }
              });
            },
            onModelHeadScaleChanged: (double v) => _settings.edit(
              (SettingsDraft d) =>
                  d.setActionModelKey(activeModelId, 'head_scale', v),
            ),
            onModelBodyScaleChanged: (double v) => _settings.edit(
              (SettingsDraft d) =>
                  d.setActionModelKey(activeModelId, 'body_scale', v),
            ),
            onModelExpressionScaleChanged: (double v) => _settings.edit(
              (SettingsDraft d) =>
                  d.setActionModelKey(activeModelId, 'expression_scale', v),
            ),
            onResetModelOverride: overrideOn
                ? () => _settings.edit(
                    (SettingsDraft d) =>
                        d.clearActionModelOverride(activeModelId),
                  )
                : null,
          ),
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
      case SettingsSection.mods:
        // 2026-10-17（rc4）：导演已升为一级分区，扩展区只剩各 Mod 自己的卡片。
        // 这里**不再**接动作幅度 / 调试面板（它们随导演卡走）。
        return ModsSection(
          mods: _mods,
          loading: _adminLoading,
          // 只决定卡片副标题（ID · 版本 · 协议版本）画不画（2026-10-08）。
          devMode: _devMode,
          error: _adminError,
          busyId: _busyId,
          // T2（2026-10-13）：底部「保存并重载」据此把各卡片尚未提交的配置
          // 一起提交。导演不再走这里（它已不是 Mod）。
          drafts: _modDrafts,
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
          // 表情/动作调试与临时幅度**已搬到**一级「导演」卡（见
          // settings/sections/director_section.dart 的 directorDebug）——
          // 这一页只剩开关与诊断。
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
