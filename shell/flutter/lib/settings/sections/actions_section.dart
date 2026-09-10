/// 「动作与互动」分区（规格 §4.1.5）。
///
/// 两组：**A. 舞台与口型**（纯本地 `DisplayPrefs`）、**C. 动作记录**。
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
import '../../design/tokens.dart';
import '../../settings/display_prefs.dart';
import '../../ui/action_history_view.dart';
import '../../ui/field_row.dart';
import '../../ui/section_header.dart';

class ActionsSection extends StatelessWidget {
  const ActionsSection({
    required this.prefs,
    required this.onPrefsChanged,
    required this.actionLog,
    this.devMode = false,
    super.key,
  });

  /// 本地显示偏好（纯本地，`localStorage`，**不走设置草稿**）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;

  /// 动作日志（P2 的成果）。
  final ActionLogController actionLog;

  final bool devMode;

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
