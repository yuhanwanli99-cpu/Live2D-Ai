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

/// 聊天列表保留的**最大条数**。
///
/// 取值理由：聊天历史**不落盘**（用户裁决），纯内存；500 条足以覆盖一次长会话，
/// 又不至于让 `ListView` 的语义树与读屏浏览退化。超出后**丢最旧的**——
/// 无限增长的长会话会让「往上翻」变成不可用操作。
///
/// 刻意在**视图层**裁剪而不是数据层：数据层裁剪会让「完整对话」无法导出排查。
const int kChatHistoryLimit = 500;

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
    this.onOpenSessions,
    this.onRetryLast,
    this.announcement,
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

  /// 会话列表入口（只有 compact 传；非 compact 的入口在 AppBar）。
  final VoidCallback? onOpenSessions;

  /// 最后一条失败消息的重试。
  final VoidCallback? onRetryLast;

  /// 节流后的播报文本（转给流式气泡的 `liveRegion`）。
  final String? announcement;

  @override
  Widget build(BuildContext context) {
    // 只渲染最后 [kChatHistoryLimit] 条（**不**在数据层裁剪：
    // 数据层裁剪会让「完整对话」无法导出/排查，而视图层裁剪只影响渲染）。
    final List<ChatMessage> visible = messages.length > kChatHistoryLimit
        ? messages.sublist(messages.length - kChatHistoryLimit)
        : messages;

    // 聊天面用**独立底色**与舞台分隔：舞台是纯黑（`stageBackdrop`），
    // 聊天面板盖在它上面时必须自己撑出一个面，否则气泡看起来是浮在黑底上。
    return ColoredBox(
      color: appPaletteOf(context).surface,
      // 聊天面板是一个面板 → 一个焦点组（规格 §9.2-1）。
      child: FocusTraversalGroup(
        policy: OrderedTraversalPolicy(),
        child: Column(
          children: <Widget>[
            // ── 头部：**只有**设置入口 ──
            //
            // 状态胶囊**不在这里**：它已经常驻在 AppBar 上（与连接徽标并列）。
            // 同一个相位在两处渲染就是「同一件事长出两套视觉」的微缩版——
            // 规格 §6.3 硬规则 1 明确禁止（同类项目的原病就是 6 个状态共用 1 种
            // 视觉，以及反过来一个状态散成多处）。测试里有一条断言
            // 「AppBar 只有 1 个状态胶囊」把这个重复钉死。
            if (onOpenSettings != null || onOpenSessions != null)
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
                    // 文字按钮（与 AppBar 里那两个同源同文案）——用户裁决
                    // 「尽量少用图片用文字做按钮」。
                    //
                    // compact 下 AppBar 不放这两个按钮（那一屏本来就窄，
                    // 再挤会跟状态胶囊、连接徽标打架），入口全部收在这里。
                    if (onOpenSessions != null)
                      TextButton(
                        onPressed: onOpenSessions,
                        child: const Text('会话'),
                      ),
                    if (onOpenSettings != null)
                      TextButton(
                        onPressed: onOpenSettings,
                        child: const Text('设置'),
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
              child: visible.isEmpty
                  ? const _ChatEmptyState()
                  : ListView.builder(
                      // 从底部开始：IM 的常规。**不**给 itemExtent（气泡高度可变）。
                      reverse: true,
                      padding: const EdgeInsets.symmetric(vertical: Space.s2),
                      itemCount: visible.length,
                      itemBuilder: (BuildContext context, int index) {
                        final ChatMessage m =
                            visible[visible.length - 1 - index];
                        return MessageBubble(
                          message: m,
                          onRetry: m.isPlaceholder ? onRetryLast : null,
                          // 只给**最后一条**（正在流式的那个）挂播报；
                          // 历史消息传 null，否则读屏会把整段历史重念一遍。
                          announcement:
                              index == 0 && m.streaming ? announcement : null,
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
            //
            // 发送键是一个**圆形里的上箭头**，不是写着「发送」的按钮
            //（2026-09-11 用户裁决：「尤其是发送，用上键加圆圈而不是发送」）。
            // 它出现在所有 IM 里，形状本身就表意，写字只多占一块地方。
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
                        hintText: '说点什么…',
                      ),
                    ),
                  ),
                  const SizedBox(width: Space.s2),
                  // 一轮进行中时，同一个位置变「停止」——避免用户以为发了没反应。
                  _RoundActionButton(
                    icon: (phase == UiPhase.thinking || phase == UiPhase.speaking)
                        ? Icons.stop_rounded
                        : Icons.arrow_upward_rounded,
                    tooltip: (phase == UiPhase.thinking || phase == UiPhase.speaking)
                        ? '停止本轮'
                        : '发送（Enter）',
                    onPressed:
                        (phase == UiPhase.thinking || phase == UiPhase.speaking)
                        ? onStop
                        : onSend,
                  ),
                ],
              ),
            ),
          ],
        ),
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

/// 输入区那个**圆形按钮**（发送 / 停止共用）。
///
/// 单独抽出来是因为它有两个必须一起成立的性质：
/// 1. **形状恒定为圆**（发送与停止切换时不能一个圆一个方，那会跳一下）；
/// 2. 尺寸固定 40——比 Material 的 `IconButton` 默认 48 小一档，
///    与输入框的实际高度对齐，不会把输入行撑高。
class _RoundActionButton extends StatelessWidget {
  const _RoundActionButton({
    required this.icon,
    required this.tooltip,
    required this.onPressed,
  });

  final IconData icon;
  final String tooltip;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final AppPalette palette = appPaletteOf(context);
    return Tooltip(
      message: tooltip,
      child: IconButton(
        onPressed: onPressed,
        // 读屏念的是动作（「发送」「停止本轮」），不是图标名。
        icon: Icon(icon),
        style: IconButton.styleFrom(
          backgroundColor: palette.accent,
          foregroundColor: palette.onAccent,
          shape: const CircleBorder(),
          minimumSize: const Size(40, 40),
          maximumSize: const Size(40, 40),
          padding: EdgeInsets.zero,
        ),
      ),
    );
  }
}
