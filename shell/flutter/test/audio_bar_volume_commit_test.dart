/// F-0002-1（P1）· 音量滑杆的**提交时机**：拖动过程只更新草稿与显示，
/// 松手（`onChangeEnd`）才提交给宿主 ⇒ 拖一次只落盘一次、只下发一次。
///
/// # 为什么选 onChangeEnd 而不是「在 main 里防抖」
///
/// 1. `_update`（`main.dart`）的返回值就是「**有没有真的写进本机存储**」，
///    而它的失败文案（无痕 / 配额满 / 存储被禁）是 rc.3 §9.1 特意做出来的
///    诚实性契约——改成防抖之后这个同步返回值就没有真话可说了；
/// 2. `main.dart` 在 VM 测试里**加载不了**（它经 `app/browser_io.dart` 依赖
///    `package:web`），写在那种地方的时序没有任何回归能钉住；
/// 3. 滑杆本身就在本文件里，`onChangeEnd` 由 Flutter 的 `Slider` 语义保证
///    （拖动结束、点击、**方向键调整**三条路都会触发，见 SDK
///    `material/slider.dart` 的 `_handleDragEnd` / `increaseAction`）。
///
/// # 语义不变的部分
///
/// - 松手时提交的值 = 滑杆终值 = 界面读数（下面逐条断言）；
/// - 拖动过程中读数**跟着手指走**（草稿），不是等到松手才动；
/// - 宿主因别的原因重建（流式 delta）时滑块**不许跳回旧值**——草稿还在。
///
/// 代价（如实记录）：拖动过程中**不再逐帧**改 `_audio.volume`，
/// 也就是「拖的时候听不到音量在变，松手生效」。这是这个选项的固有代价
/// （另一条是 main 里防抖，见上）。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/ui/audio_bar.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

Widget _bar({
  double volume = 0.5,
  required List<double> volumes,
  bool muted = false,
}) => AudioBar(
  volume: volume,
  muted: muted,
  onVolumeChanged: volumes.add,
  onMutedChanged: (_) {},
);

/// 当前滑杆显示的值。
double _sliderValue(WidgetTester tester) =>
    tester.widget<Slider>(find.byType(Slider)).value;

/// 把焦点交给滑杆（**触摸点击不会自动聚焦**：SDK 只在 a11y 聚焦 / 语义
/// `focus` 动作里 `requestFocus`，所以键盘回归必须先显式聚焦）。
Future<void> _focusSlider(WidgetTester tester) async {
  final Iterable<Focus> focuses = tester.widgetList<Focus>(
    find.descendant(of: find.byType(Slider), matching: find.byType(Focus)),
  );
  final Focus target = focuses.firstWhere((Focus f) => f.focusNode != null);
  target.focusNode!.requestFocus();
  await tester.pump();
}

/// 一次**细粒度**拖动（10 段位移）：现状实现会在每一段都回调一次。
Future<void> _fineDrag(WidgetTester tester, {double step = 4}) async {
  final TestGesture gesture = await tester.startGesture(
    tester.getCenter(find.byType(Slider)),
  );
  for (int i = 0; i < 10; i++) {
    await gesture.moveBy(Offset(step, 0));
    await tester.pump();
  }
  await gesture.up();
  await tester.pump();
}

void main() {
  testWidgets('一次拖动只提交一次（现状：每帧一次 ⇒ 每帧重写整份偏好）', (
    WidgetTester tester,
  ) async {
    final List<double> volumes = <double>[];
    await tester.pumpWidget(_wrap(_bar(volumes: volumes)));

    await _fineDrag(tester);

    expect(
      volumes.length,
      1,
      reason: '拖动中的每一帧都提交 = 每帧 jsonEncode 整份偏好 + setItem（含舞台图）',
    );
  });

  testWidgets('提交的值 = 滑杆终值 = 界面读数（语义不许变）', (
    WidgetTester tester,
  ) async {
    final List<double> volumes = <double>[];
    await tester.pumpWidget(_wrap(_bar(volume: 0.4, volumes: volumes)));

    await _fineDrag(tester);

    expect(volumes, hasLength(1), reason: '只提交一次');
    expect(volumes.single, _sliderValue(tester), reason: '落盘值必须是滑杆终值');
    final String shown = tester
        .widget<Text>(find.textContaining('%'))
        .data!;
    expect(
      shown,
      '${(volumes.single * 100).round()}%',
      reason: '读数与落盘值不许各说各话',
    );
  });

  testWidgets('拖动过程中读数跟着手指（草稿，不是松手才动）', (
    WidgetTester tester,
  ) async {
    final List<double> volumes = <double>[];
    await tester.pumpWidget(_wrap(_bar(volume: 0.5, volumes: volumes)));

    final TestGesture gesture = await tester.startGesture(
      tester.getCenter(find.byType(Slider)),
    );
    for (int i = 0; i < 10; i++) {
      await gesture.moveBy(const Offset(4, 0));
      await tester.pump();
    }
    // **还没松手**：读数必须已经变了。
    expect(find.text('50%'), findsNothing, reason: '拖动中读数要跟着手指');
    expect(
      volumes,
      isEmpty,
      reason: '没松手就不许提交（这正是本条的修法）',
    );
    await gesture.up();
    await tester.pump();
    expect(volumes, hasLength(1));
  });

  testWidgets('拖动中宿主因别的原因重建：滑块不许跳回旧值', (
    WidgetTester tester,
  ) async {
    final List<double> volumes = <double>[];
    await tester.pumpWidget(_wrap(_bar(volume: 0.5, volumes: volumes)));

    final TestGesture gesture = await tester.startGesture(
      tester.getCenter(find.byType(Slider)),
    );
    for (int i = 0; i < 10; i++) {
      await gesture.moveBy(const Offset(4, 0));
      await tester.pump();
    }
    final double duringDrag = _sliderValue(tester);

    // 宿主重建（流式 delta 的真实形态）：volume 还是**旧值** 0.5。
    await tester.pumpWidget(_wrap(_bar(volume: 0.5, volumes: volumes)));

    expect(
      _sliderValue(tester),
      duringDrag,
      reason: '没有草稿的话，宿主一重建滑块就跳回 0.5（然后用户松手又跳回来）',
    );
    await gesture.up();
    await tester.pump();
  });

  testWidgets('方向键（读屏 / 键盘）也走提交，不是静默丢失的通道', (
    WidgetTester tester,
  ) async {
    final List<double> volumes = <double>[];
    await tester.pumpWidget(_wrap(_bar(volume: 0.5, volumes: volumes)));

    await _focusSlider(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pump();

    expect(volumes, isNotEmpty, reason: '方向键调完音量必须提交');
    expect(
      volumes.last,
      greaterThan(0.5),
      reason: '方向键右 = 调大；如果只有 onChanged 提交，这里会是 0.5 或为空',
    );
    expect(volumes.last, _sliderValue(tester));
  });
}
