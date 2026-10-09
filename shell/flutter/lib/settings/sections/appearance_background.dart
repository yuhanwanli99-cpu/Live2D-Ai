/// 「外观与互动」分区的**背景域**（2026-09-28 · Stage B · R6-b 第 0 步抽取）。
///
/// # 这个文件是怎么来的
///
/// `appearance_section.dart` 一度把「外观 / 舞台单图轮播 / 舞台与口型 / 互动」
/// 四个域全装在一个文件里（**1489 行**，超 ≤500 上限且不满足 ≤1000 豁免带）。
/// 本文件是它的 **`part`**（不是独立库：私有 widget 名与分区共用一个库），
/// 承载**壳背景**那一整块：来源与选图、背景库管理与预览、逐图样式、
/// 铺法 / 位置 / 不透明度 / 遮罩 / 模糊、壳背景轮播。
///
/// `part` 而不是 `import`：抽取要求**只搬不改**，而这些 widget 全是私有名
/// （`_BackgroundBlock` / `_LibraryManager` …）；`part` 让「搬」这件事在语言层面
/// 零改动——没有可见性变化、没有 API 变化，抽取前后渲染结果逐像素相同。
///
/// # 舞台那一侧不在这里
///
/// 「舞台单图轮播」（`stagePlaylist`，走渲染面 `stage-bg` 帧）仍住在
/// `appearance_section.dart`：它管的是**舞台那张图**，通道与壳背景库不同
/// （见 `docs/architecture/background-parity-vscode-background.md` §7.1 DEC-2），
/// 与「舞台与口型」同域。
///
/// # 行数拆分（2026-10-06，R4-T2 落地本文件登记的那条 Stage C3 债）
///
/// 拆分前 **1552 行**（= 抽取基线 1011 行 + R6-b 的新 UI 约 540 行，超 ≤1000
/// 豁免上限，头注当时**如实登记为既有债**，并写下了拆分口径）。本轮照那条
/// 口径按**顶层类边界**拆成三个 part（都是 `appearance_section.dart` 这个库的
/// part；`part` 继承库的 import，零可见性改动）：
///
/// | 内容 | 去处 | 行数 |
/// | --- | --- | --- |
/// | 运行时上下文 + 提交滑杆 + 背景块外壳 | 本文件 | 724 |
/// | 背景库管理（`_LibraryManager` / `_MoreOptions` / `_StyleOverrideRow` / `_UsageBar` / `_LibraryRow` / `_RowNotice`） | `appearance_background_library.dart` | 644 |
/// | 逐图样式编辑器 + 图块（`_ItemStyleEditor` / `_ImageTile` / `_AlignPad`） | `appearance_background_style.dart` | 192 |
///
/// `_StyleOverrideRow` 跟着**唯一使用它的** `_LibraryManager` 走，不是按目录
/// 切——「类不能跨 part」是这种拆分唯一的硬约束，其余按调用关系就近。
part of 'appearance_section.dart';

// ─────────────────── 运行时上下文（DEC-6 / 交接项 9b） ───────────────────

/// 背景域的**运行时**上下文：外壳注入设置面板，外观区据此回答两件事——
/// 「此刻画面上的是**哪一项**」与「背景库字节还在读回吗」。
///
/// # 为什么是 InheritedWidget 而不是 AppearanceSection 的构造参数
///
/// 外观分区的构造点在 app/shell_settings.dart（part of main.dart），而「当前项」
/// 只有 AppShell 知道：来源 = 舞台那张时是那张图，来源 = 背景库时要按
/// **运行时轮播索引**取（见 app_shell.dart 的 _currentBackground）。
/// 判据**只留一处**——让外观区自己再算一遍就会长出第二份，而 DEC-6 要修的
/// 正是「设置页永远看第 0 项」这个不一致。
///
/// 运行时索引**不落盘**：它不是用户偏好，刷新后从头开始。
class BackgroundRuntimeScope extends InheritedWidget {
  const BackgroundRuntimeScope({
    required this.index,
    required this.current,
    required this.hydrating,
    required super.child,
    super.key,
  });

  /// 轮播到了背景库第几项（来源不是背景库时没有意义）。
  final int index;

  /// 外壳此刻**真的会画**的那一项（null = 没东西画）。
  final BackgroundItem? current;

  /// 背景库字节是否还在水合（IndexedDB 读回中）。
  final bool hydrating;

  /// 取当前上下文；没有外壳注入时返回 null。
  static BackgroundRuntimeScope? maybeOf(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<BackgroundRuntimeScope>();

  /// 「当前项」：外壳注入的优先。
  ///
  /// **没有注入**时回落 prefs.effectiveBackground（= 第 0 项）——只有孤立 pump
  /// AppearanceSection 的 widget 测试会走到这一支；生产路径由
  /// AppShell._settingsSurface 注入，回归见
  /// test/appearance_background_runtime_test.dart（那条钉的就是「一定注入了」）。
  static BackgroundItem? currentOf(BuildContext context, DisplayPrefs prefs) =>
      maybeOf(context)?.current ?? prefs.effectiveBackground;

  @override
  bool updateShouldNotify(BackgroundRuntimeScope oldWidget) =>
      index != oldWidget.index ||
      current != oldWidget.current ||
      hydrating != oldWidget.hydrating;
}

/// 库项的**身份**（F-0003-3：选中 / 展开都存身份，**不存下标**）。
///
/// 下标在重排之后会指向别的项——那正是「勾选第 0 项、拖动后删掉的是别人」的
/// 根因。身份跟着项走：重排之后它还是同一张图。
String backgroundItemIdentity(BackgroundItem item) => switch (item) {
  BackgroundImage(:final String id) => "image:$id",
  BackgroundPattern(:final int id) => "pattern:$id",
};

/// 背景库管理模式的开关 key（测试按它进管理模式，D1）。
const Key kBackgroundManageToggleKey = ValueKey<String>("bg-manage-toggle");


/// 背景库：加图、清空、不透明度、两项以上才出现的轮播。
///
/// 当前这一项同时投影到舞台。**不再有内置图案**（2026-10-08）：用户只用
/// 自己的图，库里没有图时舞台与壳都回到纯色。
class _BackgroundBlock extends StatelessWidget {
  const _BackgroundBlock({
    required this.prefs,
    required this.message,
    required this.failed,
    required this.onAddImage,
    required this.onClearLibrary,
    required this.onRemoveItem,
    required this.onChanged,
    this.onReorderItem,
    this.onRemoveMany,
    this.onPreviewItem,
  });

  final DisplayPrefs prefs;
  final String? message;
  final bool failed;
  final VoidCallback? onAddImage;
  final VoidCallback? onClearLibrary;
  final ValueChanged<int>? onRemoveItem;
  final ValueChanged<DisplayPrefs> onChanged;

  /// 拖动排序（旧下标 → 新下标）。
  final void Function(int oldIndex, int newIndex)? onReorderItem;

  /// 批量删除（**升序**的下标集合）。
  final ValueChanged<List<int>>? onRemoveMany;

  /// 点缩略图 = 先看这一张。
  final ValueChanged<int>? onPreviewItem;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    final List<BackgroundItem> items = prefs.backgrounds;
    final BackgroundRuntimeScope? runtime = BackgroundRuntimeScope.maybeOf(
      context,
    );
    final BackgroundItem? current = BackgroundRuntimeScope.currentOf(
      context,
      prefs,
    );
    final bool hydrating = runtime?.hydrating ?? false;
    final String? currentIdentity = current == null
        ? null
        : backgroundItemIdentity(current);
    final bool canSlide = items.length >= 2;
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          // 2026-10-09：三级功能介绍全删——「背景」这个组标题下不再挂说明。
          Text('背景', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s2),
          _LibraryManager(
            items: items,
            palette: palette,
            currentIdentity: currentIdentity,
            onReorder: onReorderItem ?? _noopReorder,
            onRemove: onRemoveItem,
            onRemoveMany: onRemoveMany,
            onPreview: onPreviewItem,
          ),
          const SizedBox(height: Space.s2),
          if (hydrating)
            Padding(
              padding: const EdgeInsets.only(bottom: Space.s1),
              child: EmphasizedText(
                '**正在读回背景库…**（本机存储里的清单与字节）——读回来之前先不改它。',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: colors.contentMuted,
                ),
              ),
            ),
          Wrap(
            spacing: Space.s2,
            runSpacing: Space.s2,
            children: <Widget>[
              FilledButton.tonal(
                onPressed: hydrating ? null : onAddImage,
                child: const Text('添加图片'),
              ),
              if (items.isNotEmpty)
                TextButton(
                  onPressed: hydrating ? null : onClearLibrary,
                  child: const Text('清空背景库'),
                ),
            ],
          ),
          if (message != null)
            Padding(
              padding: const EdgeInsets.only(top: Space.s1),
              child: EmphasizedText(
                message!,
                style: theme.textTheme.labelSmall?.copyWith(
                  color: failed ? palette.warning : colors.contentMuted,
                ),
              ),
            ),
          const SizedBox(height: Space.s2),
          CommitSliderField(
            label: '不透明度（图）',
            icon: Icons.opacity,
            value: prefs.backgroundOpacity,
            min: DisplayPrefs.minBackgroundOpacity,
            max: DisplayPrefs.maxBackgroundOpacity,
            onCommit: (double v) =>
                onChanged(prefs.copyWith(backgroundOpacity: v)),
            // 数字项只留一行范围（数从控件现有 min/max 抄）。
            description: '0.0-1.0',
            minLabel: '看不见',
            maxLabel: '压满',
          ),
          if (canSlide) ...<Widget>[
            const Divider(),
            ToggleField(
              label: '轮播',
              icon: Icons.slideshow,
              value: prefs.slideInterval > 0,
              onChanged: (bool on) =>
                  onChanged(prefs.copyWith(slideInterval: on ? 30 : 0)),
            ),
            if (prefs.slideInterval > 0) ...<Widget>[
              CommitSliderField(
                label: '轮播间隔',
                icon: Icons.timer_outlined,
                value: prefs.slideInterval.toDouble(),
                min: DisplayPrefs.minSlideIntervalSeconds.toDouble(),
                max: DisplayPrefs.maxSlideIntervalSeconds.toDouble(),
                percentage: false,
                suffix: ' 秒',
                onCommit: (double v) =>
                    onChanged(prefs.copyWith(slideInterval: v.round())),
                description: '5-300',
                minLabel: '快',
                maxLabel: '慢',
              ),
              ToggleField(
                label: '随机顺序',
                icon: Icons.shuffle,
                value: prefs.slideRandom,
                onChanged: (bool v) => onChanged(prefs.copyWith(slideRandom: v)),
              ),
            ],
          ],
          const Divider(),
          CommitSliderField(
            label: '透明程度（界面）',
            icon: Icons.layers_outlined,
            value: prefs.uiTransparency,
            min: DisplayPrefs.minUiTransparency,
            max: DisplayPrefs.maxUiTransparency,
            onCommit: (double v) => onChanged(prefs.copyWith(uiTransparency: v)),
            description: '0.0-1.0',
            minLabel: '不透明',
            maxLabel: '最透',
          ),
        ],
      ),
    );
  }
}
