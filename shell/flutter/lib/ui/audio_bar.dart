/// 音频条（L3）：主音量滑杆 + 本机静音开关 + 服务端静音只读徽标 +
/// autoplay 解锁提示。规格 §7.4，**常驻**在聊天面板底部、输入框上方。
///
/// # 三个轴，三套词（§7.1 / §7.2 的命名契约，逐字照抄）
///
/// | 轴 | 谁控制 | UI |
/// |---|---|---|
/// | 主音量 `volume` | 用户 | 滑杆 |
/// | 本机静音 `muted` | 用户 | 图标开关（文案「本机静音」） |
/// | 服务端静音 | 用户**不可控** | 只读徽标（文案「服务端静音中」） |
///
/// **同一个词不能同时命名「我能听到」与「有没有声音被生产出来」。**
/// 这正是 OBS 把 `Monitor and Output` 改名 `Monitoring Enabled` 的教训
/// 【调研 control §6.3-3、§8-14】。
///
/// **本机静音时滑杆不置灰、不归零**——保留用户原值。静音是一个正交的布尔，
/// 不是「音量 0」；把它做成「音量 0」会让用户取消静音后发现音量被改掉了
/// （同类项目里就有用 `volume = 0.0000001` 做静音的 hack）。
library;

import 'package:flutter/material.dart';

import '../design/tokens.dart';
import 'theme.dart';

class AudioBar extends StatelessWidget {
  const AudioBar({
    required this.volume,
    required this.muted,
    required this.onVolumeChanged,
    required this.onMutedChanged,
    this.serverMuted = false,
    this.audioUnlocked = true,
    this.onEnableSound,
    super.key,
  });

  /// 0..1（`DisplayPrefs.volume`）。
  final double volume;

  /// 本机静音（`DisplayPrefs.muted`，出厂 `false`）。
  final bool muted;

  final ValueChanged<double> onVolumeChanged;
  final ValueChanged<bool> onMutedChanged;

  /// 服务端是否在静音输出（WS `audio.muted`，**只读观测值**）。
  final bool serverMuted;

  /// WebAudio 是否已解锁（浏览器 autoplay 限制）。
  final bool audioUnlocked;

  /// 解锁动作（用户点了「启用声音」）。
  final VoidCallback? onEnableSound;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final int percent = (volume * 100).round();

    // **不要**在外面套一层 `Semantics(container: true, label: '音频')`。
    // 那样会把整条音频条**合并成一个语义节点**：滑杆、静音开关、服务端徽标、
    // 「启用声音」按钮都变成读不到/点不到的碎片，读屏只会念一句
    // 「音频 80%」。这正是「静默失效」在无障碍上的形态——
    // 测试断言了「滑杆节点自身的标签是主音量、值是 80%」把这条钉死。
    // **固定 Tab 顺序**（规格 §9.2-1）：不写的话顺序跟随 Widget 树，
    // 重构时会静默变化——而「音量 → 静音」是用户的肌肉记忆顺序。
    return FocusTraversalGroup(
      policy: OrderedTraversalPolicy(),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(
          Space.s3,
          Space.s1,
          Space.s2,
          Space.s1,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              children: <Widget>[
                Icon(
                  muted ? Icons.volume_off : Icons.volume_up,
                  size: 18,
                  color: muted
                      ? colors.contentMuted
                      : theme.colorScheme.onSurface,
                ),
                Expanded(
                  // 1 = 主音量（Tab 第一站）。
                  // 读屏要能念出百分比。**不要**在外面套一层 `Semantics(label:)`：
                  // `Slider` 自身是语义叶子节点，外层 label 会变成一个独立节点
                  // 而不是它的标签（第一版就是这么写的，测试直接抓到
                  // 「找不到 '主音量 80%' 这个语义标签」）。
                  // 正确做法是用 `Slider` 自己的两个语义入口：
                  // `label`（节点标签）+ `semanticFormatterCallback`（节点值）。
                  child: FocusTraversalOrder(
                    order: const NumericFocusOrder(1),
                    child: Slider(
                      value: volume,
                      label: '主音量',
                      // **不在静音时禁用滑杆**：静音与音量正交。
                      onChanged: onVolumeChanged,
                      semanticFormatterCallback: (double v) =>
                          '${(v * 100).round()}%',
                    ),
                  ),
                ),
                SizedBox(
                  width: 44,
                  child: Text(
                    '$percent%',
                    textAlign: TextAlign.end,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: colors.contentMuted,
                    ),
                  ),
                ),
                const SizedBox(width: Space.s1),
                // 2 = 本机静音（Tab 第二站）。
                FocusTraversalOrder(
                  order: const NumericFocusOrder(2),
                  // **文字开关，不是喇叭图标**（用户裁决「尽量少用图片用文字
                  // 做按钮」）。这一处尤其值得换成文字：「静音」这两个字正好
                  // 承载了 §7.2 那条**必须区分两个轴**的约定——喇叭划一道杠
                  // 说不清是「你这台设备听不到」还是「服务端没出声」。
                  child: Tooltip(
                    message: '本机静音',
                    child: TextButton(
                      onPressed: () => onMutedChanged(!muted),
                      child: Text(muted ? '已静音' : '静音'),
                    ),
                  ),
                ),
              ],
            ),
            if (muted)
              _Hint(
                // 静音时**明确说明**：口型保留、音量原值还在。
                text: '本机静音中——你这台设备听不到（口型保留）；取消静音后按 $percent% 播放',
                tone: colors.contentMuted,
              ),
            if (serverMuted) _ServerMutedBadge() else const SizedBox.shrink(),
            if (!audioUnlocked) ...<Widget>[
              _Hint(text: '点击任意位置或按任意键以启用声音', tone: appPaletteOf(context).warning),
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: onEnableSound,
                  icon: const Icon(Icons.volume_up, size: 16),
                  label: const Text('启用声音'),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// 服务端静音徽标（**只读**，不是第二个开关）。
class _ServerMutedBadge extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final TextTheme text = Theme.of(context).textTheme;
    final AppColors colors = appColorsOf(context);

    return Padding(
      padding: const EdgeInsets.only(top: Space.s1),
      child: Semantics(
        label: '服务端静音中，服务端没有发出声音，任何客户端都听不到',
        excludeSemantics: true,
        child: DecoratedBox(
          decoration: BoxDecoration(
            color: colors.serverMutedBadgeSurface,
            borderRadius: BorderRadius.circular(AppRadius.xs),
            border: Border.all(color: colors.serverMutedBadgeBorder),
          ),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: Space.s2,
              vertical: 2,
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(
                  Icons.campaign_outlined,
                  size: 13,
                  color: appPaletteOf(context).warning,
                ),
                const SizedBox(width: Space.s1),
                Expanded(
                  child: Text(
                    // 逐字照抄 §7.2；把环境变量名写出来，用户才知道去哪关。
                    '服务端静音中（LIVE2D_AI_MUTE_AUDIO=1）——服务端没有发出声音'
                    '（任何客户端都听不到，需在启动参数里关闭）',
                    style: text.labelSmall?.copyWith(color: appPaletteOf(context).warning),
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

class _Hint extends StatelessWidget {
  const _Hint({required this.text, required this.tone});

  final String text;
  final Color tone;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(left: Space.s1, bottom: OpticalNudge.thin),
    child: Text(
      text,
      style: Theme.of(context).textTheme.labelSmall?.copyWith(color: tone),
    ),
  );
}
