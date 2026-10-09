part of 'package:live2d_ai_shell/main.dart';

extension _ShellBackgroundScaleWiring on _ShellRootState {
  void syncSlideshow(DisplayPrefs prefs) {
    _slideshow
      ..setLibrary(
        length: prefs.backgrounds.length,
        randomOrder: prefs.slideRandom,
      )
      ..start(prefs.slideInterval, onAdvance: _onBackgroundAdvance);
    // 库清空 / 减项 / 换来源之后，**运行时索引跟着同一夹持**（F-0001-2 第二形态）。
    //
    // 旧实现只让渲染层在取值时 clamp 一下：库从 5 项清成 1 项之后
    // `_backgroundIndex` 还是 4。DEC-6 之后设置页的「当前」标记与
    // 铺法 / 位置区读的正是这个索引，于是它会指到夹持后的那一项，而索引本身
    // 没被改回来 —— 下一轮 advance 又从 4 起算，与「从头开始」的直觉不符。
    // 现在索引的合法范围由 `_slideshow` 说了算（setLibrary 里已经夹过），
    // 宿主这一份**只跟随**。
    final int clamped = _slideshow.index;
    if (_backgroundIndex != clamped) {
      _rebuild(() => _backgroundIndex = clamped);
    }
  }

  /// 切到背景库第 N 项（点缩略图 / 拖完排序后回到第一项时用）。
  ///
  /// **只改运行时索引，不落盘**——「读到第几张」不是用户偏好，
  /// 刷新后从头开始才符合直觉。
  void _jumpBackground(int index) {
    if (!mounted) return;
    // 必须**同时**推给轮播控制器（F-0001-2）：它自己有一份 `_index`，
    // 只改宿主那一个的话，下一次 onAdvance 会从**旧的** `_index` 往前走，
    // 把用户刚预览的那张无端切走。`jumpTo` 自带区间夹持、库为空时是
    // 空操作；它**不碰**偏好、不发任何帧（预览是纯运行时动作）。
    _slideshow.jumpTo(index);
    _lastSentStageBg = DisplayPrefs.stageProjectionUrl(widget.prefs, index);
    _rebuild(() => _backgroundIndex = index);
  }

  /// 轮播前进一步。
  ///
  /// 只改运行时索引。外壳重建时把当前库项投影进 `Live2DStage.stageImage`，
  /// 由舞台自己补发。图案和超限图投影为 null，舞台回到纯色，壳仍画该项。
  void _onBackgroundAdvance(int index) {
    if (!mounted) return;
    _lastSentStageBg = DisplayPrefs.stageProjectionUrl(widget.prefs, index);
    _rebuild(() => _backgroundIndex = index);
  }

  void _refresh() {
    // 本模型覆盖上下文（阶段5 D40）随时可能变：换模型（`_activateModel` →
    // `_loadAdmin` → 这里）、保存设置、重新加载都会走到 `_refresh()`。
    // 两个真源（`/app/status` 的 active_model_id、`GET /settings` 的
    // action.active_model_id + models）也都在本方法前后写入，所以统一在这里
    // 推一次 + 排一次防抖下发——挂在别处就会出现「界面变了、舞台没变」。
    _pushActionScalesModelContext();
    _scheduleActionScalesSync();
    _rebuild(() {});
  }

  /// 把「当前模型 + 本模型覆盖」推给 syncer（阶段5 D40）。
  ///
  /// 主真源 = `GET /settings` 的顶层 `active_model_id`（W5r 契约，由
  /// `ActionSettingsView` 携带）与 `action.models`。但换模型后 settings
  /// **不会自动重取**，而 `/app/status` 每次 `_loadAdmin`（`_activateModel`
  /// → `_loadAdmin`）都会刷新——两者同源，取非空的那份最新值，
  /// 这样「换模型后有效值自动重算」才成立。
  void _pushActionScalesModelContext() {
    final ActionSettingsView? action = _settings.remote?.action;
    final Object? live = _status['active_model_id'];
    final String fromStatus = live is String ? live : '';
    final String id = fromStatus.isNotEmpty
        ? fromStatus
        : (action?.activeModelId ?? '');
    _actionScalesSyncer.setModelContext(
      activeModelId: id.isEmpty ? null : id,
      overrides: action == null
          ? const <String, Map<String, double>>{}
          : toModelOverrideMap(action.models),
    );
  }

  /// settings 一变：**先**刷新模型上下文，**再**排一次防抖下发。
  ///
  /// 顺序不能反：先 schedule 会把「上一份 active_model_id」算出的载荷发出去，
  /// 换模型时就会有一帧旧模型的覆盖值（用户看到的是「换皮后幅度闪回旧值」）。
  void _onActionScalesModelContextChanged() {
    _pushActionScalesModelContext();
    _scheduleActionScalesSync();
  }
}
