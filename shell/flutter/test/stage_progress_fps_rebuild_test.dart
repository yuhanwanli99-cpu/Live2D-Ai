/// `progress` / `fps` 两条**派生值**通知的出口（2026-10-01，F-0010-2 / F-0001-4）。
///
/// # 现场
///
/// 渲染面每秒发一帧 `fps`、加载期连续发 `progress`；桥两条都 `notifyListeners()`。
/// 但「通知」不等于「有人重建」：
///   * **FPS 角标**读 `bridge.fps`，从前靠 `_onBridgeChanged` 末尾那句
///     无条件 `setState(() {})` 刷新——数值是新的，代价是**每一次**通知
///     （每秒 fps、加载期每帧 progress、每条 ack）都把整棵舞台重建一遍，
///     连平台视图那一层一起；
///   * **加载百分比**画在 `StageHost` 的幕布里，而那份 `progress` 是宿主
///     build 时的快照（`main.dart` 现读 `stageKey.currentState?.bridge?.progress`）——
///     宿主不是订阅者，桥再怎么通知它也不会重建，百分比就冻在那一帧。
///
/// # 现在的分工（本文件守的就是它）
///
/// ```text
/// fps      ──► bridge.fpsListenable ──► _FpsBadge（只重建这个小药丸）
/// progress ──► Live2DStage.onProgress ──► 宿主 setState 一个 double? 字段
/// phase/就绪 ─► Live2DStage 自己 setState（build 真的读了它）
/// ```
///
/// # 为什么用 `debugAttachTransport`
///
/// `flutter test` 跑在 VM 上，`buildLive2DHost` 解析到 `live2d_host_stub.dart`，
/// 占位实现**从不回调** `onTransport` ⇒ 挂上去的舞台 `_bridge` 恒为 null，
/// 整条链一行都驱动不了（`test/asset_guard_preset_dispatch_test.dart` 末尾
/// 有一条取证用例钉着这个事实）。`debugAttachTransport` 是给测试的唯一口子，
/// 生产路径不动。
library;

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

class _FakeTransport implements Live2DTransport {
  final StreamController<String> _controller =
      StreamController<String>.broadcast();

  @override
  Stream<String> get events => _controller.stream;

  @override
  Future<void> send(String json) async {}

  @override
  Future<void> dispose() async {
    await _controller.close();
  }

  void emit(String frame) => _controller.add(frame);
}

String _progressFrame(double value) =>
    jsonEncode(<String, Object?>{
      'version': 1,
      'type': 'progress',
      'payload': <String, Object?>{'progress': value},
    });

String _fpsFrame(int value) => jsonEncode(<String, Object?>{
  'version': 1,
  'type': 'fps',
  'payload': <String, Object?>{'fps': value},
});

const String _readyFrame = '{"version":1,"type":"ready","payload":{}}';

/// 舞台 + 一个**像宿主那样**消费进度的小壳（`main.dart` 接法的同款）。
///
/// `hostBuilds` 数的是「宿主重建了几次」——这正是 F-0001-4 缺的那一环。
class _HostHarness extends StatefulWidget {
  const _HostHarness({required this.stageKey, required this.onBuild});

  final GlobalKey<Live2DStageState> stageKey;
  final void Function(int builds) onBuild;

  @override
  State<_HostHarness> createState() => _HostHarnessState();
}

class _HostHarnessState extends State<_HostHarness> {
  double? progress;
  int builds = 0;

  @override
  Widget build(BuildContext context) {
    widget.onBuild(builds);
    builds++;
    return Column(
      children: <Widget>[
        Expanded(
          child: Live2DStage(
            key: widget.stageKey,
            onProgress: (double? value) {
              // 与 `onPhaseChanged`/`onAck` 同款的宿主接法：只 setState
              // 一个 double? 字段，值没变就不动（加载期进度帧可能很密）。
              if (value == progress) return;
              setState(() => progress = value);
            },
          ),
        ),
        // StageHost 的加载幕布文本（同款口径），独立于舞台。
        Text(
          progress == null
              ? '模型加载中…'
              : '模型加载中… ${(progress! * 100).round()}%',
        ),
      ],
    );
  }
}

/// 一次 `emit` 要两帧才落到界面上：流事件在微任务里投递，而
/// `tester.pump()` 的帧已经先建完了（第一帧只是把事件收进来）。
Future<void> _settleEvent(WidgetTester tester) async {
  await tester.pump();
  await tester.pump();
}

void main() {
  final Finder hostSubtree = find.descendant(
    of: find.byType(Live2DStage),
    matching: find.byType(Builder),
  );

  group('bridge：progress/fps 通知与订阅口', () {
    test('fps 帧 → 值 + 通用通知 + 订阅口（具体数值）', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      int notifies = 0;
      int listenableNotifies = 0;
      bridge.addListener(() => notifies++);
      bridge.fpsListenable.addListener(() => listenableNotifies++);

      transport.emit(_fpsFrame(30));
      await Future<void>.delayed(Duration.zero);
      expect(bridge.fps, 30);
      expect(bridge.fpsListenable.value, 30);
      expect(notifies, 1, reason: '通用通知照旧发（协议与既有订阅方不能变）');
      expect(listenableNotifies, 1, reason: '角标订阅的是这条');

      transport.emit(_fpsFrame(42));
      await Future<void>.delayed(Duration.zero);
      expect(bridge.fps, 42);
      expect(bridge.fpsListenable.value, 42);
      expect(notifies, 2);
      expect(listenableNotifies, 2);

      bridge.dispose();
    });

    test('progress 帧 → 值更新（`onProgress` 的数据源）', () async {
      final _FakeTransport transport = _FakeTransport();
      final Live2DBridge bridge = Live2DBridge(transport);
      int notifies = 0;
      bridge.addListener(() => notifies++);

      transport.emit(_progressFrame(0.4));
      await Future<void>.delayed(Duration.zero);
      expect(bridge.progress, 0.4);
      expect(notifies, 1);

      bridge.dispose();
    });
  });

  group('F-0001-4：progress 通知 → 宿主重建 +1、幕布文本真的变', () {
    testWidgets('推一次 progress → 宿主重建一次，文本是具体百分比', (WidgetTester tester) async {
      final GlobalKey<Live2DStageState> stageKey =
          GlobalKey<Live2DStageState>();
      int hostBuilds = 0;
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: _HostHarness(
              stageKey: stageKey,
              onBuild: (int builds) => hostBuilds = builds,
            ),
          ),
        ),
      );
      // 只有这一条口子能在 VM 里挂上 fake transport。
      final _FakeTransport transport = _FakeTransport();
      stageKey.currentState!.debugAttachTransport(transport);
      await tester.pump();
      expect(find.text('模型加载中…'), findsOneWidget);

      final int before = hostBuilds;
      transport.emit(_progressFrame(0.4));
      await _settleEvent(tester);

      expect(
        hostBuilds,
        before + 1,
        reason: 'progress 通知没有让宿主重建 —— 幕布百分比会一直冻在旧值（F-0001-4）',
      );
      expect(
        find.text('模型加载中… 40%'),
        findsOneWidget,
        reason: '重建了但文本不是新进度（断言具体百分比，不是「不为空」）',
      );

      // 再推一帧新的 → 再重建一次、文本再来一次具体值。
      transport.emit(_progressFrame(0.75));
      await _settleEvent(tester);
      expect(hostBuilds, before + 2);
      expect(find.text('模型加载中… 75%'), findsOneWidget);
    });

    testWidgets('同值 progress 不重复回调（加载期不把宿主拖进无谓重建）', (
      WidgetTester tester,
    ) async {
      final GlobalKey<Live2DStageState> stageKey =
          GlobalKey<Live2DStageState>();
      int hostBuilds = 0;
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: _HostHarness(
              stageKey: stageKey,
              onBuild: (int builds) => hostBuilds = builds,
            ),
          ),
        ),
      );
      final _FakeTransport transport = _FakeTransport();
      stageKey.currentState!.debugAttachTransport(transport);
      await tester.pump();

      transport.emit(_progressFrame(0.4));
      await _settleEvent(tester);
      final int afterFirst = hostBuilds;
      transport.emit(_progressFrame(0.4));
      await _settleEvent(tester);
      expect(
        hostBuilds,
        afterFirst,
        reason: '同值进度重复回调 → 宿主被拆房重建（加载期会刷很多帧）',
      );
      expect(find.text('模型加载中… 40%'), findsOneWidget);
    });
  });

  group('F-0010-2：fps 角标自己重建，不吃整棵舞台', () {
    testWidgets('推一次 fps → 角标文本变成具体值', (WidgetTester tester) async {
      final GlobalKey<Live2DStageState> stageKey =
          GlobalKey<Live2DStageState>();
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(
            body: Live2DStage(key: stageKey),
          ),
        ),
      );
      final _FakeTransport transport = _FakeTransport();
      stageKey.currentState!.debugAttachTransport(transport);
      await tester.pump();

      // 角标只在 ready 之后出现（显隐由舞台自己的 build 决定）。
      transport.emit(_readyFrame);
      await _settleEvent(tester);
      expect(find.textContaining('FPS'), findsNothing, reason: '还没收到 fps 帧');

      transport.emit(_fpsFrame(30));
      await _settleEvent(tester);
      expect(find.text('30 FPS'), findsOneWidget);

      transport.emit(_fpsFrame(42));
      await _settleEvent(tester);
      expect(
        find.text('42 FPS'),
        findsOneWidget,
        reason: 'fps 角标停在旧值 —— 通知没人消费（F-0010-2 的旧现场）',
      );
      expect(find.text('30 FPS'), findsNothing);
    });

    testWidgets('**最小重建面**：fps 推送不重建平台视图那一层', (WidgetTester tester) async {
      final GlobalKey<Live2DStageState> stageKey =
          GlobalKey<Live2DStageState>();
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(body: Live2DStage(key: stageKey)),
        ),
      );
      final _FakeTransport transport = _FakeTransport();
      stageKey.currentState!.debugAttachTransport(transport);
      await tester.pump();
      transport.emit(_readyFrame);
      await _settleEvent(tester);

      expect(hostSubtree, findsOneWidget, reason: '平台视图子树找不到，下面的相等断言会恒真');
      final Builder before = tester.widget<Builder>(hostSubtree);

      transport.emit(_fpsFrame(42));
      await _settleEvent(tester);
      final Builder after = tester.widget<Builder>(hostSubtree);
      // 先证明这一帧**真的被消费了**（否则下面的「没重建」可能是事件还没到）。
      expect(find.text('42 FPS'), findsOneWidget);

      expect(
        identical(before, after),
        isTrue,
        reason: '每秒一帧 fps 把整棵舞台（含平台视图那一层）重建了一遍 —— '
            '角标应当只重建自己（`_FpsBadge` 订阅 `fpsListenable`）',
      );
    });

    testWidgets('对照：阶段/就绪变化**仍然**重建舞台（角标要能出现）', (WidgetTester tester) async {
      final GlobalKey<Live2DStageState> stageKey =
          GlobalKey<Live2DStageState>();
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(body: Live2DStage(key: stageKey)),
        ),
      );
      final _FakeTransport transport = _FakeTransport();
      stageKey.currentState!.debugAttachTransport(transport);
      await tester.pump();
      expect(hostSubtree, findsOneWidget);
      final Builder before = tester.widget<Builder>(hostSubtree);

      transport.emit(_readyFrame);
      await _settleEvent(tester);

      final Builder after = tester.widget<Builder>(hostSubtree);
      expect(
        identical(before, after),
        isFalse,
        reason: '就绪态是 build 真的读了的东西 —— 它变了必须重建（否则角标永远不出现）',
      );
      expect(find.textContaining('FPS'), findsNothing);
    });
  });
}
