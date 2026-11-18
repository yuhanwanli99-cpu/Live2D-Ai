/// 左侧会话栏（2026-11-18）：两种宽度、当前会话名字、切换 / 新建 / 管理接线。
///
/// 会话栏是「宽屏上看不出在哪段对话」那条审计的落点之一，所以这里不验像素，
/// 只验三件会**静默回归**的事：
/// 1. 窄轨（收起 / medium）仍然显示**当前会话名字**；
/// 2. 行点击 / 新建 / 全部会话都真的接上了回调（不是画了个死按钮）；
/// 3. 宽度常量只有一处真源（`sessionRailWidthFor`），且 medium 窄轨不会把舞台
///    压到下限以下。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/chat_session.dart';
import 'package:live2d_ai_shell/design/breakpoints.dart';
import 'package:live2d_ai_shell/ui/session_rail.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

ChatSession session(String id, String title, {int minute = 0, int messages = 0}) {
  final ChatSession s = ChatSession(
    id: id,
    title: title,
    createdAt: DateTime(2026, 11, 18, 9, minute),
    updatedAt: DateTime(2026, 11, 18, 9, minute),
  );
  for (int i = 0; i < messages; i++) {
    s.messages.add(ChatMessage(role: ChatRole.user, text: '第 $i 条'));
  }
  return s;
}

Widget harness({
  required List<ChatSession> sessions,
  String? activeId,
  bool expanded = true,
  bool canExpand = true,
  VoidCallback? onToggle,
  ValueChanged<String>? onSelect,
  VoidCallback? onNew,
  VoidCallback? onManageSessions,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: Row(
      children: <Widget>[
        SessionRail(
          sessions: sessions,
          activeId: activeId,
          expanded: expanded,
          canExpand: canExpand,
          onToggle: onToggle ?? () {},
          onSelect: onSelect ?? (_) {},
          onNew: onNew ?? () {},
          onManageSessions: onManageSessions ?? () {},
        ),
        const Expanded(child: SizedBox.shrink()),
      ],
    ),
  ),
);

void main() {
  group('宽度：一处真源 + 满足舞台下限', () {
    test('sessionRailWidthFor：compact 0 / medium 窄轨 / expanded 跟随展开', () {
      expect(sessionRailWidthFor(SizeClass.compact, expanded: true), 0);
      expect(
        sessionRailWidthFor(SizeClass.medium, expanded: true),
        kSessionRailCollapsedWidth,
        reason: 'medium 不允许展开：会（248+320）把舞台挤到 stageMinWidth 以下',
      );
      expect(
        sessionRailWidthFor(SizeClass.expanded, expanded: false),
        kSessionRailCollapsedWidth,
      );
      expect(
        sessionRailWidthFor(SizeClass.expanded, expanded: true),
        kSessionRailExpandedWidth,
      );
      // medium 的下限：900 - 窄轨 - 聊天 - 两条分隔线。
      expect(
        900 - kSessionRailCollapsedWidth - 320 - 2,
        greaterThanOrEqualTo(480),
      );
    });
  });

  group('展开态：一列会话 + 三个入口', () {
    testWidgets('列出会话与条数；点一行回传 id', (WidgetTester tester) async {
      final List<String> picked = <String>[];
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[
            session('a', '甲会话', minute: 0, messages: 3),
            session('b', '乙会话', minute: 1),
          ],
          activeId: 'b',
          onSelect: picked.add,
        ),
      );
      await tester.pump();

      expect(find.text('最近会话'), findsOneWidget);
      expect(find.text('甲会话'), findsOneWidget);
      expect(find.text('乙会话'), findsOneWidget);
      expect(find.textContaining('3 条'), findsOneWidget);
      expect(find.text('空会话'), findsOneWidget);

      await tester.tap(find.text('甲会话'));
      await tester.pump();
      expect(picked, <String>['a']);
    });

    testWidgets('新建 / 全部会话 / 收起 都接上了回调', (WidgetTester tester) async {
      int created = 0;
      int managed = 0;
      int toggled = 0;
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲会话')],
          activeId: 'a',
          onNew: () => created++,
          onManageSessions: () => managed++,
          onToggle: () => toggled++,
        ),
      );
      await tester.pump();

      await tester.tap(find.text('新建'));
      await tester.tap(find.text('全部会话'));
      await tester.tap(find.text('收起'));
      await tester.pump();

      expect(created, 1);
      expect(managed, 1);
      expect(toggled, 1);
    });

    testWidgets('空列表不一片空白：给一句可执行的说明', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(harness(sessions: const <ChatSession>[]));
      await tester.pump();
      expect(find.textContaining('还没有会话'), findsOneWidget);
    });
  });

  group('窄轨：**当前会话名字**必须留下', () {
    testWidgets('收起态显示当前标题（长标题不溢出）', (WidgetTester tester) async {
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[
            session('a', '甲会话', minute: 0),
            session('b', '乙会话', minute: 1),
          ],
          activeId: 'b',
          expanded: false,
        ),
      );
      await tester.pump();

      expect(find.text('乙会话'), findsOneWidget);
      expect(tester.takeException(), isNull);
      expect(
        tester.getSize(find.byType(SessionRail)).width,
        kSessionRailCollapsedWidth,
      );
    });

    testWidgets('没有会话时窄轨显示「还没有会话」（不是空白）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        harness(sessions: const <ChatSession>[], expanded: false),
      );
      await tester.pump();
      expect(find.text('还没有会话'), findsOneWidget);
    });

    testWidgets('canExpand：点箭头 = 展开；不能展开时点它 = 打开会话列表', (
      WidgetTester tester,
    ) async {
      int toggled = 0;
      int managed = 0;
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲会话')],
          activeId: 'a',
          expanded: false,
          canExpand: true,
          onToggle: () => toggled++,
          onManageSessions: () => managed++,
        ),
      );
      await tester.pump();
      await tester.tap(find.byTooltip('展开会话栏'));
      await tester.pump();
      expect(toggled, 1);
      expect(managed, 0);

      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲会话')],
          activeId: 'a',
          expanded: false,
          canExpand: false,
          onToggle: () => toggled++,
          onManageSessions: () => managed++,
        ),
      );
      await tester.pump();
      await tester.tap(find.byTooltip('打开会话列表'));
      await tester.pump();
      expect(managed, 1);
      expect(toggled, 1);
    });

    testWidgets('窄轨的「新建」也接上了', (WidgetTester tester) async {
      int created = 0;
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲会话')],
          activeId: 'a',
          expanded: false,
          onNew: () => created++,
        ),
      );
      await tester.pump();
      await tester.tap(find.byTooltip('新建会话'));
      await tester.pump();
      expect(created, 1);
    });
  });
}
