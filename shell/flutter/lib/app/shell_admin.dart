/// 管理面接线（**`part of` 组合根**）：模型库 / Mod / 诊断 / 密钥。
///
/// 2026-09-13（rc.3 N1）从 `main.dart` **原样搬出**的行为边界之一——
/// 这里只有「哪个回调接哪个 API」以及落地时的 `_refresh()`，没有新逻辑。
/// 状态字段仍留在 `main.dart` 的 `_ShellRootState` 里：Dart 私有是**库级**的，
/// 且其中几个被 `test/transient_results_test.dart` 的源码扫描钉住。
///
/// 用 `part` + 扩展方法而不是独立协作者的理由见 `main.dart` 头注。
///
part of 'package:live2d_ai_shell/main.dart';

extension _ShellAdminWiring on _ShellRootState {
  // ── 管理面加载（模型 / Mod / 诊断） ───────────────────────────────

  /// 取一次应用状态（拿 `dev_mode`）。
  Future<void> _loadAppStatus() async {
    try {
      final Map<String, Object?> status = await _api.fetchStatus();
      final bool dev = status['dev_mode'] == true;
      if (mounted && dev != _devMode) {
        _devMode = dev;
        _refresh();
      }
    } catch (_) {
      // 忽略：保持 false。
    }
  }

  Future<void> _loadAdmin() async {
    _adminLoading = true;
    _adminError = null;
    _refresh();
    try {
      final List<ModelInfo> models = await _modelsApi.list();
      final List<ModInfo> mods = await _modsApi.list();
      final Map<String, Object?> status = await _diagApi.status();
      final AppCapabilities caps = await _diagApi.capabilities();
      // 密钥真源状态（键名 + 是否已设置）。**永不包含值**。
      // 失败不连坐：`/env` 挂了不该让整个管理面板报错，所以单独兜底。
      EnvStatus env = _envStatus;
      try {
        env = await _envApi.list();
      } on ApiException {
        env = _envStatus;
      }
      if (!mounted) return;
      final Object? activeModel = status['active_model_id'];
      _models = models;
      _mods = mods;
      _status = status;
      if (activeModel is String) _modelName = activeModel;
      _capabilities = caps;
      _envStatus = env;
      _adminLoading = false;
      _refresh();
      // 日志只有 dev_mode 才可读——**分开取**，403 不该让整个面板失败。
      await _loadLogs();
    } on ApiException catch (e) {
      if (!mounted) return;
      _adminError = e.toString();
      _adminLoading = false;
      _refresh();
    }
  }

  /// 保存一个密钥到 `.env`（`PUT /api/v1/env`），写完**立即生效**。
  ///
  /// 契约（rc.2）：`.env` 是唯一密钥真源；后端写完刷新快照 + 重建 LLM/TTS client，
  /// 所以这里不需要（也不该）提示「重启」或「重跑 ignite.sh」。
  ///
  /// 值**只在参数里出现**：不写进 state、不进日志、不回显。保存后重读一次
  /// `GET /api/v1/env` 只为刷新「已设置/未设置」这个布尔。
  Future<void> _saveEnvKey(String key, String value) async {
    await _envApi.write(key, value);
    if (!mounted) return;
    try {
      final EnvStatus env = await _envApi.list();
      if (!mounted) return;
      _envStatus = env;
      _refresh();
    } on ApiException {
      // 写成功了但读状态失败：不动现有状态（`EnvKeyField` 自己的提示仍然准确）。
    }
  }

  Future<void> _loadLogs() async {
    try {
      final List<LogLine> lines = await _diagApi.logs();
      if (!mounted) return;
      _logs = lines;
      _logsError = null;
      _refresh();
    } on ApiException catch (e) {
      if (!mounted) return;
      _logs = const <LogLine>[];
      // 403 dev_mode_required 是**预期**结果，原文照显（不伪装成空日志）。
      _logsError = e.code == 'dev_mode_required'
          // 文案要指**界面上的开关**：过去说「需服务端以 dev_mode 启动」，
          // 而那时界面上根本打不开它（开关被自己所在的分区藏起来了）。
          ? '日志需要先打开「开发模式」（设置 → 开发模式）'
          : e.toString();
      _refresh();
    }
  }

  /// 激活模型：**改 registry + 让舞台真的换皮**，两件都做完才算成功。
  ///
  /// 契约（rc.2 冻结）：后端 `POST /models/{id}/activate` 只登记 + 回一个可 GET 的
  /// `model_url`，**不推送、不通知渲染面**；换模由这里 `sendSync(model:)` 发起，
  /// 并以渲染面 `loaded` 回执为准。**收条之前一律不说「已切换」**——
  /// 那正是「激活了但没换皮」最伤人的地方：用户看界面说成功了，舞台还是旧皮。
  Future<void> _activateModel(String id) async {
    _busyId = id;
    _adminMessage = '正在切换模型…';
    _refresh();
    try {
      final ActivateResult r = await _modelsApi.activate(id);
      if (!mounted) return;
      if (r.modelUrl.isEmpty) {
        // 没给可加载地址 = 换不了皮，如实说，别报成功。
        _busyId = null;
        _adminMessage = '已登记 ${r.activeId}，但服务端没有返回可加载的 model_url，舞台未切换';
        _refresh();
        await _loadAdmin();
        return;
      }
      // 先记住：retry / 重建 iframe 后仍加载这个模型。
      _activeModelUrl = r.modelUrl;
      _refresh();
      final bool swapped =
          await _stageKey.currentState?.swapModel(r.modelUrl) ?? false;
      if (!mounted) return;
      _busyId = null;
      if (swapped) {
        _adminMessage = '已切换到 ${r.activeId}（舞台已确认生效）';
      } else if (r.requiresRestart) {
        _adminMessage = '已切换到 ${r.activeId}，但渲染面未确认；服务端称需重启生效';
      } else {
        _adminMessage =
            '已登记 ${r.activeId}，但渲染面未回执：舞台可能仍是上一个模型（可重试或看舞台错误层）';
      }
      _refresh();
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      _busyId = null;
      _adminMessage = '激活失败：${e.message}';
      _refresh();
    }
  }

  /// 导入一个已在 `assets/models/<id>/` 下的模型目录。
  ///
  /// 后端不做文件上传（ZIP 上传是后置项，且此前被能力快照谎报为 supported）：
  /// 用户放好文件，这里登记 + 校验。**先刷新 registry 视图，再让用户点激活**——
  /// 不自动激活：激活会让舞台换皮，属于用户可见的动作，不该由一次「导入」代劳。
  Future<void> _importModel(String id) async {
    _busyId = id;
    _adminMessage = '正在导入 $id…';
    _refresh();
    try {
      await _modelsApi.import(id);
      if (!mounted) return;
      _busyId = null;
      _adminMessage = '已导入 $id，点「激活」即可换皮';
      _refresh();
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      _busyId = null;
      _adminMessage = '导入失败：${e.message}';
      _refresh();
    }
  }

  Future<void> _toggleMod(String id, bool enabled) async {
    _busyId = id;
    _refresh();
    try {
      await _modsApi.setEnabled(id, enabled);
      if (!mounted) return;
      _busyId = null;
      _adminMessage = '${enabled ? '已启用' : '已停用'} $id';
      _refresh();
      await _loadAdmin();
    } on ApiException catch (e) {
      if (!mounted) return;
      _busyId = null;
      _adminMessage = '操作失败：${e.message}';
      _refresh();
    }
  }

  Future<void> _copyDiagnostics(DiagnosticsSnapshot snapshot) async {
    await copySnapshot(snapshot);
    if (!mounted) return;
    _copied = true;
    _refresh();
    await Future<void>.delayed(const Duration(seconds: 2));
    if (mounted) {
      _copied = false;
      _refresh();
    }
  }
}
