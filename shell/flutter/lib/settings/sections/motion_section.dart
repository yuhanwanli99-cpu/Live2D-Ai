/// 「Live2D 动作」页（2026-10-09 从「外观与互动」拆出）。
///
/// 只拿两组：**舞台与口型**（模型缩放、口型灵敏度、口型同步、待机小动作、
/// 开发者档位）与**互动**（允许拖动与缩放）。全是纯本地 DisplayPrefs，
/// 不需要草稿，也没有保存按钮——拖滑杆时就想看到舞台反应。
///
/// **不在这里**：动作幅度三条与「本模型覆盖」（搬到导演卡片
/// settings/mods/director_panel.dart）；配色与背景（留在「主题」页）。
library;

import 'package:flutter/material.dart';

import '../../settings/display_prefs.dart';
import '../../ui/commit_slider_field.dart';
import '../../ui/field_row.dart';
import '../../ui/group_card.dart';

class MotionSection extends StatelessWidget {
  const MotionSection({
    required this.prefs,
    required this.onPrefsChanged,
    this.devMode = false,
    super.key,
  });

  /// 本地显示偏好（纯本地，localStorage，**不走设置草稿**）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;
  final bool devMode;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        GroupCard(
          title: '舞台与口型',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              CommitSliderField(
                label: '模型缩放',
                icon: Icons.zoom_out_map,
                value: prefs.scale,
                // **区间从这里读，不写字面量**：这两个数在 DisplayPrefs 里
                // 已经声明过一次（clampScale 也读它们）。
                min: DisplayPrefs.minScale,
                max: DisplayPrefs.maxScale,
                onCommit: (double v) => onPrefsChanged(prefs.copyWith(scale: v)),
                description: '0.5-2.0',
                minLabel: '更远',
                maxLabel: '更近',
              ),
              CommitSliderField(
                label: '口型灵敏度',
                icon: Icons.graphic_eq,
                value: prefs.mouthSensitivity,
                min: DisplayPrefs.minMouthSensitivity,
                max: DisplayPrefs.maxMouthSensitivity,
                onCommit: (double v) =>
                    onPrefsChanged(prefs.copyWith(mouthSensitivity: v)),
                description: '0.2-3.0',
                minLabel: '克制',
                maxLabel: '夸张',
              ),
              ToggleField(
                label: '口型同步',
                icon: Icons.record_voice_over_outlined,
                value: prefs.lipSync,
                onChanged: (bool v) => onPrefsChanged(prefs.copyWith(lipSync: v)),
              ),
              ToggleField(
                label: '待机小动作',
                icon: Icons.self_improvement,
                value: prefs.idleEnabled,
                onChanged: (bool v) =>
                    onPrefsChanged(prefs.copyWith(idleEnabled: v)),
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
                ),
            ],
          ),
        ),
        GroupCard(
          title: '互动',
          child: ToggleField(
            label: '允许拖动与缩放',
            icon: Icons.pan_tool_outlined,
            value: prefs.allowDragZoom,
            onChanged: (bool v) =>
                onPrefsChanged(prefs.copyWith(allowDragZoom: v)),
          ),
        ),
      ],
    );
  }
}
