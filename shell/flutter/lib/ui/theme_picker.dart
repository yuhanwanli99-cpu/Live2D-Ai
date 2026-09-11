/// 四选一配色选择器（**颜色本身就是按钮**）。
///
/// # 为什么不用 `SegmentedButton`
///
/// 分段控件只用**当前**主题的配色来画自己——四个选项看起来一模一样，
/// 只能靠「黑/白/蓝/灰」四个字去想象点下去会变成什么。而这个选择器的全部
/// 意义就是「我想先看看哪个舒服」，所以每个选项**用那套主题自己的配色画自己**：
/// 底色是它的舞台底、字是它的墨色、描边是它的线色。点之前就能看见结果。
///
/// 选中态用**加粗描边 + 一个实心圆点**表达，不靠颜色（颜色在这里已经
/// 被承载语义了，不能同时用来表示「选中」）。
///
/// 全程**没有图片**：四个圆角矩形 + 四个汉字（用户裁决「尽量少用图片用文字做按钮」）。
library;

import 'package:flutter/material.dart';

import '../design/theme_id.dart';
import '../design/tokens.dart';
import 'soft_motion.dart';
import 'theme.dart';

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
                  // 说明放在标签右边而不是下面：这一行与四个色块是同一件事。
                  value.hint,
                  style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: colors.contentMuted,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: Space.s2),
          // 用 `Wrap` 而不是 `Row`：窄屏（compact 的侧板只有 ~320 px）下
          // 四个选项放不下，换行比挤压或溢出好。
          Wrap(
            spacing: Space.s2,
            runSpacing: Space.s2,
            children: <Widget>[
              for (final AppThemeId id in AppThemeId.values)
                _ThemeSwatch(
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

class _ThemeSwatch extends StatelessWidget {
  const _ThemeSwatch({
    required this.id,
    required this.selected,
    required this.onTap,
  });

  final AppThemeId id;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    // **这一块用被展示的那套配色画自己**，不是当前主题的配色。
    final AppPalette shown = AppPalette.of(id);
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
          borderRadius: BorderRadius.circular(AppRadius.md),
          child: AnimatedContainer(
            // 走 `appMotion` 而不是直接给令牌：**令牌对、闸门漏**是这类改动
            // 最阴的失败方式——观感正常，只有开了「减少动画」的用户那里它照动，
            // 而且不报错（2026-09-11，P1 的 motion_wiring 扫描抓到的就是它）。
            duration: appMotion(context, AppDurations.fast),
            curve: Motion.state,
            width: 64,
            height: 44,
            decoration: BoxDecoration(
              color: shown.stage,
              borderRadius: BorderRadius.circular(AppRadius.md),
              border: Border.all(
                // 选中 = 更粗的、不透明的描边；未选中 = 当前主题的分隔线。
                color: selected
                    ? appPaletteOf(context).accent
                    : appColorsOf(context).hairline,
                width: selected ? 2 : 1,
              ),
            ),
            child: Stack(
              children: <Widget>[
                Center(
                  child: Text(
                    id.label,
                    style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      fontWeight: FontWeight.w600,
                      // **不是当前主题的墨色**：这一块展示的是 `shown` 那套。
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
                      size: 6,
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
