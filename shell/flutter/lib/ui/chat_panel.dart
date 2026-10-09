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
/// 取值理由：这是**视图层**的渲染上限，**不是**存储上限——会话是**落盘**的
/// （`ChatSessionStore` → localStorage，`main.dart` 注入 `saveChatSessions`）。
/// 500 条足以覆盖一次长会话，又不至于让 `ListView` 的语义树与读屏浏览退化。
/// 超出后**丢最旧的**——无限增长的长会话会让「往上翻」变成不可用操作。
///
/// 刻意在**视图层**裁剪而不是数据层：数据层裁剪会让「完整对话」无法导出排查。
///
/// （2026-09-13 rc.3 更正：原注释写「聊天历史**不落盘**（用户裁决），纯内存」——
/// 那是多会话存储接线之前的口径，早就不成立了。）
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
    this.playingSentenceSeq,
    this.backdropVisible = false,
    this.listenSupported = false,
    this.listening = false,
    this.listenStatus,
    this.listenError,
    this.onToggleListen,
    this.listenNote,
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

  /// 失败气泡的「重试」= **重发上一条用户消息**。
  ///
  /// **null = 没有上一条可重发 ⇒ 按钮不出现**（`MessageBubble` 只在非空时
  /// 画那颗按钮）——不摆按下去没反应的假出路（2026-10-06 裁决 R4-T5）。
  final VoidCallback? onRetryLast;

  /// 节流后的播报文本（转给流式气泡的 `liveRegion`）。
  final String? announcement;

  /// **当前正在播放的句号**（`sentence_seq`）；`null` = 本轮没有朗读高亮。
  ///
  /// 面板只是把它转给气泡——「哪一段该亮」由气泡拿纯函数在自己的
  /// [ChatMessage.spoken] 上算（见 `chat/spoken_highlight.dart`）。
  final int? playingSentenceSeq;

  /// 壳背后是否有全局背景图。
  ///
  /// 有才让面板留一点透（读 [AppColors.panelAlpha]，其下界是
  /// [AppColors.kMinPanelAlpha]），没有就保持原来的不透明面——
  /// 所以不开壳背景时这里的观感与改动前**逐像素一致**。
  final bool backdropVisible;

  /// 这个构建有没有语音识别能力（没有 → 「听」按钮禁用并说明原因）。
  final bool listenSupported;

  /// 正在「听」（常驻唤醒词检测）。
  final bool listening;

  /// 「听」按钮旁的一行状态（未在听时 null，不占位）。
  final String? listenStatus;

  /// 「听」失败时的一句话（权限 / 设备 / 服务不可达 / 主链忙）。
  final String? listenError;

  /// 点「听」：开始 / 结束一次听写（2026-10-08 起产品路径只有这一种用法）。
  final VoidCallback? onToggleListen;

  /// 一行**诚实说明**（Web Speech 需联网、音频出本机）。为空不渲染。
  final String? listenNote;

  @override
  Widget build(BuildContext context) {
    // 只渲染最后 [kChatHistoryLimit] 条（**不**在数据层裁剪：
    // 数据层裁剪会让「完整对话」无法导出/排查，而视图层裁剪只影响渲染）。
    final List<ChatMessage> visible = messages.length > kChatHistoryLimit
        ? messages.sublist(messages.length - kChatHistoryLimit)
        : messages;

    // 聊天面用**独立底色**与舞台分隔：舞台是纯黑（`stageBackdrop`），
    // 聊天面板盖在它上面时必须自己撑出一个面，否则气泡看起来是浮在黑底上。
    //
    // 2026-09-14（rc.5）：壳背后有背景时，这个面留一点透让背景透出来；
    // 没有背景时**仍然不透明**（「透」的选项在没有背景时不该有可见效果）。
    //
    // 2026-09-27：透明度不再是那个写死的 0.86，而是读 [AppColors.panelAlpha]
    // ——「界面透明程度」偏好的**唯一落点**。设置面板与组卡片无条件读它；
    // 聊天面板多一道 [backdropVisible] 闸门，**这是有意的**：
    // 背后没有图时把聊天面板做成半透，只会让整块界面变灰（底下是
    // 不透明的纯色底，半透等于「少画一层」），而不是「透出背景」。
    // 设置里的说明文案已经把这条差异写清楚。
    final AppPalette palette = appPaletteOf(context);
    final ThemeData theme = Theme.of(context);
    final AppColors appColors = appColorsOf(context);
    return ColoredBox(
      color: backdropVisible
          ? palette.surface.withValues(alpha: appColors.panelAlpha)
          : palette.surface,
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
            // ── 头部：会话 / 设置 ──
            //
            // 2026-09-27：头部**不再是一行裸 widget**，而是「面板自己的标题区」——
            // 一条 hairline 把标题区与内容区分开，两侧的面是**同一张**
            // （`panelAlpha` 那个透明度），所以「统一透明」是结构上的事实，
            // 不是靠把两个颜色调成一样。
            //
            // 为什么之前看不出来是头部：它和内容共用一个 `ColoredBox`，
            // 中间什么都没有，于是一眼看过去是「聊天面板顶部有一排按钮」，
            // 而不是「这个面板有标题区」。
            if (onOpenSettings != null || onOpenSessions != null)
              DecoratedBox(
                decoration: BoxDecoration(
                  border: Border(bottom: BorderSide(color: appColors.hairline)),
                ),
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(
                    Space.s2,
                    Space.s1,
                    Space.s2,
                    Space.s1,
                  ),
                  child: Row(
                    children: <Widget>[
                      const Spacer(),
                      // 文字按钮（与 AppBar 里那两个同源同文案）——用户裁决
                      // 「尽量少用图片，用文字做按钮」。
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
                          // 朗读高亮：只有与本句号对得上的**助手**气泡会亮
                          //（判据在气泡里；序号对不上时所有气泡都不亮）。
                          highlightSeq: playingSentenceSeq,
                          // 失败占位气泡才给「重试」；回调为 null（没有上一条
                          // 可重发）时 `MessageBubble` 整颗按钮都不画。
                          onRetry: m.isPlaceholder ? onRetryLast : null,
                          // 只给**最后一条**（正在流式的那个）挂播报；
                          // 历史消息传 null，否则读屏会把整段历史重念一遍。
                          announcement: index == 0 && m.streaming
                              ? announcement
                              : null,
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
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  // 常态语音检测：一行状态 + 一行可读错误（贴在「听」按钮上方，
                  // 不弹 toast——一闪而过的提示读不完）。
                  if (listenStatus != null ||
                      listenError != null ||
                      listenNote != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: Space.s1),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          if (listenStatus != null || listenError != null)
                            Text(
                              listenError ?? listenStatus!,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: listenError != null
                                    ? theme.colorScheme.error
                                    : appColorsOf(context).contentMuted,
                              ),
                            ),
                          // P0-4 诚实性：Web Speech 是**云端**识别、必须联网、
                          // 音频会出本机——不能让人以为本地可用。
                          if (listenNote != null)
                            Text(
                              listenNote!,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: appColorsOf(context).contentMuted,
                              ),
                            ),
                        ],
                      ),
                    ),
                  Row(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: <Widget>[
                  _ListenButton(
                    supported: listenSupported,
                    listening: listening,
                    onToggle: onToggleListen,
                  ),
                  const SizedBox(width: Space.s1),
                  Expanded(
                    child: TextField(
                      controller: input,
                      minLines: 1,
                      maxLines: 4,
                      textInputAction: TextInputAction.send,
                      onSubmitted: (_) => onSend(),
                      decoration: const InputDecoration(hintText: '说点什么…'),
                    ),
                  ),
                  const SizedBox(width: Space.s2),
                  // 一轮进行中时，同一个位置变「停止」——避免用户以为发了没反应。
                  _RoundActionButton(
                    icon:
                        (phase == UiPhase.thinking || phase == UiPhase.speaking)
                        ? Icons.stop_rounded
                        : Icons.arrow_upward_rounded,
                    tooltip:
                        (phase == UiPhase.thinking || phase == UiPhase.speaking)
                        ? '停止本轮'
                        : '发送（Enter）',
                    onPressed:
                        (phase == UiPhase.thinking || phase == UiPhase.speaking)
                        ? onStop
                        : onSend,
                  ),
                ],
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

/// 「听」按钮：**两态点按**（2026-10-08 起不再有按住说话）。
///
/// - 没在录 → 「听」：点一下开始录音；
/// - 正在录 → 「停」：再点一下结束，定稿进输入框；
/// - 不支持 / 未接线 → **禁用并说明**，不摆一个按不动的入口。
///
/// 指针不再由外层 [GestureDetector] 独占，也不再有「点按还是按住」的计时判定
/// ——那个判定服务的是 PTT，而产品路径已经不要它了。
class _ListenButton extends StatelessWidget {
  const _ListenButton({
    required this.supported,
    required this.listening,
    required this.onToggle,
  });

  final bool supported;
  final bool listening;
  final VoidCallback? onToggle;

  @override
  Widget build(BuildContext context) {
    final bool enabled = supported && onToggle != null;
    final String tooltip = !supported
        ? '这个浏览器没有语音识别（需要桌面版 Chrome / Edge）'
        : listening
        ? '再点一次结束，字会进输入框'
        : '点一下开始听';
    final Widget visual = listening
        ? FilledButton(
            onPressed: enabled ? onToggle : null,
            style: FilledButton.styleFrom(
              minimumSize: const Size(52, 40),
              padding: const EdgeInsets.symmetric(horizontal: Space.s2),
            ),
            child: const Text('停'),
          )
        : OutlinedButton(
            onPressed: enabled ? onToggle : null,
            style: OutlinedButton.styleFrom(
              minimumSize: const Size(52, 40),
              padding: const EdgeInsets.symmetric(horizontal: Space.s2),
            ),
            child: const Text('听'),
          );
    return Tooltip(message: tooltip, child: visual);
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
