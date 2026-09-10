/// 单条消息气泡（L3）：角色标签、流式光标、来源徽标、失败态。
///
/// 只依赖纯模型 [ChatMessage]（`chat/chat_message.dart`）——**不** import
/// `chat_controller.dart`，那样会把 `package:web` 拖进来，让整个气泡不可测。
library;

import 'package:flutter/material.dart';

import '../chat/chat_message.dart';
import '../design/tokens.dart';
import 'theme.dart';

class MessageBubble extends StatelessWidget {
  const MessageBubble({
    required this.message,
    this.onRetry,
    this.announcement,
    super.key,
  });

  final ChatMessage message;

  /// 失败态下的「重试」；`null` 时不显示。
  final VoidCallback? onRetry;

  /// **节流后**的播报文本（`LiveRegionThrottle.announcement`）。
  ///
  /// 传 `null` = 这条气泡不参与 live region（历史消息不该被反复播报）。
  /// 之所以不让气泡自己看 `message.text`：`text_delta` 是毫秒级的，
  /// 直接挂 `liveRegion` 会把读屏淹掉——节流必须在**外面**做完再喂进来。
  final String? announcement;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AppColors colors = appColorsOf(context);
    final bool isUser = message.role == ChatRole.user;
    final bool failed = message.isPlaceholder;

    final Color background = failed
        ? appPaletteOf(context).dangerSurface
        : (isUser ? theme.colorScheme.primaryContainer : appPaletteOf(context).surfaceAlt);
    final Color foreground = failed
        ? colors.contentMuted
        : (isUser
              ? theme.colorScheme.onPrimaryContainer
              : theme.colorScheme.onSurface);

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
          // 角色标签：**文字**，不靠左右对齐单独表意（灰度/读屏都要能分辨）。
          Padding(
            padding: const EdgeInsets.only(bottom: 2, left: 2, right: 2),
            child: Text(
              message.role.label,
              style: theme.textTheme.labelSmall?.copyWith(
                color: colors.contentMuted,
              ),
            ),
          ),
          Semantics(
            // `liveRegion`：**只在流式中的助手气泡**上开。读屏会优先播报
            // 这里的 `label`（已节流），历史消息保持普通节点。
            liveRegion: announcement != null && message.streaming,
            label: announcement == null
                ? '${message.role.label}说：${message.text}'
                : '${message.role.label}说：$announcement',
            excludeSemantics: true,
            child: Container(
              constraints: const BoxConstraints(maxWidth: 460),
              decoration: BoxDecoration(
                color: background,
                borderRadius: BorderRadius.circular(AppRadius.md),
                border: failed
                    ? Border.all(color: appPaletteOf(context).dangerBorder)
                    : null,
              ),
              padding: const EdgeInsets.symmetric(
                horizontal: Space.s3,
                vertical: Space.s2,
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.end,
                    children: <Widget>[
                      Flexible(
                        child: SelectableText(
                          message.text.isEmpty && message.streaming
                              ? '…'
                              : message.text,
                          style: theme.textTheme.bodyMedium?.copyWith(
                            color: foreground,
                          ),
                        ),
                      ),
                      // 流式光标：只在**正文已经来了**的时候显示，
                      // 否则会与上面的「…」重复。
                      if (message.streaming && message.text.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.only(left: 2),
                          child: Text(
                            '▍',
                            style: theme.textTheme.bodyMedium?.copyWith(
                              color: foreground,
                            ),
                          ),
                        ),
                    ],
                  ),
                  if (failed && onRetry != null)
                    Align(
                      alignment: Alignment.centerLeft,
                      child: TextButton(
                        onPressed: onRetry,
                        child: const Text('重试'),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}
