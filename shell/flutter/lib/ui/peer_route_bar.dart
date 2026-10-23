/// 「这条发给」路由控件 + Pi 侧列表气泡（0.4.5 rc.2）。
///
/// 只做**呈现**：路由、忙闲与条目都在 `chat/peer_route.dart` 的
/// [PeerChatController] 里（纯 VM 可测）。这里不自己判忙、不自己发请求。
///
/// 视觉与聊天气泡同源：用户条目靠右用 `bubbleUser`，Pi 条目靠左用
/// `bubbleAssistant` 并带一个可见的「Pi」标签（这是执行单要求的身份标识）；
/// 错误条目走**系统行**形态（危险色 + 警告图标），**不**画成 Pi 的气泡——
/// 失败不能被看成「Pi 回了一句」。
library;

import 'package:flutter/material.dart';

import '../chat/peer_route.dart';
import '../design/tokens.dart';
import 'theme.dart';

/// 输入框上方的路由条：标题 + 三个选项 + 一句逐字说明。
class PeerRouteBar extends StatelessWidget {
  const PeerRouteBar({required this.peer, super.key});

  final PeerChatController peer;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: Space.s2),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Text(
                kPeerRouteTitle,
                style: theme.textTheme.labelLarge?.copyWith(
                  color: colors.contentMuted,
                ),
              ),
              const SizedBox(width: Space.s2),
              Expanded(
                child: Wrap(
                  spacing: Space.s1,
                  runSpacing: Space.s1,
                  children: <Widget>[
                    for (final PeerRoute route in PeerRoute.values)
                      ChoiceChip(
                        label: Text(route.label),
                        selected: peer.route == route,
                        // claude: onSelected 为 null ⇒ Flutter 的禁用态，
                        // 点不动也选不上（不是「点了没反应」的自绘假禁用）。
                        onSelected: route.selectable
                            ? (bool selected) {
                                if (selected) peer.selectRoute(route);
                              }
                            : null,
                      ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: Space.s1),
          Text(
            kPeerRouteNote,
            style: theme.textTheme.bodySmall?.copyWith(
              color: colors.contentMuted,
            ),
          ),
        ],
      ),
    );
  }
}

/// 侧列表的一条：用户原文 / Pi 回复 / 错误。
class PeerEntryBubble extends StatelessWidget {
  const PeerEntryBubble({required this.entry, super.key});

  final PeerEntry entry;

  @override
  Widget build(BuildContext context) {
    if (entry.kind == PeerEntryKind.error) {
      return _PeerErrorRow(text: entry.text);
    }
    final ThemeData theme = Theme.of(context);
    final AppPalette palette = appPaletteOf(context);
    final AppColors colors = appColorsOf(context);
    final bool isUser = entry.kind == PeerEntryKind.user;
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: Space.s3,
        vertical: Space.s1,
      ),
      child: Column(
        crossAxisAlignment: isUser
            ? CrossAxisAlignment.end
            : CrossAxisAlignment.start,
        children: <Widget>[
          if (!isUser)
            Padding(
              padding: const EdgeInsets.only(left: Space.s1, bottom: Space.s1),
              child: Text(
                'Pi',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: palette.accent,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          DecoratedBox(
            decoration: BoxDecoration(
              color: isUser ? palette.bubbleUser : palette.bubbleAssistant,
              borderRadius: BorderRadius.circular(colors.radius(AppRadius.lg)),
            ),
            child: Padding(
              padding: const EdgeInsets.symmetric(
                horizontal: Space.s3,
                vertical: Space.s2,
              ),
              child: SelectableText(
                entry.text,
                style: theme.textTheme.bodyMedium?.copyWith(color: palette.ink),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// 失败条目：危险面 + 一行可读原因。**不是** Pi 的气泡。
class _PeerErrorRow extends StatelessWidget {
  const _PeerErrorRow({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppPalette palette = appPaletteOf(context);
    final AppColors colors = appColorsOf(context);
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: Space.s3,
        vertical: Space.s1,
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: palette.dangerSurface,
          borderRadius: BorderRadius.circular(colors.radius(AppRadius.sm)),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: Space.s2,
            vertical: Space.s1,
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Icon(Icons.error_outline, color: palette.danger),
              const SizedBox(width: Space.s1),
              Expanded(
                child: Text(
                  text,
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: palette.danger,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
