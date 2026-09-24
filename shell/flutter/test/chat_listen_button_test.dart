/// 聊天主界面「听」按钮回归（L1 常态语音检测的**产品主路径**）。
///
/// 契约：
/// - 常驻：四态（不支持 / 未在听 / 在听 / 失败）下按钮都在；
/// - 不支持 / 未接线 → 禁用（不摆一个按不动的入口）；
/// - 在听时按钮变「停」，状态行说明唤醒词；
/// - 失败时按钮旁**可读**显示（不弹 toast）。
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
  bool pttActive = false,
  String? listenStatus,
  String? listenError,
  String? listenNote,
  String? listenBlockedReason,
  VoidCallback? onToggleListen,
  VoidCallback? onPressStart,
  VoidCallback? onPressRelease,
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
  listenBlockedReason: listenBlockedReason,
  pttActive: pttActive,
  onToggleListen: onToggleListen ?? () {},
  onPressStart: onPressStart,
  onPressRelease: onPressRelease,
);

void main() {
  testWidgets('常驻：默认（未在听）显示「听」且可点', (WidgetTester tester) async {
    int taps = 0;
    await tester.pumpWidget(_wrap(_panel(onToggleListen: () => taps++)));
    final Finder button = find.widgetWithText(OutlinedButton, '听');
    expect(button, findsOneWidget);
    // 点在外层 GestureDetector 的落点上（按钮被 AbsorbPointer 包住，不自己收指针）。
    await tester.tapAt(tester.getCenter(button));
    expect(taps, 1);
  });

  testWidgets('不支持（非 Web / 旧浏览器）：按钮禁用，不假装能点', (WidgetTester tester) async {
    await tester.pumpWidget(_wrap(_panel(listenSupported: false)));
    final OutlinedButton button = tester.widget<OutlinedButton>(
      find.widgetWithText(OutlinedButton, '听'),
    );
    expect(button.onPressed, isNull);
  });

  testWidgets('在听：按钮变「停」，状态行报出唤醒词', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(_panel(listening: true, listenStatus: '在听：说「小可爱 ……」')),
    );
    expect(find.widgetWithText(FilledButton, '停'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, '听'), findsNothing);
    expect(find.textContaining('小可爱'), findsOneWidget);
  });

  testWidgets('失败：按钮旁可读错误（无需点开任何东西）', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(_panel(listenError: '麦克风权限被拒绝：在浏览器地址栏允许麦克风后再试')),
    );
    expect(find.textContaining('麦克风权限被拒绝'), findsOneWidget);
  });

  testWidgets('按住 ≥ 阈值触发 PTT；点按仍走常驻开关', (WidgetTester tester) async {
    int toggles = 0;
    int starts = 0;
    int releases = 0;
    await tester.pumpWidget(
      _wrap(
        _panel(
          onToggleListen: () => toggles++,
          onPressStart: () => starts++,
          onPressRelease: () => releases++,
        ),
      ),
    );
    final Finder button = find.widgetWithText(OutlinedButton, '听');
    // 点按 → 常驻开关（点在外层 GestureDetector 的落点上）。
    await tester.tapAt(tester.getCenter(button));
    expect(toggles, 1);
    expect(starts, 0);

    // 按住 ≥ 阈值 → PTT start；松手 → release。
    // 时序：手势竞技场（可滚动列表）会让 onTapDown 延迟到 kPressTimeout
    // （约 100ms），之后才是我们的 [kVoiceTapThreshold]——所以一次等过去。
    final TestGesture gesture = await tester.startGesture(tester.getCenter(button));
    await tester.pump(const Duration(milliseconds: 400));
    expect(starts, 1, reason: '越过阈值即进入 PTT');
    expect(toggles, 1, reason: '按住不应触发点按');
    await gesture.up();
    await tester.pump();
    expect(releases, 1);
  });

  testWidgets('PTT 进行中按钮显示「松」', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(_panel(pttActive: true, onPressStart: () {}, onPressRelease: () {})),
    );
    expect(find.widgetWithText(FilledButton, '松'), findsOneWidget);
    expect(find.widgetWithText(OutlinedButton, '听'), findsNothing);
  });

  testWidgets('Mod 未启用：主界面常驻红字（不是只说「识别了」）', (WidgetTester tester) async {
    await tester.pumpWidget(
      _wrap(
        _panel(listenBlockedReason: '语音输入未启用：请先启用 voice-input'),
      ),
    );
    expect(find.textContaining('voice-input'), findsOneWidget);
    final Text hint = tester.widget<Text>(
      find.textContaining('voice-input'),
    );
    // 常驻原因必须是**错误色**，与普通状态行区分开。
    expect(
      hint.style?.color,
      buildAppTheme().colorScheme.error,
      reason: '「根本不可用」要用错误色，否则看起来像普通状态',
    );
  });

  testWidgets('诚实说明：Web Speech 需联网 / 音频出本机（listenNote）', (WidgetTester tester) async {
    await tester.pumpWidget(_wrap(_panel(listenNote: kVoiceWebSpeechNote)));
    expect(find.textContaining('需联网'), findsOneWidget);
    expect(find.textContaining('音频会出本机'), findsOneWidget);
  });
}
