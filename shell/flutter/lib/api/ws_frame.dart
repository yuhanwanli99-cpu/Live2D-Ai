/// WS 协议帧解析：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// # 为什么要单独成文件（设计规格 §10.2）
///
/// `api/ws_client.dart` 必须 `import 'package:web'`（`WebSocket`），
/// 于是**整帧 decode（type 分发 + 各字段解析）在 `flutter test` 里根本跑不到**——
/// 只能在浏览器里跑。这正是 `action_state` 把对象当字符串解析那个缺陷
/// （`action` 永远为 null、`source`/`strength` 全丢）能长期存活的原因：
/// 9 个分支、每分支多个字段，一个都不在门禁里。
///
/// 本文件只负责「帧字符串 → 领域事件」这一个纯函数，**不含 WebSocket 生命周期**。
/// 连接/重连/定时器仍留在 `ws_client.dart`。
library;

import 'dart:convert';
import 'dart:typed_data';

import 'action_source.dart';

/// `/ws/state` 事件基类。
///
/// 服务端每连接包装 `{"type":...,"seq":N,"ts":"...","data":{...}}`
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
  const RuntimeStatusEvent({
    required this.event,
    this.epoch,
    super.seq,
    super.ts,
  });

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

/// 把一帧原文解析成领域事件。
///
/// **永不抛异常**：坏 JSON、非对象、缺 `type`、坏 base64 一律返回 `null`
/// （由调用方丢弃）。协议层对前端是外部输入，坏一帧不该让整条事件流断掉。
///
/// **无副作用**：这里不会因为 `shutdown_ready` 去关连接——那是连接层的决定，
/// 留在 `WsClient` 里。早先把两者混在一个 `switch` 里，正是它无法被单测的原因之一。
WsEvent? parseWsFrame(String raw) {
  final Map<String, Object?> frame;
  try {
    final Object? decoded = jsonDecode(raw);
    if (decoded is! Map) return null;
    frame = Map<String, Object?>.from(decoded);
  } catch (_) {
    return null; // 坏帧：容忍
  }
  final Object? type = frame['type'];
  if (type is! String) return null;
  final Object? rawData = frame['data'];
  final Map<String, Object?> data = rawData is Map
      ? Map<String, Object?>.from(rawData)
      : const <String, Object?>{};
  // `seq` 必须走宽容解析：写成 `(frame['seq'] as num?)` 时，服务端/中间层
  // 一旦给出字符串形式的 seq（`"3"`）或任何非数字，这里就抛
  // `type 'String' is not a subtype of type 'num?'`——与「永不抛异常」的
  // 承诺直接矛盾，而且抛在**所有** type 分支的上游：一帧坏 seq 会让整个
  // 事件流断掉，不只是丢这一帧。
  final int? seq = _intOrNull(frame['seq']);
  final String? ts = frame['ts'] is String ? frame['ts'] as String : null;

  switch (type) {
    case 'subscribe_ack':
      final Object? rawTopics = data['topics'];
      final List<String> topics = rawTopics is List
          ? rawTopics.whereType<String>().toList()
          : const <String>[];
      return SubscribeAckEvent(topics: topics, seq: seq, ts: ts);

    case 'heartbeat':
      return HeartbeatEvent(seq: seq, ts: ts);

    case 'turn_state':
      return TurnStateEvent(
        epoch: _int(data['epoch']),
        status: _str(data['status']) ?? 'unknown',
        seq: seq,
        ts: ts,
      );

    case 'runtime_status':
      return RuntimeStatusEvent(
        event: _str(data['event']) ?? 'unknown',
        epoch: _intOrNull(data['epoch']),
        seq: seq,
        ts: ts,
      );

    case 'text_delta':
      return TextDeltaEvent(
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
      final ActionDetail detail = ActionDetail.fromData(data['action']);
      return ActionStateEvent(
        epoch: _int(data['epoch']),
        kind: _str(data['kind']) ?? 'unknown',
        action: detail.action,
        strength: detail.strength,
        source: detail.source,
        seq: seq,
        ts: ts,
      );

    case 'audio':
      final String? encoded = _str(data['audio']);
      if (encoded == null) return null; // 没有音频体：丢弃
      try {
        return AudioEvent(
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
        return null; // 坏 base64：丢弃该帧
      }

    case 'error':
      return WsErrorEvent(
        // 缺 message 时退回整帧原文，便于排查。
        code: _str(data['code']) ?? 'error',
        message: _str(data['message']) ?? raw,
        seq: seq,
        ts: ts,
      );

    default:
      return UnknownWsEvent(type: type, data: data, seq: seq, ts: ts);
  }
}

/// 首次重连的延迟（毫秒）。
const int kBackoffStartMs = 1000;

/// 重连延迟上限（毫秒）。
const int kBackoffMaxMs = 30000;

/// 第 [attempt] 次重连（0 起）的延迟：`min(1000 × 2^attempt, 30000)`。
///
/// 抽成纯函数的理由：原实现把 `_backoffMs` 的自增藏在一个 `void` 方法里，
/// 「1s→2s→4s→8s→16s→30s 上限」这条**对外承诺**没有任何测试守。
/// 现在它是可枚举的算术。
///
/// 负数/异常输入按第 0 次处理（不抛）；[attempt] 大到会溢出时直接给上限。
Duration backoffForAttempt(int attempt) {
  if (attempt <= 0) return const Duration(milliseconds: kBackoffStartMs);
  if (attempt > 20) return const Duration(milliseconds: kBackoffMaxMs);
  final int ms = kBackoffStartMs << attempt; // 1000 * 2^attempt
  return Duration(milliseconds: ms > kBackoffMaxMs ? kBackoffMaxMs : ms);
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
  final double d = raw.toDouble();
  return d.isFinite ? d : null;
}

String? _str(Object? value) => value is String ? value : null;
