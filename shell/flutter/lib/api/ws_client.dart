/// `/ws/state` 客户端：**只负责连接生命周期**。
///
/// 帧解析（「帧字符串 → 领域事件」）抽到了 `ws_frame.dart`——那是纯逻辑、
/// 可在 VM 上单测；本文件依赖 `package:web`，测不了。分层理由见规格 §10.2，
/// 以及 `ws_frame.dart` 头注里「解析缺陷为什么能长期存活」的说明。
library;

import 'dart:async';
import 'dart:js_interop';

import 'package:web/web.dart' as web;

import 'ws_frame.dart';
import 'ws_liveness.dart';
import 'ws_status.dart';

export 'ws_frame.dart';
export 'ws_liveness.dart';
export 'ws_status.dart';

/// `/ws/state` 客户端。
///
/// **真实协议是单向事件流**：服务端 `ws.rs` / `connection.rs` 明确不读
/// 客户端帧，连接后立即发 `subscribe_ack`，客户端**不**发 subscribe / ping
/// （发了也会被 tungstenite 静默丢弃）。命令一律走 HTTP。
///
/// 重连：指数退避 1s→2s→4s→8s→16s→30s 上限（算术见 [backoffForAttempt]）。
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

  /// 已连续失败的连接次数（成功连上即清零）。重连延迟由它推导。
  int _attempt = 0;
  bool _disposed = false;
  WsStatus _status = WsStatus.idle;

  Stream<WsEvent> get events => _events.stream;
  Stream<WsStatus> get statuses => _statuses.stream;
  WsStatus get status => _status;

  static String _defaultUrl() {
    const String override = String.fromEnvironment('WS_URL');
    if (override.isNotEmpty) return override;
    final Uri base = Uri.base;
    final String scheme = base.scheme == 'https' ? 'wss' : 'ws';
    final String host = base.host.isEmpty ? '127.0.0.1' : base.host;
    final String port = base.hasPort ? ':${base.port}' : '';
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
      _attempt = 0;
      _setStatus(WsStatus.connected);
    }).toJS;
    socket.onmessage = ((web.MessageEvent event) {
      final JSAny? data = event.data;
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
  ///
  /// **2026-09-11 修**：这里的判据曾经是 `_socket != null`——把「有没有
  /// socket 对象」当成了「连接还活着」。但 `close()` 之后 `onclose` 是
  /// **异步**回调，这中间 `_socket` 仍非 null 而连接已经废了；而
  /// `_handleText` 里 `shutdown_ready` 走的正是「先 close 再退避重连」。
  /// 于是这份**专为「发出去没回应」而写的保险**，恰好在最需要它的那一刻
  /// 变成空操作。现在按 `readyState` 判（判据抽在 `ws_liveness.dart`，
  /// 纯逻辑、有回归）。
  void ensureConnected() {
    if (_disposed) return;
    final web.WebSocket? socket = _socket;
    if (socket != null && isLiveSocket(socket.readyState)) return;
    if (socket != null) {
      // 僵尸对象：摘掉它并立刻重开（不等 `onclose` / 退避计时器——
      // 用户正在等回复，退避的 30s 上限在这里是不可接受的）。
      _detach(socket);
      _socket = null;
    }
    _reconnectTimer?.cancel();
    _attempt = 0;
    _open();
  }

  /// 摘掉一个 socket 的全部回调并关闭它（不再由它驱动任何状态）。
  void _detach(web.WebSocket socket) {
    socket.onopen = null;
    socket.onmessage = null;
    socket.onerror = null;
    socket.onclose = null;
    try {
      socket.close();
    } catch (_) {
      // 已关闭 / 已释放
    }
  }

  void _scheduleReconnect() {
    if (_disposed) return;
    _setStatus(WsStatus.disconnected);
    _reconnectTimer?.cancel();
    final Duration delay = backoffForAttempt(_attempt);
    _attempt++;
    _reconnectTimer = Timer(delay, _open);
  }

  void _setStatus(WsStatus status) {
    _status = status;
    if (!_statuses.isClosed) _statuses.add(status);
  }

  /// 解析并下发一帧。
  ///
  /// `shutdown_ready` 的**连接处置**留在这里（解析层刻意无副作用）；事件本身
  /// 照旧继续下发，让 UI 知道发生过什么。
  void _handleText(String text) {
    final WsEvent? event = parseWsFrame(text);
    if (event == null) return;
    if (event is RuntimeStatusEvent && event.event == 'shutdown_ready') {
      _socket?.close();
      _scheduleReconnect();
    }
    if (!_events.isClosed) _events.add(event);
  }

  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _reconnectTimer?.cancel();
    final web.WebSocket? socket = _socket;
    _socket = null;
    if (socket != null) {
      _detach(socket);
    }
    _setStatus(WsStatus.closed);
    _events.close();
    _statuses.close();
  }
}
