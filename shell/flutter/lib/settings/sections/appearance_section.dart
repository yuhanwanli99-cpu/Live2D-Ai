/// 「外观与互动」分区（规格 §4.1.5）。
///
/// 三组：**外观**（配色主题 + 舞台背景图）、**舞台与口型**、**互动**——
/// 全是**纯本地 `DisplayPrefs`**，不需要草稿，也没有保存按钮。
///
/// # 文件名的来历（原名 `actions_section.dart`，2026-09-11 改名）
///
/// **这个文件从来就不是「动作」分区**——它的枚举项一直叫「外观与互动」，
/// 装的是配色主题、舞台背景图、模型缩放、口型灵敏度、互动开关。
/// 只因为当年它多带了一组**只读的「动作记录」**，文件名被叫成了 actions。
///
/// 用户裁定 LLM **无工具、只做对话**后，动作子系统（`live2d_perform_action`
/// 工具、`action_state` 帧、动作日志与手动触发入口）整个移出成品，
/// 那一组随之删除；文件名改为 `appearance_section.dart` 以名实相符。
/// 留这段是因为**下一个人很可能再按旧名误判一次**：`actions_section.dart`
/// 一出现就让人以为删掉它不影响外观设置，实际会整套删掉主题与口型。
library;

import 'package:flutter/material.dart';

import '../../design/theme_id.dart';
import '../../design/tokens.dart';
import '../../settings/display_prefs.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../../ui/emphasized_text.dart';
import '../../ui/theme.dart';
import '../../ui/theme_picker.dart';

class AppearanceSection extends StatelessWidget {
  const AppearanceSection({
    required this.prefs,
    required this.onPrefsChanged,
    this.devMode = false,
    this.onPickStageImage,
    this.onClearStageImage,
    this.stageImageMessage,
    this.stageImageFailed = false,
    super.key,
  });

  /// 本地显示偏好（纯本地，`localStorage`，**不走设置草稿**）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;

  final bool devMode;

  /// 选/清舞台背景图。为 `null` 时按钮禁用（**不是**点了没反应）。
  final VoidCallback? onPickStageImage;
  final VoidCallback? onClearStageImage;

  /// 上次选图的结果（超限时是**错误态**：图太大记不住）。
  final String? stageImageMessage;
  final bool stageImageFailed;

  @override
  Widget build(BuildContext context) {
    // 这些参数实时下发给渲染面：**没有「保存」按钮**。
    // 理由：拖滑杆时就想看到舞台反应，多一步保存会毁掉这个手感。
    // 主题同理——点一下立刻整套换掉，正是它该有的手感。
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        const SectionHeader(
          title: '外观',
          description: '四套配色任选。**只影响本机显示**，选完立刻生效，不需要保存。',
        ),
        ThemePickerField(
          value: prefs.theme,
          onChanged: (AppThemeId id) => onPrefsChanged(prefs.copyWith(theme: id)),
        ),
        _StageImageView(
          hasImage: prefs.stageImage != null,
          message: stageImageMessage,
          failed: stageImageFailed,
          onPick: onPickStageImage,
          onClear: onClearStageImage,
        ),
        const Divider(),
        const SectionHeader(
          title: '舞台与口型',
          description: '这些参数只影响**本机显示**，改动立刻下发到渲染面，不需要保存。',
        ),
        SliderField(
          label: '模型缩放',
          icon: Icons.zoom_out_map,
          value: prefs.scale,
          // **区间从这里读，不写字面量**（2026-09-11 修）：
          // 这两个数在 `DisplayPrefs` 里已经声明过一次（`clampScale` 也读它们），
          // 在 UI 里再写一遍就是第二个真源——改一处忘一处会让滑杆范围与
          // 实际 clamp 范围悄悄分叉。回归见 `test/display_prefs_test.dart`。
          min: DisplayPrefs.minScale,
          max: DisplayPrefs.maxScale,
          onChanged: (double v) => onPrefsChanged(prefs.copyWith(scale: v)),
        ),
        SliderField(
          label: '口型灵敏度',
          icon: Icons.graphic_eq,
          value: prefs.mouthSensitivity,
          min: DisplayPrefs.minMouthSensitivity,
          max: DisplayPrefs.maxMouthSensitivity,
          onChanged: (double v) =>
              onPrefsChanged(prefs.copyWith(mouthSensitivity: v)),
          description: '1.00 为标定值；觉得嘴动得太小就调大',
        ),
        ToggleField(
          label: '口型同步',
          icon: Icons.record_voice_over_outlined,
          value: prefs.lipSync,
          onChanged: (bool v) => onPrefsChanged(prefs.copyWith(lipSync: v)),
          description: '关掉后声音照放，但嘴不动',
        ),
        ToggleField(
          label: '待机小动作',
          icon: Icons.self_improvement,
          value: prefs.idleEnabled,
          onChanged: (bool v) => onPrefsChanged(prefs.copyWith(idleEnabled: v)),
          description: '呼吸 / 眨眼 / 微表情',
        ),
        if (devMode)
          SegmentedField<int>(
            label: '渲染档位',
            icon: Icons.speed,
            value: prefs.tier,
            options: const <FieldOption<int>>[
              FieldOption<int>(value: 4096, label: '4K'),
              FieldOption<int>(value: 8192, label: '8K'),
              FieldOption<int>(value: 16384, label: '16K'),
            ],
            onChanged: (int v) => onPrefsChanged(prefs.copyWith(tier: v)),
            description: '性能与画质的权衡；需要档位知识，所以进开发者层',
          ),
        const Divider(),
        const SectionHeader(
          title: '互动',
          description: '「允许拖动与缩放」管的是**拖拽 / 滚轮 / 双击复位**，'
              '与「点击角色有没有反应」无关。',
        ),
        ToggleField(
          label: '允许拖动与缩放',
          icon: Icons.pan_tool_outlined,
          value: prefs.allowDragZoom,
          onChanged: (bool v) =>
              onPrefsChanged(prefs.copyWith(allowDragZoom: v)),
          description: '关掉后模型不因拖动/滚轮移动；缩放仍可用舞台右下角的按钮',
        ),
        const SizedBox(height: Space.s3),
      ],
    );
  }
}

/// 舞台背景图那一行。
///
/// 三件事必须同时说清（否则用户会以为功能坏了）：
/// 1. **不是二选一**——背景图盖在纯色底上，清掉图底色就回来；
/// 2. 图太大时**本次有效但不记住**（`failed` 用警告色，不是静默成功）；
/// 3. 按钮是**文字**，不是图标（用户裁决「尽量少用图片用文字做按钮」）。
class _StageImageView extends StatelessWidget {
  const _StageImageView({
    required this.hasImage,
    required this.message,
    required this.failed,
    required this.onPick,
    required this.onClear,
  });

  final bool hasImage;
  final String? message;
  final bool failed;
  final VoidCallback? onPick;
  final VoidCallback? onClear;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final AppPalette palette = appPaletteOf(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s3),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text('舞台背景图', style: theme.textTheme.labelLarge),
          const SizedBox(height: Space.s1),
          EmphasizedText(
            '背景图**盖在纯色底上**：有图时看得到图，清掉后回到主题的纯色舞台。'
            '超过 ${(kStageImageMaxChars / 1024).round()} KB 的图只在本次会话有效。',
            style: theme.textTheme.labelSmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
          const SizedBox(height: Space.s2),
          Wrap(
            spacing: Space.s2,
            runSpacing: Space.s2,
            children: <Widget>[
              FilledButton.tonal(
                onPressed: onPick,
                child: Text(hasImage ? '换一张背景图' : '选择背景图'),
              ),
              if (hasImage)
                TextButton(onPressed: onClear, child: const Text('清除背景图')),
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
        ],
      ),
    );
  }
}
