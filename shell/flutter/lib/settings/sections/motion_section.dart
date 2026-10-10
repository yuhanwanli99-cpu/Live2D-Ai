/// 「Live2D 设置」页（2026-10-09 从「外观与互动」拆出；2026-10-10 改名）。
///
/// 只拿两组：**舞台与口型**（模型缩放、口型灵敏度、口型同步、待机小动作、
/// 渲染档位）与**互动**（允许拖动与缩放）。全是纯本地 DisplayPrefs。
///
/// 2026-10-10：本机偏好改动**先进草稿**（不再即时落盘 / 下发）——界面滑条显示
/// 新值，点「保存并重载」才写本机存储并刷新。渲染档位（4K/8K/16K）也**不再
/// 包在开发者模式里**。
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
    super.key,
  });

  /// 本机显示偏好**草稿**（本页只读草稿值、只改草稿；保存并重载才生效）。
  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onPrefsChanged;

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
              // 渲染档位 = 画布**最长边**（4096 / 8192 / 16384）。不再包在
              // 开发者模式里：它是用户可选的画质/性能权衡（2026-10-10）。
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
