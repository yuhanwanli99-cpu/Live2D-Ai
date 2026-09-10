/// 聊天面板（L3）：消息列表 + 音频条 + 输入区 + 设置入口。
///
/// 只依赖纯模型（`chat/chat_message.dart`、`state/ui_phase.dart`），
/// **不** import `package:web` 那条链——于是整个面板可 VM/widget 测试。
library;

import 'package:flutter/material.dart';

import '../chat/chat_message.dart';
import '../design/tokens.dart';
import '../state/ui_phase.dart';
import 'audio_bar.dart';
import 'error_banner.dart';
import 'message_bubble.dart';
import 'streaming_indicator.dart';
import 'theme.dart';

class ChatPanel extends StatelessWidget {
  const ChatPanel({
    required this.messages,
    required this.phase,
    required this.input,
    required this.onSend,
    required this.onStop,
    required this.onDismissError,
    required this.volume,
    required this.muted,
    required this.onVolumeChanged,
    required this.onMutedChanged,
    this.error,
    this.errorActions = const <ErrorAction>[],
    this.serverMuted = false,
    this.audioUnlocked = true,
    this.onEnableSound,
    this.onOpenSettings,
    this.onRetryLast,
    super.key,
  });

  final List<ChatMessage> messages;
  final UiPhase phase;

  /// 输入控制器（由宿主持有，面板不拥有它的生命周期）。
  final TextEditingController input;

  final VoidCallback onSend;
  final VoidCallback onStop;
  final VoidCallback onDismissError;

  final String? error;
  final List<ErrorAction> errorActions;

  final double volume;
  final bool muted;
  final ValueChanged<double> onVolumeChanged;
  final ValueChanged<bool> onMutedChanged;
  final bool serverMuted;
  final bool audioUnlocked;
  final VoidCallback? onEnableSound;

  /// 设置入口（compact 断点在面板头给一个齿轮按钮）。
  final VoidCallback? onOpenSettings;

  /// 最后一条失败消息的重试。
  final VoidCallback? onRetryLast;

  @override
  Widget build(BuildContext context) {
    // 聊天面用**独立底色**与舞台分隔：舞台是纯黑（`stageBackdrop`），
    // 聊天面板盖在它上面时必须自己撑出一个面，否则气泡看起来是浮在黑底上。
    return ColoredBox(
      color: AppPalette.chatPanelSurface,
      child: Column(
        children: <Widget>[
          // ── 头部：**只有**设置入口 ──
          //
          // 状态胶囊**不在这里**：它已经常驻在 AppBar 上（与连接徽标并列）。
          // 同一个相位在两处渲染就是「同一件事长出两套视觉」的微缩版——
          // 规格 §6.3 硬规则 1 明确禁止（同类项目的原病就是 6 个状态共用 1 种
          // 视觉，以及反过来一个状态散成多处）。测试里有一条断言
          // 「AppBar 只有 1 个状态胶囊」把这个重复钉死。
          if (onOpenSettings != null)
            Padding(
              padding: const EdgeInsets.fromLTRB(
                Space.s2,
                Space.s1,
                Space.s2,
                0,
              ),
              child: Row(
                children: <Widget>[
                  const Spacer(),
                  IconButton(
                    onPressed: onOpenSettings,
                    tooltip: '设置',
                    icon: const Icon(Icons.settings_outlined),
                  ),
                ],
              ),
            ),
          if (error != null)
            ErrorBanner(
              message: error!,
              onDismiss: onDismissError,
              actions: errorActions,
            ),
          // ── 消息列表 ──
          Expanded(
            child: messages.isEmpty
                ? const _ChatEmptyState()
                : ListView.builder(
                    // 从底部开始：IM 的常规。**不**给 itemExtent（气泡高度可变）。
                    reverse: true,
                    padding: const EdgeInsets.symmetric(vertical: Space.s2),
                    itemCount: messages.length,
                    itemBuilder: (BuildContext context, int index) {
                      final ChatMessage m =
                          messages[messages.length - 1 - index];
                      return MessageBubble(
                        message: m,
                        onRetry: m.isPlaceholder ? onRetryLast : null,
                      );
                    },
                  ),
          ),
          // 流式提示：贴在做消息列表下方（列表是 reverse 的，视觉上就是列表尾部）。
          // `thinking` 之外**彻底不占位**（不渲染空 SizedBox，避免留一段
          // 说不清来历的空白）。
          StreamingIndicator(phase: phase),
          // ── 音频条：常驻、在输入框**上方**（规格 §7.4） ──
          DecoratedBox(
            decoration: BoxDecoration(
              border: Border(
                top: BorderSide(color: appColorsOf(context).hairline),
              ),
            ),
            child: AudioBar(
              volume: volume,
              muted: muted,
              onVolumeChanged: onVolumeChanged,
              onMutedChanged: onMutedChanged,
              serverMuted: serverMuted,
              audioUnlocked: audioUnlocked,
              onEnableSound: onEnableSound,
            ),
          ),
          // ── 输入区 ──
          Padding(
            padding: const EdgeInsets.fromLTRB(
              Space.s3,
              Space.s1,
              Space.s3,
              Space.s3,
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.end,
              children: <Widget>[
                Expanded(
                  child: TextField(
                    controller: input,
                    minLines: 1,
                    maxLines: 4,
                    textInputAction: TextInputAction.send,
                    onSubmitted: (_) => onSend(),
                    decoration: const InputDecoration(
                      hintText: '说点什么…（Enter 发送）',
                      isDense: true,
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
                const SizedBox(width: Space.s2),
                // 一轮进行中时，发送键变「停止」——避免用户以为发了没反应。
                if (phase == UiPhase.thinking || phase == UiPhase.speaking)
                  IconButton.filledTonal(
                    onPressed: onStop,
                    tooltip: '停止',
                    icon: const Icon(Icons.stop),
                  )
                else
                  IconButton.filled(
                    onPressed: onSend,
                    tooltip: '发送',
                    icon: const Icon(Icons.send),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _ChatEmptyState extends StatelessWidget {
  const _ChatEmptyState();

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    // `Center` + `SingleChildScrollView`：有空间时居中，空间不够时**可滚动**。
    // 直接放 `Column` 会在窄屏（390 px + compact 的 3:2 分割）溢出——
    // 这是测试在 390 档抓出来的真实溢出（19 px），不是测试环境的问题。
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(Space.s4),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(
              Icons.chat_bubble_outline,
              size: 28,
              color: appColorsOf(context).contentFaint,
            ),
            const SizedBox(height: Space.s2),
            Text('说点什么吧', style: theme.textTheme.titleSmall),
            const SizedBox(height: Space.s1),
            Text(
              '回复会一边生成一边上屏，语音按句完整播放。',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodySmall?.copyWith(
                color: appColorsOf(context).contentMuted,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
