/// 「主题」页（2026-10-09 从「外观与互动」拆出）。
///
/// 只有一组：**配色 + 背景**（纯本地 DisplayPrefs）。2026-10-10 起本机偏好
/// **先进草稿**：本页改的是草稿值，点「保存并重载」才落盘并刷新。
/// 舞台与口型、渲染档位、允许拖动与缩放搬到 motion_section.dart；
/// 动作幅度三条与本模型覆盖搬到 settings/mods/director_panel.dart。
///
/// # 文件名与类名（如实记录，避免下一个人以为拿错了）
///
/// 文件名仍是 appearance_section.dart：**背景域的三个 part 与 4 处源码扫描
/// 守卫（display_prefs_test / setting_wiring_test / background_copy_test /
/// appearance_background_style_test）都按这个路径读库**，改名要同时动它们，
/// 收益为零。类名从 AppearanceSection 改成 **ThemeSection**——分区已叫「主题」，
/// 类名必须说人话（调用点已全部跟改）。
///
/// 另：曾经住在这里的「拖动只动草稿、停手才提交」滑杆已搬到
/// ui/commit_slider_field.dart（动作页也要用它，而 Dart 私有是库级的）。
library;

import 'dart:typed_data';

import 'package:flutter/material.dart';

import '../../design/background_item.dart';
import '../../design/theme_id.dart';
import '../../design/tokens.dart';
import '../../settings/display_prefs.dart';
import '../../ui/background_logic.dart';
import '../../ui/background_patterns.dart';
import '../../ui/commit_slider_field.dart';
import '../../ui/emphasized_text.dart';
import '../../ui/field_row.dart';
import '../../ui/group_card.dart';
import '../../ui/shell_backdrop.dart';
import '../../ui/theme.dart';
import '../../ui/theme_picker.dart';

part 'appearance_background.dart';
part 'appearance_background_library.dart';
part 'appearance_background_style.dart';

/// 「主题」：配色 + 背景。
class ThemeSection extends StatelessWidget {
  const ThemeSection({
    required this.prefs,
    required this.onPrefsChanged,
    this.devMode = false,
    this.onPickShellImage,
    this.onClearShellImage,
    this.shellImageMessage,
    this.shellImageFailed = false,
    this.onRemoveBackground,
    this.onRemoveBackgrounds,
    this.onReorderBackground,
    this.onPreviewBackground,
    super.key,
  });

  /// 本机显示偏好**草稿**（本页只读草稿值、只改草稿；保存并重载才生效）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;

  /// 保留形参：调用点（外壳）一直传它；主题页自身没有 dev-only 控件。
  final bool devMode;

  /// 背景库的选 / 清图。
  final VoidCallback? onPickShellImage;
  final VoidCallback? onClearShellImage;
  final String? shellImageMessage;
  final bool shellImageFailed;

  /// 从背景库移除第 N 项。
  final ValueChanged<int>? onRemoveBackground;

  /// 批量移除（**升序**的下标集合）。
  final ValueChanged<List<int>>? onRemoveBackgrounds;

  /// 拖动排序（旧下标 → 新下标）。
  final void Function(int oldIndex, int newIndex)? onReorderBackground;

  /// 点缩略图 = 切到这一张（仅本次会话，不落盘）。
  final ValueChanged<int>? onPreviewBackground;

  @override
  Widget build(BuildContext context) {
    // 这些参数实时下发给渲染面：**没有「保存」按钮**。点一下立刻整套换掉，
    // 正是主题该有的手感。
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        GroupCard(
          title: '配色与背景',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              ThemePickerField(
                value: prefs.theme,
                onChanged: (AppThemeId id) =>
                    onPrefsChanged(prefs.copyWith(theme: id)),
              ),
              _BackgroundBlock(
                prefs: prefs,
                message: shellImageMessage,
                failed: shellImageFailed,
                onAddImage: onPickShellImage,
                onClearLibrary: onClearShellImage,
                onRemoveItem: onRemoveBackground,
                onChanged: onPrefsChanged,
                onReorderItem: onReorderBackground,
                onRemoveMany: onRemoveBackgrounds,
                onPreviewItem: onPreviewBackground,
              ),
            ],
          ),
        ),
      ],
    );
  }
}
