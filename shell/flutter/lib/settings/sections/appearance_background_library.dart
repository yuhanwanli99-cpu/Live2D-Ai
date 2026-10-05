part of 'appearance_section.dart';

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

