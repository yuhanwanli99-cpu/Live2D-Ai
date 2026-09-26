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
  DirectorEventLog({this.maxLines = 200});

  /// 上限：只保留最近 [maxLines] 行（渲染面事件是长期流，无界会吃内存）。
  final int maxLines;
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
