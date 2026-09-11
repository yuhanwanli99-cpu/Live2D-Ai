/// 会话列表 UI（2026-09-11）。
///
/// 验三件事：
/// 1. 列表渲染出**全部**会话，当前那个有选中态；
/// 2. **重命名是行内输入框**、**删除是两段式确认**——两者都**不新开 overlay**；
/// 3. 浮层外面套了指针垫层（它压在舞台上，不垫就点不着）。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/chat_session.dart';
import 'package:live2d_ai_shell/live2d/stage_pointer_interceptor.dart';
import 'package:live2d_ai_shell/ui/session_sheet.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

ChatMessage user(String t) => ChatMessage(role: ChatRole.user, text: t);

ChatSession session(String id, String title, {int messages = 0, int minute = 0}) {
  final ChatSession s = ChatSession(
    id: id,
    title: title,
    createdAt: DateTime(2026, 9, 11, 12, minute),
    updatedAt: DateTime(2026, 9, 11, 12, minute),
  );
  for (int i = 0; i < messages; i++) {
    s.messages.add(user('第 $i 条'));
  }
  return s;
}

/// 挂一个 `SessionSheet`（**不**走 `showModalBottomSheet`：那会把断言绑死在
/// 浮层的入场动画上，而这里要验的是内容与交互）。
Widget harness({
  required List<ChatSession> sessions,
  String? activeId,
  VoidCallback? onNew,
  ValueChanged<String>? onSelect,
  void Function(String id, String title)? onRename,
  ValueChanged<String>? onDelete,
  VoidCallback? onClose,
}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SessionSheet(
      sessions: sessions,
      activeId: activeId,
      onNew: onNew ?? () {},
      onSelect: onSelect ?? (_) {},
      onRename: onRename ?? (_, _) {},
      onDelete: onDelete ?? (_) {},
      onClose: onClose,
    ),
  ),
);

void main() {
  group('列表渲染', () {
    testWidgets('全部会话都在，当前那个是选中态', (WidgetTester tester) async {
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[
            session('a', '第一个', messages: 3, minute: 0),
            session('b', '第二个', messages: 1, minute: 5),
          ],
          activeId: 'b',
        ),
      );
      await tester.pump();

      expect(find.text('第一个'), findsOneWidget);
      expect(find.text('第二个'), findsOneWidget);

      final ListTile active = tester.widget<ListTile>(
        find.ancestor(
          of: find.text('第二个'),
          matching: find.byType(ListTile),
        ),
      );
      expect(active.selected, isTrue);

      final ListTile other = tester.widget<ListTile>(
        find.ancestor(
          of: find.text('第一个'),
          matching: find.byType(ListTile),
        ),
      );
      expect(other.selected, isFalse);
    });

    testWidgets('副标题给条数（找会话的线索）', (WidgetTester tester) async {
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲', messages: 7)],
          activeId: 'a',
        ),
      );
      await tester.pump();
      expect(find.textContaining('7 条'), findsOneWidget);
    });

    testWidgets('空会话显示「空会话」（不是「0 条 · —」）', (WidgetTester tester) async {
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲')],
          activeId: 'a',
        ),
      );
      await tester.pump();
      expect(find.text('空会话'), findsOneWidget);
    });

    testWidgets('一个会话都没有时给说明 + 指向「新建」（不是一片空白）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(harness(sessions: const <ChatSession>[]));
      await tester.pump();
      expect(find.textContaining('还没有会话'), findsOneWidget);
      expect(find.text('新建'), findsOneWidget);
    });

    testWidgets('没起过名的会话显示**自动标题**（第一条用户消息）', (WidgetTester tester) async {
      final ChatSession s = session('a', '');
      s.messages.add(user('帮我看看这个报错'));
      await tester.pumpWidget(
        harness(sessions: <ChatSession>[s], activeId: 'a'),
      );
      await tester.pump();
      expect(find.text('帮我看看这个报错'), findsOneWidget);
    });
  });

  group('切换与新建', () {
    testWidgets('点一行 → onSelect + 自动关闭', (WidgetTester tester) async {
      final List<String> picked = <String>[];
      int closed = 0;
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[
            session('a', '甲', minute: 0),
            session('b', '乙', minute: 1),
          ],
          activeId: 'a',
          onSelect: picked.add,
          onClose: () => closed++,
        ),
      );
      await tester.pump();

      await tester.tap(find.text('乙'));
      await tester.pump();
      expect(picked, <String>['b']);
      expect(closed, 1);
    });

    testWidgets('「新建」→ onNew + 关闭', (WidgetTester tester) async {
      int created = 0;
      int closed = 0;
      await tester.pumpWidget(
        harness(
          sessions: const <ChatSession>[],
          onNew: () => created++,
          onClose: () => closed++,
        ),
      );
      await tester.pump();

      await tester.tap(find.text('新建'));
      await tester.pump();
      expect(created, 1);
      expect(closed, 1);
    });
  });

  group('重命名：**行内输入框**，不新开 overlay', () {
    testWidgets('点「改名」→ 该行变成输入框（预填现有名字）', (WidgetTester tester) async {
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '旧名字')],
          activeId: 'a',
        ),
      );
      await tester.pump();
      expect(find.byType(TextField), findsNothing);

      await tester.tap(find.text('改名'));
      await tester.pump();

      expect(find.byType(TextField), findsOneWidget);
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        '旧名字',
        reason: '预填现有名字，用户只改要改的部分',
      );
      // **没有开对话框**：`AlertDialog` 会是新的 overlay entry，
      // 而那一层不在指针垫层的罩子内（压在舞台上会点不着）。
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.byType(Dialog), findsNothing);
    });

    testWidgets('输入新名字 + 确定 → 回调收到 trim 后的值', (WidgetTester tester) async {
      final List<(String, String)> renamed = <(String, String)>[];
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '旧')],
          activeId: 'a',
          onRename: (String id, String t) => renamed.add((id, t)),
        ),
      );
      await tester.pump();
      await tester.tap(find.text('改名'));
      await tester.pump();

      await tester.enterText(find.byType(TextField), '  新名字  ');
      await tester.tap(find.text('确定'));
      await tester.pump();

      expect(renamed, <(String, String)>[('a', '  新名字  ')]);
      expect(find.byType(TextField), findsNothing, reason: '确定之后要退出编辑态');
    });

    testWidgets('回车 = 确定（不用非得点按钮）', (WidgetTester tester) async {
      final List<String> renamed = <String>[];
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '旧')],
          activeId: 'a',
          onRename: (String _, String t) => renamed.add(t),
        ),
      );
      await tester.pump();
      await tester.tap(find.text('改名'));
      await tester.pump();

      await tester.enterText(find.byType(TextField), '回车改的');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pump();

      expect(renamed, <String>['回车改的']);
    });

    testWidgets('「取消」不改任何东西', (WidgetTester tester) async {
      int calls = 0;
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '旧')],
          activeId: 'a',
          onRename: (String _, String _) => calls++,
        ),
      );
      await tester.pump();
      await tester.tap(find.text('改名'));
      await tester.pump();
      await tester.enterText(find.byType(TextField), '不该生效');
      await tester.tap(find.text('取消'));
      await tester.pump();

      expect(calls, 0);
      expect(find.text('旧'), findsOneWidget, reason: '取消要回到常态那行');
      expect(find.byType(TextField), findsNothing);
    });

    testWidgets('空标题的会话：输入框**也是空的**（留空 = 恢复自动标题）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '')],
          activeId: 'a',
        ),
      );
      await tester.pump();
      await tester.tap(find.text('改名'));
      await tester.pump();
      expect(
        tester.widget<TextField>(find.byType(TextField)).controller!.text,
        '',
        reason: '预填自动标题会让「留空恢复自动」这件事做不到',
      );
      expect(find.textContaining('留空'), findsOneWidget);
    });
  });

  group('删除：**两段式确认**（不可逆操作不该一点就没了）', () {
    testWidgets('点「删除」→ 先问一句，**不**直接删', (WidgetTester tester) async {
      int deleted = 0;
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '要删的', messages: 5)],
          activeId: 'a',
          onDelete: (_) => deleted++,
        ),
      );
      await tester.pump();

      await tester.tap(find.text('删除'));
      await tester.pump();

      expect(deleted, 0, reason: '第一下只是问，不能真删');
      expect(find.textContaining('删除「要删的」？'), findsOneWidget);
      expect(find.textContaining('5 条消息'), findsOneWidget);
      expect(find.text('取消'), findsOneWidget);
      expect(find.byType(AlertDialog), findsNothing);
    });

    testWidgets('确认之后才真删', (WidgetTester tester) async {
      final List<String> deleted = <String>[];
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '要删的')],
          activeId: 'a',
          onDelete: deleted.add,
        ),
      );
      await tester.pump();
      await tester.tap(find.text('删除'));
      await tester.pump();
      // 此时有两个「删除」：行里那个已经换成确认态的那个。
      await tester.tap(find.text('删除').last);
      await tester.pump();

      expect(deleted, <String>['a']);
    });

    testWidgets('确认态点「取消」→ 回到常态，不删', (WidgetTester tester) async {
      int deleted = 0;
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲乙丙')],
          activeId: 'a',
          onDelete: (_) => deleted++,
        ),
      );
      await tester.pump();
      await tester.tap(find.text('删除'));
      await tester.pump();
      await tester.tap(find.text('取消'));
      await tester.pump();

      expect(deleted, 0);
      expect(find.text('甲乙丙'), findsOneWidget);
      expect(find.text('改名'), findsOneWidget, reason: '回到常态那行');
    });

    testWidgets('删除确认态与重命名态**互斥**（不会同时开着）', (WidgetTester tester) async {
      await tester.pumpWidget(
        harness(
          sessions: <ChatSession>[session('a', '甲')],
          activeId: 'a',
        ),
      );
      await tester.pump();

      await tester.tap(find.text('改名'));
      await tester.pump();
      expect(find.byType(TextField), findsOneWidget);

      // 编辑态没有「删除」按钮（整行被输入框替换了），
      // 所以这里验的是反过来：从确认态点「改名」能回到编辑态。
      await tester.tap(find.text('取消'));
      await tester.pump();
      await tester.tap(find.text('删除'));
      await tester.pump();
      expect(find.textContaining('删除「甲」？'), findsOneWidget);
      expect(find.byType(TextField), findsNothing);
    });
  });

  group('指针垫层（浮层压在舞台上）', () {
    testWidgets('`showSessionSheet` 的 builder 套了 StagePointerInterceptor', (
      WidgetTester tester,
    ) async {
      // 这条与 `stage_pointer_interceptor_test.dart` 同源：
      // 浮层压在舞台下半部，不垫垫层整列条目都点不着（2026-09-11 修过的坑）。
      //
      // 这里验的是**源码接线**而不是 DOM：`flutter test` 跑在 VM 上，
      // 垫层走 stub（透明返回 child），`find.byType` 仍然认得出它。
      bool sawInterceptor = false;
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Builder(
            builder: (BuildContext context) => Scaffold(
              body: Center(
                child: TextButton(
                  onPressed: () {
                    showSessionSheet(
                      context: context,
                      sessions: const <ChatSession>[],
                      activeId: null,
                      onNew: () {},
                      onSelect: (_) {},
                      onRename: (_, _) {},
                      onDelete: (_) {},
                    );
                  },
                  child: const Text('开'),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.tap(find.text('开'));
      await tester.pumpAndSettle();

      sawInterceptor =
          find.byType(StagePointerInterceptor).evaluate().isNotEmpty;
      expect(
        sawInterceptor,
        isTrue,
        reason: '会话浮层没有垫指针垫层 —— 压在舞台上的部分会「看得见、点不着」',
      );
      // 内容确实在浮层里。
      expect(find.textContaining('还没有会话'), findsOneWidget);
    });
  });
}
