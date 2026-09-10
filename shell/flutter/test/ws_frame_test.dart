import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/action_source.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';

/// **真实抓包**的 `/ws/state` 原文（本机 v0.4.6，2026-09-10）。
///
/// 抓取方式：连上 `ws://127.0.0.1:18080/ws/state`，`POST /api/v1/chat
/// {"text":"说两句话"}`，逐帧记下**每种 `type` 的第一条原文**。
/// 抓取时服务端带 `LIVE2D_AI_MUTE_AUDIO=1`（跑测试一律静音，见 AGENTS.md），
/// 所以 `audio` 帧的 PCM 是**零**、`muted:true`——这不是夹具失真，
/// 而正是「静音时服务端下发零 PCM + 真实 volume」这条约定的现场证据。
///
/// **不要凭印象改写这些夹具**：它们的作用就是把「服务端到底发什么形状」钉死。
/// 协议真变了、改夹具之前，先确认服务端确实变了（`web_api/ws/events.rs`）。
const String kRealSubscribeAck =
    '{"type":"subscribe_ack","seq":1,"ts":"2026-09-10T12:14:39.028Z",'
    '"data":{"topics":[]}}';

const String kRealTextDelta =
    '{"data":{"epoch":0,"text":"好呀，那我就说两句话啦。\\n\\n","ts_ms":8590},'
    '"seq":364,"ts":"2026-09-10T12:14:48.829Z","type":"text_delta"}';

/// 同一个 `type` 的第二种形态：**完成锚点**（没有 `text`，只有 `completed`）。
const String kRealTextDeltaAnchor =
    '{"data":{"completed":false,"epoch":0},"seq":3,'
    '"ts":"2026-09-10T12:13:43.310Z","type":"text_delta"}';

const String kRealTurnStateCompleted =
    '{"data":{"epoch":0,"status":"completed"},"seq":564,'
    '"ts":"2026-09-10T12:14:53.166Z","type":"turn_state"}';

/// `outcome_completed == false` 的那一支。这条来自一次**上游 401** 的真实失败轮
/// （服务器没读到 `.env` 里的 key）——正好当失败路径的夹具。
const String kRealTurnStateFailed =
    '{"data":{"epoch":0,"status":"failed"},"seq":2,'
    '"ts":"2026-09-10T12:13:43.310Z","type":"turn_state"}';

const String kRealActionStateRuleFallback =
    '{"data":{"action":{"action":"surprise","source":"rule_fallback","strength":3},'
    '"epoch":0,"kind":"perform"},"seq":566,'
    '"ts":"2026-09-10T12:14:53.166Z","type":"action_state"}';

/// `audio` 帧的**字段顺序与全部字段**（含解析层目前**故意不取**的 `slice_ms`）。
///
/// 原文里 `audio` 是 1280 个 `A`（960 字节零 PCM）；这里在运行时用
/// `base64Encode` 现造，避免把 1.3 KB 的 `AAAA…` 抄进源码。
/// `slice_ms: 20` 服务端有、[AudioEvent] 没有——保留在夹具里是为了让
/// 「解析层忽略了哪些字段」这件事**可见**，而不是悄悄不存在。
String realAudioFrame({required bool muted, required double volume}) {
  final String b64 = base64Encode(Uint8List(960)); // 960 B = 480×s16 = 20ms@24k
  return '{"data":{"audio":"$b64","end":false,"epoch":0,"muted":$muted,'
      '"sample_rate":24000,"slice_ms":20,"start":false,"volume":$volume},'
      '"seq":2,"ts":"2026-09-10T12:15:43.530Z","type":"audio"}';
}

/// `heartbeat` 帧形状固定（服务端每 10s 一条，`{"type":"heartbeat"}` 加包装），
/// 抓包脚本当时把它过滤掉了，这里按 `connection.rs` 的构造手写。
const String kHeartbeat =
    '{"type":"heartbeat","seq":9,"ts":"2026-09-10T12:15:00.000Z"}';

void main() {
  group('真实帧 → 领域事件（每种 type 一条，全部来自实抓）', () {
    test('subscribe_ack：首帧，topics 可为空列表', () {
      final WsEvent? e = parseWsFrame(kRealSubscribeAck);
      expect(e, isA<SubscribeAckEvent>());
      final SubscribeAckEvent a = e! as SubscribeAckEvent;
      expect(a.topics, isEmpty);
      expect(a.seq, 1);
      expect(a.ts, '2026-09-10T12:14:39.028Z');
    });

    test('text_delta：正文片段带 text 与 ts_ms', () {
      final TextDeltaEvent e = parseWsFrame(kRealTextDelta)! as TextDeltaEvent;
      expect(e.epoch, 0);
      expect(e.text, '好呀，那我就说两句话啦。\n\n', reason: '换行必须原样保留');
      expect(e.completed, isNull, reason: '这一支没有 completed 字段');
    });

    test('text_delta：完成锚点没有 text，只有 completed', () {
      final TextDeltaEvent e =
          parseWsFrame(kRealTextDeltaAnchor)! as TextDeltaEvent;
      expect(e.text, isNull, reason: '锚点帧不带正文——不能把 null 当空串写进气泡');
      expect(e.completed, isFalse);
    });

    test('turn_state：completed / failed 两种 status 都进来', () {
      final TurnStateEvent ok =
          parseWsFrame(kRealTurnStateCompleted)! as TurnStateEvent;
      expect(ok.status, 'completed');
      expect(ok.epoch, 0);

      final TurnStateEvent bad =
          parseWsFrame(kRealTurnStateFailed)! as TurnStateEvent;
      expect(bad.status, 'failed', reason: '失败轮也要能让 UI 收口，不能吞掉');
    });

    test('action_state：data.action 是对象；rule_fallback 档位要落对', () {
      final ActionStateEvent e =
          parseWsFrame(kRealActionStateRuleFallback)! as ActionStateEvent;
      expect(e.kind, 'perform');
      expect(e.action, 'surprise');
      expect(e.strength, 3);
      expect(e.source, ActionSource.ruleFallback);
      expect(e.hasAction, isTrue);
      expect(e.source.isAiInitiated, isTrue);
    });

    test('heartbeat：只带 seq/ts', () {
      final WsEvent? e = parseWsFrame(kHeartbeat);
      expect(e, isA<HeartbeatEvent>());
      expect(e!.seq, 9);
    });
  });

  group('audio 帧：口型的数据来源（静音与否都要能驱动）', () {
    test('静音帧：PCM 全零但 volume 是真的', () {
      final AudioEvent e =
          parseWsFrame(realAudioFrame(muted: true, volume: 0.001))! as AudioEvent;
      expect(e.muted, isTrue);
      expect(e.sampleRate, 24000);
      expect(e.pcm.length, 960, reason: '20ms @ 24kHz 单声道 s16le');
      expect(e.pcm.every((int b) => b == 0), isTrue, reason: '静音 = 零 PCM');
      expect(
        e.volume,
        closeTo(0.001, 1e-9),
        reason: '关键：PCM 被清零了，但 volume 必须还在——否则静音时嘴不动',
      );
      expect(e.start, isFalse);
      expect(e.end, isFalse);
    });

    test('非静音帧：muted=false 且 volume 有值', () {
      final AudioEvent e = parseWsFrame(
        realAudioFrame(muted: false, volume: 0.4123),
      )! as AudioEvent;
      expect(e.muted, isFalse);
      expect(e.volume, closeTo(0.4123, 1e-9));
    });

    test('volume 缺失/非法 → null（调用方回退本地 RMS，而不是拿 NaN 驱动）', () {
      String frame(String volumeJson) =>
          '{"type":"audio","data":{"audio":"AAAA","sample_rate":24000,'
          '"volume":$volumeJson}}';

      expect(
        (parseWsFrame(frame('null'))! as AudioEvent).volume,
        isNull,
      );
      expect(
        (parseWsFrame(frame('"loud"'))! as AudioEvent).volume,
        isNull,
        reason: '字符串不是数字',
      );
    });

    test('sample_rate 缺失 → 24000（与 TtsSettings 默认一致）', () {
      final AudioEvent e = parseWsFrame(
        '{"type":"audio","data":{"audio":"AAAA"}}',
      )! as AudioEvent;
      expect(e.sampleRate, 24000);
      expect(e.pcm.length, 3);
    });

    test('没有 audio 字段 / 坏 base64 → null（丢一帧，不断流）', () {
      expect(parseWsFrame('{"type":"audio","data":{"epoch":0}}'), isNull);
      expect(
        parseWsFrame('{"type":"audio","data":{"audio":"!!!not-base64!!!"}}'),
        isNull,
      );
    });
  });

  group('坏帧：永不抛异常（协议是外部输入）', () {
    test('非法 JSON / 合法但非对象 / 缺 type / type 非字符串', () {
      expect(parseWsFrame(''), isNull);
      expect(parseWsFrame('{'), isNull);
      expect(parseWsFrame('not json at all'), isNull);
      expect(parseWsFrame('[1,2,3]'), isNull, reason: 'JSON 数组不是帧');
      expect(parseWsFrame('"a string"'), isNull);
      expect(parseWsFrame('null'), isNull);
      expect(parseWsFrame('{"seq":1}'), isNull, reason: '缺 type');
      expect(parseWsFrame('{"type":123}'), isNull, reason: 'type 必须是字符串');
      expect(parseWsFrame('{"type":null}'), isNull);
    });

    test('data 不是对象 → 当空对象，帧本身仍然可用', () {
      final TurnStateEvent e =
          parseWsFrame('{"type":"turn_state","data":"oops"}')! as TurnStateEvent;
      expect(e.epoch, 0);
      expect(e.status, 'unknown', reason: '缺 status 也要给出可显示的值');
    });

    test('seq/ts 类型不对 → 只是为 null，不影响事件本体', () {
      final WsEvent? e = parseWsFrame(
        '{"type":"turn_state","seq":"nope","ts":42,"data":{"status":"completed"}}',
      );
      expect(e, isA<TurnStateEvent>());
      expect(e!.seq, isNull);
      expect(e.ts, isNull);
      expect((e as TurnStateEvent).status, 'completed');
    });

    test('epoch 可以是数字字符串（宽容解析）', () {
      final TurnStateEvent e =
          parseWsFrame('{"type":"turn_state","data":{"epoch":"7"}}')!
              as TurnStateEvent;
      expect(e.epoch, 7);
    });
  });

  group('前向兼容：新 type 不许把老前端打挂', () {
    test('未知 type → UnknownWsEvent，data 原样带出', () {
      final WsEvent? e = parseWsFrame(
        '{"type":"brand_new_thing","seq":3,"data":{"a":1,"b":[2]}}',
      );
      expect(e, isA<UnknownWsEvent>());
      final UnknownWsEvent u = e! as UnknownWsEvent;
      expect(u.type, 'brand_new_thing');
      expect(u.data['a'], 1);
      expect(u.data['b'], <Object?>[2]);
    });

    test('已知 type 上的**新增**字段被忽略，不报错', () {
      // 服务端加字段是本项目明确允许的（缺省即默认值），解析侧必须容忍。
      final TextDeltaEvent e =
          parseWsFrame(
                '{"type":"text_delta","data":{"epoch":1,"text":"hi",'
                '"brand_new_field":{"nested":true}}}',
              )!
              as TextDeltaEvent;
      expect(e.epoch, 1);
      expect(e.text, 'hi');
    });

    test('`error` 帧：服务端当前还不发（events.rs 里标 P1 partial），解析侧先备好', () {
      final WsErrorEvent e =
          parseWsFrame('{"type":"error","data":{"code":"llm_upstream","message":"上游 401"}}')!
              as WsErrorEvent;
      expect(e.code, 'llm_upstream');
      expect(e.message, '上游 401');
    });

    test('`error` 帧缺 message → message 为空、原文进 raw（不上屏 JSON）', () {
      final String raw = '{"type":"error","data":{"code":"x"}}';
      final WsErrorEvent e = parseWsFrame(raw)! as WsErrorEvent;
      expect(e.code, 'x');
      expect(
        e.message,
        isEmpty,
        reason: 'message 是直接上屏的字段——绝不能是整帧 JSON',
      );
      expect(e.raw, raw, reason: '整帧原文仍要留给诊断');
    });
  });

  group('重连退避：1s→2s→4s→…→30s 上限（对外承诺，必须有测试守）', () {
    test('前 6 次翻倍，之后钉在 30s', () {
      expect(backoffForAttempt(0).inMilliseconds, 1000);
      expect(backoffForAttempt(1).inMilliseconds, 2000);
      expect(backoffForAttempt(2).inMilliseconds, 4000);
      expect(backoffForAttempt(3).inMilliseconds, 8000);
      expect(backoffForAttempt(4).inMilliseconds, 16000);
      expect(backoffForAttempt(5).inMilliseconds, 30000, reason: '16s×2=32s → 夹到 30s');
      expect(backoffForAttempt(6).inMilliseconds, 30000);
      expect(backoffForAttempt(50).inMilliseconds, 30000);
    });

    test('异常输入不抛，也不给出超过上限的值', () {
      expect(backoffForAttempt(-1).inMilliseconds, kBackoffStartMs);
      expect(backoffForAttempt(-999).inMilliseconds, kBackoffStartMs);
      expect(backoffForAttempt(0x7fffffff).inMilliseconds, kBackoffMaxMs);
    });

    test('单调不减（不会出现越重试越快）', () {
      int prev = 0;
      for (int i = -2; i < 40; i++) {
        final int ms = backoffForAttempt(i).inMilliseconds;
        expect(ms, greaterThanOrEqualTo(prev), reason: 'attempt=$i');
        expect(ms, lessThanOrEqualTo(kBackoffMaxMs));
        prev = ms;
      }
    });

    test('常量自洽：起点 < 上限，且都是整秒', () {
      expect(kBackoffStartMs, lessThan(kBackoffMaxMs));
      expect(kBackoffStartMs % 1000, 0);
      expect(kBackoffMaxMs % 1000, 0);
    });
  });

  group('纯函数契约：解析无副作用、可重复调用', () {
    test('同一帧解析两次结果一致（不缓存、不改内部状态）', () {
      final WsEvent a = parseWsFrame(kRealActionStateRuleFallback)!;
      final WsEvent b = parseWsFrame(kRealActionStateRuleFallback)!;
      expect(a, isA<ActionStateEvent>());
      expect((a as ActionStateEvent).action, (b as ActionStateEvent).action);
      expect(a.source, b.source);
    });

    test('解析层不碰连接：shutdown_ready 只产出事件，不关闭任何东西', () {
      // 早先 `shutdown_ready` 的关连接逻辑与 type 分发混在一个 switch 里，
      // 正是整帧 decode 无法单测的原因。现在它只是数据。
      final RuntimeStatusEvent e =
          parseWsFrame(
                '{"type":"runtime_status","data":{"event":"shutdown_ready"}}',
              )!
              as RuntimeStatusEvent;
      expect(e.event, 'shutdown_ready');
      expect(e.epoch, isNull, reason: '这一支服务端确实不带 epoch');
    });
  });
}
