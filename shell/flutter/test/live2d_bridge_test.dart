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
  _mainSwapModelTests();
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

  // ── 舞台角标 + 回执 ────────────────────────────────────────────
  //
  // 2026-09-11：这里原有一组 `action-state`（动作通道）测试，
  // 随动作子系统（`live2d_perform_action` 工具 / `action_state` 帧 /
  // `sendActionState`）整条移除。缩放 / 背景 / 回执三条通路不受影响。

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

// ── A2（rc.2 2026-09-12）：换模必须等回执 ──────────────────────────
//
// `POST /models/{id}/activate` 只改后端 registry；真正换皮发生在 iframe 里
// （`sync.payload.model` → 渲染面 reload → `loaded` 帧）。这三条守的就是
// 「收条之前不说已切换」，也是「激活了但没换皮」的回归钉子。

void _mainSwapModelTests() {
  test('swapModel：发 sync(model) 并等 loaded → true', () async {
    final transport = _FakeTransport();
    final bridge = Live2DBridge(transport);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);

    final Future<bool> pending = bridge.swapModel('/models/neko/neko.model3.json');
    await Future<void>.delayed(Duration.zero);
    // 发出去的必须是 sync + model（渲染面只认这个字段）。
    expect(transport.sent.last, contains('"type":"sync"'));
    expect(transport.sent.last, contains('/models/neko/neko.model3.json'));

    transport.emit(
      '{"version":1,"type":"loaded","payload":{"url":"/models/neko/neko.model3.json"}}',
    );
    expect(await pending, isTrue);
    expect(bridge.model, '/models/neko/neko.model3.json');
    bridge.dispose();
  });

  test('swapModel：回执报的是**别的** url → false（没换成）', () async {
    final transport = _FakeTransport();
    final bridge = Live2DBridge(transport);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);

    final Future<bool> pending = bridge.swapModel('/models/wanted.model3.json');
    await Future<void>.delayed(Duration.zero);
    transport.emit(
      '{"version":1,"type":"loaded","payload":{"url":"/models/other.model3.json"}}',
    );
    expect(await pending, isFalse, reason: '换了别的模型不算「换成」');
    bridge.dispose();
  });

  test('swapModel：没有回执 → false（宁可说没换成，也不谎报）', () async {
    final transport = _FakeTransport();
    final bridge = Live2DBridge(transport);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);

    final bool ok = await bridge.swapModel(
      '/models/never.model3.json',
      timeout: const Duration(milliseconds: 30),
    );
    expect(ok, isFalse);
    bridge.dispose();
  });

  test('swapModel：渲染面已在 error 态 → 立刻 false（不白等超时）', () async {
    final transport = _FakeTransport();
    final bridge = Live2DBridge(transport);
    transport.emit('{"version":1,"type":"error","payload":{"message":"boom"}}');
    await Future<void>.delayed(Duration.zero);
    expect(bridge.phase, Live2DBridgePhase.error);

    final bool ok = await bridge.swapModel(
      '/models/x.model3.json',
      timeout: const Duration(seconds: 5),
    );
    expect(ok, isFalse);
    bridge.dispose();
  });
}
