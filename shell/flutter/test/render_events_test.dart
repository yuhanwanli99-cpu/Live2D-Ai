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
}
