/// 聊天气泡：Markdown 渲染 + 复制 + 宽度（2026-09-11，前端加强计划 P2-3）。
///
/// 解析规则本身在 `chat_markdown_test.dart` 里验；这里验的是**它真的接上了**
/// ——解析器写得再好，气泡如果还在用 `SelectableText` 直接把原文塞进去，
/// 用户看到的仍然是 `**`。
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/soft_motion.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child, {double width = 340}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: Center(child: SizedBox(width: width, child: child)),
  ),
);

ChatMessage msg(String text, {bool streaming = false, bool failed = false}) =>
    ChatMessage(
      role: ChatRole.assistant,
      text: text,
      streaming: streaming,
      // 失败态 = 占位消息（`isPlaceholder`）。
      failed: failed,
    );

/// 取气泡里那段可选中的富文本。
List<TextSpan> spansOf(WidgetTester tester) {
  final SelectableText rich = tester.widget<SelectableText>(
    find.byType(SelectableText),
  );
  final InlineSpan? root = rich.textSpan;
  if (root is! TextSpan) return const <TextSpan>[];
  return <TextSpan>[
    for (final InlineSpan s in root.children ?? const <InlineSpan>[])
      if (s is TextSpan) s,
  ];
}

void main() {
  _reasoningSectionTests();
  group('P2-3：正文真的渲染 Markdown', () {
    testWidgets('`**粗体**` 走粗体 span，且**星号不在文本里**', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(MessageBubble(message: msg('这是**重点**。'))));
      final List<TextSpan> spans = spansOf(tester);
      expect(
        spans.map((TextSpan s) => s.text).join(),
        '这是重点。',
        reason: '星号被原样画出来了 —— 用户看到的是 `**重点**`',
      );
      final TextSpan bold = spans.firstWhere((TextSpan s) => s.text == '重点');
      expect(bold.style?.fontWeight, FontWeight.w600);
    });

    testWidgets('`` `行内码` `` 走等宽 + 底色', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('跑 `flutter test` 即可'))),
      );
      final TextSpan code = spansOf(
        tester,
      ).firstWhere((TextSpan s) => s.text == 'flutter test');
      expect(code.style?.fontFamily, 'monospace');
      expect(code.style?.backgroundColor, isNotNull);
    });

    testWidgets('`- ` 开头的行变成 `· ` 独立行', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('- 第一条\n- 第二条'))),
      );
      expect(
        spansOf(tester).map((TextSpan s) => s.text).join(),
        '· 第一条\n· 第二条',
      );
    });

    testWidgets('**纯文本消息不进 `Text.rich`**（选择/复制行为保持原样）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('今天天气不错，去散步吧。'))),
      );
      expect(
        tester.widget<SelectableText>(find.byType(SelectableText)).textSpan,
        isNull,
        reason: '纯文本也走了 rich 路径 —— 没有理由付那个选择行为的代价',
      );
    });

    testWidgets('流式中的半句话不会被当成 Markdown 解析（记号只在完整时才认）', (
      WidgetTester tester,
    ) async {
      // 流式是**逐字**到达的：`**重` 这种半截状态会短暂出现。
      // 它必须原样显示，而不是把用户看到的字吃掉一半。
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('这是**重', streaming: true))),
      );
      final SelectableText t = tester.widget<SelectableText>(
        find.byType(SelectableText),
      );
      // 半截记号 → 走纯文本路径 → `textSpan` 为空、`data` 是原文。
      expect(t.textSpan, isNull);
      expect(t.data, '这是**重');
    });
  });

  group('P2-3：复制按钮', () {
    testWidgets('非流式且非空的消息有「复制」', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(MessageBubble(message: msg('一段回答'))));
      expect(find.text('复制'), findsOneWidget);
    });

    testWidgets('流式中**没有**复制（复制到的是半句话）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('正在', streaming: true))),
      );
      expect(find.text('复制'), findsNothing);
    });

    testWidgets('点一下把**原文**（含记号）放进剪贴板', (WidgetTester tester) async {
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

      await tester.pumpWidget(wrap(MessageBubble(message: msg('看这段 `x = 1`'))));
      await tester.tap(find.text('复制'));
      await tester.pump();

      final MethodCall clipboard = calls.firstWhere(
        (MethodCall c) => c.method == 'Clipboard.setData',
      );
      expect(
        (clipboard.arguments as Map<Object?, Object?>)['text'],
        '看这段 `x = 1`',
        reason: '复制的是渲染后的文本 —— 用户拿到的记号少了，会以为自己看错',
      );

      // 「已复制」的提示有一个定时器，跑完它（否则测试结束时会报 pending timer）。
      await tester.pump(const Duration(milliseconds: 1300));
      await tester.pumpAndSettle();
    });

    testWidgets('复制后就地提示「已复制」，并且**不阻塞**别的东西', (WidgetTester tester) async {
      tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
        SystemChannels.platform,
        (MethodCall call) async => null,
      );
      addTearDown(
        () => tester.binding.defaultBinaryMessenger.setMockMethodCallHandler(
          SystemChannels.platform,
          null,
        ),
      );

      await tester.pumpWidget(wrap(MessageBubble(message: msg('内容'))));
      await tester.tap(find.text('复制'));
      await tester.pump();
      expect(find.text('已复制'), findsOneWidget);

      // 提示会自己收回去（否则按钮就永久变成「已复制」了）。
      await tester.pump(const Duration(milliseconds: 1300));
      await tester.pumpAndSettle();
      expect(find.text('复制'), findsOneWidget);
    });
  });

  group('P2-3：宽度约束是「相对可用宽度」而不是写死的 460', () {
    testWidgets('气泡在 320 px 的聊天列里不会溢出', (WidgetTester tester) async {
      // 这个宽度就是 medium 断点下聊天列的真实宽度。
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('一段' * 200)), width: 320),
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('把窗口缩到手机宽度也不溢出（约束跟着视口走）', (WidgetTester tester) async {
      // 用 `tester.view` 而不是 `setSurfaceSize`：后者改的是渲染视图尺寸，
      // 而这里要断言的是 **MediaQuery 读到的宽度**，两者不是同一个入口。
      tester.view.physicalSize = const Size(400, 800);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(),
          home: Scaffold(body: MessageBubble(message: msg('一段' * 200))),
        ),
      );
      expect(tester.takeException(), isNull);
      // 约束真的读了视口：400 * 0.92 = 368。
      final BoxConstraints c = tester
          .widget<Container>(
            find
                .descendant(
                  of: find.byType(MessageBubble),
                  matching: find.byType(Container),
                )
                .first,
          )
          .constraints!;
      expect(c.maxWidth, closeTo(400 * 0.92, 0.5));
    });
  });

  group('P2-3：失败态仍然有重试（别在改版里弄丢）', () {
    testWidgets('失败消息同时给「复制」与「重试」', (WidgetTester tester) async {
      int retried = 0;
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: msg('出错了', failed: true),
            onRetry: () => retried++,
          ),
        ),
      );
      expect(find.text('复制'), findsOneWidget);
      await tester.tap(find.text('重试'));
      expect(retried, 1);
    });
  });

  group('P2-3：动效——「已复制」的切换走 SoftSwap', () {
    testWidgets('存在 SoftSwap（不是硬切）', (WidgetTester tester) async {
      await tester.pumpWidget(wrap(MessageBubble(message: msg('内容'))));
      expect(find.byType(SoftSwap), findsWidgets);
    });
  });
}

// ── 思考折叠区（2026-09-13，推理模型）────────────────────────────────
//
// 现场：`deepseek-flash` 的思考与正文共用输出预算；截断时正文一个字都上不了屏，
// 而思考其实收到了 1200+ 字。所以思考既要有地方显示（折叠区），
// 也要在「只有思考」时默认展开并说明白——否则用户看到的仍然是「无返回」。
void _reasoningSectionTests() {
  ChatMessage withReasoning(
    String text,
    String reasoning, {
    bool streaming = false,
  }) => ChatMessage(
    role: ChatRole.assistant,
    text: text,
    reasoning: reasoning,
    streaming: streaming,
  );

  group('思考折叠区', () {
    testWidgets('默认折叠：只显示「已思考 N 字」，正文完整可见', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: withReasoning('你好。', '我先想一想该怎么打招呼。'),
          ),
        ),
      );
      expect(find.textContaining('已思考'), findsOneWidget);
      // 折叠状态下思考正文不上屏。
      expect(find.textContaining('我先想一想'), findsNothing);
      // 而真正的回复在屏上（折叠区不能把回复挤掉）。
      expect(find.textContaining('你好。'), findsOneWidget);
    });

    testWidgets('点标题展开/收起', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(message: withReasoning('你好。', '我先想一想。')),
        ),
      );
      await tester.tap(find.textContaining('已思考'));
      await tester.pumpAndSettle();
      expect(find.textContaining('我先想一想'), findsOneWidget);
      // 再点收起。
      await tester.tap(find.textContaining('思考（'));
      await tester.pumpAndSettle();
      expect(find.textContaining('我先想一想'), findsNothing);
    });

    testWidgets('流式中：标题是「思考中…」，条数实时长', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: withReasoning('', '正在推理', streaming: true),
          ),
        ),
      );
      expect(find.textContaining('思考中'), findsOneWidget);
    });

    testWidgets('只有思考、没有正文 → 默认展开 + 说明行（并**不**摆「…」占位）', (
      WidgetTester tester,
    ) async {
      await tester.pumpWidget(
        wrap(
          MessageBubble(
            message: withReasoning('', '我算一下：7²+11²+13²=339。'),
          ),
        ),
      );
      // 默认展开：思考就是本轮唯一内容，折叠起来等于什么都没显示。
      expect(find.textContaining('我算一下'), findsOneWidget);
      // 说明行同时给出「发生了什么」与「下一步」（文案本身在 turn_liveness 测试里钉）。
      expect(find.textContaining('只有思考'), findsOneWidget);
      expect(find.textContaining('输出 token 上限'), findsOneWidget);
      // 不能同时摆一个「…」占位——那会让人以为还在等（其实已收口）。
      expect(find.text('…'), findsNothing);
    });

    testWidgets('没有思考时：不出现任何思考区（普通回复保持原样）', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: msg('你好。'))),
      );
      expect(find.textContaining('已思考'), findsNothing);
      expect(find.textContaining('思考'), findsNothing);
    });
  });
}
