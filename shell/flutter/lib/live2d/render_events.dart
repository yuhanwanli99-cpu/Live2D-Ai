/// 渲染面 ack 事件（协议 §7，wire 名冻结见 O13）。
///
/// # 这是什么 / 为什么单独成文件
///
/// 渲染面（iframe）在事件级回执五条消息（**不做每帧状态流、不做前端镜像**，V8）：
///
/// | wire type | 语义 |
/// | --- | --- |
/// | preset-applied | cue 生效（做了什么就回什么） |
/// | preset-replaced | 同槽被新 cue 顶掉（旧 cue 的结束点） |
/// | preset-expired | 非 hold cue 到点（「动作做完」的来源） |
/// | preset-dropped | 皮套缺该参数（**最该暴露的事实**，不静默） |
/// | segment-ended | 某段音频播完（唤醒条件的另一半） |
///
/// 它们的去向是「渲染面 → 前端 → **汇入日志文本** → 喂导演」（协议 §7.3）。
/// 解析与文本化都是**纯逻辑**，因此放这里在 VM 上可回归——它是
/// 「前端遇未知字段忽略」（V11）与「ack 进导演日志」两条判据的落点。
library;

import 'dart:convert';

import '../api/ws_frame.dart';

/// 五条渲染面事件（O13 冻结名，三轨共用，不得各自起名）。
enum RenderAckKind { applied, replaced, expired, dropped, segmentEnded }

/// wire type → 语义。**这就是 O13 的冻结表**；表外的 type 一律忽略。
const Map<String, RenderAckKind> kRenderAckTypes = <String, RenderAckKind>{
  'preset-applied': RenderAckKind.applied,
  'preset-replaced': RenderAckKind.replaced,
  'preset-expired': RenderAckKind.expired,
  'preset-dropped': RenderAckKind.dropped,
  'segment-ended': RenderAckKind.segmentEnded,
};

/// 一条渲染面事件（payload 见协议 §7.2）。
///
/// **未知字段一律忽略**（V11）：只取下面这些键，其余键既不报错也不保存。
class RenderEvent {
  const RenderEvent({
    required this.kind,
    required this.type,
    this.epoch,
    this.tsMs,
    this.seq,
    this.field,
    this.id,
    this.x,
    this.y,
    this.z,
    this.intensity,
    this.clamped = false,
    this.degraded = false,
    this.reason,
    this.seg,
    this.ttlMs,
  });

  final RenderAckKind kind;

  /// 原始 wire type（preset-applied …），日志文本直接用。
  final String type;
  final int? epoch;
  final int? tsMs;
  final int? seq;
  final String? field;
  final String? id;
  final double? x;
  final double? y;
  final double? z;
  final int? intensity;
  final bool clamped;
  final bool degraded;
  final String? reason;
  final int? seg;

  /// 可选：渲染面若带上生效时长（§7.2 未列，缺省即 null）。
  final int? ttlMs;
}

/// 解析一条渲染面事件；**表外 type 返回 null**（忽略，不抛）。
///
/// 宽容规则（V11）：缺字段 → 对应 getter 为 null；类型不对 → 当缺字段；
/// 未知字段 → 直接丢弃。
RenderEvent? parseRenderEvent(String type, Map<String, Object?> payload) {
  final RenderAckKind? kind = kRenderAckTypes[type];
  if (kind == null) return null;
  return RenderEvent(
    kind: kind,
    type: type,
    epoch: _intOrNull(payload['epoch']),
    tsMs: _intOrNull(payload['ts_ms']),
    seq: _intOrNull(payload['seq']),
    field: _str(payload['field']),
    id: _str(payload['id']),
    x: _doubleOrNull(payload['x']),
    y: _doubleOrNull(payload['y']),
    z: _doubleOrNull(payload['z']),
    intensity: _intOrNull(payload['intensity']),
    clamped: payload['clamped'] == true,
    degraded: payload['degraded'] == true,
    reason: _str(payload['reason']),
    seg: _intOrNull(payload['seg']),
    ttlMs: _intOrNull(payload['ttl_ms']),
  );
}

/// 把一条事件拼成**给导演看的一行**（协议 §7.3 的「日志文本」）。
///
/// **不含正文**：只带字段名 / 轴值 / 强度 / 钳位与降级标志（§7.2 的
/// reason 是参数名，不是台词）。
String formatRenderEvent(RenderEvent e) {
  final StringBuffer b = StringBuffer('[render] ');
  b.write(e.type);
  if (e.seg != null) b.write(' seg=${e.seg}');
  if (e.seq != null) b.write(' seq=${e.seq}');
  if (e.field != null) b.write(' field=${e.field}');
  if (e.id != null && e.id!.isNotEmpty) b.write(' id=${e.id}');
  if (e.x != null) b.write(' x=${e.x!.toStringAsFixed(3)}');
  if (e.y != null) b.write(' y=${e.y!.toStringAsFixed(3)}');
  if (e.z != null) b.write(' z=${e.z!.toStringAsFixed(3)}');
  if (e.intensity != null) b.write(' intensity=${e.intensity}');
  b.write(' clamped=${e.clamped}');
  b.write(' degraded=${e.degraded}');
  if (e.reason != null && e.reason!.isNotEmpty) {
    b.write(' reason=${e.reason}');
  }
  return b.toString();
}

/// 喂导演的**有界**日志文本（渲染面事件按到达顺序累积）。
class DirectorEventLog {
  DirectorEventLog({this.maxLines = 200, this.onEvent});

  /// 上限：只保留最近 [maxLines] 行（渲染面事件是长期流，无界会吃内存）。
  final int maxLines;

  /// 每条事件落地后的**旁路回调**（缺省 null = 无旁路，行为与改动前逐字一致）。
  ///
  /// 阶段5 W5a 用它把渲染面 ack **同时**喂进「导演可观测」的 B/C 栏
  /// （`DirectorObserverFeed`）：宿主仍只写 onRenderEvent: _directorLog.add
  /// （既有接线判据），观测是 add 的旁路，日志文本本身一行不变。
  final void Function(RenderEvent event)? onEvent;

  final List<String> _lines = <String>[];

  List<String> get lines => List<String>.unmodifiable(_lines);
  int get length => _lines.length;
  bool get isEmpty => _lines.isEmpty;

  /// 整段文本（换行分隔）；空日志 = 空串。
  String get text => _lines.join('\n');

  void add(RenderEvent event) {
    _lines.add(formatRenderEvent(event));
    while (_lines.length > maxLines) {
      _lines.removeAt(0);
    }
    onEvent?.call(event);
  }

  void clear() => _lines.clear();
}

int? _intOrNull(Object? value) {
  if (value is num) return value.toInt();
  if (value is String) return int.tryParse(value);
  return null;
}

double? _doubleOrNull(Object? value) {
  final num? raw = switch (value) {
    num n => n,
    String s => num.tryParse(s),
    _ => null,
  };
  if (raw == null) return null;
  final double d = raw.toDouble();
  return d.isFinite ? d : null;
}

String? _str(Object? value) => value is String ? value : null;

// ───────────────────────── 阶段5 W5a：导演可观测（设置 → 开发工具）的纯逻辑
//
// 本段只做**纯逻辑**（不 import Flutter）：观测记录、有界环形缓冲、preset 请求
// 记录与「请求 → 生效」配对文本、sentence_ready 日志行解析、D 栏句子记录。
// 单例 hub（ChangeNotifier）住在 settings/sections/director_observer_section.dart；
// 观测缓冲只住内存，**不写任何持久化**（本栏的红线）。

/// 观测缓冲的默认上限（阶段5 §2 B 栏冻结：上限 200）。
const int kObserverBufferCapacity = 200;

/// 「生效」侧的两条渲染面 ack（C 栏配对只看这两条）。
const Set<RenderAckKind> kPresetAckKinds = <RenderAckKind>{
  RenderAckKind.applied,
  RenderAckKind.dropped,
};

/// B 栏覆盖的已知 wire type（**排序用**；未知类型照记，type 用 wire 名）。
const List<String> kObserverKnownTypes = <String>[
  'action_cue',
  'text_delta',
  'text_fallback',
  'turn_state',
  'error',
  'runtime_status',
  'stage-clock',
  'preset-applied',
  'preset-replaced',
  'preset-expired',
  'preset-dropped',
  'segment-ended',
];

/// 一条通用观测记录（B 栏）：时间戳 + wire type / 来源 + 一行文本。
class ObserverRecord {
  const ObserverRecord({
    required this.at,
    required this.type,
    required this.source,
    required this.text,
    this.epoch,
    this.seq,
  });

  /// 到达时刻（本机时钟，只用于显示；不落盘）。
  final DateTime at;

  /// wire type：渲染面 ack 用 O13 冻结名，WS 帧用帧的 type（未知类型照记）。
  final String type;

  /// 来源：ws（服务端 WS 帧）/ render（渲染面 ack）/ audio（音频时钟采样）。
  final String source;

  /// 一行文本（ack 日志文本**不含正文**；正文只在 D 栏）。
  final String text;

  final int? epoch;
  final int? seq;
}

/// 时间戳的显示形式（HH:MM:SS.mmm）。纯函数。
String formatObserverTime(DateTime at) {
  String two(int v) => v.toString().padLeft(2, '0');
  return '${two(at.hour)}:${two(at.minute)}:${two(at.second)}'
      '.${at.millisecond.toString().padLeft(3, '0')}';
}

/// 有界环形缓冲：只保留最近 [capacity] 条（第 capacity+1 条挤掉最旧）。
///
/// 纯逻辑、无 IO：容量回归（cap=200）与「按类型过滤」都在 VM 上单测。
class ObserverBuffer {
  ObserverBuffer({this.capacity = kObserverBufferCapacity})
    : assert(capacity > 0);

  final int capacity;
  final List<ObserverRecord> _records = <ObserverRecord>[];

  List<ObserverRecord> get records =>
      List<ObserverRecord>.unmodifiable(_records);
  int get length => _records.length;
  bool get isEmpty => _records.isEmpty;

  void add(ObserverRecord record) {
    // 阶段5 维护者补丁（D44）：stage-clock 是 **30ms 的连续信号**，不是事件。
    // 逐条入环会让 200 条容量在约 6 秒内被时钟灌满，action_cue / ack 全被挤掉，
    // B 栏实际不可用。这里按 (type, seq) **折叠**：同一段只保留最新一条。
    if (record.type == 'stage-clock') {
      _records.removeWhere(
        (ObserverRecord r) => r.type == record.type && r.seq == record.seq,
      );
    }
    _records.add(record);
    while (_records.length > capacity) {
      _records.removeAt(0);
    }
  }

  void clear() => _records.clear();

  /// 按 wire type 过滤；[types] 为 null / 空集 = 不过滤（全部）。
  List<ObserverRecord> filterByTypes(Iterable<String>? types) {
    if (types == null) return records;
    final Set<String> wanted = types.where((String t) => t.isNotEmpty).toSet();
    if (wanted.isEmpty) return records;
    return List<ObserverRecord>.unmodifiable(
      _records.where((ObserverRecord r) => wanted.contains(r.type)),
    );
  }
}

/// 一条**前端下发的 preset 请求**（C 栏左列）。
class PresetRequest {
  const PresetRequest({
    required this.id,
    required this.source,
    required this.ts,
    this.intensity,
    this.field,
    this.x,
    this.y,
    this.z,
    this.hold,
    this.at,
    this.seq,
    this.sentenceSeq,
    this.epoch,
  });

  final String id;
  final String source;
  final DateTime ts;
  final double? intensity;
  final String? field;
  final double? x;
  final double? y;
  final double? z;
  final bool? hold;

  /// 锚点（now / seg:N / after_prev）。
  final String? at;
  final int? seq;
  final int? sentenceSeq;
  final int? epoch;
}

/// 「请求 → 生效」配对结果（两条可直接上屏的文本）。
class PresetOverridePair {
  const PresetOverridePair({
    required this.request,
    required this.applied,
    this.ack,
  });

  final String request;
  final String applied;

  /// 配到的渲染面 ack；null = 尚无 ack。
  final RenderEvent? ack;

  bool get hasAck => ack != null;
}

/// C 栏左列文本：一行里带上请求的 intensity 与各轴值。
String formatPresetRequestLine(PresetRequest r) {
  final StringBuffer b = StringBuffer('请求');
  if (r.field != null && r.field!.isNotEmpty) b.write(' field=${r.field}');
  if (r.id.isNotEmpty && r.id != 'none') b.write(' id=${r.id}');
  if (r.intensity != null) {
    b.write(' intensity=${_trimNumber(r.intensity!)}');
  }
  if (r.x != null) b.write(' x=${r.x!.toStringAsFixed(2)}');
  if (r.y != null) b.write(' y=${r.y!.toStringAsFixed(2)}');
  if (r.z != null) b.write(' z=${r.z!.toStringAsFixed(2)}');
  if (r.hold != null) b.write(' hold=${r.hold}');
  if (r.at != null && r.at!.isNotEmpty) b.write(' at=${r.at}');
  if (r.seq != null) b.write(' seq=${r.seq}');
  if (r.sentenceSeq != null) b.write(' sentence_seq=${r.sentenceSeq}');
  if (r.epoch != null) b.write(' epoch=${r.epoch}');
  b.write(' source=${r.source}');
  return b.toString();
}

/// C 栏右列文本：渲染面 ack 的**最终值 + 标志**；没有 ack 就如实写出来。
String formatPresetAppliedLine(RenderEvent? e) {
  if (e == null) return '生效 尚无 ack';
  final StringBuffer b = StringBuffer('生效');
  if (e.x != null) b.write(' x=${e.x!.toStringAsFixed(3)}');
  if (e.y != null) b.write(' y=${e.y!.toStringAsFixed(3)}');
  if (e.z != null) b.write(' z=${e.z!.toStringAsFixed(3)}');
  if (e.id != null && e.id!.isNotEmpty) b.write(' id=${e.id}');
  b.write(' clamped=${e.clamped}');
  b.write(' degraded=${e.degraded}');
  if (e.reason != null && e.reason!.isNotEmpty) {
    b.write(' reason=${e.reason}');
  }
  b.write(' type=${e.type}');
  if (e.field != null && e.field!.isNotEmpty) b.write(' field=${e.field}');
  return b.toString();
}

/// 给一条请求找**生效侧**的 ack：同 field 的最近一条 applied/dropped；
/// 请求有 seq 时优先 seq 相同的那条；找不到返回 null（界面写「尚无 ack」）。
RenderEvent? matchPresetAck(PresetRequest req, Iterable<RenderEvent> acks) {
  if (req.field == null || req.field!.isEmpty) return null;
  final List<RenderEvent> candidates = <RenderEvent>[
    for (final RenderEvent e in acks)
      if (e.field == req.field && kPresetAckKinds.contains(e.kind)) e,
  ];
  if (candidates.isEmpty) return null;
  if (req.seq != null) {
    for (final RenderEvent e in candidates.reversed) {
      if (e.seq == req.seq) return e;
    }
  }
  return candidates.last;
}

/// 一次配对（左「请求」+ 右「生效」）。纯函数，可单测。
PresetOverridePair presetOverridePair(
  PresetRequest req,
  Iterable<RenderEvent> acks,
) {
  final RenderEvent? ack = matchPresetAck(req, acks);
  return PresetOverridePair(
    request: formatPresetRequestLine(req),
    applied: formatPresetAppliedLine(ack),
    ack: ack,
  );
}

/// D 栏左列的一条「送 TTS 的净化文本」（来自前端已收的 text_delta / text_fallback）。
class TtsSentence {
  const TtsSentence({
    required this.text,
    required this.at,
    this.epoch,
    this.seq,
    this.origin = 'text_delta',
  });

  final String text;
  final DateTime at;
  final int? epoch;
  final int? seq;

  /// 来源帧类型（text_delta / text_fallback）。
  final String origin;

  /// 字符数（按码点计，CJK 与 ASCII 口径一致）。
  int get chars => text.runes.length;
}

/// D 栏右列的一条原始段（来自日志端点的 sentence_ready 行）。
class SentenceReadyLog {
  const SentenceReadyLog({
    required this.text,
    this.epoch,
    this.sentenceSeq,
    this.tsMs,
  });

  final String text;
  final int? epoch;
  final int? sentenceSeq;
  final int? tsMs;

  int get chars => text.runes.length;

  /// 去重键 =（epoch, sentence_seq, text）——同一句会被多个 Mod 各记一行。
  String get dedupeKey => '${epoch ?? ''}\u0001${sentenceSeq ?? ''}\u0001$text';
}

/// sentence_ready 的日志标记（真实行：… 收到事件 sentence_ready: {json}）。
const String kSentenceReadyMarker = 'sentence_ready:';

/// 从日志行里**语义解析** sentence_ready：
/// - 只收带 JSON payload 的行（解析失败 / 无 payload 的行忽略）；
/// - 按（epoch, sentence_seq, text）去重（同句会被多个 Mod 各记一行）；
/// - 保序（第一次出现的顺序）。纯函数，可单测。
List<SentenceReadyLog> parseSentenceReadyLogs(Iterable<String> lines) {
  final List<SentenceReadyLog> out = <SentenceReadyLog>[];
  final Set<String> seen = <String>{};
  for (final String line in lines) {
    final Map<String, Object?>? payload = _sentenceReadyPayload(line);
    if (payload == null) continue;
    final Object? rawText = payload['text'];
    if (rawText is! String || rawText.isEmpty) continue;
    final SentenceReadyLog entry = SentenceReadyLog(
      text: rawText,
      epoch: _intOrNull(payload['epoch']),
      sentenceSeq: _intOrNull(payload['sentence_seq']),
      tsMs: _intOrNull(payload['ts_ms']),
    );
    if (!seen.add(entry.dedupeKey)) continue;
    out.add(entry);
  }
  return out;
}

Map<String, Object?>? _sentenceReadyPayload(String line) {
  final int marker = line.indexOf(kSentenceReadyMarker);
  if (marker < 0) return null;
  final int brace = line.indexOf('{', marker + kSentenceReadyMarker.length);
  if (brace < 0) return null;
  final String? json = _balancedObject(line, brace);
  if (json == null) return null;
  try {
    final Object? decoded = jsonDecode(json);
    if (decoded is Map) return _stringKeys(decoded);
  } catch (_) {
    // 解析失败的行忽略：不是所有日志行都带 JSON。
  }
  return null;
}

/// 从 [start]（必须是左花括号）截出**配平的** JSON 对象文本；配不平返回 null。
String? _balancedObject(String src, int start) {
  int depth = 0;
  bool inString = false;
  bool escaped = false;
  for (int i = start; i < src.length; i++) {
    final String c = src[i];
    if (inString) {
      if (escaped) {
        escaped = false;
      } else if (c == '\\') {
        escaped = true;
      } else if (c == '"') {
        inString = false;
      }
      continue;
    }
    if (c == '"') {
      inString = true;
      continue;
    }
    if (c == '{') {
      depth++;
      continue;
    }
    if (c == '}') {
      depth--;
      if (depth == 0) return src.substring(start, i + 1);
    }
  }
  return null;
}

/// WS 帧 → B 栏一行；**不认识的帧返回 null**（subscribe_ack / heartbeat 不记，
/// 心跳会淹没有限缓冲）。未知 type 照记，type 用 wire 名。
ObserverRecord? observeWsEvent(WsEvent event, DateTime at) {
  if (event is ActionCueEvent) {
    return ObserverRecord(
      at: at,
      type: 'action_cue',
      source: 'ws',
      epoch: event.epoch,
      seq: event.seq,
      text: _actionCueSummary(event),
    );
  }
  if (event is TextDeltaEvent) {
    final int chars = event.text?.runes.length ?? 0;
    final String body = event.completed == true ? '完成锚点' : '$chars 字符';
    return ObserverRecord(
      at: at,
      type: 'text_delta',
      source: 'ws',
      epoch: event.epoch,
      seq: event.seq,
      text: 'text_delta $body',
    );
  }
  if (event is TextFallbackEvent) {
    final int chars = event.text?.runes.length ?? 0;
    return ObserverRecord(
      at: at,
      type: 'text_fallback',
      source: 'ws',
      epoch: event.epoch,
      seq: event.seq,
      text: 'text_fallback $chars 字符（整段兜底）',
    );
  }
  if (event is TurnStateEvent) {
    return ObserverRecord(
      at: at,
      type: 'turn_state',
      source: 'ws',
      epoch: event.epoch,
      seq: event.seq,
      text: 'turn_state status=${event.status}',
    );
  }
  if (event is WsErrorEvent) {
    return ObserverRecord(
      at: at,
      type: 'error',
      source: 'ws',
      epoch: event.epoch,
      seq: event.seq,
      text: 'error code=${event.code} stage=${event.stage ?? '-'} '
          'fatal=${event.fatal}',
    );
  }
  if (event is RuntimeStatusEvent) {
    return ObserverRecord(
      at: at,
      type: 'runtime_status',
      source: 'ws',
      epoch: event.epoch,
      seq: event.seq,
      text: 'runtime_status event=${event.event}',
    );
  }
  if (event is UnknownWsEvent) {
    return ObserverRecord(
      at: at,
      type: event.type,
      source: 'ws',
      text: '${event.type} keys=${event.data.length}',
    );
  }
  return null;
}

String _actionCueSummary(ActionCueEvent event) {
  final StringBuffer b = StringBuffer('action_cue cues=${event.cues.length}');
  if (event.baseline) b.write(' baseline=true');
  if (event.reason != null && event.reason!.isNotEmpty) {
    b.write(' reason=${event.reason}');
  }
  if (event.cues.isNotEmpty) b.write(' first=${_cueBrief(event.cues.first)}');
  return b.toString();
}

String _cueBrief(ActionCue cue) {
  final StringBuffer b = StringBuffer();
  if (cue.field != null && cue.field!.isNotEmpty) b.write('field=${cue.field}');
  if (cue.presetId.isNotEmpty) {
    if (b.isNotEmpty) b.write(' ');
    b.write('id=${cue.presetId}');
  }
  if (cue.intensity != 0) {
    if (b.isNotEmpty) b.write(' ');
    b.write('intensity=${cue.intensity}');
  }
  if (cue.seq != null) {
    if (b.isNotEmpty) b.write(' ');
    b.write('seq=${cue.seq}');
  }
  return b.isEmpty ? '(空 cue)' : b.toString();
}

/// text_delta / text_fallback → D 栏左列句子；其它帧 / 空文本返回 null。
TtsSentence? ttsSentenceFromWsEvent(WsEvent event, DateTime at) {
  if (event is TextDeltaEvent) {
    final String? text = event.text;
    if (text == null || text.isEmpty) return null;
    return TtsSentence(
      text: text,
      at: at,
      epoch: event.epoch,
      seq: event.seq,
      origin: 'text_delta',
    );
  }
  if (event is TextFallbackEvent) {
    final String? text = event.text;
    if (text == null || text.isEmpty) return null;
    return TtsSentence(
      text: text,
      at: at,
      epoch: event.epoch,
      seq: event.seq,
      origin: 'text_fallback',
    );
  }
  return null;
}

/// 渲染面 ack → B 栏一行（复用喂导演的日志文本）。
ObserverRecord observeRenderEvent(RenderEvent event, DateTime at) =>
    ObserverRecord(
      at: at,
      type: event.type,
      source: 'render',
      epoch: event.epoch,
      seq: event.seq,
      text: formatRenderEvent(event),
    );

/// 音频时钟采样 → B 栏一行（协议 §6.5 / O13）。
ObserverRecord observeStageClock({
  int? seg,
  required int posMs,
  required bool playing,
  required DateTime at,
}) => ObserverRecord(
  at: at,
  type: 'stage-clock',
  source: 'audio',
  text: 'stage-clock seg=${seg ?? '-'} pos_ms=$posMs playing=$playing',
);

String _trimNumber(num value) {
  final double d = value.toDouble();
  if (d == d.roundToDouble() && d.abs() < 1e15) return d.toInt().toString();
  return '$value';
}

Map<String, Object?> _stringKeys(Map<Object?, Object?> raw) {
  final Map<String, Object?> out = <String, Object?>{};
  raw.forEach((Object? k, Object? v) {
    if (k is String) out[k] = v;
  });
  return out;
}
