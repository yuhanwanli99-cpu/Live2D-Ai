/// 朗读高亮：**纯判据 + 气泡真的接上了**（只做朗读高亮，不碰播放时序）。
///
/// 判据住在 `lib/chat/spoken_highlight.dart`（纯 Dart，VM 可测）；气泡那一半
/// 在这里用 widget 测试证明它真的画出来了——解析器写得再好，气泡不用它，
/// 用户看到的还是没高亮的正文。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/chat/chat_message.dart';
import 'package:live2d_ai_shell/chat/spoken_highlight.dart';
import 'package:live2d_ai_shell/ui/message_bubble.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

List<SpokenSpan> twoSentences() => <SpokenSpan>[
  SpokenSpan(seq: 1, text: '第一句。'),
  SpokenSpan(seq: 2, text: '第二句。'),
];

Widget wrap(Widget child) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(body: Center(child: child)),
);

/// 气泡里的富文本 span（与 `message_bubble_test.dart` 同一条取法）。
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
  group('纯判据：给定已上屏句子 + 当前播放句号', () {
    test('两句时高亮第二句（起止落在拼接正文里）', () {
      final SpokenHighlight? h = spokenHighlight(twoSentences(), 2);
      expect(h, isNotNull);
      expect(h!.start, 4, reason: '第一句占 4 个码元');
      expect(h.end, 8);
      expect(joinSpoken(twoSentences()).substring(h.start, h.end), '第二句。');
    });

    test('序号缺失 → 不高亮（播放号为 null / 对不上都算）', () {
      expect(spokenHighlight(twoSentences(), null), isNull);
      expect(spokenHighlight(twoSentences(), 3), isNull);
      // 服务端缺 sentence_seq 的那一条：正文里没有 span 与之对应。
      expect(spokenHighlight(const <SpokenSpan>[], 1), isNull);
    });

    test('播放结束 → 高亮消失（舞台时钟说 playing:false）', () {
      final SpokenHighlightController c = SpokenHighlightController();
      c.sentenceStarted(2);
      expect(c.playingSeq, 2, reason: '开播即高亮');
      expect(spokenHighlight(twoSentences(), c.playingSeq), isNotNull);

      c.onStageClock(playing: false);
      expect(c.playingSeq, isNull, reason: '播完必须灭');
      expect(spokenHighlight(twoSentences(), c.playingSeq), isNull);

      // playing:true 不能把它灭掉（缓冲暂停 vs 真在播是两回事，这里只认
      // 「时钟说停了」）。
      c.sentenceStarted(1);
      c.onStageClock(playing: true);
      expect(c.playingSeq, 1);
    });

    test('换轮 / 切会话清空（同一句号不会在旧气泡上复活）', () {
      final SpokenHighlightController c = SpokenHighlightController()
        ..sentenceStarted(1);
      c.clear();
      expect(c.playingSeq, isNull);
    });

    test('拼接正文与气泡正文不逐字对齐时，判据判「不适用」而不是高亮错字', () {
      // 中间夹过一条没有 sentence_seq 的 delta：text 比拼接正文多一截。
      expect(highlightFitsText('第一句。第二句。', twoSentences()), isTrue);
      expect(highlightFitsText('前缀第一句。第二句。', twoSentences()), isFalse);
      expect(highlightFitsText('', const <SpokenSpan>[]), isFalse);
    });
  });

  group('气泡：高亮真的画出来了', () {
    ChatMessage bubble() {
      final ChatMessage m = ChatMessage(role: ChatRole.assistant, text: '第一句。第二句。');
      m.spoken.addAll(twoSentences());
      return m;
    }

    testWidgets('播放第二句：该段带主题色的背景 + 加粗', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: bubble(), highlightSeq: 2)),
      );
      final List<TextSpan> spans = spansOf(tester);
      expect(
        spans.map((TextSpan s) => s.text).join(),
        '第一句。第二句。',
        reason: '高亮不许改字（既不能吞字也不能多字）',
      );
      final TextSpan marked = spans.firstWhere(
        (TextSpan s) => s.text == '第二句。',
      );
      expect(marked.style?.backgroundColor, isNotNull, reason: '要用现有主题 token 的底色');
      expect(marked.style?.fontWeight, FontWeight.w600);
      final TextSpan plain = spans.firstWhere(
        (TextSpan s) => s.text == '第一句。',
      );
      expect(plain.style?.backgroundColor, isNull, reason: '没在播的那句不许跟着亮');
    });

    testWidgets('播放结束（句号为 null）→ 一个高亮 segment 都没有', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: bubble(), highlightSeq: null)),
      );
      final List<TextSpan> spans = spansOf(tester);
      expect(
        spans.where((TextSpan s) => s.style?.backgroundColor != null),
        isEmpty,
      );
    });

    testWidgets('序号对不上：旧气泡收到别的句号也不亮', (WidgetTester tester) async {
      await tester.pumpWidget(
        wrap(MessageBubble(message: bubble(), highlightSeq: 7)),
      );
      final List<TextSpan> spans = spansOf(tester);
      expect(
        spans.where((TextSpan s) => s.style?.backgroundColor != null),
        isEmpty,
      );
    });
  });
}
