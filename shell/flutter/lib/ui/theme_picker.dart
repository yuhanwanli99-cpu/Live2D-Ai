/// 配色选择器：**每张卡用那套配色自己画自己**。
///
/// # 为什么不是四个小色块
///
/// 小色块只能露出「舞台底 + 一个汉字」——而决定一套配色观感的其实是**三张面
/// 与强调色**：面之间的亮度差决定层级够不够，色相偏移决定它像不像「一套设计过的
/// 东西」而不是「灰度 + 描边」。让用户点之前就看见这四样，比点下去再切主题
/// 反复比较要快得多（而且不用记住刚才那套长什么样）。
///
/// # 为什么不塞图标
///
/// 用户裁决「尽量少用图片，用文字做按钮」：卡片里再放一个小图标只会增加噪声，
/// 而且四个图标没有哪个能一眼说明「黑 / 白 / 蓝 / 灰」的差别。
library;

import 'package:flutter/material.dart';

import '../design/theme_id.dart';
import '../design/tokens.dart';
import 'soft_motion.dart';
import 'theme.dart';

/// 预览卡的尺寸（宽 × 高）。
///
/// 宽 132 而不是 96：三条面带要看得清，96 宽每条只剩 30 px。
const double kThemeCardWidth = 132;
const double kThemeCardHeight = 84;

/// 卡面上那三条面带（从下往上更亮）。
const List<String> kThemeCardBands = <String>[
  'surface',
  'surfaceAlt',
  'raised',
];

/// 第 [i] 条面带的颜色。
///
/// 为什么要**按名字取**而不是按位置：位次一改颜色就跟着错位，
/// 而「黑卡的第二档不是次级面」这种错肉眼看不出来。
/// 第 [i] 条面带的颜色（[kThemeCardBands] 的同一个下标）。
///
/// 为什么要**公开**：测试要逐档断言「这张卡真的画了它自己那套的三级面」，
/// 而按下标取色在位次被调整时会**静默错位**——那种错肉眼看不出来。
Color themeBandColor(AppPalette p, int i) => switch (i) {
  0 => p.surface,
  1 => p.surfaceAlt,
  _ => p.raised,
};

class ThemePickerField extends StatelessWidget {
  const ThemePickerField({
    required this.value,
    required this.onChanged,
    super.key,
  });

  /// 当前选中的主题。
  final AppThemeId value;
  final ValueChanged<AppThemeId> onChanged;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Text('配色', style: Theme.of(context).textTheme.labelLarge),
              const SizedBox(width: Space.s2),
              Expanded(
                child: Text(
                  // 说明放在标签右边而不是下面：这一行与四张卡是同一件事。
                  value.hint,
                  style: Theme.of(context).textTheme.labelSmall
                      ?.copyWith(color: colors.contentMuted),
                ),
              ),
            ],
          ),
          const SizedBox(height: Space.s2),
          // 用 `Wrap` 而不是 `Row`：窄屏（compact 的侧板只有 ~320 px）下
          // 四张 132 宽的卡放不下，换行比挤压或溢出好。
          Wrap(
            spacing: Space.s2,
            runSpacing: Space.s2,
            children: <Widget>[
              for (final AppThemeId id in AppThemeId.values)
                _ThemeCard(
                  id: id,
                  selected: id == value,
                  onTap: () => onChanged(id),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _ThemeCard extends StatelessWidget {
  const _ThemeCard({
    required this.id,
    required this.selected,
    required this.onTap,
  });

  final AppThemeId id;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    // **这一张卡用被展示的那套配色画自己**，不是当前主题的配色。
    final AppPalette shown = AppPalette.of(id);
    // 圆角走 `colors.radius(...)`：不这么做的话「圆角幅度」滑杆对
    // 这四张卡无效（它们是自建盒子，不经过 theme 的 r()）。
    final AppColors colors = appColorsOf(context);
    final AppColors current = colors;
    return Semantics(
      button: true,
      selected: selected,
      // 读屏念出来的是「黑，纯黑舞台，冷白强调，已选中」——
      // 「已选中」由 `selected: true` 提供，不必写进 label。
      label: '${id.label}，${id.hint}',
      excludeSemantics: true,
      child: Tooltip(
        message: id.hint,
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(colors.radius(AppRadius.md)),
          child: AnimatedContainer(
            key: ValueKey<String>('theme-card-${id.wire}'),
            // 走 `appMotion` 而不是直接给令牌：**令牌对、闸门漏**是这类改动
            // 最阴的失败方式——观感正常，只有开了「减少动画」的用户那里它照动。
            duration: appMotion(context, AppDurations.fast),
            curve: Motion.state,
            width: kThemeCardWidth,
            height: kThemeCardHeight,
            decoration: BoxDecoration(
              // 底 = 那套配色的**舞台底**：舞台本来就是界面里最大的一块，
              // 卡片先让人看见它。
              color: shown.stage,
              borderRadius: BorderRadius.circular(colors.radius(AppRadius.md)),
              border: Border.all(
                // 选中 = 更粗的、不透明的描边；未选中 = 当前主题的分隔线。
                // 颜色在这里已被承载语义，不能同时表示「选中」。
                color: selected
                    ? appPaletteOf(context).accent
                    : current.hairline,
                width: selected ? 2 : 1,
              ),
            ),
            child: Stack(
              children: <Widget>[
                // 三条面带：从上到下更亮（surface → surfaceAlt → raised）。
                // 这是「层级够不够」的那一半。
                Positioned(
                  left: 0,
                  right: 0,
                  bottom: Space.s3,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      for (int i = 0; i < kThemeCardBands.length; i++)
                        Container(
                          key: ValueKey<String>('theme-band-${id.wire}-$i'),
                          height: 3,
                          color: themeBandColor(shown, i),
                        ),
                    ],
                  ),
                ),
                // 强调色角标：另一半（「这套配色有没有主色」）。
                Positioned(
                  top: Space.s1,
                  right: Space.s1,
                  child: Container(
                    width: Space.s3,
                    height: Space.s3,
                    decoration: BoxDecoration(
                      color: shown.accent,
                      borderRadius: BorderRadius.circular(
                        colors.radius(AppRadius.xs),
                      ),
                    ),
                  ),
                ),
                // 名字：字色用**那套配色的墨色**，不是当前主题的。
                Positioned(
                  left: Space.s2,
                  bottom: Space.s4,
                  child: Text(
                    id.label,
                    style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      fontWeight: FontWeight.w600,
                      color: shown.ink,
                    ),
                  ),
                ),
                if (selected)
                  Positioned(
                    right: Space.s1,
                    bottom: Space.s1,
                    child: Icon(
                      Icons.circle,
                      size: 8,
                      // 选中点用**被展示主题的强调色**，保证在黑底上也看得见。
                      color: shown.accent,
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
