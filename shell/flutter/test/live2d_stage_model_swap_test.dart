import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

class _FakeTransport implements Live2DTransport {
  final StreamController<String> _events = StreamController<String>.broadcast();
  final List<String> sent = <String>[];

  @override
  Stream<String> get events => _events.stream;

  @override
  Future<void> send(String json) async => sent.add(json);

  @override
  Future<void> dispose() async {
    await _events.close();
  }

  void emit(String frame) => _events.add(frame);
}

class _Harness extends StatefulWidget {
  const _Harness({required this.stageKey, this.onSwapFailed});

  final GlobalKey<Live2DStageState> stageKey;
  final ValueChanged<String>? onSwapFailed;

  @override
  State<_Harness> createState() => _HarnessState();
}

class _HarnessState extends State<_Harness> {
  String? model = '/models/a/a.model3.json';

  void select(String next) => setState(() => model = next);

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(
      body: Live2DStage(
        key: widget.stageKey,
        model: model,
        onModelSwapFailed: widget.onSwapFailed,
      ),
    ),
  );
}

void main() {
  testWidgets('didUpdateWidget：model 变化 → 热换，只认匹配的 loaded', (
    WidgetTester tester,
  ) async {
    final GlobalKey<Live2DStageState> stageKey =
        GlobalKey<Live2DStageState>();
    String? failure;
    await tester.pumpWidget(
      _Harness(stageKey: stageKey, onSwapFailed: (String m) => failure = m),
    );
    final _FakeTransport transport = _FakeTransport();
    stageKey.currentState!.debugAttachTransport(transport);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await tester.pump();
    expect(
      transport.sent.any((String f) => f.contains('a.model3.json')),
      isTrue,
    );

    final _HarnessState harness = tester.state<_HarnessState>(
      find.byType(_Harness),
    );
    harness.select('/models/b/b.model3.json');
    await tester.pump();
    expect(
      transport.sent.last,
      contains('/models/b/b.model3.json'),
      reason: 'model 变化必须发 sync，而不是重载 iframe',
    );

    // 旧帧先到：必须忽略，不能算成已切换、也不能报失败。
    transport.emit(
      '{"version":1,"type":"loaded","payload":{"url":"/models/a/a.model3.json"}}',
    );
    await tester.pump();
    expect(failure, isNull);

    // 匹配帧才结束等待；仍不报失败。
    transport.emit(
      '{"version":1,"type":"loaded","payload":{"url":"/models/b/b.model3.json"}}',
    );
    await tester.pump();
    expect(failure, isNull);
    expect(stageKey.currentState!.phase, Live2DBridgePhase.ready);
  });

  testWidgets('didUpdateWidget：换模 error → 失败回调，舞台不整体进 error', (
    WidgetTester tester,
  ) async {
    final GlobalKey<Live2DStageState> stageKey =
        GlobalKey<Live2DStageState>();
    String? failure;
    await tester.pumpWidget(
      _Harness(stageKey: stageKey, onSwapFailed: (String m) => failure = m),
    );
    final _FakeTransport transport = _FakeTransport();
    stageKey.currentState!.debugAttachTransport(transport);
    transport.emit('{"version":1,"type":"ready","payload":{}}');
    await tester.pump();

    final _HarnessState harness = tester.state<_HarnessState>(
      find.byType(_Harness),
    );
    harness.select('/models/bad/bad.model3.json');
    await tester.pump();
    transport.emit(
      '{"version":1,"type":"error","payload":{"message":"模型加载失败"}}',
    );
    await tester.pump();

    expect(failure, isNotNull);
    expect(failure, contains('模型加载失败'));
    expect(
      stageKey.currentState!.phase,
      Live2DBridgePhase.ready,
      reason: '旧模型还在渲染，不能因为一次换模失败把舞台打成 error',
    );
  });
}
