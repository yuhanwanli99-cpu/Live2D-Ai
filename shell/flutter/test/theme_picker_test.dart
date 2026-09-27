import 'dart:ui' show Tristate;

import 'package:flutter/semantics.dart' show SemanticsData;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:live2d_ai_shell/design/theme_id.dart';
import 'package:live2d_ai_shell/design/tokens.dart';
import 'package:live2d_ai_shell/ui/theme.dart';
import 'package:live2d_ai_shell/ui/theme_picker.dart';

/// 配色选择器：**点之前就要能看见结果**。
void main() {
  Widget wrap(Widget child) => MaterialApp(
    theme: buildAppTheme(),
    home: Scaffold(body: child),
  );

  /// 取某个色块容器的实际底色。
  Color swatchColor(WidgetTester tester, AppThemeId id) {
    final AnimatedContainer box = tester.widget<AnimatedContainer>(
      find
          .ancestor(
            of: find.text(id.label),
            matching: find.byType(AnimatedContainer),
          )
          .first,
    );
    return (box.decoration! as BoxDecoration).color!;
  }

  testWidgets('四个主题各渲染一个色块，且用的是**各自**的舞台底', (WidgetTester tester) async {
    await tester.pumpWidget(
      wrap(ThemePickerField(value: AppThemeId.black, onChanged: (_) {})),
    );
    for (final AppThemeId id in AppThemeId.values) {
      expect(find.text(id.label), findsOneWidget);
      expect(
        swatchColor(tester, id),
        AppPalette.of(id).stage,
        reason: '${id.label} 的色块没有用它自己的舞台底——那就成了「点了才知道」',
      );
    }
  });

  testWidgets('点另一个主题 → 回调拿到那个 id', (WidgetTester tester) async {
    final List<AppThemeId> picked = <AppThemeId>[];
    await tester.pumpWidget(
      wrap(ThemePickerField(value: AppThemeId.black, onChanged: picked.add)),
    );
    await tester.tap(find.text(AppThemeId.blue.label));
    expect(picked, <AppThemeId>[AppThemeId.blue]);
  });

  testWidgets('点当前已选中的主题也会回调（幂等由上层 `==` 挡，不由控件猜）', (WidgetTester tester) async {
    final List<AppThemeId> picked = <AppThemeId>[];
    await tester.pumpWidget(
      wrap(ThemePickerField(value: AppThemeId.gray, onChanged: picked.add)),
    );
    await tester.tap(find.text(AppThemeId.gray.label));
    expect(picked, <AppThemeId>[AppThemeId.gray]);
  });

  testWidgets('每个色块都是按钮语义，且选中态有 selected 标记（不只靠描边）', (
    WidgetTester tester,
  ) async {
    final SemanticsHandle handle = tester.ensureSemantics();
    await tester.pumpWidget(
      wrap(ThemePickerField(value: AppThemeId.blue, onChanged: (_) {})),
    );

    final Finder blueNode = find.bySemanticsLabel(
      '${AppThemeId.blue.label}，${AppThemeId.blue.hint}',
    );
    expect(blueNode, findsOneWidget);
    final SemanticsData data = tester.getSemantics(blueNode).getSemanticsData();
    expect(data.flagsCollection.isButton, isTrue);
    expect(data.flagsCollection.isSelected, Tristate.isTrue);

    // 未选中的那个不能报「已选中」。
    final SemanticsData other = tester
        .getSemantics(
          find.bySemanticsLabel(
            '${AppThemeId.white.label}，${AppThemeId.white.hint}',
          ),
        )
        .getSemanticsData();
    expect(other.flagsCollection.isSelected, Tristate.isFalse);
    handle.dispose();
  });

  testWidgets('把选择器放进浅色主题下也不炸（主题切换后重建）', (WidgetTester tester) async {
    for (final AppThemeId id in AppThemeId.values) {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(id),
          home: Scaffold(
            body: ThemePickerField(value: id, onChanged: (_) {}),
          ),
        ),
      );
      await tester.pump();
      expect(tester.takeException(), isNull, reason: '$id 下渲染失败');
      expect(find.text(AppThemeId.white.label), findsOneWidget);
    }
  });

  group('2026-09-27：大预览卡用**那套配色自己画自己**', () {
    testWidgets('每张卡的底色 = 那套配色的舞台底（不是当前主题的）', (WidgetTester tester) async {
      // 故意把选择器放在**黑**主题下：卡片仍然要画各自那套的舞台底。
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(AppThemeId.black),
          home: Scaffold(
            body: ThemePickerField(value: AppThemeId.black, onChanged: (_) {}),
          ),
        ),
      );
      await tester.pump();
      for (final AppThemeId id in AppThemeId.values) {
        final Container surface = tester.widget<Container>(
          find
              .descendant(
                of: find.byKey(ValueKey<String>('theme-card-${id.wire}')),
                matching: find.byType(Container),
              )
              .first,
        );
        final Decoration d = surface.decoration!;
        expect(
          (d as BoxDecoration).color,
          AppPalette.of(id).stage,
          reason: '${id.label} 的卡底色画错了',
        );
      }
    });

    testWidgets('三条面带 + 强调色角标都在，且用的是那套配色自己的值', (WidgetTester tester) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(AppThemeId.black),
          home: Scaffold(
            body: ThemePickerField(value: AppThemeId.black, onChanged: (_) {}),
          ),
        ),
      );
      await tester.pump();
      for (final AppThemeId id in AppThemeId.values) {
        final AppPalette p = AppPalette.of(id);
        for (int i = 0; i < kThemeCardBands.length; i++) {
          final Container band = tester.widget<Container>(
            find.byKey(ValueKey<String>('theme-band-${id.wire}-$i')),
          );
          expect(band.color, switch (kThemeCardBands[i]) {
            'surface' => p.surface,
            'surfaceAlt' => p.surfaceAlt,
            _ => p.raised,
          }, reason: '${id.label} 的第 $i 条面带画错了');
        }
        expect(
          find.descendant(
            of: find.byKey(ValueKey<String>('theme-card-${id.wire}')),
            matching: find.byWidgetPredicate(
              (Widget w) =>
                  w is Container &&
                  w.decoration is BoxDecoration &&
                  (w.decoration! as BoxDecoration).color == p.accent,
            ),
          ),
          findsOneWidget,
          reason: '${id.label} 少了强调色角标',
        );
      }
    });

    testWidgets('窄屏（320 px）下换行成两行，不溢出', (WidgetTester tester) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: buildAppTheme(AppThemeId.black),
          home: Scaffold(
            body: Center(
              child: SizedBox(
                width: 320,
                child: ThemePickerField(
                  value: AppThemeId.black,
                  onChanged: (_) {},
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pump();
      expect(tester.takeException(), isNull);
      for (final AppThemeId id in AppThemeId.values) {
        expect(
          find.byKey(ValueKey<String>('theme-card-${id.wire}')),
          findsOneWidget,
        );
      }
    });
  });
}
