import 'dart:async';

import 'package:flutter/foundation.dart';

import '../api/api_client.dart';
import '../api/ws_client.dart';
import '../audio/audio_player.dart';

enum ChatRole { user, assistant }

/// 一条聊天消息（assistant 气泡可流式追加）。
class ChatMessage {
  ChatMessage({
    required this.role,
    this.text = '',
    this.epoch,
    this.streaming = false,
    this.failed = false,
  });

  final ChatRole role;
  String text;
  final int? epoch;
  bool streaming;
  bool failed;
}

/// 聊天状态：消息列表 + 流式拼接 + turn 状态 + 错误提示。
///
/// 数据源：`POST /api/v1/chat`（发送）、`/ws/state`（流式与音频）。
class ChatController extends ChangeNotifier {
  ChatController({required this.api, required this.ws, required this.audio}) {
    _subscriptions.addAll(<StreamSubscription<Object?>>[
      ws.events.listen(_onWsEvent),
      ws.statuses.listen((status) {
        _wsStatus = status;
        notifyListeners();
      }),
    ]);
  }

  final ApiClient api;
  final WsClient ws;
  final AudioPlayer audio;

  final List<StreamSubscription<Object?>> _subscriptions =
      <StreamSubscription<Object?>>[];
  final List<ChatMessage> messages = <ChatMessage>[];

  bool _streaming = false;
  String? _error;
  WsStatus _wsStatus = WsStatus.idle;
  ChatMessage? _assistant;
  int? _audioEpoch;

  bool get streaming => _streaming;
  String? get error => _error;
  WsStatus get wsStatus => _wsStatus;

  /// 发送用户文本；成功则建流式 assistant 气泡，失败写入可见错误。
  Future<void> send(String raw) async {
    final text = raw.trim();
    if (text.isEmpty || _streaming) return;
    audio.unlock();
    // **发送即保活**：服务端重启后旧连接会失效（POST 能成功但回复走 WS），
    // 这里主动确认连接，避免「发出去没回应」。（2026-09-10）
    ws.ensureConnected();
    messages.add(ChatMessage(role: ChatRole.user, text: text));
    _streaming = true;
    _error = null;
    notifyListeners();
    try {
      final accepted = await api.sendChat(text);
      assert(accepted.accepted);
      final bubble = ChatMessage(
        role: ChatRole.assistant,
        epoch: accepted.epoch,
        streaming: true,
      );
      _assistant = bubble;
      messages.add(bubble);
      notifyListeners();
    } on ApiException catch (error) {
      _error = error.toString();
      _streaming = false;
      messages.add(
        ChatMessage(role: ChatRole.assistant, text: '⚠ ${error.message}', failed: true),
      );
      notifyListeners();
    } catch (error) {
      _error = '发送失败：$error';
      _streaming = false;
      messages.add(
        ChatMessage(role: ChatRole.assistant, text: '⚠ 发送失败：$error', failed: true),
      );
      notifyListeners();
    }
  }

  /// `POST /api/v1/chat/stop` 并收口当前气泡。
  Future<void> stop() async {
    try {
      await api.stopChat();
    } on ApiException catch (error) {
      _error = error.toString();
    } catch (error) {
      _error = '停止失败：$error';
    }
    _finishTurn();
  }

  void clearError() {
    _error = null;
    notifyListeners();
  }

  void _onWsEvent(WsEvent event) {
    switch (event) {
      case SubscribeAckEvent():
      case HeartbeatEvent():
      case ActionStateEvent():
      case UnknownWsEvent():
        break;
      case TextDeltaEvent(:final text, :final completed, :final epoch):
        if (text != null && text.isNotEmpty) _appendDelta(text, epoch);
        if (completed != null) _finishTurn(failed: completed == false);
      case TurnStateEvent(:final status):
        _finishTurn(failed: status == 'failed');
      case RuntimeStatusEvent(:final event):
        if (event == 'voice_started') {
          _streaming = true;
          notifyListeners();
        }
        // 服务端推进 epoch = 上一轮作废（停止按钮 / 抢占）。必须**立刻**打断
        // 已排到未来的音频：TTS 分片是 1 ms 级突发到达的，播放时间轴最多排到
        // 3 s 之后（实测调度提前量峰值 3.03 s）。若只靠「下一帧带新 epoch」
        // 触发打断，停止后旧音频还会继续响到新轮次开口——停止按钮等于失灵。
        // （2026-09-10，与 schedule.dart 的时间轴修复配套）
        if (event == 'new_epoch') {
          _audioEpoch = null;
          audio.interrupt();
        }
      case AudioEvent(
          :final pcm,
          :final sampleRate,
          :final epoch,
          :final start,
          :final end,
        ):
        _onAudio(pcm, sampleRate, epoch, start, end);
      case WsErrorEvent(:final message):
        _error = message;
        notifyListeners();
    }
  }

  void _appendDelta(String text, int? epoch) {
    var bubble = _assistant;
    if (bubble == null) {
      bubble = ChatMessage(role: ChatRole.assistant, epoch: epoch, streaming: true);
      messages.add(bubble);
    }
    bubble.text += text;
    _assistant = bubble;
    _streaming = true;
    notifyListeners();
  }

  void _onAudio(
    Uint8List pcm,
    int sampleRate,
    int epoch,
    bool start,
    bool end,
  ) {
    if (_audioEpoch != null && _audioEpoch != epoch) audio.interrupt();
    _audioEpoch = epoch;
    audio.feed(
      PcmFrame(pcm: pcm, sampleRate: sampleRate, start: start, end: end),
    );
  }

  void _finishTurn({bool failed = false}) {
    final bubble = _assistant;
    if (bubble != null) {
      bubble.streaming = false;
      if (bubble.text.isEmpty) {
        bubble.failed = failed;
        // 常见成因：模型本轮只调用了动作工具、没有输出文字（DeepSeek flash 在打招呼
    // 这类短 prompt 上会这样）。提示写清楚，避免用户以为是「没反应」。（2026-09-10）
    bubble.text = failed
        ? '（生成失败）'
        : '（本轮没有文字输出——通常是模型只调用了动作工具）';
      }
    }
    _assistant = null;
    _streaming = false;
    notifyListeners();
  }

  @override
  void dispose() {
    for (final subscription in _subscriptions) {
      unawaited(subscription.cancel());
    }
    super.dispose();
  }
}
