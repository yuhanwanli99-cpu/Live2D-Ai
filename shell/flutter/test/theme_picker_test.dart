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
  Widget wrap(Widget child) =>
      MaterialApp(theme: buildAppTheme(), home: Scaffold(body: child));

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

  testWidgets('四个主题各渲染一个色块，且用的是**各自**的舞台底', (
    WidgetTester tester,
  ) async {
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
      wrap(
        ThemePickerField(
          value: AppThemeId.black,
          onChanged: picked.add,
        ),
      ),
    );
    await tester.tap(find.text(AppThemeId.blue.label));
    expect(picked, <AppThemeId>[AppThemeId.blue]);
  });

  testWidgets('点当前已选中的主题也会回调（幂等由上层 `==` 挡，不由控件猜）', (
    WidgetTester tester,
  ) async {
    final List<AppThemeId> picked = <AppThemeId>[];
    await tester.pumpWidget(
      wrap(
        ThemePickerField(value: AppThemeId.gray, onChanged: picked.add),
      ),
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

  testWidgets('把选择器放进浅色主题下也不炸（主题切换后重建）', (
    WidgetTester tester,
  ) async {
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
}
