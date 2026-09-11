/// 会话入口的**接线**（2026-09-11）。
///
/// `session_sheet_test.dart` 验的是浮层自己的行为；本文件验的是
/// **外壳真的把入口画出来了、并且真的把回调接上了**——那两件事分开验，
/// 是因为 2026-09-11 修「设置面板是空壳」时学到的教训：
/// 组件写好了但没人接线，测试全绿而界面上根本点不到。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/api/ws_status.dart';
import 'package:live2d_ai_shell/app/app_shell.dart';
import 'package:live2d_ai_shell/chat/chat_session.dart';
import 'package:live2d_ai_shell/live2d/live2d_bridge.dart';
import 'package:live2d_ai_shell/settings/settings_sections.dart';
import 'package:live2d_ai_shell/state/ui_phase.dart';
import 'package:live2d_ai_shell/ui/session_sheet.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

ChatSession session(String id, String title, {int minute = 0}) => ChatSession(
  id: id,
  title: title,
  createdAt: DateTime(2026, 9, 11, 12, minute),
  updatedAt: DateTime(2026, 9, 11, 12, minute),
);

Widget shell({
  required List<ChatSession> sessions,
  String? activeId,
  VoidCallback? onNewSession,
  ValueChanged<String>? onSelectSession,
  void Function(String id, String title)? onRenameSession,
  ValueChanged<String>? onDeleteSession,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: AppShell(
    stage: const ColoredBox(color: Color(0xFF101010)),
    phase: UiPhase.idle,
    wsStatus: WsStatus.connected,
    messages: const <Never>[],
    input: TextEditingController(),
    onSend: () {},
    onStop: () {},
    onRetryConnection: () {},
    volume: 0.8,
    muted: false,
    onVolumeChanged: (_) {},
    onMutedChanged: (_) {},
    sections: visibleSections(),
    section: SettingsSection.appearance,
    onSectionChanged: (_) {},
    sectionBuilder: (BuildContext context, SettingsSection s) =>
        Text('PANE:${s.label}'),
    stagePhase: Live2DBridgePhase.ready,
    sessions: sessions,
    activeSessionId: activeId,
    onNewSession: onNewSession,
    onSelectSession: onSelectSession,
    onRenameSession: onRenameSession,
    onDeleteSession: onDeleteSession,
  ),
);

Future<void> pumpAt(WidgetTester tester, double width, Widget widget) async {
  await tester.binding.setSurfaceSize(Size(width, 800));
  addTearDown(() => tester.binding.setSurfaceSize(null));
  await tester.pumpWidget(widget);
  await tester.pumpAndSettle();
}

void main() {
  group('入口位置：非 compact 在 AppBar，compact 在聊天头', () {
    testWidgets('expanded：AppBar 有「会话」', (WidgetTester tester) async {
      await pumpAt(tester, 1400, shell(sessions: const <ChatSession>[]));
      expect(find.text('会话'), findsOneWidget);
      expect(find.text('设置'), findsOneWidget, reason: '设置入口不该被挤掉');
    });

    testWidgets('medium：AppBar 也有「会话」', (WidgetTester tester) async {
      await pumpAt(tester, 1000, shell(sessions: const <ChatSession>[]));
      expect(find.text('会话'), findsOneWidget);
    });

    testWidgets('compact：入口在聊天面板头（AppBar 太窄，不放）', (WidgetTester tester) async {
      await pumpAt(tester, 500, shell(sessions: const <ChatSession>[]));
      // AppBar 里**不**放（那一屏还要塞状态胶囊与连接徽标）。
      // 但聊天头里有，所以总数仍是 1。
      expect(find.text('会话'), findsOneWidget);
      expect(find.text('设置'), findsOneWidget);
    });
  });

  group('入口真的接上了回调（不是画了个死按钮）', () {
    testWidgets('点「会话」→ 浮层打开，列出外壳传进来的会话', (WidgetTester tester) async {
      await pumpAt(
        tester,
        1400,
        shell(
          sessions: <ChatSession>[
            session('a', '甲会话', minute: 0),
            session('b', '乙会话', minute: 1),
          ],
          activeId: 'b',
        ),
      );

      await tester.tap(find.text('会话'));
      await tester.pumpAndSettle();

      expect(find.byType(SessionSheet), findsOneWidget);
      expect(find.text('甲会话'), findsOneWidget);
      expect(find.text('乙会话'), findsOneWidget);
    });

    testWidgets('在建浮层里点一行 → onSelectSession 收到那个 id', (WidgetTester tester) async {
      final List<String> picked = <String>[];
      await pumpAt(
        tester,
        1400,
        shell(
          sessions: <ChatSession>[
            session('a', '甲会话', minute: 0),
            session('b', '乙会话', minute: 1),
          ],
          activeId: 'b',
          onSelectSession: picked.add,
        ),
      );

      await tester.tap(find.text('会话'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('甲会话'));
      await tester.pumpAndSettle();

      expect(picked, <String>['a']);
    });

    testWidgets('点「新建」→ onNewSession 被调用', (WidgetTester tester) async {
      int created = 0;
      await pumpAt(
        tester,
        1400,
        shell(
          sessions: const <ChatSession>[],
          onNewSession: () => created++,
        ),
      );

      await tester.tap(find.text('会话'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('新建'));
      await tester.pumpAndSettle();

      expect(created, 1);
    });

    testWidgets('重命名回调贯通（浮层 → 外壳 → 宿主）', (WidgetTester tester) async {
      final List<(String, String)> renamed = <(String, String)>[];
      await pumpAt(
        tester,
        1400,
        shell(
          sessions: <ChatSession>[session('a', '旧名')],
          activeId: 'a',
          onRenameSession: (String id, String t) => renamed.add((id, t)),
        ),
      );

      await tester.tap(find.text('会话'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('改名'));
      await tester.pumpAndSettle();
      // **必须限定在浮层内**：聊天面板自己也有一个 `TextField`（输入框），
      // `find.byType(TextField)` 会匹配到两个 —— 这正是「测试要像真界面一样
      // 精确」的一个小例子。
      await tester.enterText(
        find.descendant(
          of: find.byType(SessionSheet),
          matching: find.byType(TextField),
        ),
        '新名',
      );
      await tester.tap(find.text('确定'));
      await tester.pumpAndSettle();

      expect(renamed, <(String, String)>[('a', '新名')]);
    });

    testWidgets('删除回调贯通（两段式确认之后）', (WidgetTester tester) async {
      final List<String> deleted = <String>[];
      await pumpAt(
        tester,
        1400,
        shell(
          sessions: <ChatSession>[session('a', '要删的')],
          activeId: 'a',
          onDeleteSession: deleted.add,
        ),
      );

      await tester.tap(find.text('会话'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('删除'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('删除').last);
      await tester.pumpAndSettle();

      expect(deleted, <String>['a']);
    });
  });
}
