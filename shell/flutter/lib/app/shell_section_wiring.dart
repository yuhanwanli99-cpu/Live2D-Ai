part of 'package:live2d_ai_shell/main.dart';

extension _ShellSectionWiring on _ShellRootState {
  Future<void> _testLlm() async {
    final int epoch = _resultEpoch;
    _rebuild(() => _llmTesting = true);
    try {
      final SettingsTestOutcome o = await _api.testLlm();
      if (!mounted || epoch != _resultEpoch) return;
      _rebuild(() {
        _llmTesting = false;
        // 文案**逐字未变**；变的只是「成败」不再由这段文字推出来——它直接
        // 来自服务端的 `o.ok`（`TestOutcome.ok`）。
        _llmTest = FieldTestResult(
          ok: o.ok,
          message: o.ok
              ? 'ok · ${o.latencyMs ?? '?'} ms'
                    '${o.modelEcho == null || o.modelEcho!.isEmpty ? '' : ' · 模型：${o.modelEcho}'}'
              : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}',
        );
      });
    } on ApiException catch (e) {
      if (!mounted || epoch != _resultEpoch) return;
      _rebuild(() {
        _llmTesting = false;
        _llmTest = FieldTestResult(ok: false, message: '失败：${e.message}');
      });
    }
  }

  Future<void> _testTts() async {
    final int epoch = _resultEpoch;
    _rebuild(() => _ttsTesting = true);
    try {
      final SettingsTestOutcome o = await _api.testTts();
      if (!mounted || epoch != _resultEpoch) return;
      _rebuild(() {
        _ttsTesting = false;
        _ttsTest = FieldTestResult(
          ok: o.ok,
          message: o.ok
              // 成功时把服务端的 note 也带上（例如「上游可达但未提供 /models」）
              // ——否则用户会以为「自检通过 = 合成没问题」，而下一次合成失败时
              // 又回到「明明通过了却不行」的困惑。
              ? 'ok · ${o.latencyMs ?? '?'} ms${o.note == null ? '' : ' · ${o.note}'}'
              : '失败：${o.errorMessage ?? o.errorCode ?? '未知原因'}',
        );
      });
    } on ApiException catch (e) {
      if (!mounted || epoch != _resultEpoch) return;
      _rebuild(() {
        _ttsTesting = false;
        _ttsTest = FieldTestResult(ok: false, message: '失败：${e.message}');
      });
    }
  }

  /// 打开设置前的兜底：**懒加载一次**。
  void _onSectionChanged(SettingsSection next) => _gotoSection(next);

  /// 切换设置分区（**唯一入口**）。
  ///
  /// # 为什么必须走这一个入口（2026-09-11，P2-2）
  ///
  /// 过去有两个地方各自 `setState(() => _section = …)`：分区 chip（这里）
  /// 与错误横幅的「去 LLM 设置 / 去语音合成设置」按钮。于是**内联结果**
  /// （`_llmTest` / `_ttsTest` / `_adminMessage` / `_stageImageMessage`）
  /// 会一直挂在 State 上：用户测出「失败：401」，
  /// 修好配置、切到别的分区、再回来——**那句失效的结论还在**，
  /// 而它描述的已经是上一套配置了。
  ///
  /// 现在切分区**先清掉这些一次性结果**，并且用 [_resultEpoch] 把
  /// 「切走之后才回来的响应」也丢掉（否则那个迟到的结果会落在用户已经
  /// 离开的分区上，看起来像是刚测的）。
  void _gotoSection(SettingsSection next) {
    if (next == _section) return;
    // 离开「外观与互动」前把待发的本模型覆盖落下：用户拖完立刻切分区，
    // 不能把那一次改动留在防抖窗口里丢掉。
    _modelOverrideCoalescer.flush();
    _rebuild(() {
      _section = next;
      _clearTransientResults();
    });
    unawaited(_ensureSettingsLoaded());
  }

  /// 记录一次 Mod 变更：挂常驻提示 + 弹一条带入口的 SnackBar。
  ///
  /// **为什么两处都要**：SnackBar 会消失（用户可能正好没看屏幕），常驻提示
  /// （Mod 分区顶部 + 聊天区顶部）才是「我重启了没有」的备忘。两者文案同源
  /// （`modRestartNoticeText` / `modRestartSnackText`），不会互相矛盾。
  ///
  /// 刻意**不**在 `_clearTransientResults` 里清掉它：切分区不是「已经重启」。
  void _notifyModChanged(String what) {
    _rebuild(() => _modRestartNotice = modRestartNoticeText(what));
    final ScaffoldMessengerState? messenger = ScaffoldMessenger.maybeOf(context);
    messenger
      ?..clearSnackBars()
      ..showSnackBar(
        SnackBar(
          content: Text(modRestartSnackText(what)),
          duration: const Duration(seconds: 8),
        ),
      );
  }

  /// 用户关掉常驻提示（只有他能判断「已经重启过了」）。
  void _dismissModRestart() {
    if (_modRestartNotice == null) return;
    _rebuild(() => _modRestartNotice = null);
  }

  void _clearTransientResults() {
    _resultEpoch++;
    _llmTest = null;
    _llmTesting = false;
    _ttsTest = null;
    _ttsTesting = false;
    _adminMessage = null;
    _stageImageMessage = null;
    _stageImageFailed = false;
    _shellImageMessage = null;
    _shellImageFailed = false;
    _modelOverrideMessage = null;
    _modelOverrideFailed = false;
    _copied = false;
    _adminError = null;
  }
}
