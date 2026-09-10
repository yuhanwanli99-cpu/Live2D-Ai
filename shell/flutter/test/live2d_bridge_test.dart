import 'dart:async';

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
