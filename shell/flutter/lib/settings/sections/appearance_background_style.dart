part of 'appearance_section.dart';

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
