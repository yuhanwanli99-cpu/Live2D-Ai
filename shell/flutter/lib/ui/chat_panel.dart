/// 聊天面板（L3）：消息列表 + 音频条 + 输入区 + 设置入口。
///
/// 只依赖纯模型（`chat/chat_message.dart`、`state/ui_phase.dart`），
/// **不** import `package:web` 那条链——于是整个面板可 VM/widget 测试。
library;

import 'dart:async';

import 'package:flutter/material.dart';

import '../chat/chat_message.dart';
import '../design/tokens.dart';
import '../state/ui_phase.dart';
import '../voice/voice_listen_controller.dart' show kVoiceTapThreshold;
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
    this.backdropVisible = false,
    this.listenSupported = false,
    this.listening = false,
    this.listenStatus,
    this.listenError,
    this.onToggleListen,
    this.onPressStart,
    this.onPressRelease,
    this.pttActive = false,
    this.listenNote,
    this.listenBlockedReason,
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

  /// 点按「听」：常驻唤醒开 / 关。
  final VoidCallback? onToggleListen;

  /// 按住说话（PTT）开始 / 结束（P0-4）。为空时按钮只有「点按」一种用法。
  final VoidCallback? onPressStart;
  final VoidCallback? onPressRelease;

  /// 当前是否在按住说话（按钮显示「松」）。
  final bool pttActive;

  /// 一行**诚实说明**（Web Speech 需联网、音频出本机）。为空不渲染。
  final String? listenNote;

  /// 「听」根本不可用的**常驻**原因（如 voice-input Mod 未启用）。
  ///
  /// 与 [listenError] 的区别：那是一次识别失败的结果，这是「按钮还没点就已经
  /// 知道不行」的状态——必须**先**红字说清，而不是让用户说完才发现端点 403。
  final String? listenBlockedReason;

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
                      listenNote != null ||
                      listenBlockedReason != null)
                    Padding(
                      padding: const EdgeInsets.only(bottom: Space.s1),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          // 常驻红字（Mod 未启用）**优先**：它是「点了也没用」的
                          // 前置原因，比任何一次识别结果都更该先被看到。
                          if (listenBlockedReason != null)
                            Text(
                              listenBlockedReason!,
                              style: theme.textTheme.bodySmall?.copyWith(
                                color: theme.colorScheme.error,
                              ),
                            ),
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
                    pttActive: pttActive,
                    onToggle: onToggleListen,
                    onPressStart: onPressStart,
                    onPressRelease: onPressRelease,
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

/// 「听」按钮：**一个按钮三种用法**（P0-4）。
///
/// - **点按**（< [kVoiceTapThreshold] 松手）→ 常驻唤醒开 / 关；
/// - **按住**（≥ 阈值）→ 按住说话（PTT）：按下开始，松手提交（不要求唤醒词）；
/// - 不支持 / 未接线 → **禁用并说明**，不摆一个按不动的入口。
///
/// 实现用 [GestureDetector] 的 down/up/cancel + 计时器判定「点按还是按住」，
/// 而不是 Material 的 `onLongPress`（它 500ms 才触发，200ms 的按会什么都不做）。
/// 视觉仍是 Material 按钮（文字按钮，项目口径），但指针由外层 GestureDetector
/// 独占（[AbsorbPointer]）——所以按钮的 `onPressed` 只是「看起来可点」。
class _ListenButton extends StatefulWidget {
  const _ListenButton({
    required this.supported,
    required this.listening,
    required this.pttActive,
    required this.onToggle,
    required this.onPressStart,
    required this.onPressRelease,
  });

  final bool supported;
  final bool listening;
  final bool pttActive;
  final VoidCallback? onToggle;
  final VoidCallback? onPressStart;
  final VoidCallback? onPressRelease;

  @override
  State<_ListenButton> createState() => _ListenButtonState();
}

class _ListenButtonState extends State<_ListenButton> {
  Timer? _holdTimer;
  bool _holding = false;

  @override
  void dispose() {
    _holdTimer?.cancel();
    super.dispose();
  }

  bool get _enabled => widget.supported && widget.onToggle != null;

  void _onDown(TapDownDetails _) {
    if (!_enabled) return;
    _holding = false;
    _holdTimer?.cancel();
    _holdTimer = Timer(kVoiceTapThreshold, () {
      if (!mounted) return;
      _holding = true;
      widget.onPressStart?.call();
    });
  }

  void _onUp(TapUpDetails _) {
    _holdTimer?.cancel();
    _holdTimer = null;
    if (_holding) {
      _holding = false;
      widget.onPressRelease?.call();
    } else {
      widget.onToggle?.call();
    }
  }

  void _onCancel() {
    _holdTimer?.cancel();
    _holdTimer = null;
    if (_holding) {
      _holding = false;
      widget.onPressRelease?.call();
    }
  }

  @override
  Widget build(BuildContext context) {
    final bool active = widget.listening || widget.pttActive;
    final String tooltip = !widget.supported
        ? '这个浏览器没有语音识别（需要桌面版 Chrome / Edge）'
        : widget.pttActive
        ? '松手发送（按住说话）'
        : widget.listening
        ? '点按停止听；按住说话'
        : '点按开始听：说「小可爱 ……」；按住说话（不要求唤醒词）';
    final Widget visual = active
        ? FilledButton(
            onPressed: _enabled ? () {} : null,
            style: FilledButton.styleFrom(
              minimumSize: const Size(52, 40),
              padding: const EdgeInsets.symmetric(horizontal: Space.s2),
            ),
            child: Text(widget.pttActive ? '松' : '停'),
          )
        : OutlinedButton(
            onPressed: _enabled ? () {} : null,
            style: OutlinedButton.styleFrom(
              minimumSize: const Size(52, 40),
              padding: const EdgeInsets.symmetric(horizontal: Space.s2),
            ),
            child: const Text('听'),
          );
    return Tooltip(
      message: tooltip,
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onTapDown: _enabled ? _onDown : null,
        onTapUp: _enabled ? _onUp : null,
        onTapCancel: _enabled ? _onCancel : null,
        // 按钮自己不吃指针：点按 / 按住都由上面的 GestureDetector 判定。
        child: AbsorbPointer(child: visual),
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
