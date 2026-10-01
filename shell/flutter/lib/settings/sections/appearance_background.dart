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
/// # 行数（**如实标注：超 1000，是既有债**）
///
/// 现状 **1551 行** = 抽取基线 1011 行（背景域原样搬出） + R6-b 的新 UI 约 540 行。
/// **超 ≤1000 的豁免上限**，按项目口径**如实标注为既有债**：
///
/// - 规模来自 `appearance_section.dart` 原本的 **1489 行**；
/// - Stage B 的任务口径是「**只搬不改**」（先抽、先证明行为零变化），
///   抽取那一刻**不许**顺手拆小——那正是「搬运 + 重构同时做」的经典事故面；
/// - 本轮又在这里落地了 §5.3 的 UI（四档铺法 / tileSize / 逐图样式 /
///   管理模式 / 两套轮播互斥），所以它比抽取基线更大。
///
/// Stage C3 是唯一能继续拆的地方；拆分口径写在这里，接手的照它拆：
///
/// | 该拆出去的东西 | 本文件里的行数（当前实测） | 建议去处 |
/// | --- | --- | --- |
/// | 背景库管理面板（`_LibraryManager` :799 / `_UsageBar` :1109 / `_LibraryRow` :1157 / `_RowNotice` :1342） | ~450 | `appearance_background_library.dart` |
/// | 逐图样式编辑器（`_StyleOverrideRow` :1036 / `_ItemStyleEditor` :1366） | ~180 | 同一个库文件 |
/// | 渲染参数（`_BackgroundBlock` :275 / `_fitControls` :648 / `_MoreOptions` :725 / `_ImageTile` :1464 / `_AlignPad` :1491） | ~700 | `appearance_background_style.dart` |
/// | 运行时上下文与提交滑杆（`BackgroundRuntimeScope` :60 / helpers / `_CommitSliderField` :163） | ~180 | 两个库文件共用，或留在本文件 |
///
/// 拆完本文件应当只剩「背景块的外壳」。`appearance_section.dart` 这一侧
/// 同时从 1489 行降到 **709 行**（它自己那条头注也写着豁免理由）。
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

/// 「换掉（或清掉）某一项样式」的新项。
///
/// 为什么不直接用 BackgroundImage.copyWith：它的 clearStyle 是**三项一起清**的
/// 开关，而逐图编辑器要能只清一项；copyWith(opacity: null) 又表达不了「清掉」
/// （那里的 null 是「不改」）。
///
/// **dataUrl 原样带走**：丢了它这张图会退回「字节还没读回来」，缩略图与本项
/// 渲染一起消失，直到下一次水合。
BackgroundImage styledBackgroundImage(
  BackgroundImage item, {
  double? opacity,
  int? fit,
  int? align,
  bool dropOpacity = false,
  bool dropFit = false,
  bool dropAlign = false,
}) => BackgroundImage(
  id: item.id,
  dataUrl: item.dataUrl,
  opacity: dropOpacity ? null : (opacity ?? item.opacity),
  fit: dropFit ? null : (fit ?? item.fit),
  align: dropAlign ? null : (align ?? item.align),
);

/// 位置垫的 key（测试按它钉「位置区什么时候显示」）。
const Key kBackgroundAlignPadKey = ValueKey<String>("bg-align-pad");

/// 背景库管理模式的开关 key（测试按它进管理模式，D1）。
const Key kBackgroundManageToggleKey = ValueKey<String>("bg-manage-toggle");

// ─────────────── 滑杆：拖动只动草稿，停手才提交（交接项 9a） ───────────────

/// 「拖动只动草稿、停手才提交」的滑杆。
///
/// # 为什么需要这一层
///
/// SliderField.onChanged 是**每帧**回调，而外观区每一条滑杆的落点都是
/// 「改偏好 → 整份 jsonEncode + setItem + 下发渲染面」。用户设了舞台壁纸时
/// （偏好里那张 base64 有协议预算 1.5 M 字符），拖一次滑杆 = 每帧一次 MB 级
/// 同步序列化。音量滑杆 2026-09-28 已经改成 onChangeEnd（见 ui/audio_bar.dart），
/// 这里把外观区剩下的滑杆统一到同一约定：
///
/// | 事件 | 做什么 |
/// | --- | --- |
/// | onChanged（拖动中，每帧） | 只更新**草稿**：读数跟着手指走 |
/// | 停手 AppDurations.base（防抖） | 把草稿**提交**给宿主，落盘一次 |
///
/// # 为什么是防抖而不是 Slider.onChangeEnd
///
/// SliderField 住 ui/field_row.dart ——本波次的文件归属之外，且它被十几个分区
/// 共用；在外观区自建一层只影响本分区。**代价（如实记录）**：拖动中途停顿超过
/// 防抖窗口会多提交一次（「一次连续拖动至多落一次盘」仍成立）；Stage C3 若给
/// SliderField 补上 onChangeEnd，这一层应当删掉、改回直接用 SliderField。
///
/// 语义**一字不改**：拖动中界面读数跟手；停手后落盘值 == 滑杆终值 == 界面读数
/// （宿主回灌的值优先，不拿草稿硬撑）。
class _CommitSliderField extends StatefulWidget {
  const _CommitSliderField({
    required this.label,
    required this.icon,
    required this.value,
    required this.min,
    required this.max,
    required this.onCommit,
    this.percentage = true,
    this.suffix = "",
    this.description,
    this.minLabel,
    this.maxLabel,
  });

  final String label;
  final IconData icon;
  final double value;
  final double min;
  final double max;

  /// 停手那一刻的提交（**一次连续拖动至多一次**）。
  final ValueChanged<double> onCommit;
  final bool percentage;
  final String suffix;
  final String? description;
  final String? minLabel;
  final String? maxLabel;

  /// 防抖窗口 = AppDurations.base（200 ms）。
  ///
  /// 为什么不自己写一个毫秒数：外观区的时长受**令牌门禁**管
  /// （design_tokens_lint_test：「UI 动效时长只能取 AppDurations 的 4 档」），
  /// 而「停手多久算停手」与「内容切换用多久」在观感上是同一量级。
  static const Duration debounce = AppDurations.base;

  @override
  State<_CommitSliderField> createState() => _CommitSliderFieldState();
}

class _CommitSliderFieldState extends State<_CommitSliderField> {
  /// 拖动中的草稿（null = 跟随 widget.value）。
  ///
  /// 除了「停手才提交」之外还有一条理由：拖动过程中宿主会因为**别的原因**重建
  /// （每个流式增量都在重建整棵壳），草稿留在 State 里，滑杆才不会被旧值弹回去。
  double? _draft;
  Timer? _pending;

  @override
  void didUpdateWidget(_CommitSliderField oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 宿主把值回灌了（提交成功 / 别处改了这项）⇒ 草稿使命结束。
    // 判据是「值变了」而不是「值等于草稿」：宿主若把值夹持成别的数，
    // 界面必须显示宿主那份。
    if (widget.value != oldWidget.value) {
      _pending?.cancel();
      _pending = null;
      _draft = null;
    }
  }

  @override
  void dispose() {
    _pending?.cancel();
    super.dispose();
  }

  void _onDrag(double next) {
    setState(() => _draft = next);
    _pending?.cancel();
    _pending = Timer(_CommitSliderField.debounce, _commit);
  }

  void _commit() {
    _pending?.cancel();
    _pending = null;
    final double? draft = _draft;
    if (draft == null) return;
    // 先把草稿交还给宿主：宿主回填多少就显示多少（它可能夹持）。
    setState(() => _draft = null);
    widget.onCommit(draft);
  }

  @override
  Widget build(BuildContext context) => SliderField(
    label: widget.label,
    icon: widget.icon,
    value: _draft ?? widget.value,
    min: widget.min,
    max: widget.max,
    percentage: widget.percentage,
    suffix: widget.suffix,
    description: widget.description,
    onChanged: _onDrag,
    minLabel: widget.minLabel,
    maxLabel: widget.maxLabel,
  );
}

/// 背景库那一整块（2026-09-27）。
///
/// # 它取代了原来那两段（「舞台背景图」+「壳背景」）
///
/// 旧形态的问题不是文案长，是**结构**：两个字段、两个真相、两种清空方式，
/// 而用户心里只有一件事——「给这个壳换张背景」。现在合成一块：
///
/// 1. **背景库**（图 + 内置图案，有序，可增可删）—— 用户能表达「不止一张」；
/// 2. **铺法 / 位置 / 透明度 / 模糊 / 遮罩**—— 只作用在壳，不碰舞台；
/// 3. **轮播**（间隔 + 随机 + 过渡）；
/// 4. **与舞台同步**（默认开）—— 开时壳画舞台那张，背景库留着不渲染。
///
/// 三条文案纪律与旧的一样：不是二选一、失败要说实话、按钮用文字。
class _BackgroundBlock extends StatelessWidget {
  const _BackgroundBlock({
    required this.prefs,
    required this.message,
    required this.failed,
    required this.onAddImage,
    required this.onClearLibrary,
    required this.onPickStageImage,
    required this.onClearStageImage,
    required this.onRemoveItem,
    required this.onAddPattern,
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

  /// 「来源 = 舞台那张」时的那两个按钮（换 / 清）。
  final VoidCallback? onPickStageImage;
  final VoidCallback? onClearStageImage;
  final ValueChanged<int>? onRemoveItem;
  final ValueChanged<int>? onAddPattern;
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
    // ── 当前项（DEC-6）──
    //
    // 来源是背景库时，「当前」= **运行时轮播索引**指向的那一项（外壳注入），
    // 不再是「永远第 0 项」；来源是舞台那张时 = 舞台那张图。
    // 判据只在外壳一处（AppShell._currentBackground），这里只消费。
    final BackgroundRuntimeScope? runtime = BackgroundRuntimeScope.maybeOf(
      context,
    );
    final BackgroundItem? current = BackgroundRuntimeScope.currentOf(
      context,
      prefs,
    );
    final bool hydrating = runtime?.hydrating ?? false;
    final bool librarySource =
        prefs.backgroundSource == DisplayPrefs.backgroundSourceLibrary;
    final String? currentIdentity = librarySource && current != null
        ? backgroundItemIdentity(current)
        : null;
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Text('背景', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          EmphasizedText(
            '背景铺在**聊天与侧栏背后**（舞台由渲染面自己画，不归这里）。'
            '下面每个滑杆都标了它管的是**哪一层**，并且把算出来的数字写出来。',
            style: theme.textTheme.labelSmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
          const SizedBox(height: Space.s2),
          SegmentedField<int>(
            label: '背景来源',
            icon: Icons.layers_outlined,
            value: prefs.backgroundSource,
            options: const <FieldOption<int>>[
              FieldOption<int>(
                value: DisplayPrefs.backgroundSourceLibrary,
                label: '背景库',
              ),
              FieldOption<int>(
                value: DisplayPrefs.backgroundSourceStageImage,
                label: '舞台那张',
              ),
            ],
            onChanged: (int v) =>
                onChanged(prefs.copyWith(backgroundSource: v)),
            description:
                prefs.backgroundSource == DisplayPrefs.backgroundSourceLibrary
                ? prefs.backgrounds.isEmpty
                      ? '正在画**背景库**，但库里还是空的（下面加一项，或挑一个内置图案）'
                      : '正在画**背景库**里的 ${prefs.backgrounds.length} 项之一'
                : prefs.stageImage == null
                ? '来源是**舞台背景图**，但还没选过图（背景库里的不参与渲染）'
                : '正在画**舞台背景图**那一张（背景库里的不参与渲染）',
          ),
          const SizedBox(height: Space.s2),
          if (prefs.backgroundSource ==
              DisplayPrefs.backgroundSourceLibrary) ...<Widget>[
            _LibraryManager(
              items: items,
              prefs: prefs,
              palette: palette,
              // 「当前」那一项的身份（DEC-6）：外壳按运行时轮播索引给，
              // 不是库里第 0 项。
              currentIdentity: currentIdentity,
              onReorder: onReorderItem ?? _noopReorder,
              onRemove: onRemoveItem,
              onRemoveMany: onRemoveMany,
              onPreview: onPreviewItem,
              // 逐图样式（§5.3 第 2 条）：只改这一项的覆盖，列表其余原样。
              onItemChanged: (int index, BackgroundImage next) {
                if (index < 0 || index >= items.length) return;
                final List<BackgroundItem> list = List<BackgroundItem>.of(items);
                list[index] = next;
                onChanged(prefs.copyWith(backgrounds: list));
              },
            ),
            const SizedBox(height: Space.s2),
            Wrap(
              spacing: Space.s1,
              runSpacing: Space.s1,
              children: <Widget>[
                for (final int id in BackgroundPatternId.values)
                  ActionChip(
                    label: Text(backgroundPatternLabel(id)),
                    onPressed: () => onAddPattern?.call(id),
                  ),
              ],
            ),
            const SizedBox(height: Space.s2),
          ],
          // 按钮**跟着来源走**：来源是背景库时，「添加图片」加进的就是
          // 正在被渲染的那个列表。不存在「点了没反应」的按钮。
          if (librarySource) ...<Widget>[
            // 水合中（交接项 9b）：读回还没落地，此刻的写操作会让用户以为
            // 「加进去了」而清单马上被读回覆盖 —— 显式禁用 + 说明，不做
            // 一个按了没反应的按钮（P4）。
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
          ] else
            Wrap(
              spacing: Space.s2,
              runSpacing: Space.s2,
              children: <Widget>[
                FilledButton.tonal(
                  onPressed: onPickStageImage,
                  child: Text(prefs.stageImage == null ? '选择舞台背景图' : '换一张'),
                ),
                if (prefs.stageImage != null)
                  TextButton(
                    onPressed: onClearStageImage,
                    child: const Text('清除舞台背景图'),
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
          // ── 渲染参数（两种同步模式下都生效）──
          const SizedBox(height: Space.s2),
          // 铺法与位置只对**图片**有意义（图案是 `CustomPainter` 画满整块，
          // 没有「原图尺寸」可裁可留边）。当前项是图案时把它们收起来，
          // 而不是留两个按了没区别的控件。
          //
          // 「当前项」= 外壳按**运行时轮播索引**给的那一项（DEC-6，不再是
          // 永远第 0 项）：轮播走到第 2 张，这里显示的就是第 2 张的属性。
          if (current is BackgroundImage)
            ..._fitControls(context, prefs, current, onChanged),
          _CommitSliderField(
            label: '不透明度（图）',
            icon: Icons.opacity,
            value: prefs.backgroundOpacity,
            min: DisplayPrefs.minBackgroundOpacity,
            max: DisplayPrefs.maxBackgroundOpacity,
            onCommit: (double v) =>
                onChanged(prefs.copyWith(backgroundOpacity: v)),
            description: '**图**本身有多实。0 = 完全不画背景（回到纯色面）；'
                '拉高时下面的遮罩会跟着加强，聊天文字仍然读得出来',
            minLabel: '看不见',
            maxLabel: '压满',
          ),
          _CommitSliderField(
            label: '透明程度（界面）',
            icon: Icons.layers_outlined,
            value: prefs.uiTransparency,
            min: DisplayPrefs.minUiTransparency,
            max: DisplayPrefs.maxUiTransparency,
            onCommit: (double v) =>
                onChanged(prefs.copyWith(uiTransparency: v)),
            // 面板不透明度**直接写出来**：这一项没有别的可见表现，
            // 数字是唯一能让用户确认「它真的动了」的东西。
            description: '**面板**有多透：设置面板、组卡片、聊天面板、顶栏'
                '一起变，当前不透明度 **${(AppColors.panelAlphaFor(prefs.uiTransparency) * 100).round()}%**。'
                '${prefs.hasBackground ? '背后有图，所以能看出差别' : '背后没图时聊天面板与顶栏保持不透明（底下是纯色底，半透只会让界面发灰）'}。'
                '界面越透，下面的遮罩越强，聊天文字仍然读得出来',
            minLabel: '不透明',
            maxLabel: '最透',
          ),
          SegmentedField<int>(
            label: '遮罩（图上方）',
            icon: Icons.contrast,
            value: prefs.backgroundScrim,
            options: const <FieldOption<int>>[
              FieldOption<int>(value: 0, label: '自动'),
              FieldOption<int>(value: 1, label: '无'),
              FieldOption<int>(value: 2, label: '轻'),
              FieldOption<int>(value: 3, label: '重'),
            ],
            onChanged: (int v) => onChanged(prefs.copyWith(backgroundScrim: v)),
            // **把算出来的强度写出来**：这一项的效果本来就是「更暗一点」，
            // 不给数字的话用户改完看不出区别，只能当成又一个无效控件。
            description:
                '压在背景上的一层主题色，让聊天文字在任何图上都读得出来。'
                '当前强度 **${(scrimAlphaFor(imageOpacity: prefs.backgroundOpacity, level: prefs.backgroundScrim, uiTransparency: prefs.uiTransparency) * 100).round()}%**'
                '（${prefs.hasBackground ? '强度随图的不透明度与界面透明度算出' : '没有背景时不画'}）',
          ),
          _MoreOptions(
            title: '更多外观',
            // 一句话说明它为什么被折起来：默认用不到，但它与「不透明度」
            // 是一对（一张太花的图通常要「更实」或「更糊」才压得住）。
            summary: '模糊等低频选项',
            children: <Widget>[
              _CommitSliderField(
                label: '模糊（图）',
                icon: Icons.blur_on,
                value: prefs.backgroundBlur,
                min: 0,
                max: DisplayPrefs.maxBackgroundBlur,
                percentage: false,
                suffix: ' px',
                onCommit: (double v) =>
                    onChanged(prefs.copyWith(backgroundBlur: v)),
                description: '只作用在壳的背景上，**模糊不到舞台**',
                minLabel: '清晰',
                maxLabel: '弥散',
              ),
            ],
          ),
          const Divider(),
          // ── 壳背景轮播（DEC-2 / DEC-7b）──
          //
          // **两个轮播块按来源互斥**：来源 = 背景库时只有这一块（壳自己换图）；
          // 来源 = 舞台那张时只有下面那张卡的「舞台单图轮播」（走渲染面
          // stage-bg 帧）。两套列表、两套预算、两条下发通道，同时摆出来
          // 用户只会以为它们是同一个开关（rc.5 §9.2 的原始问题）。
          //
          // 旧形态在来源 = 舞台那张时**照样显示**这三项，而 syncSlideshow 把库长
          // 强制成 0（定时器根本不起）——控件转、画面不转，文案还写「按间隔换
          // 下一张」。所以这里**隐藏 + 一句说明**（P4 禁止静默失效）。
          if (!librarySource)
            EmphasizedText(
              '「背景来源」当前是**舞台那张**：壳背景轮播转的是**背景库**里的图，'
              '而库里的图现在不参与渲染，所以这一块先不出现。'
              '要轮播舞台那张，用下面的「舞台单图轮播」。',
              style: theme.textTheme.labelSmall?.copyWith(
                color: colors.contentMuted,
              ),
            )
          else ...<Widget>[
            ToggleField(
              label: '壳背景轮播',
              icon: Icons.slideshow,
              value: prefs.slideInterval > 0,
              onChanged: (bool on) =>
                  onChanged(prefs.copyWith(slideInterval: on ? 30 : 0)),
              description: prefs.backgrounds.length < 2
                  ? '背景库至少要 2 项才会转'
                  : '按间隔换下一张',
            ),
            if (prefs.slideInterval > 0) ...<Widget>[
              _CommitSliderField(
                label: '轮播间隔',
                icon: Icons.timer_outlined,
                value: prefs.slideInterval.toDouble(),
                min: DisplayPrefs.minSlideIntervalSeconds.toDouble(),
                max: DisplayPrefs.maxSlideIntervalSeconds.toDouble(),
                percentage: false,
                suffix: ' 秒',
                onCommit: (double v) =>
                    onChanged(prefs.copyWith(slideInterval: v.round())),
                minLabel: '快',
                maxLabel: '慢',
              ),
              ToggleField(
                label: '随机顺序',
                icon: Icons.shuffle,
                value: prefs.slideRandom,
                onChanged: (bool v) =>
                    onChanged(prefs.copyWith(slideRandom: v)),
                description: '不会连着两次同一张',
              ),
            ],
          ],
        ],
      ),
    );
  }
}

/// 背景库**管理面板**（2026-09-27）。
///
/// # 它管的四件事
///
/// | 能力 | 为什么需要 |
/// | --- | --- |
/// **拖动排序** | 轮播按**顺序**走，顺序就是用户对「先放谁」的表达 |
/// **点一下预览** | 加完图先看一眼，而不是改完设置再退回主界面找 |
/// **多选 + 批量删** | 一次清掉试了几张的图，比一张张点 × 快得多 |
/// **占用显示** | 配额是**真的会满**的；不说清楚，用户只会看到「加不进去」 |
///
/// # 为什么不内置「重命名」
///
/// 一张背景图没有天然的标题（dataURL 里没有名字），起名要用户自己想，
/// 于是就变成一个「不知道该起什么」的空输入框。图案有固定名（渐变 / 光晕…），
/// 图没有——所以**不给这个能力**，而不是给一个必然被留空的输入框。
///
/// # 状态住在哪
///
/// **本地 state**（`_LibraryManager`）：选中集合、是否处于管理模式，
/// 都不是用户偏好，刷新后不该留。真正的数据（顺序、增减）在 `DisplayPrefs`。
/// 「更多」的折叠区。
///
/// 2026-09-27 引入：**少露一个控件**比**少一个能力**好——所以低频但有用的
/// 选项（模糊、将来的九宫格微调）折到这里，而不是被删掉。
///
/// 默认收起、标题上带一行摘要（所以用户知道里面还有东西）。
/// 铺法 / 平铺贴片 / 位置 这三件（§5.3 第 1 条 + DEC-7a）。
///
/// 拆成一个函数的理由：**显隐规则有三条**（图案时整组收起、贴片只在平铺档、
/// 位置只在完整档），散在 build 里下一个改的人一定会漏掉一条——而漏掉的表现
/// 正是「留一个按了没区别的控件」。
///
/// 显示的是**当前项生效**的值（逐图覆盖 ?? 全局）：轮播换一张，这里跟着换；
/// 当前项单独设过的那一项**禁用 + 说清去哪儿改**（改全局对它不生效，留着
/// 就是个假控件）。
List<Widget> _fitControls(
  BuildContext context,
  DisplayPrefs prefs,
  BackgroundImage current,
  ValueChanged<DisplayPrefs> onChanged,
) {
  final ThemeData theme = Theme.of(context);
  final AppColors colors = appColorsOf(context);
  final int effectiveFit = DisplayPrefs.effectiveImageFit(
    current,
    prefs.imageFit,
  );
  final bool fitOverridden = current.fit != null;
  final bool alignOverridden = current.align != null;
  final String fitName = DisplayPrefs.fitName(effectiveFit);
  return <Widget>[
    SegmentedField<int>(
      label: '铺法（图）',
      icon: Icons.photo_size_select_large_outlined,
      value: effectiveFit,
      enabled: !fitOverridden,
      options: const <FieldOption<int>>[
        FieldOption<int>(value: DisplayPrefs.fitCover, label: '铺满'),
        FieldOption<int>(value: DisplayPrefs.fitContain, label: '完整'),
        FieldOption<int>(value: DisplayPrefs.fitStretch, label: '拉伸'),
        FieldOption<int>(value: DisplayPrefs.fitTile, label: '平铺'),
      ],
      onChanged: (int v) => onChanged(prefs.copyWith(imageFit: v)),
      description: fitOverridden
          ? '当前这一项**单独设过**铺法（$fitName）——这里改的是全局默认值，'
                '对它不生效。要改它，请到它那一行点「样式」再点「改回跟随全局」。'
          : '铺满 = 裁掉边铺满；完整 = 整张都在（留边）；拉伸 = 变形铺满；'
                '平铺 = 按贴片重复。位置垫只在「完整」下出现——另外三档都铺满'
                '整块，对齐没有可见效果。',
    ),
    if (effectiveFit == DisplayPrefs.fitTile)
      _CommitSliderField(
        label: '平铺贴片',
        icon: Icons.grid_on,
        value: prefs.tileSize,
        min: DisplayPrefs.minTileSize,
        max: DisplayPrefs.maxTileSize,
        percentage: false,
        suffix: ' px',
        onCommit: (double v) => onChanged(prefs.copyWith(tileSize: v)),
        description: '一块贴片的边长；只对「平铺」这一档有效',
        minLabel: '细密',
        maxLabel: '粗大',
      ),
    // 位置**只在「完整」下出现**（DEC-7a）。
    //
    // 这两行的条件与注释曾经**正好相反**：注释写「位置只在完整下出现」，
    // 条件写 prefs.imageFit != 1（= 除了完整以外都显示）。裁决是**对齐注释**：
    // cover / stretch / tile 三档都把整块铺满，对齐真的没有可见效果，
    // 摆在那儿只会让人以为是自己按错了。
    if (effectiveFit == DisplayPrefs.fitContain) ...<Widget>[
      Padding(
        padding: const EdgeInsets.only(bottom: Space.s1),
        child: Text('位置', style: theme.textTheme.labelLarge),
      ),
      _AlignPad(
        key: kBackgroundAlignPadKey,
        value: DisplayPrefs.effectiveImageAlign(current, prefs.imageAlign),
        enabled: !alignOverridden,
        onChanged: (int v) => onChanged(prefs.copyWith(imageAlign: v)),
      ),
      if (alignOverridden)
        EmphasizedText(
          '当前这一项**单独设过**位置——这里改的是全局默认值，对它不生效。',
          style: theme.textTheme.labelSmall?.copyWith(
            color: colors.contentMuted,
          ),
        ),
    ],
  ];
}

class _MoreOptions extends StatefulWidget {
  const _MoreOptions({
    required this.title,
    required this.children,
    this.summary,
  });

  final String title;
  final String? summary;
  final List<Widget> children;

  @override
  State<_MoreOptions> createState() => _MoreOptionsState();
}

class _MoreOptionsState extends State<_MoreOptions> {
  bool _open = false;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        InkWell(
          onTap: () => setState(() => _open = !_open),
          borderRadius: BorderRadius.circular(AppRadius.sm),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: Space.s1,
              vertical: Space.s1,
            ),
            child: Row(
              children: <Widget>[
                Icon(
                  _open ? Icons.expand_less : Icons.expand_more,
                  size: 16,
                  color: colors.contentMuted,
                ),
                const SizedBox(width: Space.s1),
                Text(widget.title, style: theme.textTheme.labelLarge),
                const SizedBox(width: Space.s2),
                if (!_open && widget.summary != null)
                  Expanded(
                    child: Text(
                      widget.summary!,
                      style: theme.textTheme.labelSmall?.copyWith(
                        // 2026-09-28（F-0006-2 / task-16）：这是**文字**，不是装饰。
                        // `contentFaint` 的自注是「仅装饰/图标」，被当文字色用时
                        // 白主题只有 3.96–4.07（< AA 4.5）。承载文字的次要文本取
                        // `contentMuted`（四套主题合成后 ≥ 6.0）。
                        color: colors.contentMuted,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
        if (_open)
          Padding(
            padding: const EdgeInsets.only(top: Space.s2),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              mainAxisSize: MainAxisSize.min,
              children: widget.children,
            ),
          ),
      ],
    );
  }
}

/// 未接上排序回调时的占位（空实现，不抛）。
void _noopReorder(int oldIndex, int newIndex) {}

class _LibraryManager extends StatefulWidget {
  const _LibraryManager({
    required this.items,
    required this.prefs,
    required this.palette,
    this.currentIdentity,
    required this.onReorder,
    required this.onRemove,
    required this.onRemoveMany,
    required this.onPreview,
    this.onItemChanged,
  });

  final List<BackgroundItem> items;

  /// 全局值（逐图样式的「跟随全局」要显示它）。
  final DisplayPrefs prefs;

  final AppPalette palette;

  /// 「此刻画面上那一项」的身份（外壳按**运行时轮播索引**给，DEC-6）。
  final String? currentIdentity;

  final void Function(int oldIndex, int newIndex) onReorder;
  final ValueChanged<int>? onRemove;
  final ValueChanged<List<int>>? onRemoveMany;
  final ValueChanged<int>? onPreview;

  /// 某一项的逐图样式被改了（下标 + 新项）。
  final void Function(int index, BackgroundImage next)? onItemChanged;

  @override
  State<_LibraryManager> createState() => _LibraryManagerState();
}

class _LibraryManagerState extends State<_LibraryManager> {
  /// 选中的**项身份**（图片按 id、图案按图案 id 编码成一个串）——**不是下标**（F-0003-3）。
  ///
  /// 下标在重排之后会指向别的项：勾选第 0 项、再把原第 2 项拖到第 0 位，
  /// 点「删除所选」时旧实现删的是**现在第 0 位那一项**（用户没勾的那张）。
  /// 身份跟着项走，重排不改变「谁被选中」。
  final Set<String> _selected = <String>{};

  /// 展开了「样式」的那几项（身份）——与选中一样是本地 UI 状态，不落盘。
  final Set<String> _expanded = <String>{};

  /// 管理模式：**显式开关**（D1）。
  ///
  /// 旧判据是 selected.isNotEmpty 或 items.length >= 2 ——于是库里满 2 项之后
  /// 点缩略图只会勾选，「预览」（面板文档里四项能力之一）**永远不可达**。
  /// 现在预览是默认行为、管理是显式进的一个模式。
  bool _managing = false;

  int get _items => widget.items.length;

  @override
  void didUpdateWidget(_LibraryManager oldWidget) {
    super.didUpdateWidget(oldWidget);
    // 项被删掉 / 换掉 ⇒ 它的勾选与展开状态跟着走，否则「已选 N 项」会数幽灵。
    final Set<String> live = <String>{
      for (final BackgroundItem it in widget.items) backgroundItemIdentity(it),
    };
    _selected.removeWhere((String id) => !live.contains(id));
    _expanded.removeWhere((String id) => !live.contains(id));
  }

  /// 勾选 / 取消勾选某一项（按身份）。
  void _toggle(String identity) {
    setState(() {
      if (!_selected.remove(identity)) _selected.add(identity);
    });
  }

  /// 当前勾选项的**下标**（按现在的顺序算，升序）。
  ///
  /// 身份 → 下标的换算**只在交付给宿主那一刻做**，读的永远是「现在的顺序」，
  /// 所以重排之后它自动指对。
  List<int> get _selectedIndices {
    final List<int> out = <int>[
      for (int i = 0; i < widget.items.length; i++)
        if (_selected.contains(backgroundItemIdentity(widget.items[i]))) i,
    ];
    out.sort();
    return out;
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);

    if (widget.items.isEmpty) {
      return Text(
        '背景库是空的 —— 下面挑一个内置图案，或添加一张自己的图。',
        style: theme.textTheme.labelSmall?.copyWith(color: colors.contentMuted),
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        // 占用 + **管理模式开关**（D1）：预览是默认行为，管理要显式进。
        Row(
          children: <Widget>[
            Expanded(child: _UsageBar(items: widget.items)),
            TextButton(
              key: kBackgroundManageToggleKey,
              onPressed: () => setState(() {
                _managing = !_managing;
                if (!_managing) _selected.clear();
              }),
              child: Text(_managing ? '完成' : '管理'),
            ),
          ],
        ),
        if (_managing)
          Padding(
            padding: const EdgeInsets.only(bottom: Space.s1),
            child: Text(
              '管理模式：点一行勾选，可批量删除；点「样式」单独调这一张。'
              '点「完成」回到「点一下预览这一张」。',
              style: theme.textTheme.labelSmall?.copyWith(
                color: colors.contentMuted,
              ),
            ),
          ),
        const SizedBox(height: Space.s2),
        // ReorderableListView 而不是 Wrap：拖动排序是内建能力
        // （自带拖动手柄、键盘可达、onReorder 回调），自己用
        // LongPressDraggable 拼一个既不完整又没有无障碍。
        ReorderableListView.builder(
          shrinkWrap: true,
          primary: false,
          buildDefaultDragHandles: false,
          padding: EdgeInsets.zero,
          itemCount: widget.items.length,
          onReorderItem: (int oldIndex, int newIndex) =>
              widget.onReorder(oldIndex, newIndex),
          proxyDecorator: _dragProxy,
          itemBuilder: (BuildContext context, int index) {
            final BackgroundItem row = widget.items[index];
            final String identity = backgroundItemIdentity(row);
            return _LibraryRow(
              key: ValueKey<String>('lib-$identity-$index'),
              index: index,
              item: row,
              prefs: widget.prefs,
              palette: widget.palette,
              selected: _selected.contains(identity),
              managing: _managing,
              current: identity == widget.currentIdentity,
              styleExpanded: _expanded.contains(identity),
              onTap: () => _tap(index),
              onToggle: () => _toggle(identity),
              onToggleStyle: () => setState(() {
                if (!_expanded.remove(identity)) _expanded.add(identity);
              }),
              onStyleChanged: widget.onItemChanged == null
                  ? null
                  : (BackgroundImage next) =>
                        widget.onItemChanged!(index, next),
              onRemove: widget.onRemove == null
                  ? null
                  : () => widget.onRemove!(index),
              dragHandle: _items > 1
                  ? ReorderableDragStartListener(
                      index: index,
                      child: Icon(
                        Icons.drag_indicator,
                        size: 16,
                        color: colors.contentFaint,
                      ),
                    )
                  : null,
            );
          },
        ),
        if (_selected.isNotEmpty) ...<Widget>[
          const SizedBox(height: Space.s2),
          Row(
            children: <Widget>[
              Text(
                '已选 ${_selected.length} 项',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: colors.contentMuted,
                ),
              ),
              const Spacer(),
              TextButton(
                onPressed: () => setState(_selected.clear),
                child: const Text('取消选择'),
              ),
              TextButton(
                onPressed: () {
                  // 身份 → **现在的下标**（升序）：重排之后依然指对（F-0003-3）。
                  widget.onRemoveMany?.call(_selectedIndices);
                  setState(_selected.clear);
                },
                child: const Text('删除所选'),
              ),
            ],
          ),
        ],
      ],
    );
  }

  void _tap(int index) {
    final String identity = backgroundItemIdentity(widget.items[index]);
    if (_managing) {
      _toggle(identity);
      return;
    }
    // 非管理模式：点一下就是「先看这张」——**任意库大小下都可达**（D1）。
    widget.onPreview?.call(index);
  }

  /// 拖动时的浮起外观（不缩放：缩放会让旁边的行跟着抖）。
  static Widget _dragProxy(
    Widget child,
    int index,
    Animation<double> animation,
  ) => Material(
    elevation: 0,
    color: Colors.transparent,
    child: Opacity(opacity: 0.9, child: child),
  );
}

/// 逐图样式的一行（§5.3 第 2 条）。
///
/// 「跟随全局」是**显式状态**：跟随中显示全局值 + 一个「单独设置」按钮；设过
/// 之后显示控件 + 一个「改回跟随全局」按钮。不用 0 / -1 之类的 magic 值冒充
/// 「没设过」——那会让「跟随全局」与「显式设成 0」不可区分。
///
/// 按 field 生成的 key 是给测试用的（文案会改，键位不会）。
class _StyleOverrideRow extends StatelessWidget {
  const _StyleOverrideRow({
    required this.field,
    required this.label,
    required this.following,
    required this.globalText,
    required this.onSet,
    required this.onClear,
    required this.child,
  });

  final String field;
  final String label;
  final bool following;
  final String globalText;
  final VoidCallback onSet;
  final VoidCallback onClear;
  final Widget child;

  static Key statusKey(String field) =>
      ValueKey<String>('bg-style-status-$field');
  static Key setKey(String field) => ValueKey<String>('bg-style-set-$field');
  static Key clearKey(String field) => ValueKey<String>('bg-style-clear-$field');

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.only(top: Space.s1),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  following ? '$label：跟随全局（$globalText）' : '$label：单独设置',
                  key: statusKey(field),
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: following
                        ? colors.contentMuted
                        : theme.colorScheme.primary,
                  ),
                ),
              ),
              TextButton(
                key: following ? setKey(field) : clearKey(field),
                onPressed: following ? onSet : onClear,
                child: Text(following ? '单独设置' : '改回跟随全局'),
              ),
            ],
          ),
          if (!following) child,
        ],
      ),
    );
  }
}

/// 占用条：**明说**「占了多少」。
///
/// # 为什么它不再是一根进度条（2026-09-27）
///
/// 它原来对着一个 1.5 M 字符的「总预算」画进度——那个预算来自
/// localStorage 的 5 MB 配额。字节搬去 IndexedDB 之后**没有本地总预算了**
/// （配额由浏览器按磁盘剩余空间判），画一根进度条就得编一个分母，
/// 而那个分母是假的：明明只用了 2%，进度条也只到 2%，
/// 用户只会以为「我快用满了」而随手删图。
///
/// 所以现在只报**实际占用**（`N 项 · 12.4 MB`），
/// 真满了由 [BackgroundStore.put] 返回 `false`、界面如实说。
class _UsageBar extends StatelessWidget {
  const _UsageBar({required this.items});

  final List<BackgroundItem> items;

  static String _size(int bytes) {
    if (bytes < 1024 * 1024) return '${(bytes / 1024).round()} KB';
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final int bytes = DisplayPrefs.backgroundsBytes(items);
    final int images = items.whereType<BackgroundImage>().length;
    // 离 8 项上限只差一两项时提一句：那是**真的会拦住用户**的那一关。
    final bool nearCount = items.length >= kBackgroundMaxCount - 1;
    return Row(
      children: <Widget>[
        Text(
          images == 0
              ? '${items.length} 个内置图案（不占空间）'
              : '${items.length} 项 · 图片占用 ${_size(bytes)}',
          style: theme.textTheme.labelSmall?.copyWith(
            color: nearCount
                ? appPaletteOf(context).warning
                : colors.contentMuted,
          ),
        ),
        const Spacer(),
        Text(
          '上限 $kBackgroundMaxCount 项 / 单张 ${_size(kBackgroundImageMaxBytes)}',
          style: theme.textTheme.labelSmall?.copyWith(
            // 与上面那条摘要同理（task-16）：这是**文字**（量程说明），不是装饰。
            color: colors.contentMuted,
          ),
        ),
      ],
    );
  }
}

/// 库里的一行（缩略图 + 名字 + 勾选 + 拖动手柄 + 移除 + 逐图样式）。
///
/// 三个「不许静默」的细节：
/// - **当前**那一项标出来（外壳按运行时索引给，DEC-6）；
/// - 字节形态坏的项**明说画不出来**（DEC-5 的 UI 侧），不静默给个空块；
/// - 「样式」按钮只在图片上出现（图案由 CustomPainter 画，没有逐图样式）。
class _LibraryRow extends StatelessWidget {
  const _LibraryRow({
    required super.key,
    required this.index,
    required this.item,
    required this.prefs,
    required this.palette,
    required this.selected,
    required this.managing,
    required this.current,
    required this.styleExpanded,
    required this.onTap,
    required this.onToggle,
    required this.onToggleStyle,
    required this.onStyleChanged,
    required this.onRemove,
    required this.dragHandle,
  });

  final int index;
  final BackgroundItem item;
  final DisplayPrefs prefs;
  final AppPalette palette;
  final bool selected;
  final bool managing;

  /// 这一项就是**此刻画面上**的那一项。
  final bool current;

  /// 「样式」折叠区是否展开。
  final bool styleExpanded;

  final VoidCallback onTap;
  final VoidCallback onToggle;
  final VoidCallback onToggleStyle;

  /// 逐图样式变了（null = 宿主没接线，不显示编辑区）。
  final ValueChanged<BackgroundImage>? onStyleChanged;

  final VoidCallback? onRemove;
  final Widget? dragHandle;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final BackgroundItem row = item;
    final BackgroundImage? image = row is BackgroundImage ? row : null;
    final ValueChanged<BackgroundImage>? styleChanged = onStyleChanged;
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s1),
      child: Material(
        color: selected
            ? palette.accent.withValues(alpha: 0.12)
            : Colors.transparent,
        borderRadius: BorderRadius.circular(AppRadius.sm),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            InkWell(
              onTap: onTap,
              borderRadius: BorderRadius.circular(AppRadius.sm),
              child: Padding(
                padding: const EdgeInsets.all(Space.s1),
                child: Row(
                  children: <Widget>[
                    if (managing)
                      Checkbox(
                        value: selected,
                        onChanged: (_) => onToggle(),
                        visualDensity: VisualDensity.compact,
                      )
                    else
                      const SizedBox(width: Space.s2),
                    ClipRRect(
                      borderRadius: BorderRadius.circular(AppRadius.xs),
                      child: SizedBox(
                        width: Space.s6,
                        height: Space.s6,
                        child: switch (item) {
                          BackgroundPattern(:final int id) => PatternPreview(
                            id: id,
                            colors: patternColorsFor(id, palette),
                            size: Space.s6,
                          ),
                          // 字节还没读回来时画占位块，而不是给 Image.memory 传 null
                          // （那会走 errorBuilder，静悄悄的）。
                          BackgroundImage(:final String? dataUrl) =>
                            dataUrl == null
                                ? ColoredBox(color: palette.surfaceAlt)
                                : _ImageTile(
                                    dataUrl: dataUrl,
                                    palette: palette,
                                  ),
                        },
                      ),
                    ),
                    const SizedBox(width: Space.s2),
                    Expanded(
                      child: Text(
                        _label(item, index, current: current),
                        style: theme.textTheme.labelLarge,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    if (image != null && styleChanged != null)
                      TextButton(
                        onPressed: onToggleStyle,
                        child: Text(styleExpanded ? '收起样式' : '样式'),
                      ),
                    if (onRemove != null)
                      IconButton(
                        onPressed: onRemove,
                        tooltip: '移除这一项',
                        iconSize: 14,
                        visualDensity: VisualDensity.compact,
                        style: IconButton.styleFrom(
                          backgroundColor: palette.stage.withValues(
                            alpha: 0.55,
                          ),
                          foregroundColor: palette.ink,
                        ),
                        icon: const Icon(Icons.close),
                      ),
                    if (dragHandle != null) ...<Widget>[
                      const SizedBox(width: Space.s1),
                      dragHandle!,
                    ],
                  ],
                ),
              ),
            ),
            // 坏图 / 未读回的字节：**明说**（DEC-5 的 UI 侧）。
            if (image != null && image.isCorrupt)
              _RowNotice(
                '这一项的字节**不是图片 dataURL**——画不出来。移除后重新导入一张。',
                tone: palette.warning,
              ),
            if (image != null && image.bytesPending)
              _RowNotice(
                '这一项的字节还在读回（水合中）——读完才会出现在画面上。',
                tone: colors.contentMuted,
              ),
            if (image != null && styleExpanded && styleChanged != null)
              _ItemStyleEditor(
                item: image,
                prefs: prefs,
                onChanged: styleChanged,
              ),
          ],
        ),
      ),
    );
  }

  /// 一行的名字：图案用固定名，图片给「第 N 张 + 大小」，当前那张再标一下。
  ///
  /// 为什么不显示文件名：它在导入时就被 FileReader 丢掉了，
  /// 界面上编一个「图 3」不算撒谎，但也不会更有用。
  static String _label(
    BackgroundItem item,
    int index, {
    required bool current,
  }) {
    final String base = switch (item) {
      BackgroundPattern(:final int id) => backgroundPatternLabel(id),
      BackgroundImage(:final String? dataUrl) =>
        '图片 ${index + 1} · ${_size(dataUrl)}',
    };
    // 「当前」= 外壳按**运行时轮播索引**算出来的那一项（DEC-6）：
    // 轮播走到第 2 张，这里标的就是第 2 张。
    return current ? '$base · 当前' : base;
  }

  static String _size(String? dataUrl) {
    if (dataUrl == null) return '读取中';
    if (dataUrl.length < 1024 * 1024) {
      return '${(dataUrl.length / 1024).round()} KB';
    }
    return '${(dataUrl.length / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
}

/// 行下的一句实话（坏图 / 字节还在读回）。
class _RowNotice extends StatelessWidget {
  const _RowNotice(this.text, {required this.tone});

  final String text;
  final Color tone;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(
      left: Space.s1,
      right: Space.s1,
      bottom: Space.s1,
    ),
    child: EmphasizedText(
      text,
      style: Theme.of(context).textTheme.labelSmall?.copyWith(color: tone),
    ),
  );
}

/// 逐图样式编辑器（§5.3 第 2 条 / DEC-5 的 UI 侧）。
///
/// 三项都可单独覆盖、**缺省回落全局**；「跟随全局」是模型里的 null，
/// 不是任何 magic 数字。
class _ItemStyleEditor extends StatelessWidget {
  const _ItemStyleEditor({
    required this.item,
    required this.prefs,
    required this.onChanged,
  });

  final BackgroundImage item;
  final DisplayPrefs prefs;
  final ValueChanged<BackgroundImage> onChanged;

  @override
  Widget build(BuildContext context) {
    final int opacityPct = (prefs.backgroundOpacity * 100).round();
    final int alignCell = prefs.imageAlign + 1;
    return Padding(
      padding: const EdgeInsets.only(
        left: Space.s1,
        right: Space.s1,
        bottom: Space.s1,
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          EmphasizedText(
            '这三项**单独设过就覆盖全局**；没设过 = 跟随全局。'
            '「改回跟随全局」之后立刻回到上面那个全局值。',
            style: Theme.of(context).textTheme.labelSmall?.copyWith(
              color: appColorsOf(context).contentMuted,
            ),
          ),
          _StyleOverrideRow(
            field: 'opacity',
            label: '不透明度',
            following: item.opacity == null,
            globalText: '$opacityPct%',
            onSet: () => onChanged(
              styledBackgroundImage(item, opacity: prefs.backgroundOpacity),
            ),
            onClear: () =>
                onChanged(styledBackgroundImage(item, dropOpacity: true)),
            child: _CommitSliderField(
              label: '这一张的不透明度',
              icon: Icons.opacity,
              value: item.opacity ?? prefs.backgroundOpacity,
              min: DisplayPrefs.minBackgroundOpacity,
              max: DisplayPrefs.maxBackgroundOpacity,
              onCommit: (double v) =>
                  onChanged(styledBackgroundImage(item, opacity: v)),
              minLabel: '看不见',
              maxLabel: '压满',
            ),
          ),
          _StyleOverrideRow(
            field: 'fit',
            label: '铺法',
            following: item.fit == null,
            globalText: DisplayPrefs.fitName(prefs.imageFit),
            onSet: () =>
                onChanged(styledBackgroundImage(item, fit: prefs.imageFit)),
            onClear: () => onChanged(styledBackgroundImage(item, dropFit: true)),
            child: SegmentedField<int>(
              label: '这一张的铺法',
              icon: Icons.photo_size_select_large_outlined,
              value: item.fit ?? prefs.imageFit,
              options: const <FieldOption<int>>[
                FieldOption<int>(value: DisplayPrefs.fitCover, label: '铺满'),
                FieldOption<int>(value: DisplayPrefs.fitContain, label: '完整'),
                FieldOption<int>(value: DisplayPrefs.fitStretch, label: '拉伸'),
                FieldOption<int>(value: DisplayPrefs.fitTile, label: '平铺'),
              ],
              onChanged: (int v) =>
                  onChanged(styledBackgroundImage(item, fit: v)),
            ),
          ),
          _StyleOverrideRow(
            field: 'align',
            label: '位置',
            following: item.align == null,
            globalText: '九宫格第 $alignCell 格',
            onSet: () =>
                onChanged(styledBackgroundImage(item, align: prefs.imageAlign)),
            onClear: () =>
                onChanged(styledBackgroundImage(item, dropAlign: true)),
            child: _AlignPad(
              value: item.align ?? prefs.imageAlign,
              onChanged: (int v) =>
                  onChanged(styledBackgroundImage(item, align: v)),
            ),
          ),
        ],
      ),
    );
  }
}

/// 缩略图：走 [decodeDataUrlBytes]，坏图退回纯色面而不是红屏。
class _ImageTile extends StatelessWidget {
  const _ImageTile({required this.dataUrl, required this.palette});

  final String dataUrl;
  final AppPalette palette;

  @override
  Widget build(BuildContext context) {
    final Uint8List? bytes = decodeDataUrlBytes(dataUrl);
    if (bytes == null) return ColoredBox(color: palette.surfaceAlt);
    return Image.memory(
      bytes,
      fit: BoxFit.cover,
      // 缩略图只按 96 px 解码：不缩放的话一张 400 KB 的图会以原尺寸进缓存，
      // 8 张就是几十 MB。
      cacheWidth: kPatternPreviewSize.toInt(),
      gaplessPlayback: true,
      errorBuilder: (_, _, _) => ColoredBox(color: palette.surfaceAlt),
    );
  }
}

/// 3×3 位置选择垫：中心那格是当前值。
///
/// 为什么不是下拉：九个位置**看一眼就知道**，下拉要开一次再关一次。
/// enabled = false 时九格都点不动（当前项单独设过位置，改全局对它不生效）——
/// 禁用而不是「点了没反应」。
class _AlignPad extends StatelessWidget {
  const _AlignPad({
    required this.value,
    required this.onChanged,
    this.enabled = true,
    super.key,
  });

  final int value;
  final ValueChanged<int> onChanged;
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    Widget cell(int index) {
      final bool selected = index == value;
      return Semantics(
        button: true,
        selected: selected,
        label: '位置 ${index + 1}',
        excludeSemantics: true,
        child: InkWell(
          onTap: enabled ? () => onChanged(index) : null,
          child: Container(
            width: Space.s5,
            height: Space.s5,
            decoration: BoxDecoration(
              color: selected
                  ? palette.accent.withValues(alpha: 0.55)
                  : colors.hoverWash,
              borderRadius: BorderRadius.circular(AppRadius.xs),
              border: Border.all(
                color: selected ? palette.accent : colors.hairline,
                width: selected ? 2 : 1,
              ),
            ),
          ),
        ),
      );
    }

    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s2),
      child: Row(
        children: <Widget>[
          Icon(Icons.crop_free, size: 16, color: colors.contentMuted),
          const SizedBox(width: Space.s2),
          for (int row = 0; row < 3; row++) ...<Widget>[
            for (int col = 0; col < 3; col++) ...<Widget>[
              cell(row * 3 + col),
              if (col < 2) const SizedBox(width: Space.s1),
            ],
            if (row < 2) const SizedBox(height: Space.s1),
          ],
        ],
      ),
    );
  }
}
