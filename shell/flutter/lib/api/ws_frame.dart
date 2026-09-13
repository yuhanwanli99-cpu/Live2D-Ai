/// WS 协议帧解析：**纯逻辑、无 web 依赖**，可在 VM 上单测。
///
/// # 为什么要单独成文件（设计规格 §10.2）
///
/// `api/ws_client.dart` 必须 `import 'package:web'`（`WebSocket`），
/// 于是**整帧 decode（type 分发 + 各字段解析）在 `flutter test` 里根本跑不到**——
/// 只能在浏览器里跑。这正是历史上 `action_state` 把对象当字符串解析那个缺陷
/// （`action` 永远为 null、`source`/`strength` 全丢）能长期存活的原因：
/// 9 个分支、每分支多个字段，一个都不在门禁里。
/// （`action_state` 分支本身已于 2026-09-11 随动作子系统删除——LLM 无工具、
/// 服务端不再下发该帧；这段记录保留，是为了说明本文件为什么必须存在。）
///
/// 本文件只负责「帧字符串 → 领域事件」这一个纯函数，**不含 WebSocket 生命周期**。
/// 连接/重连/定时器仍留在 `ws_client.dart`。
library;

import 'dart:convert';
import 'dart:typed_data';

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

/// `reasoning_delta`：推理模型的**思考**增量（2026-09-13）。
///
/// 与 [TextDeltaEvent] 严格分开，理由有二：
/// 1. **语义不同**：`text_delta` 是「会对上声音的正文」（服务端在该句语音合成完毕
///    才发），思考没有声音可对；
/// 2. **不能混**：若把思考当正文追加到气泡里，界面会把内心独白当回复显示。
///
/// 它只进气泡的「思考」折叠区，**不落盘**（见 `ChatMessage.reasoning`）。
class ReasoningDeltaEvent extends WsEvent {
  const ReasoningDeltaEvent({
    this.epoch,
    this.text,
    super.seq,
    super.ts,
  });

  final int? epoch;
  final String? text;
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
    this.sentenceSeq,
    this.wav,
    super.seq,
    super.ts,
  });

  final int epoch;
  final Uint8List pcm;
  final int sampleRate;

  /// 某一句的**首片**（2026-09-11 新契约：每句恰好一次）。
  ///
  /// **不可当攒句闸门**：旧服务端这里是**轮级**标记（`previous != token` 记账，
  /// 实测正文只有一句时收到 0 个 `start=true`）。前端的用法是防御性的——
  /// 累积中途遇到 `start` 说明上一句缺了 `end`，丢弃半截重来。见
  /// `lib/audio/sentence_assembler.dart` 头注。
  final bool start;

  /// 某一句的**末片**：前端攒句的**唯一闸门**（`end=true` → 封 WAV 去播）。
  final bool end;

  /// 服务端按本片**原始**样本算出的 RMS ∈ [0,1]（`data.volume`）。
  ///
  /// **静音时这是口型的唯一来源**：服务端静音会把 PCM 置零（谁都发不出声），
  /// 但 `volume` 仍是真实值——所以「不出声但嘴照动」。因此客户端驱动口型
  /// **优先用本字段**，只有它缺失（旧服务端）时才回退到本地 RMS。
  final double? volume;

  /// 服务端是否处于静音输出（`data.muted`，观测/诊断用）。
  final bool muted;

  /// 同一句的分片归组序号（`data.sentence_seq`，可选；从 1 严格递增）。
  ///
  /// 新旧兼容：缺字段 → `null`，前端退化为「上次封口后继续累积」，不报错。
  final int? sentenceSeq;

  /// **整句 WAV 直通**（`data.wav` base64，前向兼容钩子）。
  ///
  /// 当前服务端只发 `audio` 分片，所以恒为 `null`；将来若帧上直接带封好的
  /// 整句 WAV，前端直接拿它播、不再攒片（见 `sentence_assembler.dart`）。
  final Uint8List? wav;
}

/// 服务端 `error` 帧。
///
/// **2026-09-11 起后端真的会发这一帧了**：此前 `AppEvent` 没有错误变体，
/// 服务端投影表里也没有 `error` 分支——这个类一直存在、却永远收不到实例，
/// 所以界面上只能显示「本轮失败」而说不出原因（用户报「后端出错无具体错误
/// 代码」「前端无法知道错误信息」）。
class WsErrorEvent extends WsEvent {
  const WsErrorEvent({
    required this.code,
    required this.message,
    this.hint,
    this.stage,
    this.fatal = false,
    this.epoch,
    this.raw,
    super.seq,
    super.ts,
  });

  /// 稳定机器码（契约）：`<stage>_<suffix>`，如 `llm_upstream_401`。
  ///
  /// **永不为空**——`message`/`hint` 都可能缺省，只有它是可靠锚点。
  /// 同一个字符串也出现在后端日志里（`code=llm_upstream_401`），
  /// 用户可以直接拿它去诊断日志里搜。
  final String code;

  /// 服务端给的文案。**缺字段时为空串**（不是整帧原文）。
  ///
  /// 早先这里 `?? raw`（退回整帧 JSON），本意是「便于排查」，但 `message`
  /// 是**直接上屏**的字段——用户会看到一坨
  /// `{"type":"error","data":{"code":"llm_upstream"}}`。
  /// 整帧原文另有 [raw] 承载，排查照样拿得到。
  final String message;

  /// 服务端给的**处置提示**（如「确认 [llm] api_key_env 指向的环境变量在
  /// 后端进程环境里已设置」）。旧服务端没有这个字段 → `null`。
  final String? hint;

  /// 出错阶段：`llm` / `tts` / `decode` / `tools`（旧服务端 → `null`）。
  final String? stage;

  /// 是否致命：`false` = 已生成的语音仍会播完；`true` = 本轮立即结束。
  final bool fatal;

  /// 产生该错误的业务代次（旧服务端 → `null`）。
  ///
  /// 用来区分「当前这一轮失败」与「已经被 stop 作废的那一轮迟到的错误」。
  final int? epoch;

  /// 整帧原文（诊断用；不直接上屏）。
  final String? raw;
}

/// 把 `error` 帧拼成**给人看的一行**：`code：message（hint）`。
///
/// 为什么把 `code` 放最前：它是唯一永远在场的字段，也是与后端日志对齐的键。
/// 只有 `message` 时用户看到「上游非成功状态 401」却不知道去哪查；带上
/// `llm_upstream_401` 就能在诊断日志里精确搜到同一行。
///
/// 纯函数（无 IO），单测覆盖 `ws_frame_test.dart`。
String formatWsError({
  required String code,
  required String message,
  String? hint,
}) {
  final StringBuffer out = StringBuffer(code);
  if (message.isNotEmpty) out.write('：$message');
  if (hint != null && hint.isNotEmpty) out.write('（$hint）');
  return out.toString();
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

    case 'reasoning_delta':
      // 未知 type 在旧客户端被忽略，所以这是向后兼容的新增帧。
      return ReasoningDeltaEvent(
        epoch: _intOrNull(data['epoch']),
        text: _str(data['text']),
        seq: seq,
        ts: ts,
      );

    case 'audio':
      final String? encoded = _str(data['audio']);
      // 前向兼容的可选字段：`wav`（整句封好的 WAV，直通播放）——将来服务端
      // 只带 `wav` 不带 `audio` 分片时，这一帧仍然可用。
      final String? rawWav = _str(data['wav']);
      final bool hasWav = rawWav != null && rawWav.isNotEmpty;
      if (encoded == null && !hasWav) return null; // 没有音频体：丢弃
      try {
        Uint8List? wav;
        if (hasWav) {
          try {
            wav = base64Decode(rawWav);
          } catch (_) {
            wav = null; // 坏 wav：当作没有，仍按分片处理
          }
        }
        return AudioEvent(
          epoch: _int(data['epoch']),
          pcm: encoded == null ? Uint8List(0) : base64Decode(encoded),
          sampleRate: _intOrNull(data['sample_rate']) ?? 24000,
          start: data['start'] == true,
          end: data['end'] == true,
          volume: _doubleOrNull(data['volume']),
          muted: data['muted'] == true,
          // 可选字段：缺失 → null，前端退化为「上次封口后继续累积」，不报错。
          sentenceSeq: _intOrNull(data['sentence_seq']),
          wav: wav,
          seq: seq,
          ts: ts,
        );
      } catch (_) {
        return null; // 坏 base64：丢弃该帧
      }

    case 'error':
      return WsErrorEvent(
        code: _str(data['code']) ?? 'error',
        // 缺 message = 空串（调用方回落到 code 上屏）；整帧原文进 [raw]。
        message: _str(data['message']) ?? '',
        // 2026-09-11 新增字段；旧服务端缺省 → null（前向兼容）。
        hint: _str(data['hint']),
        stage: _str(data['stage']),
        fatal: data['fatal'] == true,
        epoch: _intOrNull(data['epoch']),
        raw: raw,
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
