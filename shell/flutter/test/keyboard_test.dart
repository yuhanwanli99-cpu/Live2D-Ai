import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/chat_panel.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: child),
);

void main() {
  group('钉子 9：**键盘路径也要解锁音频**', () {
    testWidgets('按任意键都会触发 onUserGesture（不只指针）', (WidgetTester tester) async {
      int gestures = 0;
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: AppShell(
            stage: const SizedBox.shrink(),
            phase: UiPhase.idle,
            wsStatus: WsStatus.connected,
            messages: const <ChatMessage>[],
            input: TextEditingController(),
            onSend: () {},
            onStop: () {},
            onRetryConnection: () {},
            volume: 0.8,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
            sections: visibleSections(),
            sectionBuilder: (BuildContext c, SettingsSection s) =>
                const SizedBox.shrink(),
            onUserGesture: () => gestures++,
          ),
        ),
      );
      await tester.pump();
      expect(gestures, 0, reason: '还没按任何东西');

      // 纯键盘：不点任何地方，只按键。
      await tester.sendKeyEvent(LogicalKeyboardKey.keyA);
      await tester.pump();
      expect(
        gestures,
        greaterThan(0),
        reason: '纯键盘用户按了键就必须解锁——否则他永远没有声音，'
            '而界面不会给他任何解释',
      );
    });

    testWidgets('指针路径仍然有效（既有行为保留）', (WidgetTester tester) async {
      int gestures = 0;
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: AppShell(
            stage: const SizedBox.shrink(),
            phase: UiPhase.idle,
            wsStatus: WsStatus.connected,
            messages: const <ChatMessage>[],
            input: TextEditingController(),
            onSend: () {},
            onStop: () {},
            onRetryConnection: () {},
            volume: 0.8,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
            sections: visibleSections(),
            sectionBuilder: (BuildContext c, SettingsSection s) =>
                const SizedBox.shrink(),
            onUserGesture: () => gestures++,
          ),
        ),
      );
      await tester.pump();
      await tester.tapAt(const Offset(5, 5));
      await tester.pump();
      expect(gestures, greaterThan(0));
    });

    testWidgets('键盘处理器**不吞键**（否则输入框会被劫持）', (WidgetTester tester) async {
      final TextEditingController input = TextEditingController();
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: AppShell(
            stage: const SizedBox.shrink(),
            phase: UiPhase.idle,
            wsStatus: WsStatus.connected,
            messages: const <ChatMessage>[],
            input: input,
            onSend: () {},
            onStop: () {},
            onRetryConnection: () {},
            volume: 0.8,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
            sections: visibleSections(),
            sectionBuilder: (BuildContext c, SettingsSection s) =>
                const SizedBox.shrink(),
            onUserGesture: () {},
          ),
        ),
      );
      await tester.pump();

      await tester.tap(find.byType(TextField));
      await tester.pump();
      await tester.enterText(find.byType(TextField), 'hello');
      await tester.pump();
      expect(
        input.text,
        'hello',
        reason: '键盘解锁处理器返回 false（不吞键），输入必须正常进到输入框',
      );
    });
  });

  group('P6：聊天列表上限 500', () {
    testWidgets('超过 500 条只渲染最后 500（视图层裁剪）', (WidgetTester tester) async {
      final List<ChatMessage> many = <ChatMessage>[
        for (int i = 0; i < 520; i++)
          ChatMessage(role: ChatRole.user, text: '第 $i 条'),
      ];
      await tester.pumpWidget(
        wrap(
          ChatPanel(
            messages: many,
            phase: UiPhase.idle,
            input: TextEditingController(),
            onSend: () {},
            onStop: () {},
            onDismissError: () {},
            volume: 0.8,
            muted: false,
            onVolumeChanged: (_) {},
            onMutedChanged: (_) {},
          ),
        ),
      );
      // 最新的那条在列表里；被裁掉的最旧一条不在。
      expect(find.text('第 519 条'), findsOneWidget);
      expect(find.text('第 0 条'), findsNothing);
    });

    test('上限是 500（规格 P6 的点名值）', () {
      expect(kChatHistoryLimit, 500);
    });
  });

  group('状态胶囊 + 连接徽标：文字本身就表意（不靠颜色）', () {
    testWidgets('5 个 WsStatus 各自有不同文字', (WidgetTester tester) async {
      final Set<String> labels = <String>{};
      for (final WsStatus status in WsStatus.values) {
        await tester.pumpWidget(
          MaterialApp(
            theme: buildAppTheme(),
            home: AppShell(
              stage: const SizedBox.shrink(),
              phase: UiPhase.idle,
              wsStatus: status,
              messages: const <ChatMessage>[],
              input: TextEditingController(),
              onSend: () {},
              onStop: () {},
              onRetryConnection: () {},
              volume: 0.8,
              muted: false,
              onVolumeChanged: (_) {},
              onMutedChanged: (_) {},
              sections: visibleSections(),
              sectionBuilder: (BuildContext c, SettingsSection s) =>
                  const SizedBox.shrink(),
            ),
          ),
        );
        await tester.pump();
        expect(
          find.text(status.label),
          findsWidgets,
          reason: '${status.name} 的文字标签要在界面上真的出现',
        );
        labels.add(status.label);
      }
      expect(labels, hasLength(WsStatus.values.length), reason: '5 个标签必须两两不同');
    });

    test('每个 WsStatus 的 label / description 都非空', () {
      for (final WsStatus status in WsStatus.values) {
        expect(status.label.trim(), isNotEmpty);
        expect(status.description.trim(), isNotEmpty);
      }
    });
  });
}
