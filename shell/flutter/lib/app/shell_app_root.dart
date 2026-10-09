part of 'package:live2d_ai_shell/main.dart';

class Live2DShellApp extends StatefulWidget {
  const Live2DShellApp({super.key});

  @override
  State<Live2DShellApp> createState() => _Live2DShellAppState();
}

class _Live2DShellAppState extends State<Live2DShellApp> {
  /// 本地显示偏好（主题 / 缩放 / 口型 / 音量 / 静音），持久化在 localStorage。
  DisplayPrefs _prefs = const DisplayPrefs();

  /// 背景图的字节库（IndexedDB）。
  ///
  /// **2026-09-28（F-0002-2）：一开始就是真库，没有内存占位。**
  /// `createBackgroundStore()` 是**同步**构造（web 那份的 `open` 是惰性的，
  /// 在实现内部），所以这里根本不必等水合。老实现是「先 MemoryBackgroundStore
  /// 占位 → 水合完成才换真库」，于是窗口里用户导入的字节进的是**即将被丢弃的
  /// 内存库**，界面却说「已加进背景库」——下次启动那张图被判孤儿摘除。
  /// 故障面没有变：`put` 写不进去会返回 `false`，调用方照样如实转述。
  ///（`final`：**一次构造、一个实例**——水合不再换库，见上。）
  final BackgroundStore _store = createBackgroundStore();

  /// **水合这一次读**的兜底时限。
  ///
  /// IndexedDB 在某些隐私配置下会**既不 resolve 也不 reject**（见
  /// `background_store_web.dart` 的头注）。一个「背景图功能」不该有
  /// 权力让整个应用起不来，所以给这次读一个上限，超时就不动偏好继续跑。
  ///
  /// ⚠️ 它**只**管这次读（F-0002-3）。过去这里还顶着一个「超时就退回内存库」
  /// 的假兜底（`Future.value(createBackgroundStore()).timeout(…)`）：那层
  /// timeout 永远不触发（构造是同步的、也不会抛），注释承诺的退路在线上**不存在**。
  /// 真实的兜底在 `_IdbBackgroundStore` 内部——open 失败会被记下来，之后每个
  /// 操作都**诚实地失败**（`put` 返回 false），而不是偷偷换一个库。
  static const Duration _hydrateTimeout = Duration(seconds: 3);

  /// 背景库字节是否还在水合（交接项 9b）。
  ///
  /// `true` → 外观区显示「正在读回背景库…」并禁用背景库的写操作；
  /// 这一次读结束（成功读回、或 3 s 超时兜底）就翻成 `false`。
  /// 为什么要暴露它：读回窗口里的写操作与读回结果**会打架**，而界面当时
  /// 一句话都不说——用户以为「加进去了」，清单却按读回结果被覆盖。
  bool _backgroundHydrating = true;

  @override
  void initState() {
    super.initState();
    // 读盘只在启动时一次：之后的每一次写都是 `_update` 的副作用。
    _prefs = loadDisplayPrefs();
    unawaited(_hydrateBackgrounds());
  }

  /// 把偏好里的背景清单换成**画得出来的**图（2026-09-27）。
  ///
  /// 为什么是异步而不塞进 `main()`：`runApp` 之前 await 存储，
  /// 等于让「IndexedDB 慢」变成「应用起不来」。这里改成
  /// 「先起应用，字节回来后补一次 setState」——
  /// 代价是背景可能晚几十毫秒出现（肉眼几乎看不出），
  /// 换来的是**存储故障永远拖不垮启动**。
  Future<void> _hydrateBackgrounds() async {
    final BackgroundStore store = _store;
    final BackgroundHydration hydrated = await _hydrateSafely(store, _prefs);
    if (!mounted) return;
    // **只回填背景域**（F-0013-1 / F-0001-3）：水合窗口里用户改的主题 / 音量 /
    // 透明度、以及背景库的增 / 删 / 重排，全部保留**当前最新值**。
    //
    // 旧实现是 `_prefs = hydrated.prefs`——把水合**开始时刻的快照**整体盖回去，
    // 窗口期的改动全部回滚；`migrated` 时还会把这份旧对象写回盘。
    setState(() {
      _prefs = applyBackgroundHydration(_prefs, hydrated);
      // 水合这一次读结束了（读回成功 / 超时兜底都算结束）。
      _backgroundHydrating = false;
    });
    // 搬了旧档（把大 base64 从 localStorage 挪进 IndexedDB）/ 摘掉了确认没有的
    // 孤儿就写回**合并后**的偏好（不是那份旧快照），否则那份 base64 会永远留在
    // 偏好里，下次启动再搬一次。
    if (hydrated.migrated) saveDisplayPrefs(_prefs);
  }

  Future<BackgroundHydration> _hydrateSafely(
    BackgroundStore store,
    DisplayPrefs prefs,
  ) => hydrateBackgrounds(
    prefs,
    store,
    // 剪枝的保留集要读**此刻**的偏好：窗口里刚导入的那一项的字节
    // 绝不能被当成孤儿真删掉（F-0013-1 的另一半）。
    liveKeptIds: () => _imageIdsIn(_prefs),
  ).timeout(
    _hydrateTimeout,
    // 超时 = 这次水合**什么都没算出来**：不动背景域，更不覆盖任何字段。
    onTimeout: () => const BackgroundHydration(),
  );

  /// 偏好里所有**图片**项的 id（水合剪枝的保留集用它）。
  Set<String> _imageIdsIn(DisplayPrefs prefs) => <String>{
    for (final BackgroundItem item in prefs.backgrounds)
      if (item is BackgroundImage) item.id,
  };

  /// 更新并**立即持久化**；返回**是否真的写进了本机存储**。
  ///
  /// 值没变时直接返回 `true`（没有需要写的东西）。写失败（无痕 / 配额满 /
  /// 存储被禁）由调用方决定怎么如实告诉用户——这里不再静默吞掉。
  bool _update(DisplayPrefs next) {
    if (next == _prefs) return true;
    setState(() => _prefs = next);
    return saveDisplayPrefs(next);
  }

  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'Live2D Ai',
    debugShowCheckedModeBanner: false,
    // 外观：配色（AppThemeId）+ 界面透明。描边与圆角是固定值。
    // 都只走本地偏好、不需要保存按钮。
    theme: buildAppTheme(
      _prefs.theme,
      AppMaterial(uiTransparency: _prefs.uiTransparency),
    ),
    home: ShellRoot(
      prefs: _prefs,
      store: _store,
      backgroundHydrating: _backgroundHydrating,
      onPrefsChanged: _update,
    ),
  );
}

/// 组合根：持有全部长生命周期对象，把回调接到新外壳上。
class ShellRoot extends StatefulWidget {
  const ShellRoot({
    required this.prefs,
    required this.store,
    required this.backgroundHydrating,
    required this.onPrefsChanged,
    super.key,
  });

  /// 本地显示偏好（**由根持有**；见 `Live2DShellApp` 的说明）。
  final DisplayPrefs prefs;

  /// 背景图字节库（见 `data/background_store.dart`）。
  final BackgroundStore store;

  /// 背景库字节是否还在水合（交接项 9b）：由根持有、只往下传，外观区据此
  /// 显示「正在读回背景库…」并禁用背景库的写操作。
  final bool backgroundHydrating;

  /// 上报偏好变更；返回**是否成功落盘**（失败时调用方给一句可执行文案）。
  final bool Function(DisplayPrefs) onPrefsChanged;

  @override
  State<ShellRoot> createState() => _ShellRootState();
}
