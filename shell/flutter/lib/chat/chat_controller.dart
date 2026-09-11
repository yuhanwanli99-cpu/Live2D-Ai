import 'dart:async';

import 'package:flutter/foundation.dart';

import '../actions/action_log_controller.dart';
import '../api/api_client.dart';
import 'chat_message.dart';

export 'chat_message.dart';
import '../api/ws_client.dart';
import '../audio/audio_player.dart';
import '../audio/epoch_gate.dart';

/// 聊天状态：消息列表 + 流式拼接 + turn 状态 + 错误提示。
///
/// 数据源：`POST /api/v1/chat`（发送）、`/ws/state`（流式与音频）。
class ChatController extends ChangeNotifier {
  ChatController({
    required this.api,
    required this.ws,
    required this.audio,
    ActionLogController? actionLog,
  }) : actionLog = actionLog ?? ActionLogController() {
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

  /// 动作日志接线层。**由外部注入**：动作来源可见性独立于聊天，注入后
  /// UI 可以直接监听它（单一 listenable），而不必跟着聊天消息一起重建。
  final ActionLogController actionLog;

  final List<StreamSubscription<Object?>> _subscriptions =
      <StreamSubscription<Object?>>[];
  final List<ChatMessage> messages = <ChatMessage>[];

  bool _streaming = false;
  String? _error;

  /// 最近一次错误的**机器码**（`llm_upstream_401` / `tts_transport` …）。
  ///
  /// 与 [error] 的关系：`error` 是给人看的一行（码 + 说明 + 提示），本字段是
  /// 给代码分支用的锚点——错误横幅据此决定「下一步」按钮是「去 LLM 设置」
  /// 还是「去语音合成设置」，**不去解析显示文案**（文案会改，码是契约）。
  String? _errorCode;

  WsStatus _wsStatus = WsStatus.idle;
  ChatMessage? _assistant;
  int? _audioEpoch;

  bool get streaming => _streaming;
  String? get error => _error;

  /// 最近一次错误的机器码（见 [_errorCode]）。
  String? get errorCode => _errorCode;

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
    _errorCode = null;
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
      // 本地（HTTP）失败也有码——`ApiClient._errorFrom` 从响应体里取的
      // `error.code`（`busy` / `no_supervisor` / `invalid_payload` …）。
      _errorCode = error.code;
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
    _errorCode = null;
    notifyListeners();
  }

  void _onWsEvent(WsEvent event) {
    switch (event) {
      case SubscribeAckEvent():
      case HeartbeatEvent():
      case UnknownWsEvent():
        break;
      case ActionStateEvent():
        // 这里曾经是 `break;`——事件被**丢掉**，于是 core 早已下发的 `source`
        // 在界面上完全看不见（规格 §0.3 / §8.3）。现在交给动作日志接线层。
        // 注意它自己会广播，**不**在这里调 `notifyListeners()`：动作状态与消息
        // 列表无关，混在一起只会让聊天列表在高频动作下无谓重建（原则 P6）。
        actionLog.consume(event);
      case TextDeltaEvent(:final text, :final completed, :final epoch):
        if (text != null && text.isNotEmpty) _appendDelta(text, epoch);
        if (completed != null) _finishTurn(failed: completed == false);
      case TurnStateEvent(:final status):
        _finishTurn(failed: status == 'failed');
      case final RuntimeStatusEvent status:
        final String name = status.event;
        if (name == 'voice_started') {
          _streaming = true;
          notifyListeners();
        }
        // 服务端推进 epoch = 上一轮作废（停止按钮 / 抢占）。判据抽在
        // `audio/epoch_gate.dart`（纯函数，**可单测**）——那是钉子 13，
        // 而本文件必须 `import 'package:web'`，在 `flutter test` 里加载不了。
        if (mustInterruptAudio(event)) {
          _audioEpoch = null;
          audio.interrupt();
        }
      case AudioEvent(
          :final pcm,
          :final sampleRate,
          :final epoch,
          :final start,
          :final end,
          :final volume,
        ):
        _onAudio(pcm, sampleRate, epoch, start, end, volume);
      case WsErrorEvent(:final code, :final message, :final hint):
        // **错误码上屏**（2026-09-11）：`code` 是契约锚点，也是后端日志里的
        // 字段名——只有 message 时用户根本无法定位（「上游非成功状态 401」
        // 查不到任何东西）。`hint` 由后端给，前端只显示。
        _error = formatWsError(code: code, message: message, hint: hint);
        _errorCode = code;
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
    double? volume,
  ) {
    final AudioGateDecision decision = decideAudioFrame(
      currentEpoch: _audioEpoch,
      incomingEpoch: epoch,
    );
    if (decision.interrupt) audio.interrupt();
    _audioEpoch = decision.epoch;
    audio.feed(
      PcmFrame(
        pcm: pcm,
        sampleRate: sampleRate,
        start: start,
        end: end,
        volume: volume,
      ),
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
    // **失败但没收到 `error` 帧**：老实说「不知道为什么」，并指向诊断日志。
    // 以前这里什么都不设——界面上只有一个灰掉的「（生成失败）」气泡，用户
    // 既没有码也没有去处可查（「前端无法知道错误信息」）。既然现在后端会发
    // `error` 帧（带码），收不到就说明是「旧服务端 / 帧被截断 / 真的静默失败」，
    // 这三种都该明说，而不是假装知道原因。
    if (failed && _error == null) {
      _error = '本轮失败，但服务端未给出错误详情'
          '（可打开「设置 → 开发模式 → 诊断日志」查看后端日志）';
      _errorCode = 'turn_failed_no_detail';
    }
    notifyListeners();
  }

  @override
  void dispose() {
    for (final subscription in _subscriptions) {
      unawaited(subscription.cancel());
    }
    actionLog.dispose();
    super.dispose();
  }
}
