import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import 'live2d_transport.dart';

/// 渲染面生命周期阶段（协议 v1 状态机）。
enum Live2DBridgePhase { loading, ready, error, destroyed }

/// 待发送命令（ready 前入队）。
class _Pending {
  const _Pending(this.type, this.json);
  final String type;
  final String json;
}

/// 渲染面的回执（`stage-ack`）。
///
/// 渲染面**对任何 `applied` 的下行消息**都回一条：
/// `{"type":"stage-ack","applied":true,"msg_type":<归一化后的类型>,
///   "scale":…,"offset_x":…,"offset_y":…}`
/// （`l2d-wasm-demo/src/main.rs:503-516`）。
///
/// **收条之前不得提示「已生效」**——这是 R2 定下的规矩：渲染面可能没应用成功，
/// 父页抢先说「已生效」就是在骗用户。
///
/// 两个必须知道的事实：
/// - 回执**没有 `version` 字段**（历史形状），所以不能按 v1 严格解析；
/// - `msgType` 是**归一化后**的名字：`sync`/`stage` → `stage-config`，
///   `mouth` → `audio-volume`，其余原样。
class StageAckEvent {
  const StageAckEvent({
    required this.applied,
    required this.msgType,
    this.scale,
    this.offsetX,
    this.offsetY,
  });

  /// 渲染面是否真的应用了。
  final bool applied;

  /// 归一化后的消息类型（**不是**你发出去的那个名字）。
  final String msgType;

  /// 应用后的实际缩放/偏移（**以它为准**，不要用本地值猜）。
  final double? scale;
  final double? offsetX;
  final double? offsetY;

  @override
  String toString() =>
      'StageAckEvent(applied: $applied, msgType: $msgType, scale: $scale, '
      'offset: $offsetX,$offsetY)';
}

/// 协议 v1 渲染面客户端。
///
/// 下行（Flutter → iframe）每条消息都是 JSON 字符串：
/// `{"version":1,"type":<sync|mouth|stage|destroy>,"payload":{...}}`。
///
/// 上行（iframe → Flutter）同构，但**必须容忍**历史遗留的
/// `{"type":"stage-ack",...}`（无 version）并忽略未知 type，绝不抛异常。
///
/// 状态机：loading →（收到 `ready`）→ ready；失败/超时 → error；
/// [destroy] → destroyed。ready 前的命令入队，ready 后按序 flush。
class Live2DBridge extends ChangeNotifier {
  Live2DBridge(this._transport) {
    _sub = _transport.events.listen(
      _onRaw,
      onError: (Object error) => _fail('渲染面传输错误：$error'),
    );
  }

  /// ready 等待上限：超过则给出可见错误（不静默白屏）。
  static const Duration readyTimeout = Duration(seconds: 12);

  /// mouth 节流：≤30Hz（33ms）。
  static const Duration mouthMinInterval = Duration(milliseconds: 33);

  /// 入队上限（超出丢最旧，避免无限堆积）。
  static const int maxQueue = 32;

  final Live2DTransport _transport;
  final List<_Pending> _queue = <_Pending>[];

  final StreamController<StageAckEvent> _acks =
      StreamController<StageAckEvent>.broadcast();
  StageAckEvent? _lastAck;

  /// 渲染面回执流（`stage-ack`）。
  Stream<StageAckEvent> get acks => _acks.stream;

  /// 最近一条回执（没收到过为 `null`）。
  StageAckEvent? get lastAck => _lastAck;

  StreamSubscription<String>? _sub;
  Timer? _readyTimer;
  Timer? _mouthTimer;
  DateTime? _lastMouthAt;
  double? _pendingMouth;

  Live2DBridgePhase _phase = Live2DBridgePhase.loading;
  String? _errorMessage;
  String? _model;
  double? _progress;
  int? _fps;
  bool _modelLoaded = false;
  bool _disposed = false;

  Live2DBridgePhase get phase => _phase;
  String? get errorMessage => _errorMessage;
  String? get model => _model;
  double? get progress => _progress;
  int? get fps => _fps;
  bool get isReady => _phase == Live2DBridgePhase.ready;
  bool get modelLoaded => _modelLoaded;
  int get queuedCount => _queue.length;

  /// 启动 ready 超时计时（由宿主在挂载 iframe 后调用）。
  void start() {
    _readyTimer?.cancel();
    _readyTimer = Timer(readyTimeout, () {
      if (_phase == Live2DBridgePhase.loading) {
        _fail(
          '渲染面 ${readyTimeout.inSeconds}s 内未就绪：请确认 GET /render 可达，'
          '且渲染端已实现协议 v1（会发 ready 帧）。',
        );
      }
    });
  }

  /// 协议 v1 `sync`（字段均可选）。
  ///
  /// `mouthSensitivity`：口型灵敏度，乘在渲染面「RMS → dB 映射」的结果上。
  /// 默认 1.0 是实测标定值（真实语音 p90 约开到 0.58）；渲染面会 clamp 到
  /// `[0, 4]`。字段**向后兼容**——不传即用渲染面默认值。
  Future<void> sendSync({
    String? model,
    double? scale,
    bool? dark,
    String? stageColor,
    int? tier,
    bool? clickEnabled,
    bool? paused,
    bool? lipSync,
    bool? idleEnabled,
    double? mouthSensitivity,
  }) {
    final payload = <String, Object?>{};
    if (model != null) payload['model'] = model;
    if (scale != null) payload['scale'] = scale;
    if (dark != null) payload['dark'] = dark;
    // 舞台纯色底（`#RRGGBB`）。渲染面会**校验**它，非法值回落 `dark` 的默认色。
    // 为什么底色必须由渲染面写：舞台是 iframe，iframe 内 canvas 自带不透明
    // 背景，会盖住父页画的任何颜色（见 `surface.rs::normalize_stage_color`）。
    if (stageColor != null) payload['stageColor'] = stageColor;
    if (tier != null) payload['tier'] = tier;
    // 渲染面读的是 `sync.clickEnabled`（`l2d-wasm-demo/src/main.rs:334`）——
    // 它管的是**拖动 / 滚轮 / 双击复位**，与「点击角色有没有反应」无关。
    // 所以 UI 文案是「允许拖动与缩放」，不是「点击互动」。
    if (clickEnabled != null) payload['clickEnabled'] = clickEnabled;
    if (paused != null) payload['paused'] = paused;
    if (lipSync != null) payload['lipSync'] = lipSync;
    if (idleEnabled != null) payload['idleEnabled'] = idleEnabled;
    if (mouthSensitivity != null && mouthSensitivity.isFinite) {
      payload['mouthSensitivity'] = mouthSensitivity;
    }
    return _enqueueOrSend('sync', payload);
  }

  /// 协议 v1 `stage`（暗色 / 缩放 / 档位）。
  Future<void> sendStage({bool? dark, double? scale, int? tier}) {
    final payload = <String, Object?>{};
    if (dark != null) payload['dark'] = dark;
    if (scale != null) payload['scale'] = scale;
    if (tier != null) payload['tier'] = tier;
    return _enqueueOrSend('stage', payload);
  }

  /// 协议 v1 `stage-zoom`：`dir` ∈ `in` / `out` / `reset`。
  ///
  /// 渲染面自己算缩放（±10%，clamp 0.5..2.0；`reset` 同时把偏移归零），
  /// 所以这里**不发具体数值**——发了也会被忽略（它只看 `dir`）。
  /// 应用后的真值从 [acks] 的 `scale` / `offsetX` / `offsetY` 取。
  Future<void> sendStageZoom(String dir) =>
      _enqueueOrSend('stage-zoom', <String, Object?>{'dir': dir});

  /// 协议 v1 `stage-bg`：自定义背景图（dataURL）；`null`/空串 = 清除。
  Future<void> sendStageBg(String? dataUrl) => _enqueueOrSend(
    'stage-bg',
    <String, Object?>{'dataUrl': dataUrl ?? ''},
  );

  /// 协议 v1 `mouth`（level 0..1，实时，节流 ≤30Hz）。
  Future<void> sendMouth(double level) {
    final value = level.isFinite ? _clamp01(level) : 0.0;
    final last = _lastMouthAt;
    final now = DateTime.now();
    if (last == null || now.difference(last) >= mouthMinInterval) {
      _emitMouth(value);
      return Future<void>.value();
    }
    // 节流窗口内：只保留最新值，窗口结束时补发（不丢最后一帧）。
    _pendingMouth = value;
    final wait = mouthMinInterval - now.difference(last);
    _mouthTimer ??= Timer(wait, () {
      _mouthTimer = null;
      final pending = _pendingMouth;
      _pendingMouth = null;
      if (pending != null) _emitMouth(pending);
    });
    return Future<void>.value();
  }

  /// 协议 v1 `destroy`：通知渲染面释放，并关闭桥。
  Future<void> destroy() async {
    if (_disposed) return;
    _readyTimer?.cancel();
    _mouthTimer?.cancel();
    _phase = Live2DBridgePhase.destroyed;
    notifyListeners();
    try {
      await _transport.send(
        jsonEncode(<String, Object?>{
          'version': 1,
          'type': 'destroy',
          'payload': <String, Object?>{},
        }),
      );
    } catch (error) {
      _errorMessage = '发送 destroy 失败：$error';
    }
    dispose();
  }

  void _emitMouth(double value) {
    _lastMouthAt = DateTime.now();
    unawaited(_enqueueOrSend('mouth', <String, Object?>{'level': value}));
  }

  Future<void> _enqueueOrSend(String type, Map<String, Object?> payload) {
    final json = jsonEncode(<String, Object?>{
      'version': 1,
      'type': type,
      'payload': payload,
    });
    if (_phase == Live2DBridgePhase.ready) return _send(json);
    if (type == 'mouth') {
      _queue.removeWhere((pending) => pending.type == 'mouth');
    }
    _queue.add(_Pending(type, json));
    while (_queue.length > maxQueue) {
      _queue.removeAt(0);
    }
    return Future<void>.value();
  }

  Future<void> _send(String json) async {
    if (_disposed) return;
    try {
      await _transport.send(json);
    } catch (error) {
      _fail('向渲染面发送失败：$error');
    }
  }

  Future<void> _flush() async {
    final pending = List<_Pending>.of(_queue);
    _queue.clear();
    for (final item in pending) {
      await _send(item.json);
    }
  }

  void _onRaw(String raw) {
    if (_disposed) return;
    Map<String, Object?> frame;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return;
      frame = Map<String, Object?>.from(decoded);
    } catch (_) {
      return; // 容忍坏帧 / 非 JSON：绝不抛异常
    }
    final type = frame['type'];
    if (type is! String) return; // 容忍无 type / 无 version 的遗留帧

    final rawPayload = frame['payload'];
    final payload = rawPayload is Map
        ? Map<String, Object?>.from(rawPayload)
        : const <String, Object?>{};

    switch (type) {
      case 'ready':
        _phase = Live2DBridgePhase.ready;
        _errorMessage = null;
        _readyTimer?.cancel();
        notifyListeners();
        unawaited(_flush());
      case 'loaded':
        _modelLoaded = true;
        // 渲染端历史上发 model，当前发 url；两者都接受。
        final name = payload['model'] ?? payload['url'];
        if (name is String) _model = name;
        notifyListeners();
      case 'progress':
        final value = payload['progress'];
        if (value is num) {
          _progress = value.toDouble();
          notifyListeners();
        }
      case 'error':
        final message = payload['message'];
        _fail(message is String && message.isNotEmpty ? message : '渲染面报告未知错误');
      case 'fps':
        final value = payload['fps'];
        if (value is num) {
          _fps = value.round();
          notifyListeners();
        }
      case 'stage-ack':
        // 回执是**历史形状**（没有 version），字段直接挂在根上而不是 payload。
        final ack = StageAckEvent(
          applied: frame['applied'] == true,
          msgType: frame['msg_type'] is String
              ? frame['msg_type']! as String
              : '',
          scale: frame['scale'] is num ? (frame['scale']! as num).toDouble() : null,
          offsetX: frame['offset_x'] is num
              ? (frame['offset_x']! as num).toDouble()
              : null,
          offsetY: frame['offset_y'] is num
              ? (frame['offset_y']! as num).toDouble()
              : null,
        );
        _lastAck = ack;
        if (!_acks.isClosed) _acks.add(ack);
        notifyListeners();
      default:
        break; // 未知 type → 忽略（绝不抛）
    }
  }

  void _fail(String message) {
    if (_disposed) return;
    _errorMessage = message;
    _phase = Live2DBridgePhase.error;
    _readyTimer?.cancel();
    _mouthTimer?.cancel();
    notifyListeners();
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    _readyTimer?.cancel();
    _mouthTimer?.cancel();
    unawaited(_acks.close());
    unawaited(_sub?.cancel());
    unawaited(_transport.dispose());
    super.dispose();
  }
}

double _clamp01(double value) {
  if (value < 0) return 0;
  if (value > 1) return 1;
  return value;
}
