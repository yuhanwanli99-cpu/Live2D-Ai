import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:web/web.dart' as web;

import 'actions/action_log_controller.dart';
import 'api/api_client.dart';
import 'api/commands_api.dart';
import 'api/ws_client.dart';
import 'audio/audio_player.dart';
import 'chat/chat_controller.dart';
import 'design/breakpoints.dart';
import 'design/tokens.dart';
import 'live2d/live2d_stage.dart';
import 'settings/display_prefs.dart';
import 'ui/action_history_view.dart';
import 'ui/display_panel.dart';
import 'ui/theme.dart';

/// 本地显示偏好的存储键（localStorage）。
const String kDisplayPrefsKey = 'live2d-ai.display-prefs';

/// 读取本地显示偏好：**任何异常都回落默认**（坏存储不该让舞台渲染不出来）。
DisplayPrefs loadDisplayPrefs() {
  try {
    final raw = web.window.localStorage.getItem(kDisplayPrefsKey);
    if (raw == null || raw.isEmpty) return const DisplayPrefs();
    final decoded = jsonDecode(raw);
    if (decoded is! Map) return const DisplayPrefs();
    return DisplayPrefs.fromJson(decoded.cast<String, Object?>());
  } catch (_) {
    return const DisplayPrefs();
  }
}

/// 写入本地显示偏好（失败静默：无痕模式等场景下 localStorage 可能抛错）。
void saveDisplayPrefs(DisplayPrefs prefs) {
  try {
    web.window.localStorage.setItem(
      kDisplayPrefsKey,
      jsonEncode(prefs.toJson()),
    );
  } catch (_) {
    // 忽略：偏好丢失不影响本次会话。
  }
}

void main() {
  runApp(const Live2DShellApp());
}

/// 应用根：深色 Material 3 主题。
class Live2DShellApp extends StatelessWidget {
  const Live2DShellApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Live2D Ai',
      debugShowCheckedModeBanner: false,
      theme: buildAppTheme(),
      home: const AppShell(),
    );
  }
}

/// 主页面：左/主区 Live2D 舞台 + 右侧聊天面板。
class AppShell extends StatefulWidget {
  const AppShell({super.key});

  @override
  State<AppShell> createState() => _AppShellState();
}

class _AppShellState extends State<AppShell> {
  final GlobalKey<Live2DStageState> _stageKey = GlobalKey<Live2DStageState>();
  final TextEditingController _input = TextEditingController();

  late final ApiClient _api;
  late final CommandsApi _commandsApi;
  late final WsClient _ws;
  late final AudioPlayer _audio;
  late final ChatController _chat;
  StreamSubscription<double>? _levelSubscription;

  /// 本地显示偏好（模型缩放 / 口型灵敏度 / 开关）；持久化在 localStorage。
  DisplayPrefs _prefs = const DisplayPrefs();

  @override
  void initState() {
    super.initState();
    _prefs = loadDisplayPrefs();
    _api = ApiClient();
    _commandsApi = CommandsApi();
    _ws = WsClient();
    _audio = AudioPlayer();
    // 静音与音量是纯本机输出设置（不经渲染面）：默认出声。
    _audio.muted = _prefs.muted;
    _audio.volume = _prefs.volume;
    _chat = ChatController(api: _api, ws: _ws, audio: _audio);
    // 音频 RMS → 舞台口型。
    _levelSubscription = _audio.levels.listen((level) {
      _stageKey.currentState?.setMouth(level);
    });
    _ws.connect();
    unawaited(_loadActionCatalog());
  }

  /// 取一次动作目录，把「协议名 → 中文名」灌进动作日志。
  ///
  /// **失败静默**：目录是可选资源。取不到就显示协议名（`nod` 而不是「点头」）——
  /// 看得懂，且不撒谎；比起让整个界面报错，这是更好的降级。
  /// 权威来源是服务端 `GET /api/v1/commands`，前端**不内建**映射表。
  Future<void> _loadActionCatalog() async {
    try {
      _chat.actionLog.useCatalog(await _commandsApi.fetchCommands());
    } catch (_) {
      // 忽略：保持恒等解析器（显示协议名，不撒谎）。
    }
  }

  /// 把 [_prefs] 下发给渲染面（协议 v1 `sync`）。
  void _applyPrefs() {
    _stageKey.currentState?.sync(
      scale: _prefs.scale,
      lipSync: _prefs.lipSync,
      idleEnabled: _prefs.idleEnabled,
      mouthSensitivity: _prefs.mouthSensitivity,
    );
  }

  /// 偏好变更：更新内存 + 落盘 + 立即下发。
  void _updatePrefs(DisplayPrefs next) {
    if (next == _prefs) return;
    setState(() => _prefs = next);
    saveDisplayPrefs(next);
    // 静音与音量是纯本机输出设置，不经渲染面——直接作用于 AudioPlayer。
    _audio.muted = next.muted;
    _audio.volume = next.volume;
    _applyPrefs();
  }

  @override
  void dispose() {
    unawaited(_levelSubscription?.cancel());
    _chat.dispose();
    _ws.dispose();
    _audio.dispose();
    _api.dispose();
    _commandsApi.dispose();
    _input.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _input.text;
    if (text.trim().isEmpty) return;
    _input.clear();
    await _chat.send(text);
  }

  /// 打开「外观与口型」面板。宽屏用侧栏，窄屏用底部弹层——同一个 [DisplayPanel]。
  Future<void> _openDisplayPanel() async {
    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      constraints: const BoxConstraints(maxWidth: 520),
      builder: (BuildContext sheetContext) {
        return StatefulBuilder(
          builder: (BuildContext context, StateSetter setSheetState) {
            return DisplayPanel(
              prefs: _prefs,
              onChanged: (DisplayPrefs next) {
                // 同时刷新弹层内部与宿主（弹层不随宿主 setState 重建）。
                setSheetState(() {});
                _updatePrefs(next);
              },
            );
          },
        );
      },
    );
  }

  /// 打开「动作日志」（P2：把 core 早已下发的 `source` 显示出来）。
  ///
  /// 用底部弹层而不是常驻分栏：舞台是主角（原则 P1），日志是诊断面。
  /// P3 会把入口挪进「动作与互动」分区，这里先把价值交付出来。
  Future<void> _openActionLog() async {
    // 打开即视为已读（入口按钮的未读角标清零）。
    final ActionLogController log = _chat.actionLog;
    log.markSeen();
    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      constraints: const BoxConstraints(maxWidth: 560, maxHeight: 520),
      builder: (BuildContext sheetContext) {
        return ListenableBuilder(
          listenable: log,
          builder: (BuildContext context, Widget? _) => ActionHistoryView(
            entries: log.entries,
            current: log.current,
            droppedCount: log.droppedCount,
            onClear: log.clear,
          ),
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      // 首次指针交互解锁 WebAudio（浏览器 autoplay 限制）。
      behavior: HitTestBehavior.translucent,
      onPointerDown: (_) => _audio.unlock(),
      child: Scaffold(
        appBar: AppBar(
          title: const Text('Live2D Ai'),
          actions: <Widget>[
            // 动作日志入口 + **未读 AI 动作角标**：「AI 自己触发了动作」是个
            // 转瞬即逝的信号（演完就没了）。日志静静躺在弹层里等于没提示。
            ListenableBuilder(
              listenable: _chat.actionLog,
              builder: (BuildContext context, Widget? _) {
                final int unseen = _chat.actionLog.unseenAiActions;
                return IconButton(
                  tooltip: unseen > 0 ? '动作日志（$unseen 条 AI 触发的未读）' : '动作日志',
                  onPressed: _openActionLog,
                  icon: unseen > 0
                      ? Badge.count(count: unseen, child: const Icon(Icons.history))
                      : const Icon(Icons.history),
                );
              },
            ),
            IconButton(
              tooltip: '外观与口型',
              onPressed: _openDisplayPanel,
              icon: const Icon(Icons.tune),
            ),
            Padding(
              padding: const EdgeInsets.only(right: 16),
              // 必须包在 ListenableBuilder 里：AppBar.actions 不在 body 的
              // ListenableBuilder 作用域内，否则徽标只在首帧构建一次，永远停在
              // 初始 `idle`（显示「WS 未连接」）而看不出真实状态。（2026-09-10）
              child: Center(
                child: ListenableBuilder(
                  listenable: _chat,
                  builder: (BuildContext context, Widget? _) =>
                      _WsBadge(status: _chat.wsStatus),
                ),
              ),
            ),
          ],
        ),
        body: ListenableBuilder(
          listenable: _chat,
          builder: (BuildContext context, Widget? _) {
            return LayoutBuilder(
              builder: (BuildContext context, BoxConstraints constraints) {
                final SizeClass sizeClass = Breakpoints.sizeClassOf(
                  constraints.maxWidth,
                );
                final bool wide = sizeClass.hasSideChat;
                final stage = Live2DStage(
                  key: _stageKey,
                  dark: true,
                  // 就绪后补发显示偏好：首次挂载时桥还在 loading，
                  // 以及在 retry 重建 iframe 之后（旧队列已随旧桥销毁）。
                  onReady: _applyPrefs,
                );
                final panel = _ChatPanel(
                  messages: _chat.messages,
                  streaming: _chat.streaming,
                  error: _chat.error,
                  input: _input,
                  onSend: _send,
                  onStop: () => unawaited(_chat.stop()),
                  onDismissError: _chat.clearError,
                );
                if (wide) {
                  return Row(
                    children: <Widget>[
                      Expanded(flex: 3, child: stage),
                      const VerticalDivider(width: 1),
                      SizedBox(width: 380, child: panel),
                    ],
                  );
                }
                return Column(
                  children: <Widget>[
                    Expanded(flex: 3, child: stage),
                    const Divider(height: 1),
                    Expanded(flex: 2, child: panel),
                  ],
                );
              },
            );
          },
        ),
      ),
    );
  }
}

class _WsBadge extends StatelessWidget {
  const _WsBadge({required this.status});

  final WsStatus status;

  @override
  Widget build(BuildContext context) {
    final AppColors colors = appColorsOf(context);
    final (String label, Color color) = switch (status) {
      WsStatus.idle => ('WS 未连接', colors.contentFaint),
      WsStatus.connecting => ('WS 连接中', AppPalette.warning),
      WsStatus.connected => ('WS 已连接', AppPalette.success),
      WsStatus.disconnected => ('WS 重连中', AppPalette.warning),
      WsStatus.closed => ('WS 已关闭', AppPalette.danger),
    };
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Icon(Icons.circle, size: 10, color: color),
        const SizedBox(width: Space.s1),
        Text(label, style: Theme.of(context).textTheme.bodySmall),
      ],
    );
  }
}

class _ChatPanel extends StatefulWidget {
  const _ChatPanel({
    required this.messages,
    required this.streaming,
    required this.error,
    required this.input,
    required this.onSend,
    required this.onStop,
    required this.onDismissError,
  });

  final List<ChatMessage> messages;
  final bool streaming;
  final String? error;
  final TextEditingController input;
  final VoidCallback onSend;
  final VoidCallback onStop;
  final VoidCallback onDismissError;

  @override
  State<_ChatPanel> createState() => _ChatPanelState();
}

class _ChatPanelState extends State<_ChatPanel> {
  @override
  Widget build(BuildContext context) {
    return Container(
      color: AppPalette.chatPanelSurface,
      child: Column(
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    '对话',
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                if (widget.streaming)
                  const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
              ],
            ),
          ),
          if (widget.error != null)
            _ErrorBanner(
              message: widget.error!,
              onDismiss: widget.onDismissError,
            ),
          Expanded(
            child: widget.messages.isEmpty
                ? const _EmptyHint()
                : ListView.builder(
                    reverse: true,
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                    itemCount: widget.messages.length,
                    itemBuilder: (BuildContext context, int index) {
                      final message =
                          widget.messages[widget.messages.length - 1 - index];
                      return _MessageBubble(message: message);
                    },
                  ),
          ),
          const Divider(height: 1),
          Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              children: <Widget>[
                TextField(
                  controller: widget.input,
                  enabled: !widget.streaming,
                  minLines: 1,
                  maxLines: 4,
                  textInputAction: TextInputAction.send,
                  onSubmitted: (_) => widget.onSend(),
                  decoration: const InputDecoration(
                    hintText: '输入消息，回车发送',
                    border: OutlineInputBorder(),
                    isDense: true,
                  ),
                ),
                const SizedBox(height: 8),
                Row(
                  children: <Widget>[
                    Expanded(
                      child: FilledButton.icon(
                        onPressed: widget.streaming ? null : widget.onSend,
                        icon: const Icon(Icons.send, size: 18),
                        label: const Text('发送'),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: widget.streaming ? widget.onStop : null,
                        icon: const Icon(Icons.stop, size: 18),
                        label: const Text('停止'),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _EmptyHint extends StatelessWidget {
  const _EmptyHint();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(Space.s6),
        child: Text(
          '还没有消息。\n输入文本开始一轮对话。',
          textAlign: TextAlign.center,
          style: TextStyle(color: appColorsOf(context).contentFaint),
        ),
      ),
    );
  }
}

class _ErrorBanner extends StatelessWidget {
  const _ErrorBanner({required this.message, required this.onDismiss});

  final String message;
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.fromLTRB(12, 0, 12, 8),
      padding: const EdgeInsets.fromLTRB(12, 8, 4, 8),
      decoration: BoxDecoration(
        color: AppPalette.dangerSurface,
        borderRadius: BorderRadius.circular(AppRadius.sm),
      ),
      child: Row(
        children: <Widget>[
          const Icon(Icons.error_outline, size: 18, color: AppPalette.danger),
          const SizedBox(width: 8),
          Expanded(
            child: Text(message, style: Theme.of(context).textTheme.bodySmall),
          ),
          IconButton(
            onPressed: onDismiss,
            icon: const Icon(Icons.close, size: 16),
            tooltip: '关闭',
          ),
        ],
      ),
    );
  }
}

class _MessageBubble extends StatelessWidget {
  const _MessageBubble({required this.message});

  final ChatMessage message;

  @override
  Widget build(BuildContext context) {
    final isUser = message.role == ChatRole.user;
    final scheme = Theme.of(context).colorScheme;
    return Align(
      alignment: isUser ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        constraints: const BoxConstraints(maxWidth: 320),
        margin: const EdgeInsets.symmetric(vertical: 4),
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
        decoration: BoxDecoration(
          color: isUser
              ? scheme.primaryContainer
              : (message.failed
                  ? AppPalette.dangerSurface
                  : AppPalette.bubbleAssistant),
          borderRadius: BorderRadius.circular(AppRadius.md),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              isUser ? '你' : '助手',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: appColorsOf(context).contentFaint,
              ),
            ),
            const SizedBox(height: 4),
            Text(message.text.isEmpty ? '…' : message.text),
          ],
        ),
      ),
    );
  }
}
