/// 壁纸 Mod 闭环接线（**`part of` 组合根**）——Wave 2 B 轨，2026-09-14。
///
/// # 闭环长什么样（每一环都是既有通道）
///
/// ```text
/// DisplayPrefs.stagePlaylist（Flutter 自己的偏好列表，图住这里）
///    │  「加入轮播」/「清空轮播」→ 长度变化
///    ▼
/// POST /api/v1/mods/wallpaper/config   {"config":{…,"playlist_len":N}}   （既有端点）
///    ▼
/// WallpaperRuntime（Rust）  state_json() —— 内部 Instant 推进纯策略 tick
///    ▼
/// GET /api/v1/mods/wallpaper/state     （基座 Wave 2 端点，≥5 s 轮询，停用即停）
///    ▼
/// applyWallpaperPatch（纯函数）→ DisplayPrefs.copyWith(…)
///    ▼
/// Live2DStage.sendStageBg   （既有下发通道；不碰 stage_bg.rs / framebuffer）
/// ```
///
/// # 纪律
///
/// - **只在启用且 `mode != off` 时轮询**（判据 `shouldPollWallpaperState`，纯函数）；
///   停用 / 关模式 → `Timer` 立即取消（`_syncWallpaperPolling`）；
/// - 轮询间隔 ≥5 s（`kWallpaperPollInterval`）：Mod 的节拍就是这次调用的增量，
///   它不需要更细的时钟，前端也不该为看一眼状态打更多请求；
/// - `503 state_unavailable` 是**正常**（Mod 刚停用 / worker 正忙）→ 不当错误提示；
/// - 列表长度**没变就不写**（每次写回都会重启 Mod、把策略游标冲掉）。
///
/// 用 `part` + 扩展方法的理由见 `main.dart` 头注。
part of 'package:live2d_ai_shell/main.dart';

extension _ShellWallpaperWiring on _ShellRootState {
  /// 从已加载的 Mod 列表里找 wallpaper（找不到 = 没注册）。
  ModInfo? _wallpaperMod() {
    for (final ModInfo mod in _mods) {
      if (mod.id == kWallpaperModId) return mod;
    }
    return null;
  }

  /// 现在该不该轮询（纯判据的接线侧：注册 + 启用 + mode != off）。
  bool _wallpaperPollingWanted() {
    final ModInfo? mod = _wallpaperMod();
    if (mod == null) return false;
    return shouldPollWallpaperState(
      registered: true,
      enabled: mod.enabled,
      mode: mod.config['mode'],
    );
  }

  /// 启动时轻量引导一次：只取 Mod 列表（**不**跑整套管理面加载）。
  ///
  /// 为什么不能等用户打开设置：壁纸闭环是「Mod 一启用就该开始换图」，
  /// 而 `_loadAdmin` 只在进设置面时跑。这里只多一个 GET。
  Future<void> _bootstrapWallpaper() async {
    try {
      final List<ModInfo> mods = await _modsApi.list();
      if (!mounted) return;
      _mods = mods;
      _syncWallpaperPolling();
      // 本地列表与服务端 config 可能不一致（换设备 / 手改 mods.json）——
      // 启动就对齐一次，否则 Mod 会以为「没有图可切」。
      await _syncWallpaperPlaylistLen();
    } on ApiException {
      // 拿不到就不轮询；用户进设置面时 `_loadAdmin` 会再试。
    }
  }

  /// 按当前状态**开始或停止**轮询（唯一的 Timer 生命周期入口）。
  void _syncWallpaperPolling() {
    if (_wallpaperPollingWanted()) {
      _wallpaperTimer ??= Timer.periodic(
        kWallpaperPollInterval,
        (_) => unawaited(_pollWallpaperOnce()),
      );
      return;
    }
    _wallpaperTimer?.cancel();
    _wallpaperTimer = null;
  }

  /// 取一次状态 → 落 patch（+ 顺带对齐列表长度）。
  Future<void> _pollWallpaperOnce() async {
    // 每次 tick 先复核门禁：用户在两次 tick 之间停用了壁纸，立刻停。
    if (!_wallpaperPollingWanted()) {
      _syncWallpaperPolling();
      return;
    }
    Map<String, Object?>? state;
    try {
      state = await _wallpaperApi.state();
    } on ApiException {
      // 网络抖一下不值得打断用户；下一次 tick 再试。
      return;
    }
    if (!mounted || state == null) return;

    // ① prefs_patch → DisplayPrefs（空列表 / 无 patch 都是**明确跳过**）。
    final WallpaperPatchResult result = applyWallpaperPatch(
      widget.prefs,
      state['prefs_patch'],
    );
    if (result.applied) _updatePrefs(result.prefs);

    // ② 列表长度对齐（用快照里的 `playlist_len`，它比本地缓存的 config 新）。
    unawaited(
      _syncWallpaperPlaylistLen(remoteLenOverride: state['playlist_len']),
    );
  }

  /// 把本地播放列表长度写回 Mod config（**长度没变就不写**）。
  ///
  /// `POST …/config` 是整份替换——所以从服务端现有 config 出发只覆盖
  /// `playlist_len`（`configWithPlaylistLen`），否则会把 `mode` / `interval_secs`
  /// 一起抹掉。写成功后重取一次管理面，让本地缓存跟上（否则下一次又按旧值写）。
  Future<void> _syncWallpaperPlaylistLen({Object? remoteLenOverride}) async {
    final ModInfo? mod = _wallpaperMod();
    if (mod == null) return;
    final int localLen = widget.prefs.stagePlaylist.length;
    final Object? remoteLen =
        remoteLenOverride ?? mod.config['playlist_len'];
    if (!needsPlaylistLenWriteBack(localLen: localLen, remoteLen: remoteLen)) {
      return;
    }
    try {
      final ModConfigResult result = await _modsApi.setConfig(
        kWallpaperModId,
        configWithPlaylistLen(mod.config, localLen),
      );
      if (!mounted || !result.ok) return;
      await _loadAdmin();
    } on ApiException {
      // 写失败不弹错（那会在启动瞬间冒一条无从处置的横幅）：
      // 下一次列表变化 / 下一次轮询会再试。
    }
  }
}
