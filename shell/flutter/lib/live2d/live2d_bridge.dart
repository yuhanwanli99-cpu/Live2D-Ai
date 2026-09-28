/// 协议 v1 渲染面客户端（**桥**）：下行帧编码、上行帧解析、ready 前的排队与
/// flush、`stage-ack` / 事件级回执两条上行通道。
///
/// # 行数（**豁免带内**：500 < N ≤ 1000，拆分归 Stage C3）
///
/// 本文件在本轮（2026-09-28 · R6-a2）之后约 550 行。理由：帧表 + 状态机 +
/// 队列 + 回执解析本来就是一个整体，硬拆只会多出一层纯转发的中间层；
/// 本轮只加不减（F-0002-1 的同值去重）。写在这里是**如实**，不是豁免申请。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import 'live2d_transport.dart';
import 'render_events.dart';

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
  final StreamController<String> _modelLoads =
      StreamController<String>.broadcast();

  /// 渲染面**事件级回执**流（协议 §7：四条 ack + segment-ended，O13 冻结名）。
  ///
  /// 与 [acks]（历史形状的 `stage-ack`）是两条通道：后者是「下行消息应用回声」，
  /// 前者是「表演事件」，去向是汇入日志文本喂导演（§7.3）。**不做每帧状态流、
  /// 不建前端镜像**（V8）。
  final StreamController<RenderEvent> _renderEvents =
      StreamController<RenderEvent>.broadcast();

  /// 渲染面回执流（`stage-ack`）。
  Stream<StageAckEvent> get acks => _acks.stream;

  /// 渲染面事件流（见 [_renderEvents]）。
  Stream<RenderEvent> get renderEvents => _renderEvents.stream;

  /// 渲染面**成功装载模型**的流（`loaded` 帧携带的 model / url）。
  ///
  /// 换模的唯一权威回执：`sync.payload.model` 发出去之后，渲染面要么回
  /// `loaded`（换成了），要么回 `error`（没换成）。**收条之前不得说「已切换」**
  /// ——这是 `live2d_stage.dart` 里那条 R2 规矩的同一个道理。
  Stream<String> get modelLoads => _modelLoads.stream;

  /// 最近一条回执（没收到过为 `null`）。
  StageAckEvent? get lastAck => _lastAck;

  StreamSubscription<String>? _sub;
  Timer? _readyTimer;
  Timer? _mouthTimer;
  DateTime? _lastMouthAt;
  double? _pendingMouth;

  /// 最近一次**真的交给传输面**的 `stage-bg` 值（`''` = 清除）。
  ///
  /// 只记「ready 之后当场发出去」的那一次：ready 前的发送是**入队**，
  /// 而队列会被 [maxQueue] 截断（`_queue.removeAt(0)`）——把「排过队」
  /// 当成「已送达」就会出现「用户看着有壁纸，重挂 iframe 之后永远是黑底」。
  /// 见 [sendStageBg] 与 `data/background_decode_cache.dart` 同族的 F-0002-1。
  String? _lastSentStageBg;

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

  /// 协议 v1 `sync` 换模型，并**等到渲染面回执**才返回（R2 规矩）。
  ///
  /// 返回 `true` = 渲染面回了 `loaded`，且报的正是我们发出去的那个 url（**真换了**）；
  /// `false` = 超时、渲染面报错，或回执报的是**别的** url（**没换成**）。
  ///
  /// 为什么必须等：`POST /models/{id}/activate` 只改后端 registry，真正换皮发生在
  /// 这个 iframe 里。只发不等，界面就会在舞台还没换的时候说「已切换」——
  /// 正是 rc.2 要消灭的「激活了但没换皮」。
  Future<bool> swapModel(
    String url, {
    Duration timeout = const Duration(seconds: 15),
  }) async {
    if (url.isEmpty || _disposed || _phase == Live2DBridgePhase.error) {
      return false;
    }
    // 先挂监听再发：本地 iframe 的回执可能比 await 还快。
    final Future<String> loaded = _modelLoads.stream.first;
    await sendSync(model: url);
    try {
      return await loaded.timeout(timeout) == url;
    } on TimeoutException {
      return false;
    }
  }

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
    Map<String, double>? actionScales,
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
    // 动作幅度倍率（2026-09-16）：`{head, body, expression}`，渲染面钳 [0.2, 2.2]。
    // 缺省不下发（渲染面用自己的出厂默认）；只发三个键都有限的完整对象。
    if (actionScales != null && actionScales.length == 3) {
      final bool allFinite = actionScales.values.every(
        (double v) => v.isFinite,
      );
      if (allFinite) payload['actionScales'] = actionScales;
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
  ///
  /// # 同值不重发（F-0002-1，2026-09-28）
  ///
  /// 这一帧背的是**整张图的 base64**（协议预算 ≤1.5 M 字符），而渲染面收到它
  /// 要重跑 `base64 → Blob → createImageBitmap → write_texture`。过去这里无条件
  /// 入队/发送，于是「拖一次音量滑杆」= 每帧整串重发一次。
  ///
  /// 去重**只去同值**：
  ///
  /// - 值不同（换图 / 清图）照发——这是资产契约 A6 的舞台背景透传；
  /// - **桥是新的时候一定照发**：记忆是实例字段，而 `Live2DStage._attach`
  ///   每次挂桥（首帧 / retry 重建 iframe）都新建一个 `Live2DBridge`
  ///   并重发 `stageImage`，所以「重挂后不补发」这种事故不会由去重引入；
  /// - **ready 前排队的那一次不算「发出去过」**（队列可能被截断）：
  ///   宁可 ready 之后多补一帧，也不让渲染面少一张图。
  Future<void> sendStageBg(String? dataUrl) {
    final String value = dataUrl ?? '';
    if (_phase == Live2DBridgePhase.ready) {
      if (_lastSentStageBg == value) return Future<void>.value();
      // 记在**真正交给传输面之前**：下面这条路径是同步 `_send`（await 后失败会
      // 走 `_fail` 把阶段打到 error，那时整条下行都已经不可信）。
      _lastSentStageBg = value;
    }
    return _enqueueOrSend('stage-bg', <String, Object?>{'dataUrl': value});
  }

  /// 协议 v1 preset（2026-09-15 / P0-1）。
  ///
  /// `id` 是稳定契约（`smile` / `unhappy` / `nod` …；旧 `expr_*` 仍按别名解析一版）；
  /// `"none"` = **立即撤销**当前预设（两个槽一起清）。
  /// 可选 `source`（`debug` / `director`，仅显示用）、`ttl_ms`、`intensity` ——
  /// 缺省时渲染面用自己的表（与旧 `{id}` 逐值等价）。
  ///
  /// 渲染面按自己的预设表把它翻成模型参数（表情保持 / 短动作包络，写到
  /// `final_override` 层）；**不认识的 id 静默忽略**，模型能力不足不会报错。
  Future<void> sendPreset(
    String id, {
    String? source,
    double? ttlMs,
    double? intensity,
    String? field,
    double? x,
    double? y,
    double? z,
    bool? hold,
    String? at,
    int? seq,
    int? epoch,
    int? sentenceSeq,
  }) {
    final payload = <String, Object?>{'id': id};
    if (source != null && source.isNotEmpty) payload['source'] = source;
    if (ttlMs != null && ttlMs > 0) payload['ttl_ms'] = ttlMs;
    if (intensity != null) payload['intensity'] = intensity;
    // ── v1 表演字段（编排者冻结的集成细节，C2 记入报告）──
    //
    // 消息类型仍是既有 `preset`（**不新造消息类型**）；旧键 {id,intensity,
    // ttl_ms,source} 一个不动，**只增**下列可选键。**缺 field 时语义与今天
    // 逐字相同**（preset_id 旧路径，不删不换不改）。
    if (field != null && field.isNotEmpty) payload['field'] = field;
    if (x != null && x.isFinite) payload['x'] = x;
    if (y != null && y.isFinite) payload['y'] = y;
    if (z != null && z.isFinite) payload['z'] = z;
    if (hold != null) payload['hold'] = hold;
    if (at != null && at.isNotEmpty) payload['at'] = at;
    if (seq != null) payload['seq'] = seq;
    if (epoch != null) payload['epoch'] = epoch;
    if (sentenceSeq != null) payload['sentence_seq'] = sentenceSeq;
    return _enqueueOrSend('preset', payload);
  }

  /// 协议 v1 `stage-clock`（协议 §6.5 / O13 冻结 wire 名）：
  /// `{version:1,type:"stage-clock",payload:{seg,pos_ms,playing}}`。
  ///
  /// 渲染面用它**替代 performance.now()** 当 `now_ms`——编排的时间基准是音频
  /// 播放时钟（V7 §6.1）。[seg] 为 `null` 时照发（渲染面可据此判断「还没有
  /// 段」）；`pos_ms` 钳到 ≥ 0。
  Future<void> sendStageClock({
    int? seg,
    required int posMs,
    required bool playing,
  }) {
    return _enqueueOrSend('stage-clock', <String, Object?>{
      'seg': seg,
      'pos_ms': posMs < 0 ? 0 : posMs,
      'playing': playing,
    });
  }

  /// 协议 v1 mouth（level 0..1，实时，节流 ≤30Hz）。
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
        if (name is String) {
          _model = name;
          // 换模等待方按这个名字确认「换的正是我要的那个」。
          if (!_modelLoads.isClosed) _modelLoads.add(name);
        }
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
      case 'preset-applied':
      case 'preset-replaced':
      case 'preset-expired':
      case 'preset-dropped':
      case 'segment-ended':
        // 渲染面事件级 ack（协议 §7）。字段在 `payload` 里；**表外字段忽略**
        // （V11）——解析器只取 §7.2 声明的键。
        final RenderEvent? event = parseRenderEvent(type, payload);
        if (event != null && !_renderEvents.isClosed) {
          _renderEvents.add(event);
          notifyListeners();
        }
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
    unawaited(_modelLoads.close());
    unawaited(_renderEvents.close());
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
