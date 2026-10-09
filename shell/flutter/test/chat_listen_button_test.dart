/// 聊天主界面「听」按钮回归（2026-10-08：点一下开始、再点一下结束）。
///
/// 契约：
/// - 未在听 → 「听」且可点；正在听 → 「停」且可点；
/// - 不支持 / 未接线 → 禁用（不摆一个按不动的入口）；
/// - 状态行说明「正在听，说完再点一次」（定稿进输入框）；
/// - 失败时按钮旁**可读**显示（不弹 toast）；
/// - **不再有按住说话（PTT）**：长按不触发第二套动作，也没有「松」这一态。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';
import 'package:live2d_ai_shell/voice/voice_listen_controller.dart'
    show kVoiceWebSpeechNote;

Widget _wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

ChatPanel _panel({
  bool listenSupported = true,
  bool listening = false,
  String? listenStatus,
  String? listenError,
  String? listenNote,
  VoidCallback? onToggleListen,
}) => ChatPanel(
  messages: const <ChatMessage>[],
  phase: UiPhase.idle,
  input: TextEditingController(),
  onSend: () {},
  onStop: () {},
  onDismissError: () {},
  volume: 1,
  muted: false,
  onVolumeChanged: (_) {},
  onMutedChanged: (_) {},
  listenSupported: listenSupported,
  listening: listening,
  listenStatus: listenStatus,
  listenError: listenError,
  listenNote: listenNote,
  onToggleListen: onToggleListen ?? () {},
);

void main() {
  testWidgets('默认（未在听）显示「听」且可点', (WidgetTester tester) async {
    int taps = 0;
    await tester.pumpWidget(_wrap(_panel(onToggleListen: () => taps++)));
    final Finder button = find.widgetWithText(OutlinedButton, '听');
    expect(button, findsOneWidget);
    await tester.tap(button);
    expect(taps, 1);
  });

  testWidgets('不支持（非 Web / 旧浏览器）：按钮禁用，不假装能点', (WidgetTester tester) async {
    await tester.pumpWidget(_wrap(_panel(listenSupported: false)));
    final OutlinedButton button = tester.widget<OutlinedButton>(
      find.widgetWithText(OutlinedButton, '听'),
    );
    expect(button.onPressed, isNull);
  });

  testWidgets('正在听：按钮变「停」，状态行写「正在听，说完再点一次」', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      _wrap(_panel(listening: true, listenStatus: '正在听，说完再点一次')),
    );
    expect(find.widgetWithText(FilledButton, '停'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, '听'), findsNothing);
    expect(find.textContaining('正在听，说完再点一次'), findsOneWidget);
    // 「停」也是可点的（再点一下才结束）。
    final FilledButton button = tester.widget<FilledButton>(
      find.widgetWithText(FilledButton, '停'),
    );
    expect(button.onPressed, isNotNull);
  });

  testWidgets('只有点按一种用法：长按不触发第二套（PTT 已下线）', (
    WidgetTester tester,
  ) async {
    int toggles = 0;
    await tester.pumpWidget(_wrap(_panel(onToggleListen: () => toggles++)));
    final Finder button = find.widgetWithText(OutlinedButton, '听');
    final TestGesture gesture = await tester.startGesture(
      tester.getCenter(button),
    );
    await tester.pump(const Duration(milliseconds: 400));
    expect(toggles, 0, reason: '按住不再有任何立即动作');
    await gesture.up();
    await tester.pump();
    expect(toggles, 1, reason: '松手就是那一次点按');
    expect(find.text('松'), findsNothing, reason: '「松」这一态已随 PTT 下线');
  });

  testWidgets('失败：按钮旁可读错误（无需点开任何东西）', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(_panel(listenError: '麦克风权限被拒绝：在浏览器地址栏允许麦克风后再试')),
    );
    expect(find.textContaining('麦克风权限被拒绝'), findsOneWidget);
  });

  testWidgets('没听清：控制器那句话上屏（按钮旁，不弹 toast）', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_wrap(_panel(listenError: '没听清，再点一次说')));
    expect(find.textContaining('没听清，再点一次说'), findsOneWidget);
  });

  testWidgets('诚实说明：Web Speech 需联网 / 音频出本机（listenNote）', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(_wrap(_panel(listenNote: kVoiceWebSpeechNote)));
    expect(find.textContaining('需联网'), findsOneWidget);
    expect(find.textContaining('音频会出本机'), findsOneWidget);
  });
}
