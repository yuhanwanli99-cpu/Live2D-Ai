/// 桌宠模式页（2026-11-18）：切桌宠后**会话仍在**，可切换、可回工作台。
///
/// 审计现场：切桌宠后整棵外壳被换成一句「已进入桌宠，请关闭此页」——会话列表、
/// 当前标题、输入框一起卸掉，浏览器那页就停在一句占位文案上。这个文件钉住
/// 「标题 / 条数 / 会话列表 / 返回工作台」四样东西真的在这一页上。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/chat_session.dart';
import 'package:live2d_ai_shell/ui/desk_pet_mode_page.dart';
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
  List<ChatSession> sessions = const <ChatSession>[],
  String? activeId,
  String? activeTitle,
  String? activeMeta,
  ValueChanged<String>? onSelect,
  VoidCallback? onNew,
  VoidCallback? onBack,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: DeskPetModePage(
    sessions: sessions,
    activeId: activeId,
    activeTitle: activeTitle,
    activeMeta: activeMeta,
    onSelect: onSelect ?? (_) {},
    onNew: onNew ?? () {},
    onBack: onBack ?? () {},
  ),
);

void main() {
  testWidgets('当前会话标题 / 条数 / 会话列表 / 返回工作台都在', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(
      harness(
        sessions: <ChatSession>[
          session('a', '甲会话', minute: 0, messages: 2),
          session('b', '乙会话', minute: 1),
        ],
        activeId: 'b',
        activeTitle: '乙会话',
        activeMeta: '1 条 · 3 分钟前',
        onSelect: (_) {},
      ),
    );
    await tester.pump();

    expect(find.text('已进入桌宠'), findsOneWidget);
    expect(find.text('当前会话'), findsOneWidget);
    expect(find.text('乙会话'), findsNWidgets(2), reason: '当前会话卡 + 会话列表行');
    expect(find.text('1 条 · 3 分钟前'), findsOneWidget);
    expect(find.text('切换会话'), findsOneWidget);
    expect(find.text('甲会话'), findsOneWidget);
    expect(find.text('返回工作台'), findsOneWidget);
  });

  testWidgets('切换会话 / 新建 / 返回工作台 都接上了回调', (
    WidgetTester tester,
  ) async {
    final List<String> picked = <String>[];
    int created = 0;
    int backed = 0;
    await tester.pumpWidget(
      harness(
        sessions: <ChatSession>[
          session('a', '甲会话', minute: 0),
          session('b', '乙会话', minute: 1),
        ],
        activeId: 'b',
        activeTitle: '乙会话',
        activeMeta: '空会话',
        onSelect: picked.add,
        onNew: () => created++,
        onBack: () => backed++,
      ),
    );
    await tester.pump();

    await tester.tap(find.text('甲会话'));
    await tester.tap(find.text('新建会话'));
    await tester.tap(find.text('返回工作台'));
    await tester.pump();

    expect(picked, <String>['a']);
    expect(created, 1);
    expect(backed, 1);
  });

  testWidgets('没有会话时说实话，并仍可新建 / 返回', (
    WidgetTester tester,
  ) async {
    await tester.pumpWidget(harness());
    await tester.pump();
    expect(find.text('已进入桌宠'), findsOneWidget);
    expect(find.textContaining('还没有会话'), findsOneWidget);
    expect(find.text('新建会话'), findsOneWidget);
    expect(find.text('返回工作台'), findsOneWidget);
  });
}
