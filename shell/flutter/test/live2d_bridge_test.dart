import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';

class _FakeTransport implements Live2DTransport {
  final StreamController<String> _controller =
      StreamController<String>.broadcast();
  final List<String> sent = <String>[];
  bool disposed = false;

  @override
  Stream<String> get events => _controller.stream;

  @override
  Future<void> send(String json) async => sent.add(json);

  @override
  Future<void> dispose() async {
    disposed = true;
    await _controller.close();
  }

  void emit(String frame) => _controller.add(frame);
}

void main() {
  test('ready 前命令入队，ready 后按序 flush', () async {
    final transport = _FakeTransport();
    final bridge = Live2DBridge(transport);

    await bridge.sendSync(model: '/models/bai/runtime/bai.model3.json', dark: true);
    expect(transport.sent, isEmpty, reason: 'ready 前不应发送');
    expect(bridge.queuedCount, 1);

    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);

    expect(bridge.phase, Live2DBridgePhase.ready);
    expect(transport.sent, hasLength(1));
    expect(transport.sent.first, contains('"type":"sync"'));
    await bridge.destroy();
  });

  test('容忍无 version 的遗留 stage-ack 与未知 type', () async {
    final transport = _FakeTransport();
    final bridge = Live2DBridge(transport);

    transport.emit('{"type":"stage-ack","applied":true}');
    transport.emit('not json at all');
    transport.emit('{"version":1,"type":"future-type","payload":{}}');
    await Future<void>.delayed(Duration.zero);

    expect(bridge.phase, Live2DBridgePhase.loading);
    expect(bridge.errorMessage, isNull);
    bridge.dispose();
  });

  // ── P5：动作通道 + 舞台角标 + 回执 ──────────────────────────────

  group('action-state 的**根级 action**（渲染面按根读，放 payload 里会静默失效）', () {
    test('action 在信封根，state/strength 在 payload', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      await bridge.sendActionState(action: 'nod', strength: 2);

      expect(transport.sent, hasLength(1));
      final Map<String, Object?> frame =
          jsonDecode(transport.sent.single) as Map<String, Object?>;
      // 这两条断言就是「模型到底会不会动」的全部依据。
      expect(
        frame['action'],
        'nod',
        reason: '渲染面从**信封根**读 action（l2d-wasm-demo/src/main.rs:380）；'
            '放进 payload 会让 action 为空、模型不动且不报错',
      );
      expect((frame['payload']! as Map<String, Object?>)['state'], 'start');
      expect((frame['payload']! as Map<String, Object?>)['strength'], 2);
      expect(
        (frame['payload']! as Map<String, Object?>).containsKey('action'),
        isFalse,
        reason: 'payload 里的 action 是死字段，不要写进去误导后来人',
      );
      await bridge.destroy();
    });

    test('state=end 用于收口（**不是 stop**——渲染面没有 stop 分支）', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      await bridge.sendActionState(action: 'tilt', state: 'end');
      final Map<String, Object?> frame =
          jsonDecode(transport.sent.single) as Map<String, Object?>;
      expect(frame['action'], 'tilt');
      expect((frame['payload']! as Map<String, Object?>)['state'], 'end');
      expect(
        (frame['payload']! as Map<String, Object?>).containsKey('strength'),
        isFalse,
      );
      await bridge.destroy();
    });

    test('传非 start/end 的 state 会被断言拦下（渲染面会静默忽略）', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      expect(
        () => bridge.sendActionState(action: 'nod', state: 'stop'),
        throwsA(isA<AssertionError>()),
      );
      bridge.dispose();
    });

    test('空动作名不发（避免渲染面收到一个停不掉的空动作）', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);
      await bridge.sendActionState(action: '');
      expect(transport.sent, isEmpty);
      await bridge.destroy();
    });
  });

  group('stage-zoom / stage-bg：只发意图，不算数值', () {
    test('stage-zoom 发 dir，不发 scale', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      for (final String dir in <String>['in', 'out', 'reset']) {
        await bridge.sendStageZoom(dir);
      }
      expect(transport.sent, hasLength(3));
      final Map<String, Object?> last =
          jsonDecode(transport.sent.last) as Map<String, Object?>;
      expect(last['type'], 'stage-zoom');
      final Map<String, Object?> payload = last['payload']! as Map<String, Object?>;
      expect(payload['dir'], 'reset');
      expect(
        payload.containsKey('scale'),
        isFalse,
        reason: '渲染面自己算缩放（±10% clamp 0.5..2.0），发数值会被忽略',
      );
      await bridge.destroy();
    });

    test('stage-bg 用 dataUrl；传 null 变成空串（渲染面把空串当清除）', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      await bridge.sendStageBg('data:image/png;base64,AAAA');
      await bridge.sendStageBg(null);

      final Map<String, Object?> setFrame =
          jsonDecode(transport.sent[0]) as Map<String, Object?>;
      expect(setFrame['type'], 'stage-bg');
      expect(
        (setFrame['payload']! as Map<String, Object?>)['dataUrl'],
        'data:image/png;base64,AAAA',
      );
      final Map<String, Object?> clearFrame =
          jsonDecode(transport.sent[1]) as Map<String, Object?>;
      expect((clearFrame['payload']! as Map<String, Object?>)['dataUrl'], '');
      await bridge.destroy();
    });
  });

  group('stage-ack 解析（**收条前不得提示已生效**）', () {
    test('解析 applied / msg_type / scale / offset', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      final List<StageAckEvent> acks = <StageAckEvent>[];
      bridge.acks.listen(acks.add);

      // 真实形状：**没有 version**，字段在根上。
      transport.emit(
        '{"type":"stage-ack","applied":true,"msg_type":"stage-zoom",'
        '"scale":1.21,"offset_x":0.0,"offset_y":0.0}',
      );
      await Future<void>.delayed(Duration.zero);

      expect(acks, hasLength(1));
      expect(acks.single.applied, isTrue);
      expect(acks.single.msgType, 'stage-zoom');
      expect(acks.single.scale, closeTo(1.21, 1e-9));
      expect(bridge.lastAck, isNotNull);
      bridge.dispose();
    });

    test('applied=false 也要能读出来（渲染面拒绝了）', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      transport.emit('{"type":"stage-ack","applied":false,"msg_type":"sync"}');
      await Future<void>.delayed(Duration.zero);
      expect(bridge.lastAck!.applied, isFalse);
      bridge.dispose();
    });

    test('缺字段/类型不对 → 不抛，缺的为 null', () async {
      final transport = _FakeTransport();
      final bridge = Live2DBridge(transport);
      transport.emit('{"type":"stage-ack"}');
      transport.emit('{"type":"stage-ack","scale":"nope","msg_type":7}');
      await Future<void>.delayed(Duration.zero);
      expect(bridge.lastAck, isNotNull);
      expect(bridge.lastAck!.scale, isNull);
      expect(bridge.lastAck!.msgType, '');
      expect(bridge.errorMessage, isNull, reason: '坏回执不该让桥进 error');
      bridge.dispose();
    });
  });

  test('error 帧进入 error 状态并给出可见消息', () async {
    final transport = _FakeTransport();
    final bridge = Live2DBridge(transport);

    transport.emit('{"version":1,"type":"error","payload":{"message":"模型加载失败"}}');
    await Future<void>.delayed(Duration.zero);

    expect(bridge.phase, Live2DBridgePhase.error);
    expect(bridge.errorMessage, '模型加载失败');
    bridge.dispose();
  });
}
