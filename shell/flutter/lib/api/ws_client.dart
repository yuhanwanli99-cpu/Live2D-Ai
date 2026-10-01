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
  Timer? _watchdog;

  /// 最近一次**收到任何一帧**的时刻（= 服务端还活着的证据）。
  ///
  /// 名字按「心跳」取（服务端每 10 s 一条 `heartbeat`，那是周期性存活信号），
  /// 但更新面覆盖**每一帧**：流式聊天中 `text_delta` 本身就是「链路活着」的
  /// 证明，只认 heartbeat 会把活跃连接误判成静默。
  ///
  /// 半开连接（休眠唤醒 / NAT 静默丢包 / 服务端被 SIGKILL）下
  /// `readyState` 仍是 OPEN，**只有这个时刻能证明通道已经废了**。
  DateTime? _lastHeartbeatAt;

  /// 已连续失败的连接次数（成功连上即清零）。重连延迟由它推导。
  int _attempt = 0;
  bool _disposed = false;
  WsStatus _status = WsStatus.idle;

  /// 看门狗巡检周期。
  ///
  /// 取值理由：必须**远小于** [kWsHeartbeatSuspectAfter]（30 s），否则「发现
  /// 半开」的延迟会被巡检周期主导；也不能太密（每 5 s 一次读时钟，可忽略）。
  static const Duration _watchdogTick = Duration(seconds: 5);

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
      // 新连接从**此刻**开始重新计静默：不清掉旧时刻的话，看门狗会在重开后
      // 立刻用 90 s 前的旧值判死，于是变成「重开→立刻重开」的死循环。
      _lastHeartbeatAt = DateTime.now();
      _setStatus(WsStatus.connected);
      _startWatchdog();
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
      _watchdog?.cancel();
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
    // **第二重判据（2026-10-01，F-0008-1）**：`readyState == OPEN` 不等于
    // 「话还能到我这里」——半开连接下它一直是 OPEN。所以除了问 socket 的
    // 状态机，还问「最近听到过动静吗」（判据 = `ws_liveness.dart` 的纯函数）。
    // 已经静默到死链档（≈90 s）时，这个 socket 一律不信：摘掉重开，
    // 反正 POST 走的是另一条 HTTP 连接，「发出去没回应」正是这么来的。
    final bool trustworthy =
        socket != null &&
        isChannelTrustworthy(
          readyState: socket.readyState,
          liveness: heartbeatLiveness(
            lastHeartbeatAt: _lastHeartbeatAt,
            now: DateTime.now(),
          ),
        );
    if (trustworthy) return;
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

  // ─────────────────────────────────────────────────────────────────────
  // 心跳看门狗（2026-10-01，审计 F-0008-1）
  //
  // 半开连接（笔记本休眠/唤醒、NAT/代理静默丢包、服务端被 SIGKILL 而没有
  // FIN/RST 送达）下，浏览器的 `readyState` **仍然是 OPEN**——规范只在
  // 「尝试收发并失败」时才改状态，而本客户端从不发送帧。于是 `onclose` 与
  // `onerror` 都不会来，`_status` 一直写「已连接」，界面的恢复出口
  //（`mustReleaseTurnOnWsLoss` 的前提是 `status.isProblem`）永远不成立 ⇒
  // 「`POST /api/v1/chat` 成功但收不到回复」可以永久挂着。
  //
  // 服务端每 10 s 发一条心跳，那是**已经送到前端**的存活信号；下面这个周期
  // 任务就是它的消费者：静默超过阈值即摘掉重开。判据是纯函数
  //（`ws_liveness.dart`），本文件只做「巡检 + 处置」。
  // ─────────────────────────────────────────────────────────────────────

  void _startWatchdog() {
    _watchdog?.cancel();
    _watchdog = Timer.periodic(_watchdogTick, (_) => _checkHeartbeatLiveness());
  }

  /// 巡检一次：静默到期就把「看着还 OPEN」的 socket 当作半开连接处置。
  void _checkHeartbeatLiveness() {
    if (_disposed) return;
    final web.WebSocket? socket = _socket;
    // 没有 socket：退避重连正在管，不插手。
    if (socket == null) return;
    // **只对已经连上的 socket 判半开**（复核 F-V1-1，2026-10-01）：
    // - `CONNECTING`：静默是「还没开口」而不是「哑了」——`_lastHeartbeatAt`
    //   属于**上一条**连接。判死它会打断一次正在进行的建连（`ensureConnected`
    //   摘掉旧 socket 后会立刻 `_open()` 一条新的，而看门狗是**不随摘除取消**的，
    //   所以这条路径真实可达）。
    // - CLOSING/CLOSED：`onclose` 或退避计时器会管，这里再动一次只会处置两遍。
    if (socket.readyState != kSocketOpen) return;
    final HeartbeatLiveness liveness = heartbeatLiveness(
      lastHeartbeatAt: _lastHeartbeatAt,
      now: DateTime.now(),
    );
    if (liveness == HeartbeatLiveness.online) return;
    _recoverFromHalfOpen(socket);
  }

  /// 半开恢复：**先宣布通道有问题**，再摘掉僵尸并立刻重开。
  ///
  /// 顺序有讲究（与 `ChatController.stop()` 里「先本地收口再发请求」同一条教训）：
  /// ① `disconnected` 是 `WsStatus.isProblem` ⇒ `ChatController` 的
  ///    `mustReleaseTurnOnWsLoss` 会就地收口在飞的一轮（那条收口帧**永远不会
  ///    来**）；若跳过这一步直接重开（只经过 `connecting`，它不是 problem），
  ///    输入框会锁死在「停止本轮」而 `send()` 静默 return——正是用户报的
  ///    「发出去没回应」的最坏形态；
  /// ② 再摘掉重开，且**不走退避**（退避上限 30 s，而用户可能正等着回复）。
  void _recoverFromHalfOpen(web.WebSocket socket) {
    _setStatus(WsStatus.disconnected);
    _reconnectTimer?.cancel();
    _watchdog?.cancel();
    if (identical(_socket, socket)) _socket = null;
    _detach(socket);
    _attempt = 0;
    _open();
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
    // **任何一个到达的帧都是存活证据**（心跳是周期性那一种）：看门狗据此判
    // 「半开」。放在解析**之前**——解析不懂的帧同样证明链路能把字节送到。
    _lastHeartbeatAt = DateTime.now();
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
    _watchdog?.cancel();
    _watchdog = null;
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
