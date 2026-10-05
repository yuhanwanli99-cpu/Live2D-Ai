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

