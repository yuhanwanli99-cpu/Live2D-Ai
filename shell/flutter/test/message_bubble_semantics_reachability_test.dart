/// 气泡的无障碍可达性（2026-09-28，F-0005-4）。
///
/// # 这条守卫防的是什么
///
/// 气泡从前在**最外层**写了 `Semantics(excludeSemantics: true, …)`：整条气泡
/// 被折成**一个**节点。触屏与键盘照旧能点（语义排除不影响命中测试与焦点
/// 遍历），所以肉眼与键盘测试都测不出来 —— 但语义树里**根本不存在**
/// 「重试」与「复制」这两个节点：
///
/// - 失败轮的「重试」是用户**唯一的恢复入口**；
/// - 「复制」是模型给的代码片段的唯一出口。
///
/// 与 `stage_host.dart` 已经修过的同类缺陷同型（在包住覆盖层的 Stack 上写
/// excludeSemantics，把「重试」一起吃掉了）。
///
/// # 为什么用语义树查找器而不是 `tester.tap(find.text(…))`
///
/// `find.text('重试')` 查的是 **widget 树**：它在存在 `excludeSemantics` 的
/// 时候**一样能通过**（widget 一直在），所以对这条缺陷零判别力。这里一律用
/// `find.semantics.byLabel`（语义树）+ `tester.semantics.tap`（按读屏的方式
/// 激活：走 `SemanticsAction.tap`，不是命中测试）。
library;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/turn_liveness.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: Center(child: SizedBox(width: 340, child: child)),
  ),
);

ChatMessage msg(
  String text, {
  String reasoning = '',
  bool streaming = false,
  bool failed = false,
  bool unfinished = false,
}) => ChatMessage(
  role: ChatRole.assistant,
  text: text,
  reasoning: reasoning,
  streaming: streaming,
  failed: failed,
  unfinished: unfinished,
);

void main() {
  group('F-0005-4：动作条对读屏可达（失败轮的「重试」是唯一恢复入口）', () {
    testWidgets('失败轮：语义树里 find 得到「重试」，且能被读屏激活', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      int retried = 0;
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: msg('出错了', failed: true),
            onRetry: () => retried++,
          ),
        ),
      );

      final SemanticsFinder retry = find.semantics.byLabel('重试');
      expect(
        retry,
        findsOne,
        reason: '「重试」不在语义树里 —— 读屏用户失败后没有任何恢复入口',
      );
      // 读屏激活走的是 SemanticsAction.tap（不是命中测试）；`tap` 在节点
      // 没有 tap 动作时会抛 StateError，所以这一步同时钉住「可激活」。
      tester.semantics.tap(retry);
      await tester.pump();
      expect(retried, 1, reason: '语义节点存在但激活没接到「重试」上');
      handle.dispose();
    });

    testWidgets('失败轮：语义树里 find 得到「复制」，且能被读屏激活到剪贴板', (
      WidgetTester tester,
    ) async {
      final List<MethodCall> calls = <MethodCall>[];
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (MethodCall call) async {
          calls.add(call);
          return null;
        },
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          SystemChannels.platform,
          null,
        ),
      );

      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: msg('看这段 `x = 1`', failed: true),
            onRetry: () {},
          ),
        ),
      );

      final SemanticsFinder copy = find.semantics.byLabel('复制');
      expect(copy, findsOne, reason: '「复制」不在语义树里');
      tester.semantics.tap(copy);
      await tester.pump();
      expect(
        calls.any((MethodCall c) => c.method == 'Clipboard.setData'),
        isTrue,
        reason: '语义节点存在但激活没接到复制上',
      );

      // 「已复制」提示有个定时器，跑完它（否则测试结束会报 pending timer）。
      await tester.pump(const Duration(milliseconds: 1300));
      await tester.pumpAndSettle();
      handle.dispose();
    });

    testWidgets('普通轮：只有「复制」可达，没有「重试」', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(wrap(MessageBubble(message: msg('一段回答'))));

      expect(find.semantics.byLabel('复制'), findsOne);
      expect(
        find.semantics.byLabel('重试'),
        findsNothing,
        reason: '成功轮摆出「重试」会把用户引向一次多余的重发',
      );
      handle.dispose();
    });
  });

  group('F-0005-4：去掉整条排除之后，正文与说明行各自只念一遍', () {
    testWidgets('角色 + 正文合成一个节点念出来（不是裸奔、也不是念两遍）', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(wrap(MessageBubble(message: msg('你好呀'))));

      expect(
        find.semantics.byLabel('助手说：你好呀'),
        findsOne,
        reason: '正文/身份必须仍可读（去掉 excludeSemantics 不等于把语义全丢了）',
      );
      // 正文那段 `SelectableText` 自己若也成节点，就是同一个 label 的第二份
      // —— 读屏会把每句话念两遍。
      expect(
        find.semantics.byLabel('你好呀'),
        findsNothing,
        reason: '正文被念了两遍（容器 label 一遍 + SelectableText 一遍）',
      );
      handle.dispose();
    });

    testWidgets('流式：liveRegion 仍只念节流后的文本（别把 label 一起删了）', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: msg('好呀，那我就说两句。后面还在生成……', streaming: true),
            announcement: '好呀，那我就说两句。',
          ),
        ),
      );

      final SemanticsFinder bubble = find.semantics.byLabel(
        RegExp('好呀，那我就说两句。'),
      );
      expect(bubble, findsOne);
      final SemanticsNode node = bubble.evaluate().single;
      expect(
        node.getSemanticsData().flagsCollection.isLiveRegion,
        isTrue,
        reason: '流式气泡丢了 liveRegion —— 读屏不再主动念新内容',
      );
      expect(
        node.label,
        isNot(contains('后面还在生成')),
        reason: 'liveRegion 念了尚未节流的原文',
      );
      handle.dispose();
    });

    testWidgets('未收尾说明行对读屏可达（含完整解释，不是括号里的三个字）', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: msg('第一句。第二', unfinished: true),
          ),
        ),
      );

      expect(
        find.semantics.byLabel(kUnfinishedTurnCaption),
        findsOne,
        reason: '说明行不在语义树里（或与 label 里的「（未收尾）」重复播报）',
      );
      handle.dispose();
    });
  });

  group('F-0005-4：思考折叠开关本身是一个按钮（不只是能看见）', () {
    testWidgets('语义树里 find 得到「展开思考」，且能被读屏激活展开', (WidgetTester tester) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: msg('你好。', reasoning: '我先想一想该怎么打招呼。'),
          ),
        ),
      );

      // 折叠状态下思考正文不该上屏（不受本次改动影响）。
      expect(find.textContaining('我先想一想'), findsNothing);

      final SemanticsFinder toggle = find.semantics.byLabel(RegExp('展开思考'));
      expect(
        toggle,
        findsOne,
        reason: '折叠开关对读屏不可达 —— 听得见「含思考 N 字」却打不开它',
      );
      tester.semantics.tap(toggle);
      await tester.pumpAndSettle();
      expect(
        find.textContaining('我先想一想'),
        findsOneWidget,
        reason: '语义节点存在但激活没接到展开上',
      );
      handle.dispose();
    });

    testWidgets('展开后开关的 label 改成「收起思考」（读屏听得到当前状态）', (
      WidgetTester tester,
    ) async {
      final SemanticsHandle handle = tester.ensureSemantics();
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('你好。', reasoning: '我先想一想。'))),
      );

      tester.semantics.tap(find.semantics.byLabel(RegExp('展开思考')));
      await tester.pumpAndSettle();
      expect(find.semantics.byLabel(RegExp('收起思考')), findsOne);
      handle.dispose();
    });
  });
}
