/// 0.3.6 T2（Flutter）：模型 epoch 门控 + 换模成功后的 cue / 幅度重绑。
///
/// # 覆盖的契约
///
/// - `loaded` / `error` ack 带 `model_id` / `model_epoch`；Flutter **只认当前
///   epoch** 的 loaded / error / preset 回执，旧 epoch 迟到帧丢弃；
///   **字段缺失 = legacy 接受**（不得因缺字段丢帧）。
/// - 换模成功（匹配 URL 的 loaded，并采纳其 `model_epoch`）：清
///   `DirectorCuePlan` 与 `PresetStatus`、对新模型下发**一次** `preset none`、
///   按新 `model_id` 重设 `ActionScalesSyncer.setModelContext` 并补发一次有效幅度
///   （force 绕过同值去重，但仍不得冲掉 dev pin）。
/// - 换模失败：不换 epoch、不清计划、旧模型旧 cue 全不动。
/// - Flutter 发出的 `preset` 帧带当前已知 `model_epoch`。
/// - dev_mode 的导演可观测面能看到 model_id / model_epoch（不新增 WS 帧、不落盘）。
///
/// 说明：`main.dart` 经 `app/browser_io.dart` 依赖 `package:web`，Dart VM 里
/// **加载不了**；宿主接线（清计划 / 重设幅度 / 观测记录）因此用**源码扫描**钉住
/// （与 `test/action_scales_wiring_test.dart` / `wiring_test.dart` 同一先例）。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';
import 'package:live2d_ai_shell/live2d/render_events.dart';
import 'package:live2d_ai_shell/settings/sections/director_observer_section.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/dart_library.dart';
import 'support/source_scan.dart';

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

Map<String, Object?> _decode(String frame) =>
    jsonDecode(frame) as Map<String, Object?>;

Map<String, Object?> _payload(String frame) =>
    (_decode(frame)['payload']! as Map<String, Object?>);

/// 从一批已发帧里挑出某 type。
List<Map<String, Object?>> _withType(List<String> sent, String type) => <Map<String, Object?>>[
  for (final String raw in sent)
    if (_decode(raw)['type'] == type) _decode(raw),
];

void main() {
  _bridgeEpochGateTests();
  _bridgePresetStampTests();
  _stageSwapRebindTests();
  _hostWiringSourceGuards();
  _observerVisibilityTests();
}

// ─────────────────────────────────────────── ① bridge：epoch 门控

void _bridgeEpochGateTests() {
  group('epoch 门控（0.3.6 T2）', () {
    test('旧 epoch 的 loaded 迟到帧丢弃：不改身份、不结束换模；缺字段 legacy 照收', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      // 首次装载：epoch=1，带 model_id 与 field map 来源。
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/a.model3.json",'
        '"model_id":"bai","model_epoch":1,'
        '"field_map":{"source":"builtin","degraded":false}}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(bridge.modelEpoch, 1);
      expect(bridge.modelId, 'bai');
      expect(bridge.fieldMap!.source, 'builtin');
      expect(bridge.fieldMap!.degraded, isFalse);
      expect(bridge.lastLoaded!.url, '/m/a.model3.json');

      // 成功换模：渲染面把 epoch 推到 2。
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/b.model3.json",'
        '"model_id":"epsilon_free","model_epoch":2,'
        '"field_map":{"source":"external","degraded":true,"reason":"缺段"}}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(bridge.modelEpoch, 2);
      expect(bridge.modelId, 'epsilon_free');
      expect(bridge.fieldMap!.degraded, isTrue);
      expect(bridge.fieldMap!.reason, '缺段');

      // 再发一次换模，然后让**旧 epoch（1）**的匹配 URL loaded 迟到。
      final Future<bool> pending = bridge.swapModel('/m/c.model3.json');
      await Future<void>.delayed(Duration.zero);
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/c.model3.json",'
        '"model_id":"stale","model_epoch":1}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(
        bridge.modelId,
        'epsilon_free',
        reason: '旧 epoch 的 loaded 不得改当前模型身份',
      );
      expect(bridge.modelEpoch, 2);
      expect(bridge.lastLoaded!.modelId, 'epsilon_free');

      // 当前 epoch（2）的匹配帧才结束这次换模。
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/c.model3.json",'
        '"model_id":"c","model_epoch":2}}',
      );
      expect(await pending, isTrue);
      expect(bridge.modelId, 'c');
      bridge.dispose();
    });

    test('缺 model_epoch / model_id 的 loaded = legacy：照收，epoch 保持未知', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/old.model3.json"}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(bridge.model, '/m/old.model3.json');
      expect(bridge.modelLoaded, isTrue);
      expect(bridge.modelEpoch, isNull, reason: '旧渲染面不报 epoch');
      expect(bridge.lastLoaded!.modelEpoch, isNull);
      bridge.dispose();
    });

    test('旧 epoch 的 stage-ack 丢弃；缺字段 legacy 接受；当前 epoch 带进回执', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      final List<StageAckEvent> acks = <StageAckEvent>[];
      bridge.acks.listen(acks.add);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/b",'
        '"model_epoch":2}}',
      );
      await Future<void>.delayed(Duration.zero);

      // 旧 epoch（1）的回执：丢弃，不更新 lastAck、不进流。
      transport.emit(
        '{"type":"stage-ack","applied":true,"msg_type":"sync",'
        '"model_epoch":1,"scale":1.0}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(acks, isEmpty);
      expect(bridge.lastAck, isNull);

      // 当前 epoch（2）：接受，并把 model_epoch 带进回执。
      transport.emit(
        '{"type":"stage-ack","applied":true,"msg_type":"sync",'
        '"model_epoch":2,"scale":1.5}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(acks.single.modelEpoch, 2);
      expect(bridge.lastAck!.scale, 1.5);

      // 缺 model_epoch = legacy 接受（老渲染面）。
      transport.emit(
        '{"type":"stage-ack","applied":true,"msg_type":"stage-bg","scale":1.8}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(bridge.lastAck!.modelEpoch, isNull);
      expect(bridge.lastAck!.scale, 1.8);
      bridge.dispose();
    });

    test('旧 epoch 的 preset 回执丢弃；缺字段 legacy 接受；当前 epoch 带进 RenderEvent', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      final List<RenderEvent> events = <RenderEvent>[];
      bridge.renderEvents.listen(events.add);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/b",'
        '"model_epoch":2}}',
      );
      await Future<void>.delayed(Duration.zero);

      transport.emit(
        '{"version":1,"type":"preset-applied","payload":'
        '{"id":"smile","field":"expression","model_epoch":1}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(events, isEmpty, reason: '旧 epoch 的 preset 回执不得进导演日志');

      transport.emit(
        '{"version":1,"type":"preset-applied","payload":'
        '{"id":"smile","field":"expression","model_epoch":2}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(events.single.modelEpoch, 2);
      expect(formatRenderEvent(events.single), contains('model_epoch=2'));

      transport.emit(
        '{"version":1,"type":"preset-expired","payload":{"id":"smile"}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(events.last.modelEpoch, isNull, reason: '缺字段 = legacy 接受');
      bridge.dispose();
    });

    test('旧 epoch 的 error 不结束当前换模、不把桥打成 error；当前 epoch 的 error 才失败', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/a",'
        '"model_epoch":2}}',
      );
      await Future<void>.delayed(Duration.zero);

      final Future<bool> pending = bridge.swapModel('/m/bad.model3.json');
      await Future<void>.delayed(Duration.zero);
      // 旧 epoch 的 error：迟到帧，什么都不动。
      transport.emit(
        '{"version":1,"type":"error","payload":{"message":"旧失败","model_epoch":1}}',
      );
      await Future<void>.delayed(Duration.zero);
      expect(bridge.errorMessage, isNull);
      expect(bridge.lastSwapError, isNull);
      expect(bridge.phase, Live2DBridgePhase.ready);

      // 当前 epoch 的 error：结束本次换模（false），桥仍是 ready（旧模型还在）。
      transport.emit(
        '{"version":1,"type":"error","payload":{"message":"模型加载失败","model_epoch":2}}',
      );
      expect(await pending, isFalse);
      expect(bridge.lastSwapError, '模型加载失败');
      expect(bridge.phase, Live2DBridgePhase.ready);
      bridge.dispose();
    });
  });
}

// ─────────────────────────────────────────── ② bridge：preset 帧带 epoch

void _bridgePresetStampTests() {
  group('发出的 preset 帧带 model_epoch（0.3.6 T2）', () {
    test('无 loaded 时不带（旧渲染面 legacy）；采纳 epoch 后自动带上；可显式覆盖', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      transport.emit('{"version":1,"type":"ready","payload":{}}');
      await Future<void>.delayed(Duration.zero);

      await bridge.sendPreset('smile');
      expect(
        _payload(transport.sent.last).containsKey('model_epoch'),
        isFalse,
        reason: '还不知道 epoch = 旧渲染面 legacy，不应凭空造一个',
      );

      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/b",'
        '"model_epoch":2}}',
      );
      await Future<void>.delayed(Duration.zero);

      await bridge.sendPreset('nod', source: 'director', seq: 3);
      final Map<String, Object?> payload = _payload(transport.sent.last);
      expect(payload['id'], 'nod');
      expect(payload['seq'], 3);
      expect(payload['model_epoch'], 2, reason: 'Flutter 发出的 preset 帧带当前 epoch');

      await bridge.sendPreset('shake', modelEpoch: 9);
      expect(_payload(transport.sent.last)['model_epoch'], 9);
      bridge.dispose();
    });
  });
}

// ─────────────────────────────────────────── ③ stage：换模成功 / 失败

class _Harness extends StatefulWidget {
  const _Harness({
    required this.stageKey,
    this.onLoaded,
    this.onSwapped,
    this.onSwapFailed,
  });

  final GlobalKey<Live2DStageState> stageKey;
  final ValueChanged<ModelLoadedInfo>? onLoaded;
  final ValueChanged<ModelLoadedInfo>? onSwapped;
  final ValueChanged<String>? onSwapFailed;

  @override
  State<_Harness> createState() => _HarnessState();
}

class _HarnessState extends State<_Harness> {
  String? model = '/m/a.model3.json';

  void select(String next) => setState(() => model = next);

  @override
  Widget build(BuildContext context) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(
      body: Live2DStage(
        key: widget.stageKey,
        model: model,
        onModelLoaded: widget.onLoaded,
        onModelSwapped: widget.onSwapped,
        onModelSwapFailed: widget.onSwapFailed,
      ),
    ),
  );
}

/// 把舞台挂起来并走到 ready + 首次 loaded。
Future<_FakeTransport> _pumpReadyStage(
  WidgetTester tester,
  GlobalKey<Live2DStageState> stageKey, {
  ValueChanged<ModelLoadedInfo>? onLoaded,
  ValueChanged<ModelLoadedInfo>? onSwapped,
  ValueChanged<String>? onSwapFailed,
}) async {
  await tester.pumpWidget(
    _Harness(
      stageKey: stageKey,
      onLoaded: onLoaded,
      onSwapped: onSwapped,
      onSwapFailed: onSwapFailed,
    ),
  );
  final _FakeTransport transport = _FakeTransport();
  stageKey.currentState!.debugAttachTransport(transport);
  transport.emit('{"version":1,"type":"ready","payload":{}}');
  await tester.pump();
  transport.emit(
    '{"version":1,"type":"loaded","payload":{"url":"/m/a.model3.json",'
    '"model_id":"bai","model_epoch":1}}',
  );
  await tester.pump();
  return transport;
}

void _stageSwapRebindTests() {
  group('换模成功 / 失败的重绑（0.3.6 T2）', () {
    testWidgets('成功：清 PresetStatus + 下发一次 preset none（新 epoch）+ 回调带新身份', (WidgetTester tester) async {
      final GlobalKey<Live2DStageState> stageKey = GlobalKey<Live2DStageState>();
      final List<ModelLoadedInfo> loaded = <ModelLoadedInfo>[];
      final List<ModelLoadedInfo> swapped = <ModelLoadedInfo>[];
      bool statusClearedAtSwap = false;
      final _FakeTransport transport = await _pumpReadyStage(
        tester,
        stageKey,
        onLoaded: loaded.add,
        onSwapped: (ModelLoadedInfo info) {
          swapped.add(info);
          // 回调发生时，舞台**已经**清了显示快照（顺序契约）。
          statusClearedAtSwap =
              stageKey.currentState?.presetStatus.value == null;
        },
      );
      expect(loaded.single.modelId, 'bai');

      // 老模型的 preset 回执 → 调试面板快照有值。
      transport.emit(
        '{"version":1,"type":"preset-applied","payload":'
        '{"id":"smile","field":"expression","seq":4,"model_epoch":1}}',
      );
      await tester.pump();
      expect(stageKey.currentState!.presetStatus.value, isNotNull);

      tester.state<_HarnessState>(find.byType(_Harness)).select('/m/b.model3.json');
      await tester.pump();
      await tester.pump();

      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/b.model3.json",'
        '"model_id":"epsilon_free","model_epoch":2,'
        '"field_map":{"source":"external","degraded":false}}}',
      );
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(swapped, hasLength(1));
      expect(swapped.single.modelId, 'epsilon_free');
      expect(swapped.single.modelEpoch, 2);
      expect(loaded.last.modelEpoch, 2);
      expect(statusClearedAtSwap, isTrue);
      expect(stageKey.currentState!.presetStatus.value, isNull);

      final List<Map<String, Object?>> presets = _withType(transport.sent, 'preset');
      final List<Map<String, Object?>> none = <Map<String, Object?>>[
        for (final Map<String, Object?> frame in presets)
          if (_payloadOf(frame)['id'] == 'none') frame,
      ];
      expect(none, hasLength(1), reason: '换模成功对渲染面下发**一次** preset none');
      expect(
        _payloadOf(none.single)['model_epoch'],
        2,
        reason: 'preset none 必须带**新** model_epoch',
      );
    });

    testWidgets('失败：不清 PresetStatus、不发 preset none、不回调；旧模型继续', (WidgetTester tester) async {
      final GlobalKey<Live2DStageState> stageKey = GlobalKey<Live2DStageState>();
      final List<ModelLoadedInfo> swapped = <ModelLoadedInfo>[];
      String? failure;
      final _FakeTransport transport = await _pumpReadyStage(
        tester,
        stageKey,
        onSwapped: swapped.add,
        onSwapFailed: (String m) => failure = m,
      );
      transport.emit(
        '{"version":1,"type":"preset-applied","payload":'
        '{"id":"smile","field":"expression","seq":4,"model_epoch":1}}',
      );
      await tester.pump();
      expect(stageKey.currentState!.presetStatus.value, isNotNull);

      tester.state<_HarnessState>(find.byType(_Harness)).select('/m/bad.model3.json');
      await tester.pump();
      await tester.pump();
      // 失败 error 带**当前** epoch（失败不换 epoch）。
      transport.emit(
        '{"version":1,"type":"error","payload":'
        '{"message":"模型加载失败","model_epoch":1}}',
      );
      await tester.pump();
      await tester.pump();
      await tester.pump();

      expect(failure, isNotNull);
      expect(failure, contains('模型加载失败'));
      expect(swapped, isEmpty, reason: '换模失败不得触发重绑');
      expect(
        stageKey.currentState!.presetStatus.value,
        isNotNull,
        reason: '失败不清计划 / 不清显示快照',
      );
      expect(
        _withType(transport.sent, 'preset'),
        isEmpty,
        reason: '失败不得下发 preset none',
      );
      expect(
        stageKey.currentState!.phase,
        Live2DBridgePhase.ready,
        reason: '旧模型还在渲染',
      );
    });

    testWidgets('换模成功后新的 cue 用新 model_epoch 下发', (WidgetTester tester) async {
      final GlobalKey<Live2DStageState> stageKey = GlobalKey<Live2DStageState>();
      final _FakeTransport transport = await _pumpReadyStage(tester, stageKey);
      tester.state<_HarnessState>(find.byType(_Harness)).select('/m/b.model3.json');
      await tester.pump();
      await tester.pump();
      transport.emit(
        '{"version":1,"type":"loaded","payload":{"url":"/m/b.model3.json",'
        '"model_id":"epsilon_free","model_epoch":2}}',
      );
      await tester.pump();
      await tester.pump();
      await tester.pump();

      await stageKey.currentState!.applyPreset(
        'smile',
        field: 'expression',
        seq: 9,
      );
      final Map<String, Object?> last = _payload(transport.sent.last);
      expect(last['id'], 'smile');
      expect(
        last['model_epoch'],
        2,
        reason: '换模后新的 cue 必须带新 epoch，旧 cue 才可能被渲染面 Ignore',
      );
    });
  });
}

Map<String, Object?> _payloadOf(Map<String, Object?> frame) =>
    (frame['payload']! as Map<String, Object?>);

// ─────────────────────────────────────────── ④ 宿主接线源码守卫

void _hostWiringSourceGuards() {
  group('宿主接线（源码守卫；main.dart 在 VM 里加载不了）', () {
    late String shellSource;

    setUpAll(() {
      shellSource = readLibrarySource('lib/main.dart');
    });

    test('Live2DStage 构造点接上 onModelLoaded / onModelSwapped', () {
      expect(
        shellSource,
        contains('onModelLoaded: _onStageModelLoaded'),
        reason: '模型身份要进 dev 可观测面',
      );
      expect(
        shellSource,
        contains('onModelSwapped: _onStageModelSwapped'),
        reason: '换模成功要清计划 + 重设幅度',
      );
    });

    test('换模成功：清 DirectorCuePlan + 按新 model_id 重设并 force 补发幅度', () {
      final String body = balancedFrom(
        shellSource,
        'void _onStageModelSwapped(',
        '{',
        '}',
      );
      expect(
        body,
        contains('_directorCues.clear()'),
        reason: '旧 cue 不得落到新模型（0.3.14 起计划用 clear 作废）',
      );
      expect(
        body,
        contains('_actionScalesSyncer.setModelContext('),
        reason: '按新 model_id 重设 D40 上下文',
      );
      expect(
        body,
        contains('_syncActionScalesNow(force: true)'),
        reason: '补发一次有效幅度；force 绕过同值去重但保留 pin 优先',
      );
      // 顺序：先清计划，再重设幅度（清计划不能等异步）。
      expect(
        body.indexOf('_directorCues.clear'),
        lessThan(body.indexOf('_actionScalesSyncer.setModelContext')),
      );
    });

    test('loaded 身份进导演可观测面（只住内存，不落盘、不新增 WS 帧）', () {
      final String body = balancedFrom(
        shellSource,
        'void _onStageModelLoaded(',
        '{',
        '}',
      );
      expect(body, contains('DirectorObserverFeed.instance.pushModelLoaded('));
      expect(body, contains('modelId: info.modelId'));
      expect(body, contains('modelEpoch: info.modelEpoch'));
    });

    test('舞台：换模成功这一侧清快照 + 下发预设 none，且顺序是「先清快照、后发帧」', () {
      final String stage = readLibrarySource('lib/live2d/live2d_stage.dart');
      final String body = balancedFrom(stage, 'Future<void> _afterModelSwapped(', '{', '}');
      expect(body, contains('presetStatus.value = null'));
      expect(body, contains("await applyPreset('none', source: 'swap')"));
      expect(
        body.indexOf('presetStatus.value = null'),
        lessThan(body.indexOf("await applyPreset('none', source: 'swap')")),
      );
      expect(
        body.indexOf('widget.onModelSwapped?.call(info)'),
        lessThan(body.indexOf("await applyPreset('none', source: 'swap')")),
        reason: '清计划要发生在下发 preset none 之前',
      );
    });
  });
}

// ─────────────────────────────────────────── ⑤ dev 面可观测

void _observerVisibilityTests() {
  group('dev_mode 导演可观测面能看到 model_id / model_epoch（0.3.6 T2）', () {
    setUp(() => DirectorObserverFeed.instance.clear());

    test('pushModelLoaded 记一条 B 栏 render 记录（内存缓冲）', () {
      final DirectorObserverFeed feed = DirectorObserverFeed.instance;
      feed.pushModelLoaded(
        url: '/m/b.model3.json',
        modelId: 'epsilon_free',
        modelEpoch: 2,
        fieldMapSource: 'external',
        fieldMapDegraded: false,
      );
      final ObserverRecord record = feed.events.records.single;
      expect(record.type, 'loaded');
      expect(record.source, 'render');
      expect(record.epoch, 2);
      expect(record.text, contains('model_id=epsilon_free'));
      expect(record.text, contains('model_epoch=2'));
      expect(record.text, contains('field_map=external'));
      // 只进内存：没有写 localStorage / SharedPreferences 的路径（不 import 任何存储库）。
      final String section = readLibrarySource(
        'lib/settings/sections/director_observer_section.dart',
      );
      expect(section.contains('shared_preferences'), isFalse);
      expect(section.contains('localStorage'), isFalse);
    });

    testWidgets('B 栏直接显示 model_id / model_epoch / field_map', (WidgetTester tester) async {
      DirectorObserverFeed.instance.pushModelLoaded(
        url: '/m/b.model3.json',
        modelId: 'epsilon_free',
        modelEpoch: 2,
        fieldMapSource: 'external',
        fieldMapDegraded: true,
        fieldMapReason: '缺段',
      );
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: SingleChildScrollView(
              child: DirectorObserverSection(
                devMode: true,
                loadLogLines: () async => <String>[],
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(find.textContaining('model_id=epsilon_free'), findsOneWidget);
      expect(find.textContaining('model_epoch=2'), findsOneWidget);
      expect(find.textContaining('field_map=external'), findsOneWidget);
      expect(find.textContaining('field_map_degraded=true'), findsOneWidget);
    });
  });
}
