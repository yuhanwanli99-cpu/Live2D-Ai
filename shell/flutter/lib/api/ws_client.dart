import 'dart:async';
import 'dart:convert';
import 'dart:js_interop';
import 'dart:typed_data';

import 'package:web/web.dart' as web;

import 'action_source.dart';

/// `/ws/state` 事件基类。
///
/// 服务端每连接包装 `{\"type\":...,\"seq\":N,\"ts\":\"...\",\"data\":{...}}`
/// （见 `web_api/ws/connection.rs`）。
sealed class WsEvent {
  const WsEvent({this.seq, this.ts});

  final int? seq;
  final String? ts;
}

/// 连接建立后的首帧（服务端立即发）。
class SubscribeAckEvent extends WsEvent {
  const SubscribeAckEvent({required this.topics, super.seq, super.ts});

  final List<String> topics;
}

/// 10s 周期保活帧。
class HeartbeatEvent extends WsEvent {
  const HeartbeatEvent({super.seq, super.ts});
}

/// `turn_state`：一轮对话收口（completed | failed）。
class TurnStateEvent extends WsEvent {
  const TurnStateEvent({
    required this.epoch,
    required this.status,
    super.seq,
    super.ts,
  });

  final int epoch;
  final String status;
}

/// `runtime_status`：voice_started / voice_ended / new_epoch / shutdown_ready。
class RuntimeStatusEvent extends WsEvent {
  const RuntimeStatusEvent({required this.event, this.epoch, super.seq, super.ts});

  final String event;
  final int? epoch;
}

/// `text_delta`：LLM 流式正文片段，或完成锚点（`completed`）。
class TextDeltaEvent extends WsEvent {
  const TextDeltaEvent({
    this.epoch,
    this.text,
    this.completed,
    super.seq,
    super.ts,
  });

  final int? epoch;
  final String? text;
  final bool? completed;
}

/// `action_state`：动作状态。
///
/// 协议真相：`data.action` 是**对象** `{action, strength, source}`，
/// `data.kind == 'cease'` 时则没有 `action` 字段。详见 [ActionDetail] 的头注
/// （那里记录了「曾经把它当字符串解析 ⇒ 来源全丢」这个已修缺陷）。
class ActionStateEvent extends WsEvent {
  const ActionStateEvent({
    required this.epoch,
    required this.kind,
    this.action,
    this.strength,
    this.source = ActionSource.unknown,
    super.seq,
    super.ts,
  });

  final int epoch;

  /// `perform`（正在表演）或 `cease`（停止表演）。
  final String kind;

  /// 动作名；`cease` 时为 null。
  final String? action;

  /// 强度档位（`strength_levels` = `[1,2,3]`）；缺失为 null。
  final int? strength;

  /// 谁触发的（用户 / LLM / 规则兜底）。
  final ActionSource source;

  /// 本轮是否真的有动作可展示。
  bool get hasAction => action != null;
}

/// `audio`：base64 s16le 单声道 PCM 片（默认 20ms）。
class AudioEvent extends WsEvent {
  const AudioEvent({
    required this.epoch,
    required this.pcm,
    required this.sampleRate,
    this.start = false,
    this.end = false,
    this.volume,
    this.muted = false,
    super.seq,
    super.ts,
  });

  final int epoch;
  final Uint8List pcm;
  final int sampleRate;
  final bool start;
  final bool end;

  /// 服务端按本片**原始**样本算出的 RMS ∈ [0,1]（`data.volume`）。
  ///
  /// **静音时这是口型的唯一来源**：服务端静音会把 PCM 置零（谁都发不出声），
  /// 但 `volume` 仍是真实值——所以「不出声但嘴照动」。因此客户端驱动口型
  /// **优先用本字段**，只有它缺失（旧服务端）时才回退到本地 RMS。
  final double? volume;

  /// 服务端是否处于静音输出（`data.muted`，观测/诊断用）。
  final bool muted;
}

/// 服务端 `error` 帧。
class WsErrorEvent extends WsEvent {
  const WsErrorEvent({
    required this.code,
    required this.message,
    super.seq,
    super.ts,
  });

  final String code;
  final String message;
}

/// 未识别但合法的帧（前向兼容）。
class UnknownWsEvent extends WsEvent {
  const UnknownWsEvent({
    required this.type,
    required this.data,
    super.seq,
    super.ts,
  });

  final String type;
  final Map<String, Object?> data;
}

/// WS 连接状态。
enum WsStatus { idle, connecting, connected, disconnected, closed }

/// `/ws/state` 客户端。
///
/// **真实协议是单向事件流**：服务端 `ws.rs` / `connection.rs` 明确不读
/// 客户端帧，连接后立即发 `subscribe_ack`，客户端**不**发 subscribe / ping
/// （发了也会被 tungstenite 静默丢弃）。命令一律走 HTTP。
///
/// 重连：指数退避 1s→2s→4s→8s→16s→30s 上限。
///
/// **`shutdown_ready` 不再永久停连**（2026-09-10）：服务端会被反复重启，
/// 停连会让页面「POST 成功但收不到回复」；现在按普通断线退避重连。
/// 用户发送消息时另调 [`WsClient::ensureConnected`] 立即恢复。
class WsClient {
  WsClient({String? url}) : url = url ?? _defaultUrl();

  final String url;
  final StreamController<WsEvent> _events =
      StreamController<WsEvent>.broadcast();
  final StreamController<WsStatus> _statuses =
      StreamController<WsStatus>.broadcast();

  web.WebSocket? _socket;
  Timer? _reconnectTimer;
  int _backoffMs = 1000;
  bool _disposed = false;
  WsStatus _status = WsStatus.idle;

  Stream<WsEvent> get events => _events.stream;
  Stream<WsStatus> get statuses => _statuses.stream;
  WsStatus get status => _status;

  static String _defaultUrl() {
    const override = String.fromEnvironment('WS_URL');
    if (override.isNotEmpty) return override;
    final base = Uri.base;
    final scheme = base.scheme == 'https' ? 'wss' : 'ws';
    final host = base.host.isEmpty ? '127.0.0.1' : base.host;
    final port = base.hasPort ? ':${base.port}' : '';
    return '$scheme://$host$port/ws/state';
  }

  void connect() {
    if (_disposed) return;
    _open();
  }

  void _open() {
    if (_disposed) return;
    _setStatus(WsStatus.connecting);
    final web.WebSocket socket;
    try {
      socket = web.WebSocket(url);
    } catch (_) {
      _scheduleReconnect();
      return;
    }
    _socket = socket;
    socket.onopen = ((web.Event _) {
      _backoffMs = 1000;
      _setStatus(WsStatus.connected);
    }).toJS;
    socket.onmessage = ((web.MessageEvent event) {
      final data = event.data;
      if (data == null || !data.isA<JSString>()) return;
      _handleText((data as JSString).toDart);
    }).toJS;
    socket.onerror = ((web.Event _) {
      _setStatus(WsStatus.disconnected);
    }).toJS;
    socket.onclose = ((web.Event _) {
      _socket = null;
      _scheduleReconnect();
    }).toJS;
  }

  /// 立即恢复连接（用户动作触发；重置退避）。
  ///
  /// 服务端会被反复重启（配置热重载 / 重新部署），一旦连接停摆，页面就会
  /// 「POST 成功但收不到回复」。发送消息前调用本方法可保证随时自愈。
  void ensureConnected() {
    if (_disposed || _socket != null) return;
    _reconnectTimer?.cancel();
    _backoffMs = 1000;
    _open();
  }

  void _scheduleReconnect() {
    if (_disposed) return;
    _setStatus(WsStatus.disconnected);
    _reconnectTimer?.cancel();
    final delay = Duration(milliseconds: _backoffMs);
    final next = _backoffMs * 2;
    _backoffMs = next > 30000 ? 30000 : next;
    _reconnectTimer = Timer(delay, _open);
  }

  void _setStatus(WsStatus status) {
    _status = status;
    if (!_statuses.isClosed) _statuses.add(status);
  }

  void _handleText(String text) {
    Map<String, Object?> frame;
    try {
      final decoded = jsonDecode(text);
      if (decoded is! Map) return;
      frame = Map<String, Object?>.from(decoded);
    } catch (_) {
      return; // 容忍坏帧
    }
    final type = frame['type'];
    if (type is! String) return;
    final rawData = frame['data'];
    final data = rawData is Map
        ? Map<String, Object?>.from(rawData)
        : const <String, Object?>{};
    final seq = (frame['seq'] as num?)?.toInt();
    final ts = frame['ts'] is String ? frame['ts'] as String : null;

    WsEvent? event;
    switch (type) {
      case 'subscribe_ack':
        final rawTopics = data['topics'];
        final topics = rawTopics is List
            ? rawTopics.whereType<String>().toList()
            : const <String>[];
        event = SubscribeAckEvent(topics: topics, seq: seq, ts: ts);
      case 'heartbeat':
        event = HeartbeatEvent(seq: seq, ts: ts);
      case 'turn_state':
        event = TurnStateEvent(
          epoch: _int(data['epoch']),
          status: _str(data['status']) ?? 'unknown',
          seq: seq,
          ts: ts,
        );
      case 'runtime_status':
        final kind = _str(data['event']) ?? 'unknown';
        // **不再永久停连（2026-09-10 修复）**：服务端 `shutdown_ready` 只表示
        // 「这一轮收口」，而本项目服务会被反复重启。旧实现置 `_shutdown=true`
        // 后永不重连，页面就会「POST 成功但永远收不到 LLM 回复」。
        // 现改为按普通断线处理：立即按退避重连（服务回来即自动恢复）。
        if (kind == 'shutdown_ready') {
          _socket?.close();
          _scheduleReconnect();
        }
        event = RuntimeStatusEvent(
          event: kind,
          epoch: _intOrNull(data['epoch']),
          seq: seq,
          ts: ts,
        );
      case 'text_delta':
        event = TextDeltaEvent(
          epoch: _intOrNull(data['epoch']),
          text: _str(data['text']),
          completed: data['completed'] is bool
              ? data['completed'] as bool
              : null,
          seq: seq,
          ts: ts,
        );
      case 'action_state':
        // `data.action` 是**对象**，不是字符串——见 [ActionDetail] 的头注。
        final detail = ActionDetail.fromData(data['action']);
        event = ActionStateEvent(
          epoch: _int(data['epoch']),
          kind: _str(data['kind']) ?? 'unknown',
          action: detail.action,
          strength: detail.strength,
          source: detail.source,
          seq: seq,
          ts: ts,
        );
      case 'audio':
        final encoded = _str(data['audio']);
        if (encoded != null) {
          try {
            event = AudioEvent(
              epoch: _int(data['epoch']),
              pcm: base64Decode(encoded),
              sampleRate: _intOrNull(data['sample_rate']) ?? 24000,
              start: data['start'] == true,
              end: data['end'] == true,
              volume: _doubleOrNull(data['volume']),
              muted: data['muted'] == true,
              seq: seq,
              ts: ts,
            );
          } catch (_) {
            event = null; // 坏 base64：丢弃该帧
          }
        }
      case 'error':
        event = WsErrorEvent(
          code: _str(data['code']) ?? 'error',
          message: _str(data['message']) ?? text,
          seq: seq,
          ts: ts,
        );
      default:
        event = UnknownWsEvent(type: type, data: data, seq: seq, ts: ts);
    }
    if (event != null && !_events.isClosed) _events.add(event);
  }

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _reconnectTimer?.cancel();
    final socket = _socket;
    _socket = null;
    if (socket != null) {
      socket.onopen = null;
      socket.onmessage = null;
      socket.onerror = null;
      socket.onclose = null;
      try {
        socket.close();
      } catch (_) {
        // 已关闭
      }
    }
    _setStatus(WsStatus.closed);
    _events.close();
    _statuses.close();
  }
}

int? _intOrNull(Object? value) {
  if (value is num) return value.toInt();
  if (value is String) return int.tryParse(value);
  return null;
}

int _int(Object? value) => _intOrNull(value) ?? 0;

/// 宽容地解析 `double`（JSON 整数/浮点/字符串都可）。
///
/// 非有限值一律返回 `null`——调用方据此回退到本地计算，而不是拿 NaN 去驱动口型。
double? _doubleOrNull(Object? value) {
  final num? raw = switch (value) {
    num n => n,
    String s => num.tryParse(s),
    _ => null,
  };
  if (raw == null) return null;
  final d = raw.toDouble();
  return d.isFinite ? d : null;
}

String? _str(Object? value) => value is String ? value : null;
