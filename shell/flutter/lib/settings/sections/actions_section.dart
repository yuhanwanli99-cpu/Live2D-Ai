/// 「外观与互动」分区（规格 §4.1.5）。
///
/// 三组：**外观**（配色主题）、**A. 舞台与口型**、**C. 动作记录**——
/// 三者都是**纯本地 `DisplayPrefs` + 只读日志**，不需要草稿，也没有保存按钮。
///
/// # B 组「动作试演」为什么不在成品里（2026-09-11 用户裁决）
///
/// 手动触发按钮曾在 P5 接通过（`POST /api/v1/commands/{id}/invoke` +
/// 「正在演」忙碌态）。用户主动缩减本轮边界：**触发入口移出成品**，
/// 实现只保留在 git 历史与分支 `archive/action-trigger-p5`。
///
/// 留下的两个理由都写进了文档（`docs/design/web-action-trigger-archive.md`）：
/// 一是排版僵硬、观感不达标；二是**要先把动作本身做好**（动作质量决定
/// 触发入口值不值得存在），届时参考 Python 时代 `renderer/` 的做法重做。
///
/// 保留下来的「动作记录」是**只读可见性**，不是触发：它回答「刚才那下是
/// 谁动的」（AI / 规则 / 你），这条链路 core 一直在发（`action_state.source`）。
library;

import 'package:flutter/material.dart';

import '../../actions/action_log_controller.dart';
import '../../design/theme_id.dart';
import '../../design/tokens.dart';
import '../../settings/display_prefs.dart';
import '../../ui/action_history_view.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';
import '../../ui/emphasized_text.dart';
import '../../ui/theme.dart';
import '../../ui/theme_picker.dart';

class ActionsSection extends StatelessWidget {
  const ActionsSection({
    required this.prefs,
    required this.onPrefsChanged,
    required this.actionLog,
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

  /// 动作日志（P2 的成果）。
  final ActionLogController actionLog;

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
          min: 0.5,
          max: 2.0,
          onChanged: (double v) => onPrefsChanged(prefs.copyWith(scale: v)),
        ),
        SliderField(
          label: '口型灵敏度',
          icon: Icons.graphic_eq,
          value: prefs.mouthSensitivity,
          min: 0.2,
          max: 3.0,
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
        const Divider(),
        const SectionHeader(
          title: '动作记录',
          description: '谁触发的动作会记在这里——你点的、还是 AI 自己做的。',
        ),
        SizedBox(
          height: 360,
          child: ListenableBuilder(
            listenable: actionLog,
            builder: (BuildContext context, Widget? _) => ActionHistoryView(
              entries: actionLog.entries,
              current: actionLog.current,
              droppedCount: actionLog.droppedCount,
              onClear: actionLog.clear,
            ),
          ),
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
