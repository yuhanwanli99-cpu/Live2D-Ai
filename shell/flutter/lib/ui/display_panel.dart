import 'package:flutter/material.dart';

import '../settings/display_prefs.dart';

/// 外观/口型控制面板。
///
/// 这些是**纯展示层**参数：不落服务端设置，直接经协议 v1 `sync` 下发给渲染面
/// （见 `AGENTS.md` §语音输出约定：前端只做展示与调度，不复制核心逻辑）。
/// 取值区间与持久化规则见 [DisplayPrefs]。
///
/// 设计取舍：滑杆**实时**下发（拖动过程中不断 `sync`）。渲染面每次 `sync` 只做
/// 几个字段的赋值与一次参数写入，代价可忽略；换来的是「所见即所得」的调参手感。
class DisplayPanel extends StatelessWidget {
  const DisplayPanel({
    super.key,
    required this.prefs,
    required this.onChanged,
    this.showHeader = true,
  });

  final DisplayPrefs prefs;
  final ValueChanged<DisplayPrefs> onChanged;

  /// 底部弹层里自带标题，侧栏/覆盖层里也显示（便于识别）。
  final bool showHeader;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 20),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (showHeader) ...<Widget>[
            Row(
              children: <Widget>[
                const Icon(Icons.tune, size: 18),
                const SizedBox(width: 8),
                Text('外观与口型', style: theme.textTheme.titleMedium),
              ],
            ),
            const SizedBox(height: 4),
            Text(
              '仅影响本机显示，保存在浏览器本地',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 12),
          ],
          _SliderRow(
            icon: Icons.zoom_out_map,
            label: '模型缩放',
            value: prefs.scale,
            min: DisplayPrefs.minScale,
            max: DisplayPrefs.maxScale,
            // 缩放只在 1.0 附近需要细调，显示两位小数。
            display: prefs.scale.toStringAsFixed(2),
            onChanged: (double v) =>
                onChanged(prefs.copyWith(scale: DisplayPrefs.clampScale(v))),
          ),
          _SliderRow(
            icon: Icons.record_voice_over,
            label: '口型灵敏度',
            value: prefs.mouthSensitivity,
            min: DisplayPrefs.minMouthSensitivity,
            max: DisplayPrefs.maxMouthSensitivity,
            display: '×${prefs.mouthSensitivity.toStringAsFixed(2)}',
            onChanged: (double v) => onChanged(
              prefs.copyWith(
                mouthSensitivity: DisplayPrefs.clampMouthSensitivity(v),
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.only(left: 32, bottom: 8),
            child: Text(
              '1.00 为标定值；觉得嘴动得太小就调大',
              style: theme.textTheme.labelSmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
          ),
          _SliderRow(
            icon: prefs.muted ? Icons.volume_off : Icons.volume_up,
            label: '主音量',
            value: prefs.volume,
            min: DisplayPrefs.minVolume,
            max: DisplayPrefs.maxVolume,
            display: '${(prefs.volume * 100).round()}%',
            onChanged: (double v) =>
                onChanged(prefs.copyWith(volume: DisplayPrefs.clampVolume(v))),
          ),
          if (prefs.muted)
            Padding(
              padding: const EdgeInsets.only(left: 32, bottom: 8),
              child: Text(
                '当前已静音——取消静音后按此音量播放',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ),
          SwitchListTile(
            dense: true,
            contentPadding: const EdgeInsets.only(left: 8),
            secondary: Icon(prefs.muted ? Icons.volume_off : Icons.volume_up),
            title: const Text('静音'),
            subtitle: const Text('只关声音、保留口型——需要安静时用它'),
            value: prefs.muted,
            onChanged: (bool v) => onChanged(prefs.copyWith(muted: v)),
          ),
          SwitchListTile(
            dense: true,
            contentPadding: const EdgeInsets.only(left: 8),
            secondary: const Icon(Icons.graphic_eq),
            title: const Text('口型同步'),
            subtitle: const Text('关闭后模型不再随语音开合嘴部'),
            value: prefs.lipSync,
            onChanged: (bool v) => onChanged(prefs.copyWith(lipSync: v)),
          ),
          SwitchListTile(
            dense: true,
            contentPadding: const EdgeInsets.only(left: 8),
            secondary: const Icon(Icons.air),
            title: const Text('待机小动作'),
            subtitle: const Text('呼吸、眨眼与微表情'),
            value: prefs.idleEnabled,
            onChanged: (bool v) =>
                onChanged(prefs.copyWith(idleEnabled: v)),
          ),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton.icon(
              onPressed: prefs == const DisplayPrefs()
                  ? null
                  : () => onChanged(const DisplayPrefs()),
              icon: const Icon(Icons.restart_alt, size: 18),
              label: const Text('恢复默认'),
            ),
          ),
        ],
      ),
    );
  }
}

/// 一行「图标 + 标签 + 滑杆 + 当前值」。
class _SliderRow extends StatelessWidget {
  const _SliderRow({
    required this.icon,
    required this.label,
    required this.value,
    required this.min,
    required this.max,
    required this.display,
    required this.onChanged,
  });

  final IconData icon;
  final String label;
  final double value;
  final double min;
  final double max;
  final String display;
  final ValueChanged<double> onChanged;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Row(
      children: <Widget>[
        Icon(icon, size: 18),
        const SizedBox(width: 8),
        SizedBox(
          width: 76,
          child: Text(label, style: theme.textTheme.bodyMedium),
        ),
        Expanded(
          child: Slider(
            value: value.clamp(min, max),
            min: min,
            max: max,
            onChanged: onChanged,
          ),
        ),
        SizedBox(
          width: 52,
          child: Text(
            display,
            textAlign: TextAlign.right,
            style: theme.textTheme.bodySmall?.copyWith(
              fontFeatures: const <FontFeature>[FontFeature.tabularFigures()],
            ),
          ),
        ),
      ],
    );
  }
}
