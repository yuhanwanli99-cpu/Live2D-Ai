import 'dart:async';

import 'package:flutter/material.dart';

import 'api/api_client.dart';
import 'api/ws_client.dart';
import 'audio/audio_player.dart';
import 'chat/chat_controller.dart';
import 'live2d/live2d_stage.dart';

void main() {
  runApp(const Live2DShellApp());
}

/// 应用根：深色 Material 3 主题。
class Live2DShellApp extends StatelessWidget {
  const Live2DShellApp({super.key});

  @override
  Widget build(BuildContext context) {
    final scheme = ColorScheme.fromSeed(
      seedColor: const Color(0xFF7C5CFF),
      brightness: Brightness.dark,
    );
    return MaterialApp(
      title: 'Live2D Ai',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        useMaterial3: true,
        brightness: Brightness.dark,
        colorScheme: scheme,
        scaffoldBackgroundColor: const Color(0xFF0B0D12),
      ),
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
  late final WsClient _ws;
  late final AudioPlayer _audio;
  late final ChatController _chat;
  StreamSubscription<double>? _levelSubscription;

  @override
  void initState() {
    super.initState();
    _api = ApiClient();
    _ws = WsClient();
    _audio = AudioPlayer();
    _chat = ChatController(api: _api, ws: _ws, audio: _audio);
    // 音频 RMS → 舞台口型。
    _levelSubscription = _audio.levels.listen((level) {
      _stageKey.currentState?.setMouth(level);
    });
    _ws.connect();
  }

  @override
  void dispose() {
    unawaited(_levelSubscription?.cancel());
    _chat.dispose();
    _ws.dispose();
    _audio.dispose();
    _api.dispose();
    _input.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _input.text;
    if (text.trim().isEmpty) return;
    _input.clear();
    await _chat.send(text);
  }

  @override
  Widget build(BuildContext context) {
    return Listener(
      // 首次指针交互解锁 WebAudio（浏览器 autoplay 限制）。
      behavior: HitTestBehavior.translucent,
      onPointerDown: (_) => _audio.unlock(),
      child: Scaffold(
        appBar: AppBar(
          title: const Text('Live2D Ai · Flutter 外壳'),
          actions: <Widget>[
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
                final wide = constraints.maxWidth >= 900;
                final stage = Live2DStage(
                  key: _stageKey,
                  dark: true,
                  scale: 1.0,
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
    final (String label, Color color) = switch (status) {
      WsStatus.idle => ('WS 未连接', Colors.grey),
      WsStatus.connecting => ('WS 连接中', Colors.amber),
      WsStatus.connected => ('WS 已连接', Colors.greenAccent),
      WsStatus.disconnected => ('WS 重连中', Colors.orange),
      WsStatus.closed => ('WS 已关闭', Colors.redAccent),
    };
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Icon(Icons.circle, size: 10, color: color),
        const SizedBox(width: 6),
        Text(label, style: const TextStyle(fontSize: 12)),
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
      color: const Color(0xFF10131A),
      child: Column(
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
            child: Row(
              children: <Widget>[
                const Expanded(
                  child: Text(
                    '对话',
                    style: TextStyle(fontSize: 16, fontWeight: FontWeight.w700),
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
    return const Center(
      child: Padding(
        padding: EdgeInsets.all(24),
        child: Text(
          '还没有消息。\n输入文本开始一轮对话。',
          textAlign: TextAlign.center,
          style: TextStyle(color: Colors.white54),
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
        color: const Color(0xFF3A1D1D),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Row(
        children: <Widget>[
          const Icon(Icons.error_outline, size: 18, color: Colors.redAccent),
          const SizedBox(width: 8),
          Expanded(
            child: Text(message, style: const TextStyle(fontSize: 12)),
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
                  ? const Color(0xFF3A1D1D)
                  : const Color(0xFF1B2030)),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              isUser ? '你' : '助手',
              style: TextStyle(
                fontSize: 11,
                color: scheme.onSurface.withValues(alpha: 0.6),
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
