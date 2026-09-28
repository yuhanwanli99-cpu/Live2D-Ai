/// F-0002-1（P1）· 同值不重发：`stage-bg` 是**整份状态帧**（dataUrl 可达 1.5 M 字符），
/// 重发一帧的代价是渲染面**重新 base64 → Blob → createImageBitmap → write_texture**。
///
/// # 去重只许去**同值**
///
/// 这条通道背着资产契约 A6 / 舞台背景透传（改图必须立刻下发、iframe 重建必须补发），
/// 所以下面每一组都成对写：**同值必须少发**，**真变化必须照发**。
///
/// 现状（RED）：`sendStageBg` 无条件 `_enqueueOrSend`，音量滑杆每帧一次偏好变更
/// 就整串重发一次 ⇒ 设了壁纸的用户一拖滑杆，主线程同步 MB 级序列化 + iframe 大串拷贝。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';

/// 假的传输面：只记账，不真的过 postMessage。
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

  /// 数一数 `type` 是 `stage-bg` 的帧。
  int get stageBgFrames => sent.where((String json) {
    final Object? decoded = jsonDecode(json);
    return decoded is Map && decoded['type'] == 'stage-bg';
  }).length;

  /// 第 n 条 `stage-bg` 帧里的 `dataUrl`。
  List<String> get stageBgUrls => <String>[
    for (final String json in sent)
      if (jsonDecode(json) case final Map<Object?, Object?> frame
          when frame['type'] == 'stage-bg')
        ((frame['payload']! as Map<Object?, Object?>)['dataUrl']! as String),
  ];
}

const String _u1 = 'data:image/png;base64,AAAA';
const String _u2 = 'data:image/png;base64,BBBB';

void main() {
  /// 造一个**已经 ready** 的桥（真实时序：ready 之前的帧是排队，不算「发出去过」）。
  Future<(_FakeTransport, Live2DBridge)> readyBridge() async {
    final _FakeTransport transport = _FakeTransport();
    final Live2DBridge bridge = Live2DBridge(transport);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);
    return (transport, bridge);
  }

  test('同一张图连发两次：只出一帧（现状两帧 ⇒ 每个偏好变更都重发整串）', () async {
    final (_FakeTransport transport, Live2DBridge bridge) = await readyBridge();

    await bridge.sendStageBg(_u1);
    await bridge.sendStageBg(_u1);

    expect(
      transport.stageBgFrames,
      1,
      reason: '同值重发 = 渲染面重解一次整图；去重只去同值，真变化不许吞',
    );
    expect(transport.stageBgUrls, <String>[_u1]);
    await bridge.destroy();
  });

  test('改图必须照发（去重不许把真变化吞掉）', () async {
    final (_FakeTransport transport, Live2DBridge bridge) = await readyBridge();

    await bridge.sendStageBg(_u1);
    await bridge.sendStageBg(_u2);

    expect(transport.stageBgFrames, 2);
    expect(transport.stageBgUrls, <String>[_u1, _u2]);
    await bridge.destroy();
  });

  test('清图（null → 空串）也是一次真实变化，必须下发', () async {
    final (_FakeTransport transport, Live2DBridge bridge) = await readyBridge();

    await bridge.sendStageBg(_u1);
    await bridge.sendStageBg(null);
    await bridge.sendStageBg(null); // 第二次才是同值

    expect(transport.stageBgFrames, 2);
    expect(transport.stageBgUrls, <String>[_u1, '']);
    await bridge.destroy();
  });

  test('资产契约 A6：**重建 iframe**（新桥）必须重发同一张图', () async {
    // 真实时序：Live2DStage._attach 每次挂桥都新建一个 Live2DBridge，
    // 所以「去重记忆」必须随桥而亡——否则重挂之后舞台永远是黑底。
    final (_FakeTransport first, Live2DBridge bridgeA) = await readyBridge();
    await bridgeA.sendStageBg(_u1);
    expect(first.stageBgFrames, 1);
    await bridgeA.destroy();

    final (_FakeTransport second, Live2DBridge bridgeB) = await readyBridge();
    await bridgeB.sendStageBg(_u1);
    expect(
      second.stageBgFrames,
      1,
      reason: '换桥 = 渲染面是新的一份，同值也必须补发一次',
    );
    await bridgeB.destroy();
  });

  test('ready 前排队的那一帧**不算「发出去过」**：宁可多发一帧，不可丢图', () async {
    final _FakeTransport transport = _FakeTransport();
    final Live2DBridge bridge = Live2DBridge(transport);

    // ready 前：入队（队列可能被 maxQueue 截断，所以不能当成已送达）。
    await bridge.sendStageBg(_u1);
    expect(transport.stageBgFrames, 0, reason: 'ready 前不发');

    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await Future<void>.delayed(Duration.zero);
    expect(transport.stageBgFrames, 1, reason: 'ready 后按序 flush');

    await bridge.sendStageBg(_u1);
    expect(
      transport.stageBgFrames,
      2,
      reason: '排队过的帧无法证明送达（可能被 maxQueue 挤掉）⇒ 补发一次是**故意的**',
    );

    await bridge.sendStageBg(_u1);
    expect(transport.stageBgFrames, 2, reason: '补发之后进入稳态：同值不再发');
    await bridge.destroy();
  });
}
