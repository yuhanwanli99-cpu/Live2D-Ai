/// 「动作与互动」分区（规格 §4.1.5）。
///
/// 三组：**A. 舞台与口型**（纯本地 `DisplayPrefs`）、**B. 动作试演**
/// （走 core 仲裁）、**C. 动作记录**。
///
/// P4 只做 A 与 C 的整合（`DisplayPrefs` 是纯本地、不需要草稿/保存）；
/// **B 的动作触发按钮属 P5**——它需要 `POST /api/v1/commands/{id}/invoke`
/// 与「正在演」状态，那是一条独立的通道。这里**不占位**假装有，
/// 只如实说明它在下一阶段。
library;

import 'package:flutter/material.dart';

import '../../actions/action_dispatch.dart';
import '../../actions/action_log_controller.dart';
import '../../api/commands_api.dart';
import '../../design/tokens.dart';
import '../../settings/display_prefs.dart';
import '../../ui/action_grid.dart';
import '../../ui/action_history_view.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';

class ActionsSection extends StatelessWidget {
  const ActionsSection({
    required this.prefs,
    required this.onPrefsChanged,
    required this.actionLog,
    this.devMode = false,
    this.commands = const <CommandInfo>[],
    this.dispatch,
    this.onInvoke,
    this.strength = 2,
    this.onStrengthChanged,
    super.key,
  });

  /// 本地显示偏好（纯本地，`localStorage`，**不走设置草稿**）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;

  /// 动作日志（P2 的成果）。
  final ActionLogController actionLog;

  final bool devMode;

  // ── B 组：动作试演（P5 接上；走 core 仲裁，不绕过） ──

  /// 服务端给的可触发动作（**不硬编码 6 个**）。
  final List<CommandInfo> commands;

  /// 忙碌态与幂等提示。
  final ActionDispatch? dispatch;

  /// 触发（调用方负责走 `CommandsApi.invoke`）。
  final void Function(CommandInfo command)? onInvoke;

  /// 强度档位（1..3）。
  final int strength;
  final ValueChanged<int>? onStrengthChanged;

  @override
  Widget build(BuildContext context) {
    // 这些参数实时下发给渲染面：**没有「保存」按钮**。
    // 理由：拖滑杆时就想看到舞台反应，多一步保存会毁掉这个手感。
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
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
          title: '动作试演',
          description: '手动触发走 core 仲裁（不绕过）。'
              '「正在演」以服务端回报为准——重复点击不会重播。',
        ),
        if (dispatch != null && onInvoke != null)
          ActionGrid(
            commands: commands,
            strength: strength,
            onStrengthChanged: onStrengthChanged ?? (_) {},
            onInvoke: onInvoke!,
            dispatch: dispatch!,
          )
        else
          const Text('动作目录尚未加载。'),
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
