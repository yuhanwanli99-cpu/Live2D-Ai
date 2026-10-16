/// 输入区那颗**统一圆键**：没有可停的东西时是发送，有东西可停时是停止。
///
/// 纯判据与两步编排的回归在 `stage_cancel_test.dart`（`canStopPlayback` /
/// `stopPlayback`）；这里钉的是**呈现**这一层：
/// - 宿主给 `false` 时是**发送**（上箭头 + 「发送（Enter）」），按下只走 `onSend`；
/// - 宿主给 `true` 时**同一颗**圆形 IconButton 变 `stop_rounded` + 「停止」，
///   按下只走 `onStopPlayback`；
/// - 输入区**没有第二颗「停止」**（那颗独立键已删）；
/// - `onStopPlayback` 为 `null`（测试桩）时仍是停止态，但不可按、不抛；
/// - 判据是宿主的三条队列，**界面相位不再参与**。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

void main() {
  Widget wrap(Widget child) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(body: child),
  );

  Widget panel({
    bool canStopPlayback = false,
    VoidCallback? onStopPlayback,
    VoidCallback? onSend,
    UiPhase phase = UiPhase.idle,
  }) => wrap(
    ChatPanel(
      messages: const <ChatMessage>[],
      phase: phase,
      input: TextEditingController(),
      onSend: onSend ?? () {},
      onDismissError: () {},
      volume: 1,
      muted: false,
      onVolumeChanged: (_) {},
      onMutedChanged: (_) {},
      canStopPlayback: canStopPlayback,
      onStopPlayback: onStopPlayback,
    ),
  );

  testWidgets('没有可停的东西：圆键是发送，按下只走 onSend，且没有第二颗「停止」', (
    WidgetTester tester,
  ) async {
    int sends = 0;
    int stops = 0;
    await tester.pumpWidget(
      panel(
        canStopPlayback: false,
        onSend: () => sends++,
        onStopPlayback: () => stops++,
      ),
    );
    expect(find.byIcon(Icons.arrow_upward_rounded), findsOneWidget);
    expect(find.byIcon(Icons.stop_rounded), findsNothing);
    expect(find.byTooltip('发送（Enter）'), findsOneWidget);
    expect(
      find.byTooltip('停止'),
      findsNothing,
      reason: '独立「停止」键已删：输入区只留这一颗统一圆键',
    );
    expect(
      find.widgetWithText(OutlinedButton, '停止'),
      findsNothing,
      reason: '输入区不许再有写着「停止」的第二颗按钮',
    );
    await tester.tap(find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded));
    expect(sends, 1);
    expect(stops, 0);
  });

  testWidgets('有东西可停：同一颗圆键变停止，按下只走 onStopPlayback', (
    WidgetTester tester,
  ) async {
    int sends = 0;
    int stops = 0;
    await tester.pumpWidget(
      panel(
        canStopPlayback: true,
        onSend: () => sends++,
        onStopPlayback: () => stops++,
      ),
    );
    final Finder round = find.widgetWithIcon(IconButton, Icons.stop_rounded);
    expect(round, findsOneWidget);
    expect(find.byIcon(Icons.arrow_upward_rounded), findsNothing);
    expect(find.byTooltip('停止'), findsOneWidget);
    expect(find.byTooltip('发送（Enter）'), findsNothing);
    final IconButton button = tester.widget<IconButton>(round);
    expect(
      button.style?.shape?.resolve(<WidgetState>{}),
      const CircleBorder(),
      reason: '发送 / 停止共用一颗：切换时形状不跳',
    );
    await tester.tap(round);
    expect(stops, 1);
    expect(sends, 0, reason: '停止态按下不得发送');
  });

  testWidgets('没接线（测试桩）：仍是停止态，但 IconButton 不可按、不抛', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(panel(canStopPlayback: true));
    final Finder round = find.widgetWithIcon(IconButton, Icons.stop_rounded);
    expect(round, findsOneWidget);
    expect(tester.widget<IconButton>(round).onPressed, isNull);
    await tester.tap(round, warnIfMissed: false);
  });

  testWidgets('相位不再是判据：thinking 且没东西可停时圆键仍是发送', (
    WidgetTester tester,
  ) async {
    int sends = 0;
    int stops = 0;
    await tester.pumpWidget(
      panel(
        canStopPlayback: false,
        phase: UiPhase.thinking,
        onSend: () => sends++,
        onStopPlayback: () => stops++,
      ),
    );
    expect(find.byIcon(Icons.arrow_upward_rounded), findsOneWidget);
    expect(find.byTooltip('发送（Enter）'), findsOneWidget);
    expect(find.byTooltip('停止'), findsNothing);
    await tester.tap(find.widgetWithIcon(IconButton, Icons.arrow_upward_rounded));
    expect(sends, 1);
    expect(stops, 0);
  });
}
