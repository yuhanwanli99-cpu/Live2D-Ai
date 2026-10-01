/// `progress` 的**宿主消费口**真的接上了（2026-10-01，W1-e2 / F-0001-4）。
///
/// # 被修的是什么
///
/// 渲染面加载期连续发 `progress`，桥每条都 `notifyListeners()`。但宿主的加载
/// 幕布（`StageHost` 的「模型加载中… N%」）吃的是 `AppShell.stageProgress`，
/// 而 `main.dart` 从前是**现读快照**：
///
/// ```dart
/// stageProgress: _stageKey.currentState?.bridge?.progress,   // 只有外壳自己重建时才刷
/// ```
///
/// 桥的通知不会让外壳重建 ⇒ 百分比冻在加载开始那一帧。W1-e 把
/// `Live2DStage.onProgress` 做成了消费口（只在值变化时回调），W1-e2 在
/// `main.dart` 接上：持一个 `double? _stageProgress` 字段、变了才 `setState`。
///
/// # 为什么分两组测（`main.dart` 在 VM 里加载不了）
///
/// `main.dart` 经 `app/browser_io.dart` 依赖 `package:web`，在 `flutter test`
/// 里**加载不了**——所以「宿主读到新值」这件事没法用真泵测到**那个文件**。
/// 于是：
///
/// 1. **真泵组**：用**真的** `StageHost`（不是自己仿的 Text）演一遍宿主的接法
///    ——`onProgress` → `setState` → 幕布文本。推一帧 progress，断言幕布上的
///    **具体百分比**变了、宿主重建 +1，同值再推不重建。
/// 2. **结构守卫组**：扫 `main.dart`（先剥注释与字符串）断言三处接线同时在，
///    并且**旧的现读快照写法已经不在**。这一组是本文件对「真机那个文件」的
///    唯一可行守卫（同 `transient_results_test.dart` / `background_hydration_window_test.dart`
///    的先例），并且内建反例：把改动前的写法喂给同一个判据必须判**假**
///    （否则判据恒真，等于没守）。
library;

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/live2d/live2d_stage.dart';
import 'package:live2d_ai_shell/live2d/live2d_transport.dart';
import 'package:live2d_ai_shell/ui/stage_host.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

import 'support/source_scan.dart';

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

String _progressFrame(double value) => jsonEncode(<String, Object?>{
  'version': 1,
  'type': 'progress',
  'payload': <String, Object?>{'progress': value},
});

/// 宿主（`main.dart` 的接法**逐句同形**）+ **真的** `StageHost` 幕布。
///
/// 幕布不是仿的：`StageHost(phase: loading, progress: …)` 就是真机上画
/// 「模型加载中… N%」的那个 widget。
class _HostHarness extends StatefulWidget {
  const _HostHarness({required this.stageKey, required this.onBuild});

  final GlobalKey<Live2DStageState> stageKey;
  final void Function(int builds) onBuild;

  @override
  State<_HostHarness> createState() => _HostHarnessState();
}

class _HostHarnessState extends State<_HostHarness> {
  // main.dart 的同名字段与同款接法。
  double? _stageProgress;
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
            onProgress: (double? v) {
              if (mounted && v != _stageProgress) {
                setState(() => _stageProgress = v);
              }
            },
          ),
        ),
        Expanded(
          child: StageHost(
            stage: const SizedBox.expand(),
            phase: Live2DBridgePhase.loading,
            progress: _stageProgress,
          ),
        ),
      ],
    );
  }
}

Future<void> _settleEvent(WidgetTester tester) async {
  // 流事件在微任务里投递，而 `pump()` 那一帧已经先建完了。
  await tester.pump();
  await tester.pump();
}

// ── 结构守卫的判据（写在文件级，好让「反例」与「真实文件」走同一条判据） ──

/// `_ShellRootState` 里有进度字段。
final RegExp kStageProgressField = RegExp(r'double\?\s+_stageProgress\s*;');

/// `Live2DStage(...)` 上挂了 `onProgress`，回调里**同一个形参**既参与
/// 「值变了没有」的判断、又被写回 `_stageProgress`（形参名不写死，
/// 用反向引用——改个 `v` 成 `value` 不该把守卫判红）。
final RegExp kStageProgressCallback = RegExp(
  r'onProgress:\s*\(double\?\s+(\w+)\)\s*\{[\s\S]*?\1\s*!=\s*_stageProgress'
  r'[\s\S]*?_stageProgress\s*=\s*\1',
);

/// 传下去的是一份快照字段，而不是现读桥。
final RegExp kStageProgressPassedDown = RegExp(
  r'stageProgress:\s*_stageProgress\s*,',
);

/// 改动前的写法：`stageProgress:` 现读 `_stageKey.currentState?.bridge?.progress`。
final RegExp kStageProgressOldSnapshot = RegExp(
  r'stageProgress:\s*_stageKey\.currentState\?\.bridge\?\.progress',
);

/// 三处在、旧写法不在 —— 才算宿主真的消费了 progress。
bool wiresStageProgress(String src) =>
    kStageProgressField.hasMatch(src) &&
    kStageProgressCallback.hasMatch(src) &&
    kStageProgressPassedDown.hasMatch(src) &&
    !kStageProgressOldSnapshot.hasMatch(src);

void main() {
  group('真泵：宿主按 main.dart 的接法消费 progress → 幕布文本真的变', () {
    testWidgets('推一次 progress → 宿主重建 +1，幕布是**具体百分比**', (WidgetTester tester) async {
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
      // 还不知道进度 → 幕布是那句不带百分比的文案。
      expect(find.text('模型加载中…'), findsOneWidget);

      final int before = hostBuilds;
      transport.emit(_progressFrame(0.4));
      await _settleEvent(tester);

      expect(
        hostBuilds,
        before + 1,
        reason: 'progress 通知没让宿主重建 —— 幕布百分比会冻在旧值（F-0001-4）',
      );
      expect(
        find.text('模型加载中… 40%'),
        findsOneWidget,
        reason: '重建了但幕布文本不是新进度（断言具体百分比，不是「不为空」）',
      );

      transport.emit(_progressFrame(0.75));
      await _settleEvent(tester);
      expect(hostBuilds, before + 2);
      expect(find.text('模型加载中… 75%'), findsOneWidget);
    });

    testWidgets('同值 progress 不产生额外重建（加载期帧很密）', (WidgetTester tester) async {
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
        reason: '同值进度回调了两次 → 宿主被无谓重建（`v != _stageProgress` 那道闸没生效）',
      );
      expect(find.text('模型加载中… 40%'), findsOneWidget);
    });
  });

  group('结构守卫：main.dart 的三处接线（VM 里 main.dart 加载不了）', () {
    final String source = stripCommentsAndStrings(
      File('lib/main.dart').readAsStringSync(),
    );

    test('字段 / onProgress 回调 / 传下去 —— 三处同时在，且旧写法已不在', () {
      expect(
        kStageProgressField.hasMatch(source),
        isTrue,
        reason: '`_ShellRootState` 没有 `double? _stageProgress;` —— 宿主没有地方存进度',
      );
      expect(
        kStageProgressCallback.hasMatch(source),
        isTrue,
        reason: '`Live2DStage` 上没有接 `onProgress`（或回调没把值写回 `_stageProgress`）',
      );
      expect(
        kStageProgressPassedDown.hasMatch(source),
        isTrue,
        reason: '`stageProgress:` 没有吃那个字段',
      );
      expect(
        kStageProgressOldSnapshot.hasMatch(source),
        isFalse,
        reason: '还是现读 `_stageKey.currentState?.bridge?.progress` —— '
            '那只有外壳自己重建时才刷新，进度会冻住',
      );
      expect(wiresStageProgress(source), isTrue);
    });

    test('反例：改动前的写法喂给同一条判据必须判假（否则判据恒真）', () {
      const String before = '''
  double? _stageProgress;
  onProgress: (double? v) {
    if (mounted && v != _stageProgress) {
      setState(() => _stageProgress = v);
    }
  },
  stageProgress: _stageKey.currentState?.bridge?.progress,
''';
      expect(
        kStageProgressOldSnapshot.hasMatch(before),
        isTrue,
        reason: '反例本身没被识别成旧写法，这条自证失去意义',
      );
      expect(
        wiresStageProgress(before),
        isFalse,
        reason: '判据对「还在现读快照」的源码也判真 —— 那它守不住这条缺陷',
      );
    });

    test('反例：只加字段、没接回调 → 判假', () {
      const String halfWired = '''
  double? _stageProgress;
  stageProgress: _stageProgress,
''';
      expect(
        wiresStageProgress(halfWired),
        isFalse,
        reason: '字段加了但没人往里写 —— 幕布会永远停在 null',
      );
    });
  });
}
