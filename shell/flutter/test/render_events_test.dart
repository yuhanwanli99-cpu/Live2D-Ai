/// 阶段4d：渲染面 ack 四条 + segment-ended 的**消费与日志化**回归。
///
/// # 这两条判据守什么
///
/// 1. `ack_events_are_consumed_and_forwarded_to_the_director_log`：渲染面的
///    五条事件（协议 §7）必须被前端接住，并按 §7.3 汇入**喂导演的日志文本**。
///    解析与文本化是纯逻辑（`lib/live2d/render_events.dart`），所以能在 VM
///    上回归；接线（舞台 → 日志）在 `main.dart`（`package:web` 加载不了）
///    用源码扫描钉住（先例 `action_scales_wiring_test.dart`）。
/// 2. `unknown_fields_in_render_events_are_ignored`：V11——前端遇未知字段
///    忽略；表外的 type 不抛也不进日志。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/api/ws_frame.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';
import 'package:live2d_ai_shell/live2d/render_events.dart';

class _FakeTransport implements Live2DTransport {
  final StreamController<String> _controller =
      StreamController<String>.broadcast();
  final List<String> sent = <String>[];

  @override
  Stream<String> get events => _controller.stream;

  @override
  Future<void> send(String json) async => sent.add(json);

  @override
  Future<void> dispose() async {
    await _controller.close();
  }

  void emit(String frame) => _controller.add(frame);
}

void main() {
  test('ack_events_are_consumed_and_forwarded_to_the_director_log', () async {
    final _FakeTransport transport = _FakeTransport();
    final Live2DBridge bridge = Live2DBridge(transport);
    final DirectorEventLog log = DirectorEventLog();
    bridge.renderEvents.listen(log.add);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);

    // 五条 wire（O13 冻结名）各一条，payload 按协议 §7.2。
    final List<Map<String, Object?>> frames = <Map<String, Object?>>[
      <String, Object?>{
        'version': 1,
        'type': 'preset-applied',
        'payload': <String, Object?>{
          'epoch': 7, 'ts_ms': 1200, 'seq': 1, 'field': 'body',
          'x': 0.0, 'y': 0.3, 'intensity': 1,
          'clamped': false, 'degraded': false,
        },
      },
      <String, Object?>{
        'version': 1,
        'type': 'preset-replaced',
        'payload': <String, Object?>{
          'epoch': 7, 'ts_ms': 1400, 'seq': 1, 'field': 'body', 'intensity': 1,
        },
      },
      <String, Object?>{
        'version': 1,
        'type': 'preset-expired',
        'payload': <String, Object?>{
          'epoch': 7, 'ts_ms': 2300, 'seq': 2, 'field': 'head',
          'y': -0.4, 'intensity': 2, 'clamped': true,
        },
      },
      <String, Object?>{
        'version': 1,
        'type': 'preset-dropped',
        'payload': <String, Object?>{
          'epoch': 7, 'ts_ms': 2400, 'seq': 3, 'field': 'expression',
          'id': 'smile', 'degraded': true, 'reason': 'ParamMouthForm',
        },
      },
      <String, Object?>{
        'version': 1,
        'type': 'segment-ended',
        'payload': <String, Object?>{'epoch': 7, 'ts_ms': 4000, 'seg': 2},
      },
    ];
    for (final Map<String, Object?> f in frames) {
      transport.emit(jsonEncode(f));
    }
    await Future<void>.delayed(Duration.zero);

    expect(log.length, 5, reason: '四条 ack + segment_ended 全部进日志');
    final String text = log.text;
    for (final String t in <String>[
      'preset-applied',
      'preset-replaced',
      'preset-expired',
      'preset-dropped',
      'segment-ended',
    ]) {
      expect(text.contains(t), isTrue, reason: t);
    }
    // 日志带的是「当时生效的字段 + 强度 + 是否钳位/降级」，不含正文。
    expect(text.contains('field=body'), isTrue);
    expect(text.contains('clamped=true'), isTrue);
    expect(text.contains('degraded=true'), isTrue);
    expect(text.contains('reason=ParamMouthForm'), isTrue);
    expect(text.contains('seg=2'), isTrue);

    // 表外 type / 坏帧：忽略，不进日志、不抛。
    transport.emit('{"version":1,"type":"preset-future","payload":{"x":1}}');
    transport.emit('not json at all');
    await Future<void>.delayed(Duration.zero);
    expect(log.length, 5, reason: '未知 type 与坏帧都不得进日志');
    expect(bridge.errorMessage, isNull, reason: '坏帧不该让桥进 error');
    bridge.dispose();
  });

  test('接线（源码扫描）：舞台把 ack 汇进导演日志', () {
    final String stage = File(
      'lib/live2d/live2d_stage.dart',
    ).readAsStringSync();
    expect(
      stage.contains('bridge.renderEvents.listen(_onRenderEvent)'),
      isTrue,
      reason: '舞台必须订阅渲染面事件流（不订阅 = ack 永远到不了日志）',
    );
    expect(stage.contains('widget.onRenderEvent?.call(event)'), isTrue);
    final String main = File('lib/main.dart').readAsStringSync();
    expect(
      main.contains('onRenderEvent: _directorLog.add'),
      isTrue,
      reason: '宿主必须把事件汇进 DirectorEventLog（喂导演的日志文本）',
    );
    expect(main.contains('DirectorEventLog'), isTrue);
  });

  test('unknown_fields_in_render_events_are_ignored', () async {
    // ① 纯解析：表外键不进事件；类型不对的已知键当缺字段。
    final RenderEvent e = parseRenderEvent('preset-applied', <String, Object?>{
      'field': 'head',
      'y': -0.5,
      'clamped': true,
      'future_key': 1,
      'reason_note': 'x',
      'ttl_ms': '2600',
    })!;
    expect(e.field, 'head');
    expect(e.y, closeTo(-0.5, 1e-9));
    expect(e.clamped, isTrue);
    expect(e.intensity, isNull);
    expect(e.ttlMs, 2600, reason: '字符串数字也宽容解析');
    expect(parseRenderEvent('preset-future', <String, Object?>{'x': 1}), isNull);

    // ② 走真桥：带未知字段的帧不抛、不污染事件流。
    final _FakeTransport transport = _FakeTransport();
    final Live2DBridge bridge = Live2DBridge(transport);
    final List<RenderEvent> events = <RenderEvent>[];
    bridge.renderEvents.listen(events.add);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);
    transport.emit(
      '{"version":1,"type":"preset-applied","payload":'
      '{"field":"expression","id":"smile","future":{"nested":true},"unknown":7}}',
    );
    transport.emit('{"version":99,"type":"totally-unknown","payload":{}}');
    transport.emit(
      '{"version":1,"type":"preset-applied","payload":'
      '{"field":42,"x":"nope"}}',
    );
    await Future<void>.delayed(Duration.zero);
    expect(events, hasLength(2), reason: '两条 preset-applied 都算；未知 type 忽略');
    expect(events.first.id, 'smile');
    expect(events.first.x, isNull, reason: '类型不对的键当缺字段');
    expect(events.last.field, isNull, reason: 'field 类型不对 → null，不抛');
    expect(bridge.errorMessage, isNull);
    bridge.dispose();
  });

  group('阶段5 W5a：导演可观测的纯逻辑', () {
    test('环形缓冲 cap=200：第 201 条挤掉最旧', () {
      final ObserverBuffer buffer = ObserverBuffer();
      expect(buffer.capacity, 200);
      for (int i = 1; i <= 201; i++) {
        buffer.add(ObserverRecord(
          at: DateTime(2026, 9, 26),
          type: 't',
          source: 'ws',
          text: '第 $i 条',
        ));
      }
      expect(buffer.length, 200);
      expect(buffer.records.first.text, '第 2 条', reason: '第 1 条必须被挤掉');
      expect(buffer.records.last.text, '第 201 条');
    });

    test('按类型过滤：空集 = 全部；命中集合才留下', () {
      final ObserverBuffer buffer = ObserverBuffer(capacity: 4);
      buffer.add(ObserverRecord(
        at: DateTime(2026, 9, 26), type: 'action_cue', source: 'ws', text: 'a',
      ));
      buffer.add(ObserverRecord(
        at: DateTime(2026, 9, 26), type: 'stage-clock', source: 'audio', text: 'b',
      ));
      buffer.add(ObserverRecord(
        at: DateTime(2026, 9, 26), type: 'preset-applied', source: 'render', text: 'c',
      ));
      expect(buffer.filterByTypes(null), hasLength(3));
      expect(buffer.filterByTypes(const <String>[]), hasLength(3));
      expect(
        buffer.filterByTypes(<String>{'stage-clock'}).single.type,
        'stage-clock',
      );
      expect(buffer.filterByTypes(<String>{'nope'}), isEmpty);
    });

    test('sentence_ready：语义解析 + (epoch,seq,text) 去重 + 失败行忽略', () {
      const String line =
          '2026-09-26T14:33:18.582037Z  INFO mod: external-input '
          '收到事件 sentence_ready: '
          '{"epoch":0,"sentence_seq":1,"text":"你好呀","ts_ms":893}';
      final List<SentenceReadyLog> parsed = parseSentenceReadyLogs(<String>[
        line,
        line.replaceFirst('external-input', 'persona'),
        line.replaceFirst('external-input', 'memory'),
        '2026-09-26T14:33:19.1Z  INFO mod: external-input '
            '收到事件 sentence_ready: '
            '{"epoch":0,"sentence_seq":2,"text":"今天天气不错","ts_ms":1600}',
        '2026-09-26T14:33:20Z  INFO mod: external-input 收到事件 turn_ended: {"epoch":0}',
        'not json at all',
        '收到事件 sentence_ready: 没有 JSON payload',
        '收到事件 sentence_ready: {"epoch":0,"sentence_seq":3,"text":"半截"',
      ]);
      expect(
        parsed.map((SentenceReadyLog e) => e.text).toList(),
        <String>['你好呀', '今天天气不错'],
        reason: '同句被三个 Mod 各记一行 → 去重成一条；坏行忽略',
      );
      expect(parsed.first.epoch, 0);
      expect(parsed.first.sentenceSeq, 1);
      expect(parsed.first.tsMs, 893);
      expect(parsed.first.chars, 3);
      expect(parsed.last.chars, 6);

      // 去重键是三元组：同句不同 epoch 不算重复。
      expect(
        parseSentenceReadyLogs(<String>[
          'sentence_ready: {"epoch":0,"sentence_seq":1,"text":"甲"}',
          'sentence_ready: {"epoch":1,"sentence_seq":1,"text":"甲"}',
        ]),
        hasLength(2),
      );
    });

    test('未知类型照记（type 用 wire 名），心跳不进缓冲', () {
      final ObserverRecord? unknown = observeWsEvent(
        UnknownWsEvent(type: 'brand_new', data: <String, Object?>{'x': 1}),
        DateTime(2026, 9, 26),
      );
      expect(unknown, isNotNull);
      expect(unknown!.type, 'brand_new');
      expect(unknown.text, contains('keys=1'));
      expect(observeWsEvent(HeartbeatEvent(), DateTime(2026, 9, 26)), isNull);
    });

    test('请求 → 生效配对：seq 优先、同 field 最近一条、找不到写「尚无 ack」', () {
      final RenderEvent applied = parseRenderEvent(
        'preset-applied',
        <String, Object?>{
          'seq': 1,
          'field': 'head',
          'y': 0.240,
          'clamped': false,
          'degraded': true,
          'reason': 'ParamBodyAngleY',
        },
      )!;
      final RenderEvent later = parseRenderEvent(
        'preset-applied',
        <String, Object?>{'seq': 9, 'field': 'head', 'y': 0.5},
      )!;
      final PresetRequest req = PresetRequest(
        id: '',
        source: 'director',
        ts: DateTime(2026, 9, 26),
        intensity: 1,
        field: 'head',
        y: 0.30,
        seq: 1,
      );
      final PresetOverridePair pair = presetOverridePair(
        req,
        <RenderEvent>[later, applied],
      );
      expect(pair.request, contains('请求 field=head intensity=1 y=0.30'));
      expect(
        pair.applied,
        contains(
          '生效 y=0.240 clamped=false degraded=true reason=ParamBodyAngleY',
        ),
      );
      expect(pair.ack?.seq, 1, reason: '有 seq 时优先 seq 相同的那条 ack');
      expect(pair.hasAck, isTrue);

      final PresetRequest orphan = PresetRequest(
        id: '',
        source: 'director',
        ts: DateTime(2026, 9, 26),
        field: 'body',
        y: 0.1,
      );
      expect(
        presetOverridePair(orphan, <RenderEvent>[later, applied]).applied,
        '生效 尚无 ack',
      );
    });
  });
}
