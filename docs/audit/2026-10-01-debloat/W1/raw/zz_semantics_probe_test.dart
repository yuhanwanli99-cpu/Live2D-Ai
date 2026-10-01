/// verifier 对抗性探针（W1-c 语义树）：**重复播报 / 漏播报**核查。
library;

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: Center(child: SizedBox(width: 340, child: child))),
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

List<String> collectLabels() {
  final List<String> out = <String>[];
  void walk(SemanticsNode n) {
    final SemanticsData d = n.getSemanticsData();
    if (d.label.isNotEmpty) out.add(d.label);
    n.visitChildren((SemanticsNode c) {
      walk(c);
      return true;
    });
  }

  final SemanticsNode? root =
      WidgetsBinding.instance.pipelineOwner.semanticsOwner?.rootSemanticsNode;
  if (root != null) walk(root);
  return out;
}

void main() {
  testWidgets('① 正文只被播报一次（不重复）', (WidgetTester tester) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    await tester.pumpWidget(wrap(MessageBubble(message: msg('独角兽专属正文ZZZ'))));
    final List<String> labels = collectLabels();
    final List<String> hits =
        labels.where((String l) => l.contains('独角兽专属正文ZZZ')).toList();
    // ignore: avoid_print
    print('① labels=$labels');
    expect(hits.length, 1, reason: '正文必须恰好播报一次');
    handle.dispose();
  });

  testWidgets('② 思考正文不被全文播报，但有「含思考 N 字」告知', (WidgetTester tester) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    await tester.pumpWidget(
      wrap(MessageBubble(message: msg('正文A', reasoning: '秘密思考串QQQ' * 20))),
    );
    final List<String> labels = collectLabels();
    final List<String> leak =
        labels.where((String l) => l.contains('秘密思考串QQQ')).toList();
    final List<String> mentions = labels.where((String l) => l.contains('思考')).toList();
    // ignore: avoid_print
    print('② 泄漏=$leak 提到思考=$mentions');
    expect(leak, isEmpty, reason: '思考正文不该被读屏念出来');
    expect(mentions, isNotEmpty, reason: '至少要告知「有思考」');
    handle.dispose();
  });

  testWidgets('③ 失败轮：重试/复制在语义树里且可激活', (WidgetTester tester) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    int retried = 0;
    await tester.pumpWidget(
      wrap(MessageBubble(message: msg('出错了', failed: true), onRetry: () => retried++)),
    );
    expect(find.semantics.byLabel('重试'), findsOne);
    expect(find.semantics.byLabel('复制'), findsOne);
    tester.semantics.tap(find.semantics.byLabel('重试'));
    await tester.pump();
    expect(retried, 1);
    handle.dispose();
  });

  testWidgets('④ 「未收尾」说明行只出现一次（独立节点）', (WidgetTester tester) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    await tester.pumpWidget(wrap(MessageBubble(message: msg('半截话', unfinished: true))));
    final List<String> labels = collectLabels();
    final List<String> hits = labels.where((String l) => l.contains('未收尾')).toList();
    // ignore: avoid_print
    print('④ 「未收尾」=$hits 全部labels=$labels');
    expect(hits.length, 1);
    handle.dispose();
  });

  testWidgets('⑤ 流式中的助手气泡正文仍只一次', (WidgetTester tester) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    await tester.pumpWidget(wrap(MessageBubble(message: msg('流式正文YYY', streaming: true))));
    final List<String> labels = collectLabels();
    final List<String> hits = labels.where((String l) => l.contains('流式正文YYY')).toList();
    // ignore: avoid_print
    print('⑤ 流式 labels=$labels');
    expect(hits.length, 1);
    handle.dispose();
  });
}
