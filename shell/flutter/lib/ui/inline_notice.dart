/// 行内提示（notice）：**全仓库唯一一处**「一句话 + 可选动作」的样式。
///
/// 2026-09-11（前端加强计划 P1-5）新增。之前同一件事有三套写法，且都不算错，
/// 但放在一起就散：
///
/// | 位置 | 当时的样子 |
/// | --- | --- |
/// | `ui/error_banner.dart` | `dangerSurface` 底 + `dangerBorder` 边 + 图标 + 动作（**标准**） |
/// | `ui/field_row.dart:95` | 字面文本 `'⚠ $error'`（一个 Unicode 字符冒充图标） |
/// | `settings/sections/dev_tools_section.dart:165` | 同上 |
///
/// 另外「重试」按钮的强调档也混用了三种：`TextButton`（`error_banner`、
/// `message_bubble`）、`FilledButton.tonal`（`main.dart`、`stage_host`）、
/// `FilledButton.tonalIcon`（当时的 `live2d_stage`）。
///
/// 本组件的规矩：
///
/// 1. **图标只用 Material 图标**，不用 `⚠` 这种字符——字符在不同平台的字形
///    与基线都不一样，而且它躲过了「图标要能对齐」这件事。
/// 2. **一个提示里最多一个「主」动作**（`FilledButton.tonal`），其余是
///    `TextButton`。一排实心按钮会让用户不知道该按哪个。
/// 3. **语义上是一个 live region**：提示出现时读屏要念出来（错误尤其如此）。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'theme.dart';

/// 提示的严重度。决定底色/描边/图标三件事，**一处定义**。
enum NoticeSeverity {
  /// 失败（默认）。
  danger(Icons.error_outline),

  /// 需要注意但没坏。
  warning(Icons.warning_amber_rounded),

  /// 中性信息（例如「已保存」）。
  info(Icons.info_outline);

  const NoticeSeverity(this.icon);

  final IconData icon;
}

/// 提示上的一个动作。
class NoticeAction {
  const NoticeAction({
    required this.label,
    required this.onPressed,
    this.primary = false,
  });

  final String label;
  final VoidCallback onPressed;

  /// 主动作（实心 tonal 按钮）。**一条提示里最多一个**。
  final bool primary;
}

/// 一句话 + 可选动作的行内提示。
class InlineNotice extends StatelessWidget {
  const InlineNotice({
    required this.message,
    this.severity = NoticeSeverity.danger,
    this.actions = const <NoticeAction>[],
    this.onDismiss,
    this.dense = false,
    super.key,
  });

  final String message;
  final NoticeSeverity severity;
  final List<NoticeAction> actions;
  final VoidCallback? onDismiss;

  /// 紧凑形态：用在字段行下面（那里空间小，且下面通常紧跟另一个字段）。
  final bool dense;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    final (Color surface, Color border, Color ink) = switch (severity) {
      NoticeSeverity.danger => (
        palette.dangerSurface,
        palette.dangerBorder,
        palette.danger,
      ),
      NoticeSeverity.warning => (
        palette.warning.withValues(alpha: 0.14),
        palette.warning.withValues(alpha: 0.45),
        palette.warning,
      ),
      NoticeSeverity.info => (colors.hoverWash, colors.hairline, colors.contentMuted),
    };

    return Semantics(
      liveRegion: true,
      container: true,
      child: Container(
        margin: dense ? EdgeInsets.zero : const EdgeInsets.all(Space.s2),
        decoration: BoxDecoration(
          color: surface,
          borderRadius: BorderRadius.circular(AppRadius.sm),
          border: Border.all(color: border),
        ),
        padding: EdgeInsets.fromLTRB(
          Space.s3,
          dense ? Space.s1 : Space.s2,
          onDismiss == null ? Space.s3 : Space.s1,
          dense ? Space.s1 : Space.s2,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Padding(
                  padding: const EdgeInsets.only(top: OpticalNudge.hair),
                  child: Icon(severity.icon, size: 16, color: ink),
                ),
                const SizedBox(width: Space.s2),
                Expanded(
                  child: Text(
                    message,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.contentMuted,
                    ),
                  ),
                ),
                if (onDismiss != null)
                  IconButton(
                    onPressed: onDismiss,
                    icon: const Icon(Icons.close, size: 16),
                    tooltip: '关闭提示',
                    visualDensity: VisualDensity.compact,
                    color: colors.contentMuted,
                  ),
              ],
            ),
            if (actions.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(left: Space.s6, top: Space.s1),
                child: Wrap(
                  spacing: Space.s2,
                  children: <Widget>[
                    for (final NoticeAction action in actions)
                      action.primary
                          ? FilledButton.tonal(
                              onPressed: action.onPressed,
                              child: Text(action.label),
                            )
                          : TextButton(
                              onPressed: action.onPressed,
                              child: Text(action.label),
                            ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}
