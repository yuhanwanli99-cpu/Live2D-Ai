/// 组卡片（2026-09-27）的 widget 回归。
///
/// 守三件事：
/// 1. 四套主题 + 三档字号缩放下**都不溢出**（卡片是新的面，最容易在小宽度炸）；
/// 2. 卡片面用的是第三级面 [AppPalette.raised]（层级终于有了承重）；
/// 3. `description == null` 时不占位，`trailing` 挂得上。
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/ui/emphasized_text.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/ui/group_card.dart';
import 'package:live2d_ai_shell/ui/theme.dart';

Widget host(Widget child, {double width = 320}) => MaterialApp(
  theme: buildAppTheme(),
  home: Scaffold(
    body: SizedBox(width: width, child: child),
  ),
);

void main() {
  testWidgets('四套主题下都渲染得出，且不溢出', (WidgetTester tester) async {
    for (final AppThemeId id in AppThemeId.values) {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(id),
          home: Scaffold(
            body: SizedBox(
              width: 320,
              child: GroupCard(
                title: '外观',
                description: '配色与材质。**只影响本机显示**。',
                child: const Text('字段'),
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(tester.takeException(), isNull, reason: '$id 下溢出或抛异常');
      expect(find.text('外观'), findsOneWidget);
    }
  });

  testWidgets('三档字号缩放都不溢出（1.0 / 1.5 / 2.0）', (WidgetTester tester) async {
    for (final double scale in <double>[1.0, 1.5, 2.0]) {
      await tester.pumpWidget(
        host(
          GroupCard(
            title: '舞台与口型',
            description: '这些参数立刻下发到渲染面，不需要保存。',
            child: const Text('一整行比较长的说明文字，用来把卡片撑开'),
          ),
          width: 320,
        ),
      );
      await tester.pump();
      // textScaler 要在 pump 之前设才生效，所以这一轮直接改 MediaQuery。
      await tester.pumpWidget(
        MediaQuery(
          data: MediaQueryData(textScaler: TextScaler.linear(scale)),
          child: host(
            GroupCard(
              title: '舞台与口型',
              description: '这些参数立刻下发到渲染面，不需要保存。',
              child: const Text('一整行比较长的说明文字，用来把卡片撑开'),
            ),
            width: 320,
          ),
        ),
      );
      await tester.pump();
      expect(tester.takeException(), isNull, reason: 'textScaler $scale 下溢出');
    }
  });

  testWidgets('卡片面 = 第三级面 raised（层级终于有承重）', (WidgetTester tester) async {
    await tester.pumpWidget(
      host(const GroupCard(title: '外观', child: Text('字段'))),
    );
    await tester.pump();
    final DecoratedBox box = tester.widget<DecoratedBox>(
      find
          .descendant(
            of: find.byType(GroupCard),
            matching: find.byType(DecoratedBox),
          )
          .first,
    );
    final BoxDecoration d = box.decoration as BoxDecoration;
    expect(d.color, AppPalette.black.raised);
    expect(d.boxShadow, isNotNull, reason: '卡片要「浮起来」就得有影子');
    expect(d.border, isNotNull);
  });

  testWidgets('有 / 无 description 的卡片高度不同（有说明才占那行）', (
    WidgetTester tester,
  ) async {
    Future<double> heightWith(String? description) async {
      await tester.pumpWidget(
        host(
          GroupCard(
            title: '互动',
            description: description,
            child: const Text('字段'),
          ),
        ),
      );
      await tester.pump();
      return tester.getSize(find.byType(GroupCard)).height;
    }

    final double noDesc = await heightWith(null);
    final double hasDesc = await heightWith('这一行说明');
    expect(hasDesc - noDesc, greaterThan(12), reason: '没有说明时不该多画一行');
  });

  testWidgets('trailing 挂得上（整组开关）', (WidgetTester tester) async {
    await tester.pumpWidget(
      host(
        GroupCard(
          title: '互动',
          trailing: Switch(value: true, onChanged: (_) {}),
          child: const Text('字段'),
        ),
      ),
    );
    await tester.pump();
    expect(find.byType(Switch), findsOneWidget);
  });

  testWidgets('分组说明里的 `**加粗**` 不露出星号（rc.3 同款回归）', (WidgetTester tester) async {
    await tester.pumpWidget(
      host(
        const GroupCard(
          title: '外观',
          description: '配色与材质。**只影响本机显示**。',
          child: Text('字段'),
        ),
      ),
    );
    await tester.pump();
    // 裸 Text 会把星号原样画进语义标签；EmphasizedText 不会。
    expect(tester.getSemantics(find.text('字段')).label, isNot(contains('*')));
    final EmphasizedText emphasized = tester.widget<EmphasizedText>(
      find.byType(EmphasizedText),
    );
    expect(emphasized.text, contains('**'), reason: '源码里仍然是加粗标记');
  });

  testWidgets('标题是可读文本（读屏能念出来，不是纯装饰）', (WidgetTester tester) async {
    await tester.pumpWidget(
      host(const GroupCard(title: '外观', child: Text('字段'))),
    );
    await tester.pump();
    final Text title = tester.widget<Text>(
      find.descendant(of: find.byType(GroupCard), matching: find.text('外观')),
    );
    expect(title.style?.fontWeight, FontWeight.w600);
  });
}
