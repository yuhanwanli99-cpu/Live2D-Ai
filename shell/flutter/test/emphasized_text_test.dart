import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/ui/emphasized_text.dart';

/// `**强调**` 的解析与渲染。
///
/// # 为什么值得单独写测试（有真实来历）
///
/// 界面上曾经**到处是字面的两个星号**（`**只影响本机显示**`）——
/// 文案按 Markdown 习惯写，而 Flutter 的 `Text` 不认 Markdown。
/// 这个文件守的就是「星号不会漏到用户眼前」。
void main() {
  group('emphasisSpans：把 **…** 拆成粗体段', () {
    List<String?> texts(String raw) =>
        emphasisSpans(raw).map((TextSpan s) => s.text).toList();
    List<FontWeight?> weights(String raw) => emphasisSpans(raw)
        .map((TextSpan s) => s.style?.fontWeight)
        .toList();

    test('一段强调 → 三个 span，中间是粗体', () {
      expect(texts('四套配色任选。**只影响本机显示**，选完立刻生效。'), <String>[
        '四套配色任选。',
        '只影响本机显示',
        '，选完立刻生效。',
      ]);
      expect(weights('a**b**c'), <FontWeight?>[
        null,
        FontWeight.w600,
        null,
      ]);
    });

    test('多处强调都认', () {
      expect(texts('**甲**和**乙**'), <String>['甲', '和', '乙']);
    });

    test('开头/结尾强调不产生空 span', () {
      expect(texts('**全粗**'), <String>['全粗']);
      expect(texts('前**后**'), <String>['前', '后']);
    });

    test('没有标记时**只有一个 span**（不改变原有渲染路径）', () {
      final List<TextSpan> spans = emphasisSpans('普通一句话');
      expect(spans, hasLength(1));
      expect(spans.single.text, '普通一句话');
      expect(spans.single.style?.fontWeight, isNull);
    });

    test('落单的 `**` **原样保留**（写错了要看得见，不能静默吞掉）', () {
      expect(texts('半截**强调'), <String>['半截**强调']);
      expect(texts('**'), <String>['**']);
      expect(texts('a**b'), <String>['a**b']);
    });

    test('空强调（`****`）不产生空 span', () {
      expect(texts('前****后'), <String>['前', '后']);
    });

    test('base 样式被继承，粗体只叠加字重（不覆盖字号/颜色）', () {
      const TextStyle base = TextStyle(fontSize: 12, color: Color(0xFF112233));
      final List<TextSpan> spans = emphasisSpans('a**b**c', base: base);
      expect(spans[0].style?.fontSize, 12);
      expect(spans[1].style?.fontSize, 12, reason: '粗体段丢了基础字号');
      expect(spans[1].style?.color, const Color(0xFF112233));
      expect(spans[1].style?.fontWeight, FontWeight.w600);
    });

    test('空字符串 → 一个空 span（不抛）', () {
      expect(emphasisSpans(''), hasLength(1));
    });
  });

  group('EmphasizedText：画出来的是粗体，不是星号', () {
    testWidgets('渲染结果里没有字面的 `**`', (WidgetTester tester) async {
      await tester.pumpWidget(
        const MaterialApp(
          home: Scaffold(
            body: EmphasizedText('四套配色任选。**只影响本机显示**，立刻生效。'),
          ),
        ),
      );
      final Text text = tester.widget<Text>(find.byType(Text));
      final TextSpan span = text.textSpan! as TextSpan;
      final String plain = span
          .children!
          .map((InlineSpan s) => (s as TextSpan).text ?? '')
          .join();
      expect(plain.contains('**'), isFalse, reason: '星号漏到界面上了');
      expect(plain, '四套配色任选。只影响本机显示，立刻生效。');
    });

    testWidgets('强调段真的是粗体', (WidgetTester tester) async {
      await tester.pumpWidget(
        const MaterialApp(home: Scaffold(body: EmphasizedText('a**b**c'))),
      );
      final TextSpan span =
          tester.widget<Text>(find.byType(Text)).textSpan! as TextSpan;
      expect((span.children![1] as TextSpan).style?.fontWeight, FontWeight.w600);
    });
  });
}
